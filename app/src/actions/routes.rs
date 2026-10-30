//! `POST /v1/act` and `GET /v1/work/{app}/{work}/records` (module notes in
//! `actions`).
//!
//! `POST /v1/act`: a same-origin `POST` under a live workspace session, body
//! of at most `ACT_BODY_LIMIT`, either `{"utterance": "<3..500 characters>"}`
//! (parsed by the model, or deterministically for a rehearsed utterance in a
//! demo organization) or the structured `{"decide": {"app_id", "work_id",
//! "record_id", "status", "note"?}}` (the records card's Approve/Reject
//! buttons: no model, the same resolution and dry run). Answers (forma-json-v1):
//! `{"action":"create"|"decide", "app_id", "work_id", "title", "placed",
//! "command_id", "record_id", "collection_id", "schema_id", "values":[{
//! "field_id","label","display","value"}], "command_request": <the contract's
//! command_request, idempotency key included>, "approver": {"principal_id",
//! "label"}|null, "target": {"record_id","version","summary"}|null,
//! "confirm_token", "parse_ms"}`; `{"action":"open","app_id","work_id",
//! "title","placed"}`; `{"action":"place"}` (the canvas composes). Refusals:
//! the uniform 404 (no session, a storage denial, a member who may act on
//! nothing), 403 `cross_site`, 413, 422 `invalid_utterance`, 422
//! `could_not_parse`, 422 `could_not_draft` (neutral), 429 with `Retry-After`
//! (`ACT_MEMBER`), 503 `composer_unavailable` (no model key and nothing
//! rehearsed applies), 503 `unavailable`. The route runs under `ACT_DEADLINE`
//! (it waits on the model). The log carries identities, kinds, codes and
//! durations; never the utterance or a value.
//!
//! `GET /v1/work/{app}/{work}/records`: the work's current records (storage
//! `list_records`: `record_read` on the work, RLS, the work fence), newest
//! first, at most `RECORDS_LIMIT`, with the published schema, the declared
//! commands and display labels; the uniform 404 otherwise, and for a query
//! string.
use super::{
    prompt::{self, CatalogApp},
    resolve::{self, Draft, Refusal, Resolution},
    Kind, Parse, ParseFailure, Parsed, Parser,
};
use crate::{
    auth::{cookies, csrf, Auth},
    demo, directory,
    http::{self, limits, UniformPath},
    storage::{self, Lookup, RecordsListing, TrustedRequest},
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Map, Value};
use std::{sync::Arc, time::Instant};

pub const ACT_BODY_LIMIT: usize = 4096;
/// Per member: sixty actions an hour (several per rehearsed scenario).
pub const ACT_MEMBER: &[limits::Window] = &[limits::per(60, 3600)];

pub struct Act {
    auth: Arc<Auth>,
    parser: Parser,
}
impl Act {
    pub fn new(auth: Arc<Auth>, parser: Parser) -> Self {
        Self { auth, parser }
    }
}

/// The long-deadline router (`/v1/act`).
pub fn router(state: Arc<Act>) -> Router {
    Router::new()
        .route(super::PATH, post(act))
        .with_state(state)
}
/// The ordinary-deadline router (the records listing).
pub fn records_router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/records", get(records))
        .with_state(auth)
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
        Ok(Lookup::Absent) => Err(http::not_found()),
        Err(storage::Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

fn document(status: StatusCode, value: &Value) -> Response {
    let Ok(bytes) = crate::admission::encode(value) else {
        return http::unavailable();
    };
    let mut response = (status, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

/// Today in UTC as `YYYY-MM-DD` (the member's date for defaults).
fn today() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::events::timestamp(seconds)[..10].to_owned()
}

/// The request body: an utterance, or a structured decision.
pub(super) enum Body {
    Utterance(String),
    Decide {
        app: String,
        work: String,
        record: String,
        status: String,
        note: Option<String>,
    },
}

pub(super) fn body(bytes: &[u8]) -> Option<Body> {
    let value: Value = crate::admission::decode(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    if let Some(text) = object.get("utterance") {
        return prompt::utterance(text.as_str()?).map(|t| Body::Utterance(t.to_owned()));
    }
    let decide = object.get("decide")?.as_object()?;
    if !decide.keys().all(|k| {
        matches!(
            k.as_str(),
            "app_id" | "work_id" | "record_id" | "status" | "note"
        )
    }) {
        return None;
    }
    let id = |key: &str| {
        decide
            .get(key)?
            .as_str()
            .filter(|v| storage::valid_identifier(v))
            .map(str::to_owned)
    };
    let status = decide.get("status")?.as_str()?;
    if !storage::valid_local_id(status) {
        return None;
    }
    let note = match decide.get("note") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.chars().count() > 200 || text.chars().any(char::is_control) {
                return None;
            }
            (!text.is_empty()).then(|| text.to_owned())
        }
        Some(_) => return None,
    };
    Some(Body::Decide {
        app: id("app_id")?,
        work: id("work_id")?,
        record: id("record_id")?,
        status: status.to_owned(),
        note,
    })
}

fn log_refused(member: &TrustedRequest, code: &str, started: Instant) {
    tracing::info!(
        event = "action.refused",
        code,
        organization_id = member.organization(),
        principal_id = member.principal(),
        duration_ms = elapsed_ms(started),
        "action refused"
    );
}
fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn draft_document(draft: &Draft, parse_ms: u64) -> Value {
    json!({
        "action": draft.kind.as_str(),
        "app_id": draft.app,
        "work_id": draft.work,
        "title": draft.title,
        "placed": draft.placed,
        "command_id": draft.request.command,
        "record_id": draft.request.record,
        "collection_id": draft.collection,
        "schema_id": draft.request.schema,
        "values": draft.values.iter().map(|v| json!({
            "field_id": v.field, "label": v.label, "display": v.display, "value": v.value,
        })).collect::<Vec<_>>(),
        "command_request": draft.request.document(),
        "approver": draft.approver.as_ref().map(|a| json!({"principal_id": a.principal, "label": a.label})),
        "target": draft.target.as_ref().map(|t| json!({"record_id": t.record_id, "version": t.version, "summary": t.summary})),
        "confirm_token": draft.confirm_token,
        "parse_ms": parse_ms,
    })
}

fn refusal_response(refusal: &Refusal) -> Response {
    match refusal {
        Refusal::NotFound => http::not_found(),
        Refusal::StoreUnavailable => http::unavailable(),
        Refusal::CouldNotDraft(_) => {
            http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_draft")
        }
    }
}

/// `POST /v1/act`.
async fn act(State(state): State<Arc<Act>>, request: Request) -> Response {
    let (parts, body_stream) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let Ok(bytes) = to_bytes(body_stream, ACT_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let member = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let Some(body) = body(&bytes).filter(|_| parts.uri.query().is_none()) else {
        log_refused(&member, "invalid_utterance", started);
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_utterance");
    };
    let demo = demo::is_demo_org(member.organization());
    let key = format!("act:{}:{}", member.organization(), member.principal());
    if let Err(retry_after) = state.auth.limits.admit(&[(&key, ACT_MEMBER)]) {
        log_refused(&member, "too_many_requests", started);
        return http::too_many_requests(retry_after);
    }
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    let context = match stores.workspace.composer_context(&member).await {
        Ok(context) => context,
        Err(storage::Error::Denied) => {
            log_refused(&member, "context_denied", started);
            return http::not_found();
        }
        Err(_) => return http::unavailable(),
    };
    let catalog: Vec<CatalogApp> = prompt::catalog(&context.library);
    if catalog.is_empty() {
        // Nothing this member may act on: indistinguishable from a stranger.
        log_refused(&member, "nothing_actionable", started);
        return http::not_found();
    }
    let today = today();
    let (parse, utterance): (Parse, String) = match body {
        Body::Utterance(utterance) => {
            if !state.parser.enabled() && !demo {
                log_refused(&member, "composer_unavailable", started);
                return http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
            }
            match state.parser.parse(&utterance, &catalog, demo, &today).await {
                Ok(parse) => (parse, utterance),
                Err(ParseFailure::Unavailable) => {
                    log_refused(&member, "composer_unavailable", started);
                    return http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
                }
                Err(ParseFailure::CouldNotParse(code)) => {
                    log_refused(&member, code, started);
                    return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_parse");
                }
            }
        }
        Body::Decide {
            app,
            work,
            record,
            status,
            note,
        } => {
            // The structured decision: the hint is the record id itself, the
            // app the work's; resolution and the dry run judge the rest.
            let Some(catalog_app) = catalog.iter().find(|c| c.app == app) else {
                log_refused(&member, "unknown_app", started);
                return http::not_found();
            };
            let mut values = Map::new();
            values.insert("status".into(), Value::String(status));
            if let Some(note) = note {
                values.insert("note".into(), Value::String(note));
            }
            let parsed = Parsed {
                kind: Kind::Decide,
                app_hint: catalog_app.app.clone(),
                values,
                approver_name: None,
                target_record_hint: Some(record),
            };
            let _ = work;
            (
                Parse {
                    parsed,
                    rehearsed: false,
                    duration_ms: 0,
                },
                String::from("decide"),
            )
        }
    };
    tracing::info!(
        event = "action.parsed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        kind = parse.parsed.kind.as_str(),
        rehearsed = parse.rehearsed,
        has_approver = parse.parsed.approver_name.is_some(),
        value_count = parse.parsed.values.len(),
        duration_ms = parse.duration_ms,
        "utterance parsed"
    );
    let resolution = resolve::resolve(
        stores,
        &member,
        &context.library,
        &catalog,
        &parse.parsed,
        &utterance,
        &today,
    )
    .await;
    match resolution {
        Ok(Resolution::Draft(draft)) => {
            tracing::info!(
                event = "action.drafted",
                organization_id = member.organization(),
                principal_id = member.principal(),
                kind = draft.kind.as_str(),
                app_id = draft.app,
                work_id = draft.work,
                command_id = draft.request.command,
                placed = draft.placed,
                approver_resolved = draft.approver.is_some(),
                confirm_token = draft.confirm_token,
                duration_ms = elapsed_ms(started),
                "confirmation card drafted"
            );
            document(StatusCode::OK, &draft_document(&draft, parse.duration_ms))
        }
        Ok(Resolution::Open {
            app,
            work,
            title,
            placed,
        }) => {
            tracing::info!(
                event = "action.open",
                organization_id = member.organization(),
                principal_id = member.principal(),
                app_id = app,
                work_id = work,
                placed,
                duration_ms = elapsed_ms(started),
                "open resolved"
            );
            document(
                StatusCode::OK,
                &json!({"action": "open", "app_id": app, "work_id": work, "title": title, "placed": placed}),
            )
        }
        Ok(Resolution::Place) => document(StatusCode::OK, &json!({"action": "place"})),
        Err(refusal) => {
            let code = match &refusal {
                Refusal::NotFound => "denied",
                Refusal::StoreUnavailable => "unavailable",
                Refusal::CouldNotDraft(code) => code,
            };
            log_refused(&member, code, started);
            refusal_response(&refusal)
        }
    }
}

fn principal_document(principal: &str) -> Value {
    json!({"principal_id": principal, "label": directory::label(principal)})
}

/// The records listing as JSON.
pub fn records_document(listing: &RecordsListing) -> Value {
    json!({
        "app_id": listing.app,
        "work_id": listing.work,
        "publication_id": listing.publication,
        "definition_digest": listing.definition_digest,
        "work_version": listing.work_version,
        "can_write": listing.can_write,
        "collections": listing.collections.iter().map(|c| json!({
            "collection_id": c.collection,
            "schema_id": c.schema,
            "schema_version": c.schema_version,
            "fields": c.fields.iter().map(|f| {
                let mut field = json!({"id": f.id, "type": f.value_type.get("type").cloned().unwrap_or(Value::Null), "nullable": f.nullable, "writable": f.writable});
                if let Some(values) = f.value_type.get("values") {
                    field["values"] = values.clone();
                }
                field
            }).collect::<Vec<_>>(),
            "commands": c.commands.iter().map(|d| json!({
                "id": d.id, "operation": d.operation, "writable_fields": d.writable_fields,
                "transitions": d.transitions.iter().map(|(f, a, b)| json!({"field_id": f, "from": a, "to": b})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "records": listing.records.iter().map(|r| json!({
            "record_id": r.record_id,
            "collection_id": r.collection,
            "schema_id": r.schema,
            "version": r.version,
            "commit_sequence": r.commit_sequence,
            "committed_at": r.committed_at,
            "creator": principal_document(&r.creator),
            "updater": principal_document(&r.updater),
            "values": r.values.as_array().into_iter().flatten().filter_map(|item| Some(json!({
                "field_id": item.get("field_id")?,
                "value": item.get("value")?,
                "display": resolve::display(item.get("value")?),
            }))).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "truncated": listing.truncated,
    })
}

/// `GET /v1/work/{app}/{work}/records`.
async fn records(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !(storage::valid_identifier(&app) && storage::valid_identifier(&work))
        || request.uri().query().is_some()
    {
        return http::not_found();
    }
    let member = match member(&auth, request.headers()).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let Some(stores) = auth.stores.as_ref() else {
        return http::not_found();
    };
    match stores.data.list_records(&member, &app, &work).await {
        Ok(listing) => {
            tracing::info!(
                event = "records.listed",
                organization_id = member.organization(),
                principal_id = member.principal(),
                app_id = app,
                work_id = work,
                count = listing.records.len(),
                "records listed"
            );
            document(StatusCode::OK, &records_document(&listing))
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}
