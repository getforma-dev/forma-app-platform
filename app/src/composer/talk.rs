//! `POST /v1/ask` and `POST /v1/shape` (APP-122; module notes in `ask` and
//! `shape`): the two AI jobs on things already on the canvas. Two routes,
//! two closed output shapes: where the member types decides the job.
//!
//! Both are same-origin `POST`s under a live workspace session, bodies of
//! at most [`BODY_LIMIT`], no query string, run under the long deadline (each
//! may wait on the model). Order, as `/v1/compose`: fetch metadata → session
//! → available (a model, or a demo organization's rehearsed path) → member
//! limit → body → shape → the read-only read of the scoped works → answer.
//!
//! `POST /v1/ask`: `{"question": "<1..500>", "scope": {"kind": "card" |
//! "canvas", "address": "<what the chip shows>", "works": [{"app_id",
//! "work_id", "address"}]}}` (1 to 12 works; a card scope names one). Answers
//! exactly `{"text", "receipts", "references"}`.
//!
//! `POST /v1/shape`: `{"address": "<as typed>", "words": "<1..300>",
//! "target": {"kind": "card", "app_id", "work_id", "card", "fields"} |
//! {"kind": "app", "app_id", "work_id"} | {"kind": "widget", "widget",
//! "state"}}`. Answers exactly `{"target": "card"|"app"|"widget", "spec"}`.
//!
//! Refusals: the uniform 404 for no session and for any work the member
//! cannot read (another organization's included: nothing tells the two
//! apart); 403 `cross_site`; 413; 422 `invalid_request`; 422
//! `could_not_answer` (Ask: the model's reply was not an answer, a view spec
//! included) or `could_not_shape` (Shape: the reply was not the target's
//! spec, prose included); 429; 503 `composer_unavailable` (no model and
//! nothing rehearsed); 503 `unavailable`. Logs carry identities, the target
//! kind, codes, counts and durations; never the question, the words, a record
//! value or a spec.
use super::{ask, shape, Composer};
use crate::{
    auth::{cookies, csrf, Auth},
    demo,
    http::{self, limits},
    storage::{self, Lookup, TrustedRequest},
    widgets::{
        customize::{Customizer, Failure},
        state::{self, Widget},
    },
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

pub const ASK_PATH: &str = "/v1/ask";
pub const SHAPE_PATH: &str = "/v1/shape";
pub const BODY_LIMIT: usize = 8192;
/// Per member: questions (several per conversation).
pub const ASK_MEMBER: &[limits::Window] = &[limits::per(120, 3600)];
/// Per member: shapes (several per rehearsal).
pub const SHAPE_MEMBER: &[limits::Window] = &[limits::per(120, 3600)];
const ASK_MAX_TOKENS: u32 = 700;
const ASK_BUDGET: Duration = Duration::from_secs(20);
const SHAPE_MAX_TOKENS: u32 = 500;
const SHAPE_BUDGET: Duration = Duration::from_secs(12);

pub struct Talk {
    auth: Arc<Auth>,
    composer: Composer,
    customizer: Customizer,
}
impl Talk {
    pub fn new(auth: Arc<Auth>, composer: Composer) -> Self {
        let customizer = composer.customizer();
        Self {
            auth,
            composer,
            customizer,
        }
    }
    /// Whether this organization can be answered at all: a model, or the
    /// demo organization's rehearsed path.
    fn available(&self, organization: &str) -> bool {
        self.composer.enabled() || demo::is_demo_org(organization)
    }
}

/// Both routes (the binary mounts this router under the long deadline at
/// each of `ASK_PATH` and `SHAPE_PATH`).
pub fn router(state: Arc<Talk>) -> Router {
    Router::new()
        .route(ASK_PATH, post(ask_route))
        .route(SHAPE_PATH, post(shape_route))
        .with_state(state)
}

async fn member(auth: &Auth, headers: &HeaderMap) -> Result<TrustedRequest, Response> {
    let cookie = cookies::read(headers, cookies::SESSION).ok_or_else(http::not_found)?;
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => Ok(request),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            Err(http::not_found())
        }
        Ok(Lookup::Absent) | Err(storage::Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

fn document(value: &Value) -> Response {
    let Ok(bytes) = crate::admission::encode(value) else {
        return http::unavailable();
    };
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The shared front of both routes: fetch metadata, session, availability,
/// the member limit and the bounded body.
async fn front(
    state: &Talk,
    request: Request,
    route: &str,
    windows: &'static [limits::Window],
) -> Result<(TrustedRequest, Value), Response> {
    let (parts, body_stream) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return Err(http::cross_site());
    }
    let member = member(&state.auth, &parts.headers).await?;
    if parts.uri.query().is_some() {
        return Err(http::problem(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
        ));
    }
    if !state.available(member.organization()) {
        return Err(http::problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "composer_unavailable",
        ));
    }
    let key = format!("{route}:{}:{}", member.organization(), member.principal());
    if let Err(retry_after) = state.auth.limits.admit(&[(&key, windows)]) {
        return Err(http::too_many_requests(retry_after));
    }
    let bytes = match http::read_bounded(body_stream, BODY_LIMIT, http::HANDLER_DEADLINE).await {
        Ok(bytes) => bytes,
        Err(http::BodyRefused::TooLarge) => return Err(http::payload_too_large()),
        Err(http::BodyRefused::Stalled) => return Err(http::not_found()),
    };
    let value: Value = crate::admission::decode(&bytes)
        .map_err(|_| http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request"))?;
    Ok((member, value))
}

/// An address as the canvas shows it (`expenses`, `expenses.ledger`,
/// `calculator`), or empty: lowercase, digits, `-` and one `.`.
fn address(value: Option<&Value>) -> Option<String> {
    let text = match value {
        None | Some(Value::Null) => return Some(String::new()),
        Some(value) => value.as_str()?.trim().trim_start_matches('/'),
    };
    let ok = text.len() <= 81
        && text.matches('.').count() <= 1
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.');
    ok.then(|| text.to_owned())
}

fn exactly(object: &Map<String, Value>, required: &[&str], optional: &[&str]) -> bool {
    required.iter().all(|key| object.contains_key(*key))
        && object
            .keys()
            .all(|key| required.contains(&key.as_str()) || optional.contains(&key.as_str()))
}

/// The ask body (module notes): the question, the chip's address, and each
/// work with its app's address.
pub(crate) struct AskBody {
    pub question: String,
    pub address: String,
    pub works: Vec<(String, String, String)>,
}

pub(crate) fn ask_body(value: &Value) -> Option<AskBody> {
    let object = value.as_object()?;
    if !exactly(object, &["question", "scope"], &[]) {
        return None;
    }
    let question = ask::question(object.get("question")?.as_str()?)?.to_owned();
    let scope = object.get("scope")?.as_object()?;
    if !exactly(scope, &["kind", "works"], &["address"]) {
        return None;
    }
    let kind = scope.get("kind")?.as_str()?;
    let works = scope.get("works")?.as_array()?;
    let count_ok = match kind {
        "card" => works.len() == 1,
        "canvas" => (1..=storage::READ_ONLY_WORKS).contains(&works.len()),
        _ => false,
    };
    if !count_ok {
        return None;
    }
    let mut out = Vec::with_capacity(works.len());
    for work in works {
        let work = work.as_object()?;
        if !exactly(work, &["app_id", "work_id"], &["address"]) {
            return None;
        }
        let (app, id) = (
            work.get("app_id")?.as_str()?,
            work.get("work_id")?.as_str()?,
        );
        if !storage::valid_identifier(app)
            || !storage::valid_identifier(id)
            || out.iter().any(|(_, w, _)| w == id)
        {
            return None;
        }
        out.push((app.to_owned(), id.to_owned(), address(work.get("address"))?));
    }
    Some(AskBody {
        question,
        address: address(scope.get("address"))?,
        works: out,
    })
}

/// `POST /v1/ask`.
async fn ask_route(State(state): State<Arc<Talk>>, request: Request) -> Response {
    let (member, value) = match front(&state, request, "talk.ask", ASK_MEMBER).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let Some(body) = ask_body(&value) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    let keys: Vec<(&str, &str)> = body
        .works
        .iter()
        .map(|(app, work, _)| (app.as_str(), work.as_str()))
        .collect();
    let listings = match stores.data.read_only_listings(&member, &keys).await {
        Ok(listings) => listings,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    let addresses: Vec<String> = body.works.iter().map(|(_, _, a)| a.clone()).collect();
    let rows = ask::rows(&listings, &addresses);
    let log_refused = |code: &str| {
        tracing::info!(
            event = "talk.ask_refused",
            code,
            organization_id = member.organization(),
            principal_id = member.principal(),
            works = keys.len(),
            records = rows.len(),
            duration_ms = elapsed_ms(started),
            "ask refused"
        );
    };
    let rehearsed = demo::is_demo_org(member.organization())
        .then(|| ask::rehearsed(&body.question, &body.address, &rows))
        .flatten();
    let (answer, source) = match rehearsed {
        Some(answer) => (answer, "rehearsed"),
        None if !state.composer.enabled() => {
            log_refused("composer_unavailable");
            return http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
        }
        None => {
            let reply = match state
                .composer
                .complete_fast(
                    ask::SYSTEM,
                    ask::user(&body.question, &body.address, &rows),
                    ASK_MAX_TOKENS,
                    ASK_BUDGET,
                )
                .await
            {
                Ok(reply) => reply,
                Err(error) => {
                    log_refused(error.code());
                    return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_answer");
                }
            };
            let known: BTreeSet<String> = rows.iter().map(|r| r.reference.clone()).collect();
            match ask::judge(&reply.text, &known) {
                Ok(answer) => (answer, "model"),
                Err(code) => {
                    log_refused(code);
                    return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_answer");
                }
            }
        }
    };
    tracing::info!(
        event = "talk.answered",
        source,
        organization_id = member.organization(),
        principal_id = member.principal(),
        works = keys.len(),
        records = rows.len(),
        references = answer.references.len(),
        duration_ms = elapsed_ms(started),
        "ask answered"
    );
    document(&ask::document(&answer, &rows))
}

/// What a shape addresses.
pub(crate) enum Target {
    Card {
        app: String,
        work: String,
        fields: Vec<shape::Field>,
    },
    App {
        app: String,
        work: String,
    },
    Widget {
        widget: Widget,
        state: Value,
    },
}
impl Target {
    fn kind(&self) -> &'static str {
        match self {
            Self::Card { .. } => "card",
            Self::App { .. } => "app",
            Self::Widget { .. } => "widget",
        }
    }
}

pub(crate) fn shape_body(value: &Value) -> Option<(String, String, Target)> {
    let object = value.as_object()?;
    if !exactly(object, &["address", "words", "target"], &[]) {
        return None;
    }
    let address = address(object.get("address"))?;
    let words = shape::words(object.get("words")?.as_str()?)?.to_owned();
    let target = object.get("target")?.as_object()?;
    let ids = |target: &Map<String, Value>| -> Option<(String, String)> {
        let (app, work) = (
            target.get("app_id")?.as_str()?,
            target.get("work_id")?.as_str()?,
        );
        (storage::valid_identifier(app) && storage::valid_identifier(work))
            .then(|| (app.to_owned(), work.to_owned()))
    };
    let target = match target.get("kind")?.as_str()? {
        "card" if exactly(target, &["kind", "app_id", "work_id", "fields"], &["card"]) => {
            let (app, work) = ids(target)?;
            if let Some(card) = target.get("card") {
                let card = card.as_str()?;
                if card.len() > 64
                    || !card
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return None;
                }
            }
            Target::Card {
                app,
                work,
                fields: shape::fields(target.get("fields")?)?,
            }
        }
        "app" if exactly(target, &["kind", "app_id", "work_id"], &[]) => {
            let (app, work) = ids(target)?;
            Target::App { app, work }
        }
        "widget" if exactly(target, &["kind", "widget", "state"], &[]) => {
            let widget = Widget::parse(target.get("widget")?.as_str()?)?;
            let state = state::validate(widget, target.get("state")?)?;
            Target::Widget { widget, state }
        }
        _ => return None,
    };
    Some((address, words, target))
}

/// `POST /v1/shape`.
async fn shape_route(State(state): State<Arc<Talk>>, request: Request) -> Response {
    let (member, value) = match front(&state, request, "talk.shape", SHAPE_MEMBER).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let Some((address, words, target)) = shape_body(&value) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    };
    let kind = target.kind();
    // A card or an app: the member must read the work (one READ ONLY read).
    if let Target::Card { app, work, .. } | Target::App { app, work } = &target {
        let Some(stores) = state.auth.stores.as_ref() else {
            return http::not_found();
        };
        match stores
            .data
            .read_only_listings(&member, &[(app.as_str(), work.as_str())])
            .await
        {
            Ok(_) => {}
            Err(storage::Error::Denied) => return http::not_found(),
            Err(_) => return http::unavailable(),
        }
    }
    let demo = demo::is_demo_org(member.organization());
    let log_refused = |code: &str| {
        tracing::info!(
            event = "talk.shape_refused",
            code,
            target = kind,
            organization_id = member.organization(),
            principal_id = member.principal(),
            duration_ms = elapsed_ms(started),
            "shape refused"
        );
    };
    let unavailable = || http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
    let refused = || http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_shape");
    let model = |system: String| {
        let composer = state.composer.clone();
        let user = shape::user(&address, &words);
        async move {
            composer
                .complete_fast(&system, user, SHAPE_MAX_TOKENS, SHAPE_BUDGET)
                .await
        }
    };
    let (spec, source) = match &target {
        Target::Card { fields, .. } => {
            match demo
                .then(|| shape::rehearsed_card(&words, fields))
                .flatten()
            {
                Some(view) => (serde_json::to_value(view), "rehearsed"),
                None if !state.composer.enabled() => {
                    log_refused("composer_unavailable");
                    return unavailable();
                }
                None => match model(shape::card_system(fields)).await {
                    Ok(reply) => match shape::judge_card(&reply.text, fields) {
                        Ok(view) => (serde_json::to_value(view), "model"),
                        Err(code) => {
                            log_refused(code);
                            return refused();
                        }
                    },
                    Err(error) => {
                        log_refused(error.code());
                        return refused();
                    }
                },
            }
        }
        Target::App { .. } => match demo.then(|| shape::rehearsed_theme(&words)).flatten() {
            Some(theme) => (serde_json::to_value(theme), "rehearsed"),
            None if !state.composer.enabled() => {
                log_refused("composer_unavailable");
                return unavailable();
            }
            None => match model(shape::theme_system()).await {
                Ok(reply) => match shape::judge_theme(&reply.text) {
                    Ok(theme) => (serde_json::to_value(theme), "model"),
                    Err(code) => {
                        log_refused(code);
                        return refused();
                    }
                },
                Err(error) => {
                    log_refused(error.code());
                    return refused();
                }
            },
        },
        // The widget's Restyle mechanism (APP-103), reached through /shape.
        Target::Widget {
            widget,
            state: current,
        } => {
            match state
                .customizer
                .customize(*widget, current, &words, demo)
                .await
            {
                Ok(outcome) => (Ok(outcome.state), outcome.source.as_str()),
                Err(Failure::Unavailable) => {
                    log_refused("composer_unavailable");
                    return unavailable();
                }
                Err(Failure::Refused(code)) => {
                    log_refused(code);
                    return refused();
                }
            }
        }
    };
    let Ok(spec) = spec else {
        return http::unavailable();
    };
    tracing::info!(
        event = "talk.shaped",
        target = kind,
        source,
        organization_id = member.organization(),
        principal_id = member.principal(),
        duration_ms = elapsed_ms(started),
        "shape answered"
    );
    document(&json!({"target": kind, "spec": spec}))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "00000000-0000-4000-8000-000000001681";
    const WORK: &str = "00000000-0000-4000-8000-000000001684";

    #[test]
    fn ask_bodies_are_closed() {
        let good = json!({"question": "? where are the red invoices", "scope": {"kind": "card", "address": "expenses.ledger",
            "works": [{"app_id": APP, "work_id": WORK, "address": "expenses"}]}});
        let body = ask_body(&good).unwrap();
        assert_eq!(body.question, "where are the red invoices");
        assert_eq!(body.address, "expenses.ledger");
        for bad in [
            json!({"question": "x", "scope": {"kind": "card", "works": []}}),
            json!({"question": "x", "scope": {"kind": "card", "works": [{"app_id": APP, "work_id": WORK}, {"app_id": APP, "work_id": APP}]}}),
            json!({"question": "x", "scope": {"kind": "everything", "works": [{"app_id": APP, "work_id": WORK}]}}),
            json!({"question": "x", "scope": {"kind": "canvas", "works": [{"app_id": "1", "work_id": WORK}]}}),
            json!({"question": "x", "scope": {"kind": "canvas", "works": [{"app_id": APP, "work_id": WORK}]}, "spec": {}}),
            json!({"question": "x", "scope": {"kind": "canvas", "address": "<b>", "works": [{"app_id": APP, "work_id": WORK}]}}),
            json!({"question": "", "scope": {"kind": "canvas", "works": [{"app_id": APP, "work_id": WORK}]}}),
        ] {
            assert!(ask_body(&bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn shape_bodies_are_closed() {
        let card = json!({"address": "expenses.ledger", "words": "only red invoices", "target": {"kind": "card",
            "app_id": APP, "work_id": WORK, "card": "ledger", "fields": [{"id": "status", "type": "enum", "values": ["submitted"]}]}});
        assert!(matches!(
            shape_body(&card),
            Some((_, _, Target::Card { .. }))
        ));
        let app = json!({"address": "expenses", "words": "dark green, compact", "target": {"kind": "app", "app_id": APP, "work_id": WORK}});
        assert!(matches!(shape_body(&app), Some((_, _, Target::App { .. }))));
        let widget = json!({"address": "calculator", "words": "make it a TI-84", "target": {"kind": "widget", "widget": "calculator",
            "state": {"skin": "default", "accent": "default", "font": "default"}}});
        assert!(matches!(
            shape_body(&widget),
            Some((_, _, Target::Widget { .. }))
        ));
        for bad in [
            json!({"address": "calculator", "words": "x", "target": {"kind": "widget", "widget": "calculator", "state": {"skin": "<b>", "accent": "default", "font": "default"}}}),
            json!({"address": "expenses", "words": "x", "target": {"kind": "app", "app_id": APP, "work_id": WORK, "css": "x"}}),
            json!({"address": "expenses", "words": "", "target": {"kind": "app", "app_id": APP, "work_id": WORK}}),
            json!({"address": "Expenses!", "words": "x", "target": {"kind": "app", "app_id": APP, "work_id": WORK}}),
            json!({"address": "note", "words": "x", "target": {"kind": "note"}}),
        ] {
            assert!(shape_body(&bad).is_none(), "{bad}");
        }
    }
}
