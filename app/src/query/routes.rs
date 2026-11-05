//! APP-012 routes (plan Task 9; ADR-007 D7.9, D7.10, D7.14, D7.15; design
//! §§6.3, 7.2, 9, 11.3; ADR-010 §1 act 4):
//!
//! - `POST /v1/work/{app}/{work}/queries/{view}`: one typed query, JSON in
//!   and out. Body `{"parameters": {window?, destination?, filter_epoch?}}`
//!   of at most 64 KiB; 200 `{view_id, columns, rows, truncated_page,
//!   receipt}`. A view that declares no window of its own and is sent no
//!   `window` parameter computes over the whole bound revision (for the
//!   seeded Northstar `total`, 2,040,000 bytes with a receipt that records no
//!   window parameter); the S02 numbers need the window parameter. Requiring
//!   one would be a product rule, not an authority or contract rule (routes
//!   open question 3).
//! - `GET /w/{app}/{work}`: the app document (Forma 3.2 native SSR with
//!   command/live enhancement), source views using admitted defaults; when the
//!   browser carries this work's run cookie from its own earlier run, the
//!   same views run again with those parameters under a fresh authorization
//!   (reload restores, ADR-010 §1 act 4; below).
//! - `POST /w/{app}/{work}`: the run form (complete window optional, destination
//!   optional). Valid parameters are normalized into the run cookie and
//!   answered `303 See Other` back to `GET /w/{app}/{work}`, which runs every
//!   runnable view and shows each view's state and receipt; a refused form
//!   renders the page with status 422 and nothing run.
//!
//! **Reload restores (POST/redirect/GET).** The run cookie
//! (`cookies::RUN`, `__Host-`, `Secure`, `HttpOnly`, `SameSite=Strict`,
//! `Path=/`, 10 minutes, refreshed by each run it restores) holds a
//! 43-character tag followed by the base64url of the canonical validated
//! parameters, and nothing else: no authority, no secret, no filter in any
//! URL (a query string stays the uniform 404, design §11.3). The tag is the
//! process key's HMAC over the work path, the session cookie and the
//! parameter bytes (`csrf::CsrfKey::run_tag`), so the cookie restores only
//! that work's run for that session; `__Host-` forbids a path scope, so the
//! path is bound through the tag instead. An absent cookie runs the admitted
//! defaults; one that is foreign, forged, tampered, from another session
//! or not exactly normalized parameters is ignored and cleared (idle page).
//! Every restored run passes the full authorization again (`run_query`,
//! `release_result`) and produces a fresh receipt and release; the browser's
//! re-submit dialog never appears because the page the member sees is the
//! `GET`. `SameSite=Strict` keeps a cross-site link to the page from carrying
//! the cookie, so such a link uses the admitted defaults (design M45 concerns
//! state changes; this GET changes none). A closed, CSRF-protected POST body
//! with document_action=refresh selects an existing record for a declared
//! form; it only reads and does not execute a command.
//!
//! Refusal order: fetch metadata (403 `cross_site`) → bounded body (413) →
//! a live workspace session (else the uniform 404, so anonymous, pre-access,
//! expired, revoked and foreign callers are indistinguishable; a lock wait on
//! the member's own row during session resolution is also the uniform 404,
//! D7.9, decided in `resolve_session`; 503 only when the workspace store
//! itself is unavailable) → the run form's synchronizer token (403) →
//! parameters (422 `invalid_parameters`; filters travel in bodies only, so
//! any query string is refused, design §11.3; a window bound must be a
//! contract timestamp naming a real day) → authorization (`Data::run_query`;
//! `Denied` → 404) → the engine (`Busy` and a request that expired in the
//! admission queue → 429 `Retry-After: 2`; `NarrowTheQuery`, a query
//! interrupted at its deadline or killed at its memory line → 422
//! `narrow_the_query` (design §6.3: a query past the budget is 422, never a
//! partial number); `Refused` → 422 `query_refused`; `SourceUnavailable` →
//! 503 `source_unavailable`; an engine or protocol failure → 503
//! `query_failed`) → the receipt, checked against the contract's receipt rule
//! (a receipt the contract validator would refuse is a failed execution) →
//! `Data::release_result` immediately before the body is written (`Denied` →
//! 404). Nothing reaches the engine before authorization, and no number
//! leaves without release.
use super::{
    page::{self, FormValues, Outcome, Reason, WorkPages},
    receipt,
    worker::{
        Engine, Failure, QueryError, WorkerResult, INTERACTIVE_DEADLINE, RETRY_AFTER_SECONDS,
    },
};
use crate::{
    admission,
    auth::{cookies, csrf, Auth},
    http::{self, UniformPath},
    storage::{parse_parameters, Error, Lookup, QueryRun, TrustedRequest, WorkOutline, WorkView},
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::{json, Map, Value};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

/// ADR-007 D7.14: JSON query routes.
pub const QUERY_BODY_LIMIT: usize = 64 * 1024;
/// The run form's destination bound (Unicode scalar values): a page rule that
/// keeps the run cookie inside `cookies::RUN_MAX_VALUE`; the JSON route keeps
/// the contract's 2,000-character text bound.
pub const FORM_DESTINATION_CHARACTERS: usize = 256;

/// A pending engine execution.
pub type Execution<'a> =
    Pin<Box<dyn Future<Output = Result<WorkerResult, QueryError>> + Send + 'a>>;

/// What runs an authorized [`QueryRun`]: the confined engine in the binary;
/// counting and race-injecting wrappers in tests (the dispatch counter).
pub trait Executor: Send + Sync {
    fn execute<'a>(&'a self, run: &'a QueryRun, deadline: Duration) -> Execution<'a>;
    fn execute_records<'a>(
        &'a self,
        _run: &'a crate::storage::RecordComponentsRun,
        _parameters: &'a Value,
        _deadline: Duration,
    ) -> super::record_components::RecordExecution<'a> {
        Box::pin(async { Err(QueryError::Refused("record_engine_unavailable".into())) })
    }
}
impl Executor for Engine {
    fn execute<'a>(&'a self, run: &'a QueryRun, deadline: Duration) -> Execution<'a> {
        Box::pin(Engine::execute(self, run, deadline))
    }
    fn execute_records<'a>(
        &'a self,
        run: &'a crate::storage::RecordComponentsRun,
        parameters: &'a Value,
        deadline: Duration,
    ) -> super::record_components::RecordExecution<'a> {
        Box::pin(Engine::execute_records(self, run, parameters, deadline))
    }
}

/// The routes' state: sessions, the synchronizer key and the stores come
/// from [`Auth`]; `executor` is `None` only where no store is configured.
pub struct Work {
    auth: Arc<Auth>,
    executor: Option<Arc<dyn Executor>>,
    pages: Arc<dyn WorkPages>,
    pub(crate) shared_pages: Option<Arc<dyn crate::sharing::live::SharedPages>>,
}
impl Work {
    /// The sessions, synchronizer key and stores (the pitch canvas, ADR-011).
    pub(crate) fn auth(&self) -> &Arc<Auth> {
        &self.auth
    }
    pub fn new(
        auth: Arc<Auth>,
        executor: Option<Arc<dyn Executor>>,
        pages: Arc<dyn WorkPages>,
    ) -> Self {
        Self {
            auth,
            executor,
            pages,
            shared_pages: None,
        }
    }
    pub fn with_shared_pages(mut self, pages: Arc<dyn crate::sharing::live::SharedPages>) -> Self {
        self.shared_pages = Some(pages);
        self
    }
    pub(crate) fn executor(&self) -> Option<&Arc<dyn Executor>> {
        self.executor.as_ref()
    }
}

pub fn router(work: Arc<Work>) -> Router {
    Router::new()
        .route(
            "/v1/work/{app}/{work}/record-components",
            get(super::record_routes::member_route).post(super::record_routes::member_route),
        )
        .route("/v1/work/{app}/{work}/queries/{view}", post(query))
        .route("/v1/library", get(library))
        .route("/v1/library/{app}/{work}/kind", post(set_artifact_kind))
        .route("/w/{app}/{work}", get(work_page).post(run_page))
        .with_state(work)
}

/// `GET /v1/library` (ADR-011 library, APP-094): the admitted library apps
/// the signed-in member may open now, with the router sidecar's metadata, as
/// `storage::Data::library_catalog` lists them under the session's tenant
/// scope. No session, a refused member or a denied listing is the uniform
/// 404; a store outage is 503. Nothing runs and nothing is written.
async fn library(State(work): State<Arc<Work>>, headers: HeaderMap) -> Response {
    let (_, member) = match member(&work.auth, &headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &work.auth.stores else {
        return http::not_found();
    };
    let entries = match stores.data.library_catalog(&member).await {
        Ok(entries) => entries,
        Err(error) => return storage_refusal(&error),
    };
    let body = json!({
        "schema_version": 1,
        "synthetic": true,
        "apps": entries.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
    });
    match serde_json::to_vec(&body) {
        Ok(bytes) => json_response(bytes),
        Err(_) => http::unavailable(),
    }
}

/// `POST /v1/library/{app}/{work}/kind` (APP-169). Body `{"kind":"card"}`
/// records that whole work as a Card for this member. `{"kind":"app"}`
/// writes `app`, which is the reversible step back. Component count is not
/// read. A work the member cannot open is the uniform 404, the same answer
/// as a missing work.
async fn set_artifact_kind(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    if parts.uri.query().is_some()
        || !crate::storage::valid_identifier(&app)
        || !crate::storage::valid_identifier(&saved)
    {
        return http::not_found();
    }
    let Ok((_, member)) = member(auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(bytes) = http::bounded(body, 256).await else {
        return http::payload_too_large();
    };
    let kind = match serde_json::from_slice::<Value>(&bytes)
        .ok()
        .as_ref()
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        .and_then(crate::storage::parse_artifact_kind)
    {
        Some(kind) => kind,
        None => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_kind"),
    };
    match stores
        .workspace
        .set_artifact_kind(&member, &app, &saved, kind)
        .await
    {
        Ok(Some("card")) => json_response(br#"{"kind":"card"}"#.to_vec()),
        Ok(None) => json_response(br#"{"kind":"app"}"#.to_vec()),
        Ok(_) => http::unavailable(),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

fn problem(status: StatusCode, code: &'static str) -> Response {
    http::problem(status, code)
}
fn invalid_parameters() -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_parameters")
}

/// Storage refusals: `Denied` is the uniform 404 (a lock wait during session
/// resolution is already `Denied`, D7.9). Snapshot capacity is retryable;
/// a complete projection over the resource bound must be narrowed. Other
/// variants are store outages (design §9: 503).
pub fn storage_refusal(error: &Error) -> Response {
    match error {
        Error::Denied => http::not_found(),
        Error::Busy => http::too_many_requests(RETRY_AFTER_SECONDS),
        Error::ProjectionLimit => problem(StatusCode::UNPROCESSABLE_ENTITY, "narrow_the_query"),
        _ => http::unavailable(),
    }
}
/// Engine refusals (design §§6.3, 9; APP-012 review m2): a full queue or a
/// request that expired in it is 429 with `Retry-After` (nothing ran); a
/// query past its budget (interrupted, late or killed) and an unpaginated
/// result over one page are 422 `narrow_the_query`; only an engine or
/// protocol failure is 503 `query_failed`.
pub fn engine_refusal(error: &QueryError) -> Response {
    match error {
        QueryError::Busy | QueryError::Failed(Failure::QueueExpired) => {
            http::too_many_requests(RETRY_AFTER_SECONDS)
        }
        QueryError::NarrowTheQuery
        | QueryError::Failed(Failure::Deadline)
        | QueryError::Failed(Failure::Killed) => {
            problem(StatusCode::UNPROCESSABLE_ENTITY, "narrow_the_query")
        }
        QueryError::Refused(_) => problem(StatusCode::UNPROCESSABLE_ENTITY, "query_refused"),
        QueryError::SourceUnavailable => {
            problem(StatusCode::SERVICE_UNAVAILABLE, "source_unavailable")
        }
        QueryError::Failed(Failure::Engine) => {
            problem(StatusCode::SERVICE_UNAVAILABLE, "query_failed")
        }
    }
}

/// A live workspace session and its `TrustedRequest`; otherwise the response
/// to send: the uniform 404 for no, an unknown, an ended or a refused session
/// (an ended record is destroyed and its provider token revoked, as `GET
/// /workspace`; a lock wait on the member's row is refused by
/// `resolve_session`, D7.9), and 503 when the workspace store itself is
/// unavailable, which depends on no identity.
pub(crate) async fn member(
    auth: &Auth,
    headers: &HeaderMap,
) -> Result<(String, TrustedRequest), Response> {
    let cookie = cookies::read(headers, cookies::SESSION).ok_or_else(http::not_found)?;
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => Ok((cookie, request)),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            Err(http::not_found())
        }
        Ok(Lookup::Absent) => Err(http::not_found()),
        Err(error) => Err(storage_refusal(&error)),
    }
}

/// `{"parameters": {…}}` (or `{}`), nothing else; the parameters themselves
/// are judged by `storage::parse_parameters`.
fn body_parameters(bytes: &[u8]) -> Option<Value> {
    let value: Value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(bytes).ok()?
    };
    let object = value.as_object()?;
    if !object.keys().all(|k| k == "parameters") {
        return None;
    }
    let parameters = object.get("parameters").cloned().unwrap_or(json!({}));
    parse_parameters(&parameters)?;
    Some(parameters)
}

/// APP-076 G6: the structured query log. One line per release decision
/// (`query.released`, `query.release_denied`) with the receipt identity and
/// the result digest, and one per refusal before any dispatch
/// (`query.refused_before_dispatch` with its code); the engine logs
/// `query.dispatch` itself. Identities only: no parameter value, no secret.
fn log_release(
    run: &QueryRun,
    receipt: &receipt::QueryReceipt,
    outcome: &Result<(), Error>,
    started: Instant,
) {
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let result_digest = run_result_digest(receipt);
    match outcome {
        Ok(()) => tracing::info!(
            event = "query.released",
            organization_id = run.organization(),
            app_id = run.app(),
            work_id = run.work(),
            view_id = run.view(),
            receipt_id = receipt.digest,
            result_digest,
            duration_ms,
            "result released"
        ),
        Err(error) => tracing::info!(
            event = "query.release_denied",
            code = release_code(error),
            organization_id = run.organization(),
            app_id = run.app(),
            work_id = run.work(),
            view_id = run.view(),
            receipt_id = receipt.digest,
            result_digest,
            duration_ms,
            "result not released"
        ),
    }
}
fn run_result_digest(receipt: &receipt::QueryReceipt) -> &str {
    receipt.document["result_digest"].as_str().unwrap_or("")
}
fn release_code(error: &Error) -> &'static str {
    match error {
        Error::Denied => "denied",
        Error::Busy => "snapshot_busy",
        Error::ProjectionLimit => "narrow_the_query",
        _ => "store_unavailable",
    }
}
/// A refusal before any `QueryRun` exists (the route's own parameter check or
/// a storage refusal at authorization), with the path's identities.
fn log_refused(
    organization: &str,
    app: &str,
    work: &str,
    view: &str,
    code: &str,
    started: Instant,
) {
    tracing::info!(
        event = "query.refused_before_dispatch",
        code,
        organization_id = organization,
        app_id = app,
        work_id = work,
        view_id = view,
        duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "query refused before dispatch"
    );
}

fn json_response(body: Vec<u8>) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

/// `POST /v1/work/{app}/{work}/queries/{view}`.
async fn query(
    State(work): State<Arc<Work>>,
    // An undecodable path segment is the uniform 404, not a 400 (G7).
    UniformPath((app, saved, view)): UniformPath<(String, String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, work.auth.origin()) {
        return http::cross_site();
    }
    let Ok(bytes) = to_bytes(body, QUERY_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let (_, member) = match member(&work.auth, &parts.headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let refused =
        |code: &str| log_refused(member.organization(), &app, &saved, &view, code, started);
    if parts.uri.query().is_some() {
        refused("invalid_parameters");
        return invalid_parameters();
    }
    let Some(parameters) = body_parameters(&bytes) else {
        refused("invalid_parameters");
        return invalid_parameters();
    };
    match run_released(&work, &member, &app, &saved, &view, &parameters, started).await {
        Ok(document) => match serde_json::to_vec(&document) {
            Ok(body) => json_response(body),
            Err(_) => engine_refusal(&QueryError::Failed(Failure::Engine)),
        },
        Err(ViewRefusal::Storage(error)) => storage_refusal(&error),
        Err(ViewRefusal::Engine(error)) => engine_refusal(&error),
        Err(ViewRefusal::NoEngine) => http::unavailable(),
    }
}

/// Why one view run produced no released document.
#[derive(Debug)]
pub(crate) enum ViewRefusal {
    /// Authorization or release refused it, or the store failed.
    Storage(Error),
    /// The engine refused or failed it, or its receipt was nonconforming.
    Engine(QueryError),
    /// No engine is configured.
    NoEngine,
}

/// One authorized, executed, receipted and released view run (the JSON
/// query route's body), shared with the MCP `forma_run_view` tool (APP-099):
/// `Data::run_query` (authorization), the engine, the receipt checked against
/// the contract's rule, and `Data::release_result` last, immediately before
/// the document is handed back (C07). `parameters` must already have passed
/// `storage::parse_parameters`.
pub(crate) async fn run_released(
    work: &Work,
    member: &TrustedRequest,
    app: &str,
    saved: &str,
    view: &str,
    parameters: &Value,
    started: Instant,
) -> Result<Value, ViewRefusal> {
    let Some(stores) = &work.auth.stores else {
        return Err(ViewRefusal::Storage(Error::Denied));
    };
    let run = match stores
        .data
        .run_query(member, app, saved, view, parameters)
        .await
    {
        Ok(run) => run,
        Err(error) => {
            log_refused(
                member.organization(),
                app,
                saved,
                view,
                release_code(&error),
                started,
            );
            return Err(ViewRefusal::Storage(error));
        }
    };
    let Some(executor) = &work.executor else {
        return Err(ViewRefusal::NoEngine);
    };
    let result = executor
        .execute(&run, INTERACTIVE_DEADLINE)
        .await
        .map_err(ViewRefusal::Engine)?;
    // A receipt outside the contract's receipt rule is never delivered.
    let Some(receipt) = receipt::build(&run, &result) else {
        super::worker::log_failed_after_dispatch(&run, "receipt_nonconforming", result.wall);
        return Err(ViewRefusal::Engine(QueryError::Failed(Failure::Engine)));
    };
    let document = json!({
        "view_id": run.view(),
        "columns": result.columns,
        // APP-093 review M4: each column's value type, for display only.
        "column_types": super::worker::column_types_json(&result.column_types),
        "rows": result.rows,
        "truncated_page": result.truncated_page,
        "receipt": receipt.document,
    });
    // C07: the last authorization check, immediately before the body.
    let released = stores.data.release_result(member, &run).await;
    log_release(&run, &receipt, &released, started);
    released.map_err(ViewRefusal::Storage)?;
    Ok(document)
}

fn action(app: &str, saved: &str) -> String {
    format!("/w/{app}/{saved}")
}

/// The run cookie's value for validated `parameters` of one work and
/// session: tag ‖ base64url(canonical parameters). `None` if the value would
/// exceed the cookie bound (the form's destination bound prevents it).
pub fn run_cookie_value(
    auth: &Auth,
    app: &str,
    saved: &str,
    cookie: &str,
    parameters: &Value,
) -> Option<String> {
    let canonical = admission::encode(parameters).ok()?;
    let tag = auth.csrf.run_tag(app, saved, cookie, &canonical);
    let value = format!("{tag}{}", URL_SAFE_NO_PAD.encode(&canonical));
    (value.len() <= cookies::RUN_MAX_VALUE).then_some(value)
}

/// What a request's run cookie says about this work for this session.
#[derive(Debug, Eq, PartialEq)]
pub enum Restore {
    /// No run cookie: the idle page.
    Absent,
    /// A cookie that is not this session's own run of this work, as
    /// submitted: ignored and cleared.
    Invalid,
    /// This session's validated parameters for this work.
    Parameters(Value),
}

/// Reads the run cookie for `app`/`saved` under this session's cookie: the
/// tag must verify over the exact bytes, which must decode as forma-json-v1
/// to a closed, normalized parameter object.
pub fn restore(auth: &Auth, headers: &HeaderMap, app: &str, saved: &str, cookie: &str) -> Restore {
    let Some(value) = cookies::read_bounded(headers, cookies::RUN, cookies::RUN_MAX_VALUE) else {
        return Restore::Absent;
    };
    let Some((tag, encoded)) = value.split_at_checked(43) else {
        return Restore::Invalid;
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(encoded) else {
        return Restore::Invalid;
    };
    if !auth.csrf.verify_run(app, saved, cookie, &bytes, tag) {
        return Restore::Invalid;
    }
    let Ok(parameters) = admission::decode(&bytes) else {
        return Restore::Invalid;
    };
    // Exactly the normalized parameters: closed, valid and canonical.
    let normalized = parse_parameters(&parameters).is_some()
        && admission::encode(&parameters).is_ok_and(|canonical| canonical == bytes);
    if !normalized {
        return Restore::Invalid;
    }
    Restore::Parameters(parameters)
}

/// The form values a restored run shows back: the window as the
/// `datetime-local` control spells it (`YYYY-MM-DDTHH:MM:SS`, UTC).
pub fn echo_of(parameters: &Value) -> FormValues {
    let bound = |key: &str| {
        parameters["window"][key]
            .as_str()
            .and_then(|value| value.strip_suffix('Z'))
            .unwrap_or("")
            .to_owned()
    };
    FormValues {
        window_start: bound("start"),
        window_end: bound("end"),
        destination: parameters["destination"].as_str().unwrap_or("").to_owned(),
    }
}

fn idle_views(outline: &WorkOutline) -> Vec<page::CardProps> {
    outline
        .views
        .iter()
        .map(|view| page::card(view, Outcome::Idle))
        .collect()
}

pub(crate) struct DocumentForms {
    pub forms: Vec<super::canvas::FormCardProps>,
    pub choices: std::collections::BTreeMap<String, Vec<page::SelectionProps>>,
}

/// Shared native/canvas form preparation, still backed by fresh record details.
/// Callers must release their whole response after this helper and any queries.
pub(crate) async fn document_forms(
    auth: &Auth,
    member: &TrustedRequest,
    cookie: &str,
    outline: &WorkOutline,
    listing: &crate::storage::RecordsListing,
    selections: &std::collections::BTreeMap<String, String>,
) -> Result<DocumentForms, Response> {
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    let (app, saved) = (listing.app.as_str(), listing.work.as_str());
    let mut forms = Vec::new();
    let mut form_choices = std::collections::BTreeMap::new();
    if selections.keys().any(|id| {
        !outline.parts.iter().any(|part| {
            part.id == *id
                && part.kind == "form"
                && !part.creates
                && (selections[id] != "new" || part.operation == "create")
        })
    }) {
        return Err(http::not_found());
    }
    if outline.can_write {
        for part in outline.parts.iter().filter(|part| part.kind == "form") {
            let (collection, _) = outline
                .collections
                .iter()
                .find(|(_, schema)| schema == &part.schema)
                .ok_or_else(http::unavailable)?;
            let creating =
                part.operation == "create" && selections.get(&part.id).is_none_or(|id| id == "new");
            let record = if creating {
                crate::auth::capability::random_uuid().ok_or_else(http::unavailable)?
            } else {
                let selected = selections.get(&part.id).or_else(|| {
                    listing
                        .records
                        .iter()
                        .find(|record| record.collection == *collection)
                        .map(|record| &record.record_id)
                });
                let Some(selected) = selected else { continue };
                selected.clone()
            };
            let key = crate::auth::capability::random_hex::<16>().ok_or_else(http::unavailable)?;
            let detail = stores
                .data
                .read_record_detail(member, app, saved, collection, &record)
                .await
                .map_err(|e| storage_refusal(&e))?;
            if detail.definition_digest != outline.definition_digest
                || detail.publication != listing.publication
                || detail.work_version != listing.work_version
                || detail.can_write != listing.can_write
            {
                return Err(http::unavailable());
            }
            if !creating && detail.version == 0 {
                return Err(http::not_found());
            }
            if !part.creates {
                let mut choices: Vec<_> = listing
                    .records
                    .iter()
                    .filter(|row| row.collection == *collection)
                    .map(|row| {
                        let label = row
                            .values
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|item| item["value"]["value"].as_str())
                            .next()
                            .unwrap_or("Record");
                        page::SelectionProps {
                            id: row.record_id.clone(),
                            label: format!(
                                "{} · revision {}",
                                label.chars().take(100).collect::<String>(),
                                row.version
                            ),
                            selected: row.record_id == record,
                        }
                    })
                    .collect();
                if part.operation == "create" {
                    choices.insert(
                        0,
                        page::SelectionProps {
                            id: "new".into(),
                            label: "New record".into(),
                            selected: creating,
                        },
                    );
                }
                if !creating && !choices.iter().any(|item| item.id == record) {
                    choices.push(page::SelectionProps {
                        id: record.clone(),
                        label: format!("Selected record · revision {}", detail.version),
                        selected: true,
                    });
                }
                form_choices.insert(part.id.clone(), choices);
            }
            let view = crate::commands::view_for_command(
                app,
                saved,
                &detail,
                auth.csrf.token(csrf::Form::Command, cookie),
                key,
                Some(&part.command),
            );
            if let Some(mut form) = super::canvas::form_card(saved, &view) {
                form.id = part.id.clone();
                form.component = part.id.clone();
                form.title = part.title.clone();
                forms.push(form);
            }
        }
    }
    Ok(DocumentForms {
        forms,
        choices: form_choices,
    })
}

/// Read the document's outline and records under one authorization and pin.
/// Each form is a fresh authoritative record detail; a publication change or
/// later denial fails the whole page rather than combining stale premises.
async fn document(
    work: &Work,
    member: &TrustedRequest,
    cookie: &str,
    app: &str,
    saved: &str,
    selections: &std::collections::BTreeMap<String, String>,
) -> Result<(WorkOutline, page::WorkChrome), Response> {
    let stores = work.auth.stores.as_ref().ok_or_else(http::not_found)?;
    let mut reads = stores
        .data
        .work_reads(member, &[(app, saved, true)])
        .await
        .map_err(|e| storage_refusal(&e))?;
    let read = reads.pop().ok_or_else(http::unavailable)?;
    let outline = read.outline.map_err(|e| storage_refusal(&e))?;
    let listing = read
        .records
        .ok_or_else(http::unavailable)?
        .map_err(|e| storage_refusal(&e))?;
    let prepared =
        document_forms(&work.auth, member, cookie, &outline, &listing, selections).await?;
    let (record_components, record_run) = if outline
        .parts
        .iter()
        .any(|part| super::record_routes::is_record_kind(&part.kind))
    {
        match super::record_routes::member_components(
            work,
            member,
            app,
            saved,
            &super::record_components::default_parameters(),
        )
        .await
        {
            Ok(batch) => (batch.components, batch.run),
            Err(response) if response.status() == StatusCode::NOT_FOUND => return Err(response),
            Err(_) => (super::record_routes::unavailable_outline(&outline), None),
        }
    } else {
        (Vec::new(), None)
    };
    let record_cards = page::automatic_records(
        &outline,
        super::canvas::records_cards(app, saved, &listing, member.principal()),
    );
    let records_truncated = listing.truncated && !record_cards.is_empty();
    let chrome = page::WorkChrome {
        record_components,
        record_run,
        publication: listing.publication.clone(),
        work_version: listing.work_version,
        can_write: listing.can_write,
        can_edit_presentation: !crate::composer::matcher::seeded_work(saved)
            && stores
                .workspace
                .can_edit_presentation(member, app, saved)
                .await
                .map_err(|e| storage_refusal(&e))?,
        can_share: stores
            .workspace
            .can_share_live_work(member, app, saved)
            .await
            .map_err(|e| storage_refusal(&e))?,
        app: app.into(),
        work: saved.into(),
        forms: prepared.forms,
        form_choices: prepared.choices,
        record_cards,
        records_truncated,
        canvas: page::canvas_href(saved),
        ..Default::default()
    };
    Ok((outline, chrome))
}

/// Release the assembled document only while its work is still readable and
/// pinned to the same revision. Called after any potentially long view runs.
async fn release_document(
    work: &Work,
    member: &TrustedRequest,
    outline: &WorkOutline,
    chrome: &page::WorkChrome,
) -> Result<(), Response> {
    let stores = work.auth.stores.as_ref().ok_or_else(http::not_found)?;
    let mut reads = stores
        .data
        .work_reads(member, &[(chrome.app.as_str(), chrome.work.as_str(), true)])
        .await
        .map_err(|e| storage_refusal(&e))?;
    let read = reads.pop().ok_or_else(http::unavailable)?;
    let current = read.outline.map_err(|e| storage_refusal(&e))?;
    let listing = read
        .records
        .ok_or_else(http::unavailable)?
        .map_err(|e| storage_refusal(&e))?;
    if &current != outline
        || listing.publication != chrome.publication
        || listing.work_version != chrome.work_version
        || listing.can_write != chrome.can_write
    {
        return Err(http::unavailable());
    }
    if let Some(run) = &chrome.record_run {
        stores
            .data
            .release_record_components(member, run)
            .await
            .map_err(|error| storage_refusal(&error))?;
    }
    Ok(())
}

/// `GET /w/{app}/{work}`: the work page, every view idle; with this work's
/// run cookie, the views run again with its parameters (reload restores).
async fn work_page(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, _) = request.into_parts();
    let (cookie, member) = match member(&work.auth, &parts.headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    // Filters travel in bodies, never URLs (design §11.3).
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let member = Arc::new(member);
    let (outline, chrome) =
        match document(&work, &member, &cookie, &app, &saved, &Default::default()).await {
            Ok(document) => document,
            Err(refusal) => return refusal,
        };
    let csrf_token = work.auth.csrf.token(csrf::Form::Query, &cookie);
    let mut set_cookie = None;
    let (echo, views) = match restore(&work.auth, &parts.headers, &app, &saved, &cookie) {
        Restore::Absent => {
            let views =
                match run_views(&work, member.clone(), &outline, &app, &saved, &json!({})).await {
                    Ok(views) => views,
                    Err(refusal) => return refusal,
                };
            (FormValues::default(), views)
        }
        Restore::Invalid => {
            set_cookie = Some(cookies::clear(cookies::RUN));
            (FormValues::default(), idle_views(&outline))
        }
        Restore::Parameters(parameters) => {
            let views =
                match run_views(&work, member.clone(), &outline, &app, &saved, &parameters).await {
                    Ok(views) => views,
                    Err(refusal) => return refusal,
                };
            // A run that was restored keeps restoring: the cookie is refreshed.
            if let Some(value) = run_cookie_value(&work.auth, &app, &saved, &cookie, &parameters) {
                set_cookie = Some(cookies::set_run(&value));
            }
            (echo_of(&parameters), views)
        }
    };
    if let Err(refusal) = release_document(&work, &member, &outline, &chrome).await {
        return refusal;
    }
    let props = page::props(
        &outline,
        action(&app, &saved),
        csrf_token,
        &echo,
        false,
        views,
        chrome,
    );
    // Same document and release checks, with typed data for live enhancement.
    // No HTML is parsed in the browser (the full page's Trusted Types stays).
    let mut response = if parts
        .headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        == Some("application/json")
    {
        match serde_json::to_vec(&props) {
            Ok(bytes) => json_response(bytes),
            Err(_) => return http::unavailable(),
        }
    } else {
        work.pages.work(&props)
    };
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Accept"));
    if let Some(value) = set_cookie {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

/// A form timestamp: `YYYY-MM-DDTHH:MM` (the `datetime-local` control),
/// with optional `:SS` and `Z`, read as UTC, in the contract's spelling;
/// `storage::parse_parameters` then judges the window itself.
pub fn form_timestamp(value: &str) -> Option<String> {
    let value = value.trim();
    match value.len() {
        16 => Some(format!("{value}:00Z")),
        19 => Some(format!("{value}Z")),
        20 if value.ends_with('Z') => Some(value.to_owned()),
        _ => None,
    }
}

/// The run form: exactly `csrf`, `window_start`, `window_end` and
/// `destination` (empty: no filter; at most
/// [`FORM_DESTINATION_CHARACTERS`]), each once. The window is required.
pub fn form_parameters(fields: &[(String, String)]) -> Result<(Value, FormValues), FormValues> {
    let mut seen = Map::new();
    for (name, value) in fields {
        if !matches!(
            name.as_str(),
            "csrf" | "window_start" | "window_end" | "destination"
        ) || seen.insert(name.clone(), json!(value)).is_some()
        {
            return Err(FormValues::default());
        }
    }
    let field = |name: &str| {
        seen.get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let echo = FormValues {
        window_start: field("window_start"),
        window_end: field("window_end"),
        destination: field("destination"),
    };
    if echo.destination.chars().count() > FORM_DESTINATION_CHARACTERS {
        return Err(echo);
    }
    let mut parameters = json!({});
    if !echo.window_start.is_empty() || !echo.window_end.is_empty() {
        let (Some(start), Some(end)) = (
            form_timestamp(&echo.window_start),
            form_timestamp(&echo.window_end),
        ) else {
            return Err(echo);
        };
        parameters["window"] = json!({"start": start, "end": end});
    }
    if !echo.destination.is_empty() {
        parameters["destination"] = json!(echo.destination);
    }
    match parse_parameters(&parameters) {
        Some(_) => Ok((parameters, echo)),
        None => Err(echo),
    }
}

/// One view computed but not yet released.
enum Pending {
    Done(Box<page::CardProps>),
    Computed(Box<(QueryRun, WorkerResult, receipt::QueryReceipt)>),
}

async fn compute(
    work: Arc<Work>,
    member: Arc<TrustedRequest>,
    view: WorkView,
    app: String,
    saved: String,
    parameters: Value,
    started: Instant,
) -> (WorkView, Pending) {
    let Some(stores) = &work.auth.stores else {
        return (
            view.clone(),
            Pending::Done(Box::new(page::card(&view, Outcome::Denied))),
        );
    };
    let run = match stores
        .data
        .run_query(&member, &app, &saved, &view.id, &parameters)
        .await
    {
        Ok(run) => run,
        Err(error) => {
            log_refused(
                member.organization(),
                &app,
                &saved,
                &view.id,
                release_code(&error),
                started,
            );
            let card = match error {
                Error::Denied => page::card(&view, Outcome::Denied),
                Error::Busy => page::card(&view, Outcome::Loading),
                Error::ProjectionLimit => {
                    page::card(&view, Outcome::Failed(Reason::NarrowTheQuery))
                }
                _ => page::card(&view, Outcome::Failed(Reason::Store)),
            };
            return (view, Pending::Done(Box::new(card)));
        }
    };
    let Some(executor) = &work.executor else {
        let card = page::card(&view, Outcome::Failed(Reason::Store));
        return (view, Pending::Done(Box::new(card)));
    };
    let outcome = match executor.execute(&run, INTERACTIVE_DEADLINE).await {
        Ok(result) => match receipt::build(&run, &result) {
            Some(receipt) => Pending::Computed(Box::new((run, result, receipt))),
            None => {
                super::worker::log_failed_after_dispatch(
                    &run,
                    "receipt_nonconforming",
                    result.wall,
                );
                Pending::Done(Box::new(page::card(&view, Outcome::Failed(Reason::Engine))))
            }
        },
        // Nothing ran: the organization's admission was full or the request
        // expired in its queue (the busy card; review m2).
        Err(QueryError::Busy) | Err(QueryError::Failed(Failure::QueueExpired)) => {
            Pending::Done(Box::new(page::card(&view, Outcome::Loading)))
        }
        Err(error) => Pending::Done(Box::new(page::card(
            &view,
            Outcome::Failed(match error {
                QueryError::NarrowTheQuery => Reason::NarrowTheQuery,
                QueryError::Refused(_) => Reason::NotQualified,
                QueryError::SourceUnavailable => Reason::SourceUnavailable,
                QueryError::Failed(Failure::Deadline) | QueryError::Failed(Failure::Killed) => {
                    Reason::Deadline
                }
                QueryError::Failed(_) | QueryError::Busy => Reason::Engine,
            }),
        ))),
    };
    (view, outcome)
}

/// Runs every runnable view with `parameters`, concurrently (the engine
/// admits two per organization); each result is released immediately before
/// the page is rendered (C07). `Err` is the response for a failure that is
/// no view's own (a task that could not be joined).
pub(crate) async fn run_views(
    work: &Arc<Work>,
    member: Arc<TrustedRequest>,
    outline: &WorkOutline,
    app: &str,
    saved: &str,
    parameters: &Value,
) -> Result<Vec<page::CardProps>, Response> {
    let Some(stores) = &work.auth.stores else {
        return Err(http::not_found());
    };
    let started = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    for (index, view) in outline.views.iter().enumerate() {
        let task = compute(
            work.clone(),
            member.clone(),
            view.clone(),
            app.to_owned(),
            saved.to_owned(),
            parameters.clone(),
            started,
        );
        tasks.spawn(async move { (index, task.await) });
    }
    let mut computed: Vec<Option<(WorkView, Pending)>> =
        (0..outline.views.len()).map(|_| None).collect();
    while let Some(joined) = tasks.join_next().await {
        let Ok((index, outcome)) = joined else {
            return Err(http::unavailable());
        };
        computed[index] = Some(outcome);
    }
    // C07: every release re-authorizes, immediately before the page body.
    let mut views = Vec::with_capacity(computed.len());
    for entry in computed {
        let Some((view, pending)) = entry else {
            return Err(http::unavailable());
        };
        views.push(match pending {
            Pending::Done(card) => *card,
            Pending::Computed(computed) => {
                let (run, result, receipt) = *computed;
                let released = stores.data.release_result(&member, &run).await;
                log_release(&run, &receipt, &released, started);
                match released {
                    Ok(()) => page::card(&view, Outcome::Released(&result, &receipt)),
                    Err(Error::Denied) => page::card(&view, Outcome::Denied),
                    Err(_) => page::card(&view, Outcome::Failed(Reason::Store)),
                }
            }
        });
    }
    Ok(views)
}

/// Closed read-only form selection, sent in a CSRF-protected body rather
/// than a URL. At most one selected existing record per admitted form.
fn document_selections(
    fields: &[(String, String)],
) -> Option<std::collections::BTreeMap<String, String>> {
    let mut result = std::collections::BTreeMap::new();
    let mut action = false;
    for (name, value) in fields {
        match name.as_str() {
            "csrf" => {}
            "document_action" if !action && value == "refresh" => action = true,
            _ => {
                let view = name.strip_prefix("selection.")?;
                if !(view.len() <= 64
                    && view.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                    && view
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
                    || !(value == "new" || crate::storage::valid_identifier(value))
                    || result.insert(view.into(), value.clone()).is_some()
                {
                    return None;
                }
            }
        }
    }
    (action && result.len() <= 24).then_some(result)
}

/// `POST /w/{app}/{work}`: validates the run form, stores its normalized
/// parameters in the run cookie and redirects to the page, which runs them
/// (reload restores). A refused form renders the page (422) with nothing
/// run.
async fn run_page(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, work.auth.origin()) {
        return http::cross_site();
    }
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    let (cookie, member) = match member(&work.auth, &parts.headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let token = fields
        .iter()
        .filter(|(name, _)| name == "csrf")
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>();
    if !matches!(token.as_slice(), [token] if work.auth.csrf.verify(csrf::Form::Query, &cookie, token))
    {
        return http::cross_site();
    }
    let selections = if fields.iter().any(|(name, _)| name == "document_action") {
        match document_selections(&fields) {
            Some(selections) => Some(selections),
            None => return http::not_found(),
        }
    } else {
        None
    };
    let member = Arc::new(member);
    let (outline, chrome) = match document(
        &work,
        &member,
        &cookie,
        &app,
        &saved,
        selections.as_ref().unwrap_or(&Default::default()),
    )
    .await
    {
        Ok(document) => document,
        Err(refusal) => return refusal,
    };
    if selections.is_some() {
        let parameters = match restore(&work.auth, &parts.headers, &app, &saved, &cookie) {
            Restore::Parameters(parameters) => parameters,
            _ => json!({}),
        };
        let views =
            match run_views(&work, member.clone(), &outline, &app, &saved, &parameters).await {
                Ok(views) => views,
                Err(refusal) => return refusal,
            };
        if let Err(refusal) = release_document(&work, &member, &outline, &chrome).await {
            return refusal;
        }
        let props = page::props(
            &outline,
            action(&app, &saved),
            work.auth.csrf.token(csrf::Form::Query, &cookie),
            &echo_of(&parameters),
            false,
            views,
            chrome,
        );
        return if parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            == Some("application/json")
        {
            match serde_json::to_vec(&props) {
                Ok(bytes) => json_response(bytes),
                Err(_) => http::unavailable(),
            }
        } else {
            work.pages.work(&props)
        };
    }
    let (parameters, echo) = match form_parameters(&fields) {
        Ok(accepted) => accepted,
        Err(echo) => {
            let csrf_token = work.auth.csrf.token(csrf::Form::Query, &cookie);
            if let Err(refusal) = release_document(&work, &member, &outline, &chrome).await {
                return refusal;
            }
            let response = work.pages.work(&page::props(
                &outline,
                action(&app, &saved),
                csrf_token,
                &echo,
                true,
                idle_views(&outline),
                chrome,
            ));
            return crate::auth::routes::with_status(response, StatusCode::UNPROCESSABLE_ENTITY);
        }
    };
    match run_cookie_value(&work.auth, &app, &saved, &cookie, &parameters) {
        Some(value) => {
            // POST/redirect/GET: the page the member sees is the GET, which
            // runs the saved parameters; a reload repeats it with no dialog.
            let mut response = http::see_other(&action(&app, &saved));
            response
                .headers_mut()
                .append(header::SET_COOKIE, cookies::set_run(&value));
            response
        }
        None => {
            // Parameters the cookie cannot carry (unreachable through the
            // bounded form): run and render directly, with no restore.
            let views =
                match run_views(&work, member.clone(), &outline, &app, &saved, &parameters).await {
                    Ok(views) => views,
                    Err(refusal) => return refusal,
                };
            if let Err(refusal) = release_document(&work, &member, &outline, &chrome).await {
                return refusal;
            }
            work.pages.work(&page::props(
                &outline,
                action(&app, &saved),
                work.auth.csrf.token(csrf::Form::Query, &cookie),
                &echo,
                false,
                views,
                chrome,
            ))
        }
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
pub(crate) mod tests;
