//! APP-012 routes (plan Task 9; ADR-007 D7.9, D7.10, D7.14, D7.15; design
//! §§6.3, 7.2, 9, 11.3):
//!
//! - `POST /v1/work/{app}/{work}/queries/{view}`: one typed query, JSON in
//!   and out. Body `{"parameters": {window?, destination?, filter_epoch?}}`
//!   of at most 64 KiB; 200 `{view_id, columns, rows, truncated_page,
//!   receipt}`.
//! - `GET /w/{app}/{work}`: the work page (Forma 3.2 native SSR, no
//!   JavaScript) with every runnable view idle and the run form.
//! - `POST /w/{app}/{work}`: the run form (window required, destination
//!   optional); every runnable view runs and the page shows each view's
//!   state and receipt.
//!
//! Refusal order: fetch metadata (403 `cross_site`) → bounded body (413) →
//! a live workspace session (else the uniform 404, so anonymous, pre-access,
//! expired, revoked and foreign callers are indistinguishable; 503 only when
//! the workspace store itself is unavailable or saturated) → the run
//! form's synchronizer token (403) → parameters (422 `invalid_parameters`;
//! filters travel in bodies only, so any query string is refused, design
//! §11.3) → authorization (`Data::run_query`; `Denied` → 404) → the engine
//! (`Busy` → 429 `Retry-After: 2`; `NarrowTheQuery` → 422
//! `narrow_the_query`; `Refused` → 422 `query_refused`; `SourceUnavailable`
//! → 503 `source_unavailable`; `Failed` → 503 `query_failed`) → the receipt
//! → `Data::release_result` immediately before the body is written
//! (`Denied` → 404). Nothing reaches the engine before authorization, and no
//! number leaves without release.
use super::{
    page::{self, FormValues, Outcome, Reason, WorkPages},
    receipt,
    worker::{
        Engine, Failure, QueryError, WorkerResult, INTERACTIVE_DEADLINE, RETRY_AFTER_SECONDS,
    },
};
use crate::{
    auth::{cookies, csrf, Auth},
    http,
    storage::{parse_parameters, Error, Lookup, QueryRun, TrustedRequest, WorkView},
};
use axum::{
    body::to_bytes,
    extract::{rejection::PathRejection, Path, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Map, Value};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

/// ADR-007 D7.14: JSON query routes.
pub const QUERY_BODY_LIMIT: usize = 64 * 1024;

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
        .route("/w/{app}/{work}", get(work_page).post(run_page))
        .with_state(work)
}

fn problem(status: StatusCode, code: &'static str) -> Response {
    http::problem(status, code)
}
fn invalid_parameters() -> Response {
    problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_parameters")
}

/// Storage refusals: `Denied` is the uniform 404; every other variant is a
/// store outage (design §9, D7.9: 503).
pub fn storage_refusal(error: &Error) -> Response {
    match error {
        Error::Denied => http::not_found(),
        _ => http::unavailable(),
    }
}
/// Engine refusals (design §9; the APP-012 storage half's mapping).
pub fn engine_refusal(error: &QueryError) -> Response {
    match error {
        QueryError::Busy => http::too_many_requests(RETRY_AFTER_SECONDS),
        QueryError::NarrowTheQuery => problem(StatusCode::UNPROCESSABLE_ENTITY, "narrow_the_query"),
        QueryError::Refused(_) => problem(StatusCode::UNPROCESSABLE_ENTITY, "query_refused"),
        QueryError::SourceUnavailable => {
            problem(StatusCode::SERVICE_UNAVAILABLE, "source_unavailable")
        }
        QueryError::Failed(_) => problem(StatusCode::SERVICE_UNAVAILABLE, "query_failed"),
    }
}

/// A live workspace session and its `TrustedRequest`; otherwise the response
/// to send: the uniform 404 for no, an unknown, an ended or a refused session
/// (an ended record is destroyed and its provider token revoked, as `GET
/// /workspace`), and 503 when the workspace store itself is unavailable or
/// saturated (D7.9), which depends on no identity.
async fn member(auth: &Auth, headers: &HeaderMap) -> Result<(String, TrustedRequest), Response> {
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
    path: Result<Path<(String, String, String)>, PathRejection>,
    request: Request,
) -> Response {
    // An undecodable path segment is the uniform 404, not a 400.
    let Ok(Path((app, saved, view))) = path else {
        return http::not_found();
    };
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
    if parts.uri.query().is_some() {
        return invalid_parameters();
    }
    let Some(parameters) = body_parameters(&bytes) else {
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
        Err(error) => return storage_refusal(&error),
    };
    let Some(executor) = &work.executor else {
        return http::unavailable();
    };
    let result = match executor.execute(&run, INTERACTIVE_DEADLINE).await {
        Ok(result) => result,
        Err(error) => return engine_refusal(&error),
    };
    let Some(receipt) = receipt::build(&run, &result) else {
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
    if let Err(error) = stores.data.release_result(&member, &run).await {
        return storage_refusal(&error);
    }
    json_response(body)
}

fn action(app: &str, saved: &str) -> String {
    format!("/w/{app}/{saved}")
}

/// `GET /w/{app}/{work}`: the work page, every view idle.
async fn work_page(
    State(work): State<Arc<Work>>,
    path: Result<Path<(String, String)>, PathRejection>,
    request: Request,
) -> Response {
    let Ok(Path((app, saved))) = path else {
        return http::not_found();
    };
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
    let views = outline
        .views
        .iter()
        .map(|view| page::card(view, Outcome::Idle))
        .collect();
    work.pages.work(&page::props(
        &outline,
        action(&app, &saved),
        work.auth.csrf.token(csrf::Form::Query, &cookie),
        &FormValues::default(),
        false,
        views,
    ))
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
/// `destination` (empty: no filter), each once. The window is required.
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
        Err(Error::Denied) => {
            let card = page::card(&view, Outcome::Denied);
            return (view, Pending::Done(Box::new(card)));
        }
        Err(_) => {
            let card = page::card(&view, Outcome::Failed(Reason::Store));
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
            None => Pending::Done(Box::new(page::card(&view, Outcome::Failed(Reason::Engine)))),
        },
        Err(QueryError::Busy) => Pending::Done(Box::new(page::card(&view, Outcome::Loading))),
        Err(error) => Pending::Done(Box::new(page::card(
            &view,
            Outcome::Failed(match error {
                QueryError::NarrowTheQuery => Reason::NarrowTheQuery,
                QueryError::Refused(_) => Reason::NotQualified,
                QueryError::SourceUnavailable => Reason::SourceUnavailable,
                QueryError::Failed(Failure::Deadline) => Reason::Deadline,
                QueryError::Failed(_) | QueryError::Busy => Reason::Engine,
            }),
        ))),
    };
    (view, outcome)
}

/// `POST /w/{app}/{work}`: runs every runnable view with the form's window
/// and destination, concurrently (the engine admits two per organization);
/// each result is released immediately before the page is rendered.
async fn run_page(
    State(work): State<Arc<Work>>,
    path: Result<Path<(String, String)>, PathRejection>,
    request: Request,
) -> Response {
    let Ok(Path((app, saved))) = path else {
        return http::not_found();
    };
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
    let csrf_token = work.auth.csrf.token(csrf::Form::Query, &cookie);
    let (parameters, echo) = match form_parameters(&fields) {
        Ok(accepted) => accepted,
        Err(echo) => {
            let views = outline
                .views
                .iter()
                .map(|view| page::card(view, Outcome::Idle))
                .collect();
            let response = work.pages.work(&page::props(
                &outline,
                action(&app, &saved),
                csrf_token,
                &echo,
                true,
                views,
            ));
            return crate::auth::routes::with_status(response, StatusCode::UNPROCESSABLE_ENTITY);
        }
    };
    let member = Arc::new(member);
    let mut tasks = tokio::task::JoinSet::new();
    for (index, view) in outline.views.iter().enumerate() {
        let task = compute(
            work.clone(),
            member.clone(),
            view.clone(),
            app.clone(),
            saved.clone(),
            parameters.clone(),
        );
        tasks.spawn(async move { (index, task.await) });
    }
    let mut computed: Vec<Option<(WorkView, Pending)>> =
        (0..outline.views.len()).map(|_| None).collect();
    while let Some(joined) = tasks.join_next().await {
        let Ok((index, outcome)) = joined else {
            return http::unavailable();
        };
        computed[index] = Some(outcome);
    }
    // C07: every release re-authorizes, immediately before the page body.
    let mut views = Vec::with_capacity(computed.len());
    for entry in computed {
        let Some((view, pending)) = entry else {
            return http::unavailable();
        };
        views.push(match pending {
            Pending::Done(card) => *card,
            Pending::Computed(computed) => {
                let (run, result, receipt) = *computed;
                match stores.data.release_result(&member, &run).await {
                    Ok(()) => page::card(&view, Outcome::Released(&result, &receipt)),
                    Err(Error::Denied) => page::card(&view, Outcome::Denied),
                    Err(_) => page::card(&view, Outcome::Failed(Reason::Store)),
                }
            }
        });
    }
    work.pages.work(&page::props(
        &outline,
        action(&app, &saved),
        csrf_token,
        &echo,
        false,
        views,
    ))
}

#[cfg(test)]
#[path = "route_tests.rs"]
pub(crate) mod tests;
