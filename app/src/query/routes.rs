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
//! - `GET /w/{app}/{work}`: the work page (Forma 3.2 native SSR, no
//!   JavaScript) with every runnable view idle and the run form; or, when the
//!   browser carries this work's run cookie from its own earlier run, the
//!   same views run again with those parameters under a fresh authorization
//!   (reload restores, ADR-010 §1 act 4; below).
//! - `POST /w/{app}/{work}`: the run form (window required, destination
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
//! path is bound through the tag instead. A cookie that is absent renders
//! the idle page; one that is foreign, forged, tampered, from another session
//! or not exactly normalized parameters is ignored and cleared (idle page).
//! Every restored run passes the full authorization again (`run_query`,
//! `release_result`) and produces a fresh receipt and release; the browser's
//! re-submit dialog never appears because the page the member sees is the
//! `GET`. `SameSite=Strict` keeps a cross-site link to the page from carrying
//! the cookie, so such a link runs nothing (design M45 is about state
//! changes; this GET changes none, and still runs only what the same browser
//! itself submitted through the token-checked form).
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
}
impl Executor for Engine {
    fn execute<'a>(&'a self, run: &'a QueryRun, deadline: Duration) -> Execution<'a> {
        Box::pin(Engine::execute(self, run, deadline))
    }
}

/// The routes' state: sessions, the synchronizer key and the stores come
/// from [`Auth`]; `executor` is `None` only where no store is configured.
pub struct Work {
    auth: Arc<Auth>,
    executor: Option<Arc<dyn Executor>>,
    pages: Arc<dyn WorkPages>,
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
        }
    }
}

pub fn router(work: Arc<Work>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/queries/{view}", post(query))
        .route("/v1/library", get(library))
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

fn problem(status: StatusCode, code: &'static str) -> Response {
    http::problem(status, code)
}
fn invalid_parameters() -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_parameters")
}

/// Storage refusals: `Denied` is the uniform 404 (a lock wait during session
/// resolution is already `Denied`, D7.9); every other variant is a store
/// outage (design §9: 503).
pub fn storage_refusal(error: &Error) -> Response {
    match error {
        Error::Denied => http::not_found(),
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
    let Some(stores) = &work.auth.stores else {
        return http::not_found();
    };
    let run = match stores
        .data
        .run_query(&member, &app, &saved, &view, &parameters)
        .await
    {
        Ok(run) => run,
        Err(error) => {
            refused(release_code(&error));
            return storage_refusal(&error);
        }
    };
    let Some(executor) = &work.executor else {
        return http::unavailable();
    };
    let result = match executor.execute(&run, INTERACTIVE_DEADLINE).await {
        Ok(result) => result,
        Err(error) => return engine_refusal(&error),
    };
    // A receipt outside the contract's receipt rule is never delivered.
    let Some(receipt) = receipt::build(&run, &result) else {
        super::worker::log_failed_after_dispatch(&run, "receipt_nonconforming", result.wall);
        return engine_refusal(&QueryError::Failed(Failure::Engine));
    };
    let Ok(body) = serde_json::to_vec(&json!({
        "view_id": run.view(),
        "columns": result.columns,
        "rows": result.rows,
        "truncated_page": result.truncated_page,
        "receipt": receipt.document,
    })) else {
        return engine_refusal(&QueryError::Failed(Failure::Engine));
    };
    // C07: the last authorization check, immediately before the body.
    let released = stores.data.release_result(&member, &run).await;
    log_release(&run, &receipt, &released, started);
    if let Err(error) = released {
        return storage_refusal(&error);
    }
    json_response(body)
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
    let Some(stores) = &work.auth.stores else {
        return http::not_found();
    };
    let outline = match stores.data.work_outline(&member, &app, &saved).await {
        Ok(outline) => outline,
        Err(error) => return storage_refusal(&error),
    };
    let csrf_token = work.auth.csrf.token(csrf::Form::Query, &cookie);
    let mut set_cookie = None;
    let (echo, views) = match restore(&work.auth, &parts.headers, &app, &saved, &cookie) {
        Restore::Absent => (FormValues::default(), idle_views(&outline)),
        Restore::Invalid => {
            set_cookie = Some(cookies::clear(cookies::RUN));
            (FormValues::default(), idle_views(&outline))
        }
        Restore::Parameters(parameters) => {
            let views =
                match run_views(&work, Arc::new(member), &outline, &app, &saved, &parameters).await
                {
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
    let mut response = work.pages.work(&page::props(
        &outline,
        action(&app, &saved),
        csrf_token,
        &echo,
        false,
        views,
    ));
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
    let (Some(start), Some(end)) = (
        form_timestamp(&echo.window_start),
        form_timestamp(&echo.window_end),
    ) else {
        return Err(echo);
    };
    if echo.destination.chars().count() > FORM_DESTINATION_CHARACTERS {
        return Err(echo);
    }
    let mut parameters = json!({"window": {"start": start, "end": end}});
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
    let Some(stores) = &work.auth.stores else {
        return http::not_found();
    };
    let outline = match stores.data.work_outline(&member, &app, &saved).await {
        Ok(outline) => outline,
        Err(error) => return storage_refusal(&error),
    };
    let (parameters, echo) = match form_parameters(&fields) {
        Ok(accepted) => accepted,
        Err(echo) => {
            let csrf_token = work.auth.csrf.token(csrf::Form::Query, &cookie);
            let response = work.pages.work(&page::props(
                &outline,
                action(&app, &saved),
                csrf_token,
                &echo,
                true,
                idle_views(&outline),
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
                match run_views(&work, Arc::new(member), &outline, &app, &saved, &parameters).await
                {
                    Ok(views) => views,
                    Err(refusal) => return refusal,
                };
            work.pages.work(&page::props(
                &outline,
                action(&app, &saved),
                work.auth.csrf.token(csrf::Form::Query, &cookie),
                &echo,
                false,
                views,
            ))
        }
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
pub(crate) mod tests;
