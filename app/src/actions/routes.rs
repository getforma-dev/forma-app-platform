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
//! rehearsed applies), 503 `unavailable`. The session is checked before the
//! body is read (bounded in size and time) and the member's windows before
//! anything acts, the folder path included. The route runs under
//! `http::COMPOSE_DEADLINE` (it waits on the model). The log carries identities, kinds, codes and
//! durations; never the utterance or a value.
//!
//! `GET /v1/work/{app}/{work}/records`: the work's current records (storage
//! `list_records`: `record_read` on the work, RLS, the work fence), newest
//! first, at most `RECORDS_LIMIT`, with the published schema, the declared
//! commands and display labels; the uniform 404 otherwise, and for a query
//! string.
//!
//! APP-109 `/v1/notes`: the member's own saved work of the library's Notes
//! app (sidecar `library_id` "notes"), where `/note` keeps quick notes as
//! records. `GET` answers `{"app_id", "work_id"}` with the first work of that
//! app the member created and may still write (`storage::Data::own_works`),
//! or `"work_id": null`; it writes nothing. `POST` (same origin, empty body)
//! answers the same, creating that work under the member's own authority
//! when there is none (`"created": true`; the composer's `retrieved` binding,
//! no `app.placed` event: the canvas shows notes, not the app). A work shared
//! with the member is never chosen, so a note stays private unless its author
//! shares the work. The notes themselves are written only through the
//! audited command route (`add`, `edit`) and read through the records
//! listing. The uniform 404 when the member may not instantiate the app.
use super::{
    ledger,
    prompt::{self, CatalogApp},
    receipt::{self, BodyRefusal, Receipt},
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
/// Per member, receipts (a vision call each): twenty an hour, inside
/// `ACT_MEMBER` (APP-102).
pub const RECEIPT_MEMBER: &[limits::Window] = &[limits::per(20, 3600)];
/// A receipt photo (up to `RECEIPT_BODY_LIMIT`) may take longer to arrive
/// than a JSON body (`http::HANDLER_DEADLINE`); still bounded, well inside
/// the route's `COMPOSE_DEADLINE`.
pub const RECEIPT_READ_DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);

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
/// The ordinary-deadline router (the records listing, APP-109's notes work).
pub fn records_router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/records", get(records))
        .route(NOTES_PATH, get(notes_work).post(notes_work))
        .with_state(auth)
}

/// APP-109: the member's notes work.
pub const NOTES_PATH: &str = "/v1/notes";
/// The library entry whose records are notes.
pub(crate) const NOTES_LIBRARY_ID: &str = "notes";
/// The created work's context.
const NOTES_CONTEXT: &str = "Quick notes (/note)";

/// `GET` and `POST /v1/notes` (module notes above).
async fn notes_work(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let creating = request.method() == axum::http::Method::POST;
    if creating && !csrf::same_origin(request.headers(), auth.origin()) {
        return http::cross_site();
    }
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let member = match member(&auth, request.headers()).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    if creating {
        if let Err(retry_after) = auth
            .limits
            .admit_member_write(member.organization(), member.principal())
        {
            return http::too_many_requests(retry_after);
        }
    }
    let Some(stores) = auth.stores.as_ref() else {
        return http::not_found();
    };
    let Some(digest) = storage::library_sidecar().ok().and_then(|entries| {
        entries
            .iter()
            .find(|entry| entry.get("library_id").and_then(Value::as_str) == Some(NOTES_LIBRARY_ID))
            .and_then(|entry| entry.get("definition_digest").and_then(Value::as_str))
            .map(str::to_owned)
    }) else {
        return http::not_found();
    };
    let context = match stores.workspace.composer_context(&member).await {
        Ok(context) => context,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    let Some(library) = context
        .library
        .iter()
        .find(|app| app.definition_digest == digest)
    else {
        return http::not_found();
    };
    let own = match stores.data.own_works(&member, &library.app).await {
        Ok(own) => own,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    if let Some(work) = own.into_iter().find(|work| work.can_write) {
        return document(
            StatusCode::OK,
            &json!({"app_id": work.app, "work_id": work.work, "created": false}),
        );
    }
    if !creating {
        return document(
            StatusCode::OK,
            &json!({"app_id": library.app, "work_id": Value::Null, "created": false}),
        );
    }
    let Some(bindings) =
        storage::composer_bindings(&library.inputs, &library.record_schemas, &context.sources)
    else {
        return http::not_found();
    };
    match stores
        .workspace
        .create_work(
            &member,
            &library.app,
            &library.publication,
            NOTES_CONTEXT,
            &bindings,
        )
        .await
    {
        Ok(saved) => {
            tracing::info!(
                event = "notes.work_created",
                organization_id = member.organization(),
                principal_id = member.principal(),
                app_id = library.app,
                work_id = saved.work,
                "notes work created for the member"
            );
            document(
                StatusCode::OK,
                &json!({"app_id": library.app, "work_id": saved.work, "created": true}),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
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

/// The request body: an utterance, a receipt with optional words
/// (multipart, APP-102), or a structured decision.
pub(super) enum Body {
    Utterance(String),
    /// APP-101: an utterance for one app the member named with a slash
    /// command (`{"utterance", "app_id"}`): the parser sees that app alone.
    Pinned {
        utterance: String,
        app: String,
    },
    Receipt {
        utterance: String,
        receipt: Receipt,
    },
    /// The work id is required and validated in the body but not kept:
    /// resolution finds the record's work itself.
    Decide {
        app: String,
        record: String,
        status: String,
        note: Option<String>,
    },
}

/// APP-108: `{"utterance", "box_id"?}`: the words and the folder the canvas
/// named (its active chip or an @-mention), if any.
pub(super) fn folder_body(bytes: &[u8]) -> Option<(String, Option<String>)> {
    let value = crate::admission::decode(bytes).ok()?;
    let object = value.as_object()?;
    if !object.keys().all(|k| k == "utterance" || k == "box_id") {
        return None;
    }
    let utterance = prompt::utterance(object.get("utterance")?.as_str()?)?.to_owned();
    let box_id = match object.get("box_id") {
        None => None,
        Some(v) => Some(v.as_str()?.to_owned()),
    };
    Some((utterance, box_id))
}

pub(super) fn body(bytes: &[u8]) -> Option<Body> {
    let value: Value = crate::admission::decode(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() == 2 {
        let app = object.get("app_id")?.as_str()?;
        let text = object.get("utterance")?.as_str()?;
        if !storage::valid_identifier(app) {
            return None;
        }
        return prompt::utterance(text).map(|t| Body::Pinned {
            utterance: t.to_owned(),
            app: app.to_owned(),
        });
    }
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
    id("work_id")?;
    Some(Body::Decide {
        app: id("app_id")?,
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
        "approvers": draft.approvers.iter().map(|a| json!({"principal_id": a.principal, "label": a.label})).collect::<Vec<_>>(),
        "rule": draft.rule,
        "flags": draft.flags,
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
    // APP-102: a multipart body carries a receipt image (larger limit).
    let multipart = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .filter(|value| receipt::boundary(value).is_some())
        .map(str::to_owned);
    let limit = if multipart.is_some() {
        receipt::RECEIPT_BODY_LIMIT
    } else {
        ACT_BODY_LIMIT
    };
    // Review: the session first (an anonymous caller spends nothing on the
    // body), then the member's windows, then the body under a bounded read;
    // nothing below (the folder path included) runs before admission.
    let member = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let key = format!("act:{}:{}", member.organization(), member.principal());
    let receipt_key = format!(
        "act-receipt:{}:{}",
        member.organization(),
        member.principal()
    );
    let admitted = if multipart.is_some() {
        state
            .auth
            .limits
            .admit(&[(&key, ACT_MEMBER), (&receipt_key, RECEIPT_MEMBER)])
    } else {
        state.auth.limits.admit(&[(&key, ACT_MEMBER)])
    };
    if let Err(retry_after) = admitted {
        log_refused(&member, "too_many_requests", started);
        return http::too_many_requests(retry_after);
    }
    let read_deadline = if multipart.is_some() {
        RECEIPT_READ_DEADLINE
    } else {
        http::HANDLER_DEADLINE
    };
    let bytes = match http::read_bounded(body_stream, limit, read_deadline).await {
        Ok(bytes) => bytes,
        Err(http::BodyRefused::TooLarge) => return http::payload_too_large(),
        Err(http::BodyRefused::Stalled) => {
            log_refused(&member, "body_stalled", started);
            return http::not_found();
        }
    };
    // APP-108: a folder request (the canvas's active folder chip or
    // @-mention as `box_id`, or a folder named in the words) is answered by
    // the folder path (which applies the composer's or the brief's own
    // window too); anything else continues below unchanged.
    if multipart.is_none() && parts.uri.query().is_none() {
        if let Some((utterance, box_id)) = folder_body(&bytes) {
            if let Some(answer) =
                crate::boxes::intent::act(&member, &utterance, box_id.as_deref()).await
            {
                return match answer {
                    crate::boxes::intent::Answer::Done(value) => document(StatusCode::OK, &value),
                    crate::boxes::intent::Answer::NotAFolderRequest => {
                        http::problem(StatusCode::UNPROCESSABLE_ENTITY, "not_a_folder_request")
                    }
                    crate::boxes::intent::Answer::Refused(refused) => refused.response(),
                };
            }
            if box_id.is_some() {
                return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "not_a_folder_request");
            }
        }
    }
    let parsed_body = match multipart.as_deref() {
        Some(content_type) => match receipt::multipart(content_type, &bytes) {
            Ok((utterance, receipt)) => Some(Body::Receipt { utterance, receipt }),
            Err(BodyRefusal::Malformed) => None,
            Err(BodyRefusal::TooLarge) => {
                log_refused(&member, "receipt_too_large", started);
                return http::payload_too_large();
            }
            Err(BodyRefusal::NotAnImage) => {
                log_refused(&member, "invalid_receipt", started);
                return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_receipt");
            }
        },
        None => body(&bytes),
    };
    drop(bytes);
    let Some(body) = parsed_body.filter(|_| parts.uri.query().is_none()) else {
        log_refused(&member, "invalid_utterance", started);
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_utterance");
    };
    let demo = demo::is_demo_org(member.organization());
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
    let mut catalog: Vec<CatalogApp> = prompt::catalog(&context.library);
    if let Body::Pinned { app, .. } = &body {
        // APP-101: a slash command names the app; the parser does not re-pick.
        catalog.retain(|entry| &entry.app == app);
    }
    if catalog.is_empty() {
        // Nothing this member may act on: indistinguishable from a stranger.
        log_refused(&member, "nothing_actionable", started);
        return http::not_found();
    }
    let today = crate::events::today();
    let mut receipt_meta: Option<(&'static str, usize)> = None;
    // "/expense", "/ledger": no parse at all.
    let bare = match &body {
        Body::Utterance(utterance) => ledger::bare_command(utterance, &catalog),
        _ => None,
    };
    let (parse, utterance): (Parse, String) = match (body, bare) {
        (Body::Utterance(utterance), Some(parsed)) => (
            Parse {
                parsed,
                rehearsed: false,
                duration_ms: 0,
            },
            utterance,
        ),
        (Body::Receipt { utterance, receipt }, _) => {
            receipt_meta = Some((receipt.media_type, receipt.bytes.len()));
            if !state.parser.enabled() && !demo {
                log_refused(&member, "composer_unavailable", started);
                return http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
            }
            let parse = state
                .parser
                .parse_receipt(&utterance, &receipt, &catalog, demo, &today)
                .await;
            // The image leaves memory here, before resolution (DS-56).
            drop(receipt);
            match parse {
                Ok(parse) => {
                    let utterance = if utterance.is_empty() {
                        String::from("receipt")
                    } else {
                        utterance
                    };
                    (parse, utterance)
                }
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
        (Body::Utterance(utterance) | Body::Pinned { utterance, .. }, _) => {
            if !state.parser.enabled() && !demo {
                log_refused(&member, "composer_unavailable", started);
                return http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
            }
            match state.parser.parse(&utterance, &catalog, demo, &today).await {
                Ok(mut parse) => {
                    parse.parsed = ledger::with_range(parse.parsed, &utterance, &today);
                    (parse, utterance)
                }
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
        (
            Body::Decide {
                app,
                record,
                status,
                note,
            },
            _,
        ) => {
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
        receipt = receipt_meta.is_some(),
        receipt_media_type = receipt_meta.map(|(media_type, _)| media_type),
        receipt_bytes = receipt_meta.map(|(_, length)| length),
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
            let mut answer = draft_document(&draft, parse.duration_ms);
            answer["receipt"] = Value::Bool(receipt_meta.is_some());
            document(StatusCode::OK, &answer)
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
            let range = ledger::range_of(&parse.parsed.values);
            // The ledger is the expense ledger: other record apps open their records.
            let expense = super::receipt::expense_app(&catalog).is_some_and(|e| e.app == app);
            let view = if expense && ledger::wants_ledger(&utterance, &parse.parsed.values) {
                "ledger"
            } else {
                "records"
            };
            document(
                StatusCode::OK,
                &json!({
                    "action": "open", "app_id": app, "work_id": work, "title": title, "placed": placed,
                    "view": view,
                    "range": range.map(|(from, to)| json!({"from": from, "to": to})),
                }),
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
    records_document_with(listing, &[])
}

/// The records listing as JSON, each record carrying `via`: the label of the
/// agent token that committed its current version through MCP (APP-099's
/// "via Claude" chip; display provenance, never authority), or null.
pub fn records_document_with(
    listing: &RecordsListing,
    attributions: &[storage::AgentAttribution],
) -> Value {
    let via = |record: &str, version: i64| {
        attributions
            .iter()
            .rev()
            .find(|a| {
                a.record
                    .as_ref()
                    .is_some_and(|(r, v)| r == record && *v == version)
            })
            .map(|a| a.label.clone())
    };
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
        "records": listing.records.iter().map(|r| {
            let mut record = json!({
            "record_id": r.record_id,
            "collection_id": r.collection,
            "schema_id": r.schema,
            "version": r.version,
            "commit_sequence": r.commit_sequence,
            "committed_at": r.committed_at,
            "creator": principal_document(&r.creator),
            "updater": principal_document(&r.updater),
            "via": via(&r.record_id, r.version),
            "values": r.values.as_array().into_iter().flatten().filter_map(|item| Some(json!({
                "field_id": item.get("field_id")?,
                "value": item.get("value")?,
                "display": resolve::display(item.get("value")?),
            }))).collect::<Vec<_>>(),
            });
            // APP-107: the transitions that may fire on this record now (its
            // current value, the declared guards judged on its state), so a
            // card offers exactly what storage would accept.
            if let Some(command) = listing
                .collections
                .iter()
                .find(|c| c.collection == r.collection)
                .and_then(|c| c.commands.iter().find(|d| !d.transitions.is_empty()))
            {
                record["next"] = Value::Array(
                    resolve::allowed_transitions(&command.transitions, &command.rules, r)
                        .into_iter()
                        .map(|(_, to, rule)| json!({"to": to, "rule": rule}))
                        .collect(),
                );
            }
            // APP-100 (DS-59): the channel that committed this version, for
            // the canvas's origin chip; a display label only.
            if let Some(via) = crate::origins::label(&r.record_id, r.version) {
                record["via"] = Value::String(via.to_owned());
            }
            record
        }).collect::<Vec<_>>(),
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
            // APP-099: the agent labels of this work, read only after the
            // listing authorized the member; a failed read shows no chip.
            let attributions = stores
                .workspace
                .agent_attributions(&member, &app, &work)
                .await
                .unwrap_or_default();
            // APP-107: display-only annotations (a purchase request's PO
            // reference, an invoice's PO match against the purchase requests
            // this member may read now).
            let (references, flags) = super::workflows::annotate(stores, &member, &listing).await;
            let mut answer = records_document_with(&listing, &attributions);
            if let Some(rows) = answer["records"].as_array_mut() {
                for row in rows {
                    let id = row["record_id"].as_str().unwrap_or("").to_owned();
                    if let Some(reference) = references.get(&id) {
                        row["reference"] = json!(reference);
                    }
                    if let Some(flags) = flags.get(&id) {
                        row["flags"] = json!(flags);
                    }
                }
            }
            document(StatusCode::OK, &answer)
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}
