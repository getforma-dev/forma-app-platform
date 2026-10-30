//! Linking a Slack user to a member.
//!
//! `/forma link` answers (ephemerally, to that Slack user only) a URL to
//! `/slack/link?t=<token>`. The token is `base64url(payload) "." base64url(tag)`
//! where `payload = "v1:{team}:{user}:{expires}:{nonce}"` and `tag` is
//! HMAC-SHA256 under a key derived from the signing secret with its own
//! domain (`forma:slack-link-key:v1`), so a link token can never be confused
//! with a Slack signature. It lives [`LINK_SECONDS`] and is single use: the
//! nonce is consumed when a member confirms. Nothing persists it; it is never
//! logged (ADR-011 I6).
//!
//! `GET /slack/link?t=…` shows a confirmation page to a signed-in member (no
//! session: a neutral "sign in first" page). `POST /slack/link` (same-origin,
//! form body `t=…`, a live workspace session) verifies and consumes the token
//! and writes the link for **the confirming member's own organization and
//! principal** (`storage::Workspace::link_slack`); the token names only the
//! Slack side. Refusals render one neutral page each; nothing about another
//! member, organization or Slack user is disclosed.
//!
//! Recorded limitation (APP-098 DS-60): whoever opens the URL while signed in
//! becomes the member behind that Slack user, so a Slack user who sends their
//! link to a teammate would act as that teammate in Slack. The page names the
//! Slack user id and workspace before the member confirms. Proper fix: match
//! the Slack profile's verified email (`users.info`, `users:read.email`)
//! against the member's provider identity before writing the row.
use super::{Secret, Slack};
use crate::{auth::csrf, http, storage};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::hmac;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// A link token's lifetime.
pub const LINK_SECONDS: u64 = 600;
const KEY_DOMAIN: &[u8] = b"forma:slack-link-key:v1";
pub const PATH: &str = "/slack/link";

/// The link token key and the consumed nonces.
pub struct Links {
    key: hmac::Key,
    used: Mutex<HashMap<String, u64>>,
}

/// What a valid token names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    pub team: String,
    pub user: String,
    pub expires: u64,
    pub nonce: String,
}

impl Links {
    pub fn new(signing_secret: &Secret) -> Self {
        let derive = hmac::Key::new(hmac::HMAC_SHA256, signing_secret.expose().as_bytes());
        let derived = hmac::sign(&derive, KEY_DOMAIN);
        Self {
            key: hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref()),
            used: Mutex::new(HashMap::new()),
        }
    }

    /// A fresh token for `team`/`user`, valid until `now + LINK_SECONDS`.
    pub fn mint(&self, team: &str, user: &str, now: u64) -> Option<String> {
        if !(storage::slack_id(team) && storage::slack_id(user)) {
            return None;
        }
        let nonce: String = crate::auth::capability::random_bytes::<16>()?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let payload = format!("v1:{team}:{user}:{}:{nonce}", now + LINK_SECONDS);
        let tag = hmac::sign(&self.key, payload.as_bytes());
        Some(format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            URL_SAFE_NO_PAD.encode(tag.as_ref())
        ))
    }

    /// The token's claim when its tag verifies (constant time), it has not
    /// expired at `now` and its nonce was not consumed. Consumes nothing.
    pub fn check(&self, token: &str, now: u64) -> Option<Claim> {
        if token.len() > 256 {
            return None;
        }
        let (payload, tag) = token.split_once('.')?;
        let payload = URL_SAFE_NO_PAD.decode(payload).ok()?;
        let tag = URL_SAFE_NO_PAD.decode(tag).ok()?;
        hmac::verify(&self.key, &payload, &tag).ok()?;
        let payload = String::from_utf8(payload).ok()?;
        let mut parts = payload.split(':');
        let (Some("v1"), Some(team), Some(user), Some(expires), Some(nonce), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return None;
        };
        let expires: u64 = expires.parse().ok()?;
        if expires <= now || expires > now + LINK_SECONDS {
            return None;
        }
        if self.used.lock().ok()?.contains_key(nonce) {
            return None;
        }
        Some(Claim {
            team: team.to_owned(),
            user: user.to_owned(),
            expires,
            nonce: nonce.to_owned(),
        })
    }

    /// Consumes the claim's nonce; false when another confirmation won.
    pub fn consume(&self, claim: &Claim, now: u64) -> bool {
        let Ok(mut used) = self.used.lock() else {
            return false;
        };
        used.retain(|_, expires| *expires > now);
        if used.contains_key(&claim.nonce) {
            return false;
        }
        used.insert(claim.nonce.clone(), claim.expires);
        true
    }
}

/// The link URL for a token: the deployment's origin when configured, else a
/// same-origin path (local and tests).
pub fn url(origin: &str, token: &str) -> String {
    format!("{origin}{PATH}?t={token}")
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A small self-contained page (no script; the form posts to this origin).
fn page(status: StatusCode, title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title><style>body{{font:16px/1.5 system-ui,sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem;color:#111827}}h1{{font-size:1.25rem}}button{{font:inherit;padding:.5rem 1rem;border-radius:.5rem;border:0;background:#111827;color:#fff;cursor:pointer}}code{{background:#f3f4f6;padding:0 .25rem;border-radius:.25rem}}</style></head><body><h1>{title}</h1>{body}</body></html>",
        title = html_escape(title),
    );
    let mut response = (status, html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        ),
    );
    http::secure(headers);
    response
}

fn invalid() -> Response {
    page(
        StatusCode::NOT_FOUND,
        "This link is not valid",
        "<p>It expired, was already used, or was changed. In Slack, run <code>/forma link</code> again.</p>",
    )
}
fn sign_in_first() -> Response {
    page(
        StatusCode::OK,
        "Sign in to Forma first",
        "<p>Sign in to Forma in this browser, then open the link from Slack again.</p><p><a href=\"/\">Go to Forma</a></p>",
    )
}

/// The `t` query parameter, alone.
fn token_of(query: Option<&str>) -> Option<String> {
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(query?.as_bytes())
        .into_owned()
        .collect();
    match pairs.as_slice() {
        [(name, value)] if name == "t" => Some(value.clone()),
        _ => None,
    }
}

/// `GET /slack/link?t=…`.
pub async fn show(State(state): State<Arc<Slack>>, request: Request) -> Response {
    let Some(token) = token_of(request.uri().query()) else {
        return invalid();
    };
    let Some(claim) = state.links.check(&token, super::now()) else {
        return invalid();
    };
    let Some((_, member)) = crate::sharing::session(&state.auth, request.headers()).await else {
        return sign_in_first();
    };
    let who = html_escape(&crate::directory::label(member.principal()));
    page(
        StatusCode::OK,
        "Link Slack to Forma",
        &format!(
            "<p>Slack user <code>{user}</code> in workspace <code>{team}</code> will act as <strong>{who}</strong> in Forma: filing, approving and seeing what you can see on the canvas, nothing more.</p><p>Only continue if you ran <code>/forma link</code> yourself.</p><form method=\"post\" action=\"{PATH}\"><input type=\"hidden\" name=\"t\" value=\"{token}\"><button type=\"submit\">Link my Slack account</button></form>",
            user = html_escape(&claim.user),
            team = html_escape(&claim.team),
            token = html_escape(&token),
        ),
    )
}

/// `POST /slack/link`.
pub async fn confirm(State(state): State<Arc<Slack>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    let token = match fields.as_slice() {
        [(name, value)] if name == "t" => value.clone(),
        _ => return invalid(),
    };
    let now = super::now();
    let Some(claim) = state.links.check(&token, now) else {
        return invalid();
    };
    let Some((_, member)) = crate::sharing::session(&state.auth, &parts.headers).await else {
        return sign_in_first();
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    if !state.links.consume(&claim, now) {
        return invalid();
    }
    match stores
        .workspace
        .link_slack(&member, &claim.team, &claim.user)
        .await
    {
        Ok(()) => {
            tracing::info!(
                event = "slack.linked",
                organization_id = member.organization(),
                principal_id = member.principal(),
                "slack user linked"
            );
            let label = crate::directory::label(member.principal());
            let slack = state.clone();
            let user = claim.user.clone();
            tokio::spawn(async move {
                let text = format!(
                    "You're linked to Forma as *{}*. Try `/forma what's waiting for my approval`.",
                    super::text::escape(&label)
                );
                if let Err(error) = slack.api.direct(&user, &text, None).await {
                    tracing::warn!(
                        event = "slack.dm_failed",
                        code = error.code(),
                        "link confirmation DM not sent"
                    );
                }
            });
            page(
                StatusCode::OK,
                "Slack is linked",
                "<p>Go back to Slack and try <code>/forma what's waiting for my approval</code>.</p>",
            )
        }
        Err(storage::Error::Denied) => invalid(),
        Err(_) => http::unavailable(),
    }
}
