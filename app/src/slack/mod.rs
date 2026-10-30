//! APP-100 (ADR-011 pitch, `pivot/slack`): file, approve and get notified in
//! Slack through the governed action path.
//!
//! Protocol patterns only (Slack's documented request signing, the 3-second
//! acknowledgement, one interactivity endpoint dispatching on `action_id`,
//! `response_url` with `replace_original`, a short-lived link token plus a
//! mapping table); no code from any other implementation.
//!
//! Routes (absent configuration: none is mounted and every path answers the
//! uniform 404):
//! - `POST /v1/slack/commands`: `/forma <text>`. `link` answers the link URL
//!   at once (ephemeral); `unlink` revokes the caller's link; `help` or nothing
//!   answers the help text; anything else is an utterance: the answer is an
//!   ephemeral "Working…" within the acknowledgement and the work continues in
//!   `tokio::spawn`, its result posted to the command's `response_url`.
//! - `POST /v1/slack/interactivity`: `block_actions` only, dispatched on
//!   `action_id` (`forma_confirm`, `forma_cancel`, `forma_approve`,
//!   `forma_reject`); the acknowledgement is an empty 200 and the result goes
//!   to the payload's `response_url`.
//! - `POST /v1/slack/events`: `url_verification` (the challenge echoed) and
//!   `app_mention` (a help reply in the thread); Slack retries are
//!   acknowledged and ignored.
//! - `GET|POST /slack/link`: the member's confirmation page (`link`).
//!
//! Every Slack request is verified before parsing (`signature`): the raw body
//! read under [`BODY_LIMIT`], the timestamp within 300 s, the HMAC compared in
//! constant time, a replayed signature refused; any failure is the uniform
//! 401 with nothing logged but a counter.
//!
//! Identity (ADR-011 I1/I2): a Slack team and user id confer nothing. Every
//! action resolves the sender's link through `storage::Workspace::
//! resolve_slack_link`, which constructs the `TrustedRequest` exactly as the
//! session path does (unrevoked link, the member's current epoch, live
//! entitlement, active binding, registry identity); an unlinked, revoked or
//! stale sender gets only the link prompt. Everything after that is the
//! canvas's own path under that member's authority: the action parser
//! (rehearsed utterances inside a demo organization, the model otherwise), the
//! resolver and dry run, the audited command route (`commands::routes::
//! commit`, I4), the share, the events. Utterances and button values are data
//! (I5): a button carries only a draft id or a record id, judged again on
//! click. The bot token, the signing secret and link tokens are never logged
//! (I6).
pub mod api;
pub mod flows;
pub mod link;
pub mod notify;
pub mod signature;
pub mod text;

#[cfg(test)]
mod tests;

use crate::{
    actions::{resolve::Draft, Parser},
    auth::Auth,
    http,
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const BOT_TOKEN_ENV: &str = "SLACK_BOT_TOKEN";
pub const SIGNING_SECRET_ENV: &str = "SLACK_SIGNING_SECRET";
/// Test-only override of `https://slack.com/api` (the loopback stub).
pub const API_BASE_ENV: &str = "GENUI_SLACK_API_BASE";
/// The raw body read before verification (Slack's payloads are small).
pub const BODY_LIMIT: usize = 16 * 1024;
/// A held draft's lifetime (DS-49's in-memory drafts, reused for Slack).
pub const DRAFT_TTL: Duration = Duration::from_secs(600);
const MAX_DRAFTS: usize = 1024;
/// Live Slack drafts per member (as MCP's per-member cap).
const MAX_DRAFTS_PER_MEMBER: usize = 20;

pub const COMMANDS_PATH: &str = "/v1/slack/commands";
pub const INTERACTIVITY_PATH: &str = "/v1/slack/interactivity";
pub const EVENTS_PATH: &str = "/v1/slack/events";

/// A credential held in memory: never printed (its `Debug` is redacted).
#[derive(Clone)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: String) -> Option<Self> {
        let value = value.trim().to_owned();
        (!value.is_empty() && value.len() <= 512 && !value.contains(char::is_whitespace))
            .then_some(Self(value))
    }
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(redacted)")
    }
}

/// The Slack app's configuration: both values or nothing.
#[derive(Clone, Debug)]
pub struct Config {
    pub signing_secret: Secret,
    pub bot_token: Secret,
    pub api_base: String,
}
impl Config {
    /// From `SLACK_BOT_TOKEN` and `SLACK_SIGNING_SECRET` (absent or empty:
    /// Slack is off) and `GENUI_SLACK_API_BASE` (tests only).
    pub fn from_env() -> Option<Self> {
        let token = Secret::new(std::env::var(BOT_TOKEN_ENV).ok()?)?;
        let secret = Secret::new(std::env::var(SIGNING_SECRET_ENV).ok()?)?;
        let api_base = std::env::var(API_BASE_ENV)
            .ok()
            .filter(|base| !base.is_empty())
            .unwrap_or_else(|| api::DEFAULT_BASE.to_owned());
        Some(Self {
            signing_secret: secret,
            bot_token: token,
            api_base,
        })
    }
}

/// A draft held for a Slack Confirm: the resolver's typed command and who
/// may confirm it. Confirming repeats the same idempotency key, so a second
/// Confirm replays the stored outcome instead of filing twice.
pub(crate) struct Held {
    pub organization: String,
    pub principal: String,
    pub team: String,
    pub user: String,
    pub draft: Draft,
    pub created: Instant,
}

pub struct Slack {
    pub(crate) auth: Arc<Auth>,
    pub(crate) api: api::Client,
    pub(crate) parser: Parser,
    pub(crate) links: link::Links,
    signing_secret: Secret,
    replays: signature::Replays,
    pub(crate) drafts: Mutex<HashMap<String, Held>>,
    /// (organization, principal, record, version) already DMed.
    pub(crate) notified: Mutex<HashSet<(String, String, String, i64)>>,
}

impl Slack {
    pub fn new(auth: Arc<Auth>, config: Config, parser: Parser) -> Option<Arc<Self>> {
        Some(Arc::new(Self {
            api: api::Client::new(&config.api_base, config.bot_token.clone())?,
            links: link::Links::new(&config.signing_secret),
            signing_secret: config.signing_secret,
            auth,
            parser,
            replays: signature::Replays::default(),
            drafts: Mutex::new(HashMap::new()),
            notified: Mutex::new(HashSet::new()),
        }))
    }
    /// The environment's Slack app (none when unconfigured).
    pub fn from_env(auth: Arc<Auth>) -> Option<Arc<Self>> {
        Self::new(auth, Config::from_env()?, Parser::from_env())
    }

    /// The deployment's public origin for canvas and link URLs (empty in
    /// tests and locally: links are then same-origin paths, shown as text).
    pub(crate) fn origin(&self) -> &str {
        self.auth.origin()
    }

    /// Verifies a Slack request's signature over `body` (module notes).
    fn verified(&self, headers: &HeaderMap, body: &[u8]) -> bool {
        let one = |name: &str| {
            let mut values = headers.get_all(name).iter();
            match (values.next(), values.next()) {
                (Some(value), None) => value.to_str().ok(),
                _ => None,
            }
        };
        let signature = one("x-slack-signature");
        let now = now();
        let valid = signature::verify(
            self.signing_secret.expose().as_bytes(),
            one("x-slack-request-timestamp"),
            signature,
            body,
            now,
        ) && signature.is_some_and(|s| self.replays.first(s, now));
        if !valid {
            signature::count_refusal();
        }
        valid
    }

    /// Holds `draft` for its member and Slack user; the draft id is random.
    pub(crate) fn hold(
        &self,
        member: &crate::storage::TrustedRequest,
        team: &str,
        user: &str,
        draft: Draft,
    ) -> Option<String> {
        let id: String = crate::auth::capability::random_bytes::<16>()?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut drafts = self.drafts.lock().ok()?;
        drafts.retain(|_, held| held.created.elapsed() < DRAFT_TTL);
        let mine = drafts
            .values()
            .filter(|held| {
                held.organization == member.organization() && held.principal == member.principal()
            })
            .count();
        if drafts.len() >= MAX_DRAFTS || mine >= MAX_DRAFTS_PER_MEMBER {
            return None;
        }
        drafts.insert(
            id.clone(),
            Held {
                organization: member.organization().to_owned(),
                principal: member.principal().to_owned(),
                team: team.to_owned(),
                user: user.to_owned(),
                draft,
                created: Instant::now(),
            },
        );
        Some(id)
    }
}

/// Unix seconds now.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The uniform refusal of an unverifiable Slack request.
fn unauthorized() -> Response {
    http::problem(StatusCode::UNAUTHORIZED, "unauthorized")
}
/// Slack's acknowledgement: an empty 200.
fn ack() -> Response {
    let mut response = StatusCode::OK.into_response();
    http::secure(response.headers_mut());
    response
}
/// A JSON acknowledgement carrying a message (slash command replies).
fn ack_json(value: &Value) -> Response {
    let mut response = (StatusCode::OK, value.to_string()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

/// The verified raw body of a Slack request, or the uniform 401.
async fn verified_body(state: &Slack, request: Request) -> Result<(HeaderMap, Vec<u8>), Response> {
    let (parts, body) = request.into_parts();
    let Ok(bytes) = to_bytes(body, BODY_LIMIT).await else {
        signature::count_refusal();
        return Err(unauthorized());
    };
    if parts.uri.query().is_some() || !state.verified(&parts.headers, &bytes) {
        return Err(unauthorized());
    }
    Ok((parts.headers, bytes.to_vec()))
}

/// The form fields of a verified body; a repeated name refuses the body.
fn form_fields(bytes: &[u8]) -> Option<HashMap<String, String>> {
    let mut fields = HashMap::new();
    for (name, value) in url::form_urlencoded::parse(bytes).into_owned() {
        if fields.insert(name, value).is_some() {
            return None;
        }
    }
    Some(fields)
}

async fn commands(State(state): State<Arc<Slack>>, request: Request) -> Response {
    let (_, bytes) = match verified_body(&state, request).await {
        Ok(verified) => verified,
        Err(refusal) => return refusal,
    };
    let Some(fields) = form_fields(&bytes) else {
        return ack_json(&text::ephemeral(
            "Forma couldn't read that request.",
            None,
            false,
        ));
    };
    ack_json(&flows::command(&state, &fields))
}

async fn interactivity(State(state): State<Arc<Slack>>, request: Request) -> Response {
    let (_, bytes) = match verified_body(&state, request).await {
        Ok(verified) => verified,
        Err(refusal) => return refusal,
    };
    if let Some(payload) = form_fields(&bytes)
        .and_then(|fields| fields.get("payload").cloned())
        .and_then(|payload| serde_json::from_str::<Value>(&payload).ok())
    {
        flows::interaction(&state, &payload);
    }
    ack()
}

async fn events(State(state): State<Arc<Slack>>, request: Request) -> Response {
    let (headers, bytes) = match verified_body(&state, request).await {
        Ok(verified) => verified,
        Err(refusal) => return refusal,
    };
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return ack();
    };
    match body.get("type").and_then(Value::as_str) {
        Some("url_verification") => {
            let challenge = body
                .get("challenge")
                .and_then(Value::as_str)
                .filter(|c| c.len() <= 256 && c.bytes().all(|b| b.is_ascii_graphic()))
                .unwrap_or("");
            ack_json(&serde_json::json!({"challenge": challenge}))
        }
        Some("event_callback") => {
            if headers.get("x-slack-retry-num").is_none() {
                flows::event(&state, &body);
            }
            ack()
        }
        _ => ack(),
    }
}

/// The Slack routes (none when Slack is unconfigured: the uniform 404).
pub fn router(state: Option<Arc<Slack>>) -> Router {
    let Some(state) = state else {
        return Router::new();
    };
    Router::new()
        .route(COMMANDS_PATH, post(commands))
        .route(INTERACTIVITY_PATH, post(interactivity))
        .route(EVENTS_PATH, post(events))
        .route(link::PATH, get(link::show).post(link::confirm))
        .with_state(state)
}
