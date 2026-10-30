//! APP-012 route and page tests without Docker: the refusal order (nothing
//! reaches the engine before a live session and authorization), the design
//! §9 status mapping, closed bodies and forms (filters in bodies only, the
//! window required on the page), and the view-state model (no failure ever
//! renders a number). The authorized paths, the receipts and the
//! delivery-time revocation run over real sessions in
//! `storage::query_qualification::native_query_campaign` (Docker); the SSR
//! rendering of every state is the binary's
//! `work_page_renders_every_view_state_without_a_fabricated_zero`.
use super::*;
use crate::{
    auth::{cookies, DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView},
    query::{
        page::{card, display, CardProps, WorkProps},
        receipt::QueryReceipt,
        worker::result_digest,
    },
    storage::WorkView,
};
use axum::body::Body;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use tower::ServiceExt;

/// A one-shot pause after the engine computed and before the route
/// releases: the executor reports `computed` and waits for `resume`, so a
/// test can change authority between execution and delivery.
pub(crate) type Pause = (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
);

/// The dispatch counter: every `execute` the routes make, optionally through
/// the real engine, optionally paused once after computing.
#[derive(Default)]
pub(crate) struct Counting {
    pub engine: Option<Arc<dyn Executor>>,
    pub calls: AtomicUsize,
    pub pause: Mutex<Option<Pause>>,
}
impl Counting {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    /// Pauses the next execution after it computed; returns (computed, resume).
    pub fn pause_next(
        &self,
    ) -> (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (computed, computed_rx) = tokio::sync::oneshot::channel();
        let (resume_tx, resume) = tokio::sync::oneshot::channel();
        *self.pause.lock().unwrap() = Some((computed, resume));
        (computed_rx, resume_tx)
    }
}
impl Executor for Counting {
    fn execute<'a>(&'a self, run: &'a QueryRun, deadline: Duration) -> Execution<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let result = match &self.engine {
                Some(engine) => engine.execute(run, deadline).await,
                None => Err(QueryError::Failed(Failure::Engine)),
            };
            let pause = self.pause.lock().unwrap().take();
            if let Some((computed, resume)) = pause {
                let _ = computed.send(());
                let _ = resume.await;
            }
            result
        })
    }
}

/// Renders the page props as their JSON-like debug form (state assertions).
#[derive(Default)]
pub(crate) struct RecordingPages {
    pub last: Mutex<Option<WorkProps>>,
}
impl WorkPages for RecordingPages {
    fn work(&self, props: &WorkProps) -> Response {
        *self.last.lock().unwrap() = Some(props.clone());
        let mut response = (
            StatusCode::OK,
            format!("page=work;views={}", props.views.len()),
        )
            .into_response();
        http::secure(response.headers_mut());
        response
    }
}

struct NoPages;
impl Pages for NoPages {
    fn invite(&self, _: &InviteView) -> Response {
        StatusCode::OK.into_response()
    }
    fn signin(&self, _: &SigninView) -> Response {
        StatusCode::OK.into_response()
    }
    fn pending(&self, _: &PendingView) -> Response {
        StatusCode::OK.into_response()
    }
    fn denied(&self, _: &DeniedView) -> Response {
        StatusCode::OK.into_response()
    }
    fn workspace(&self, _: &WorkspaceView) -> Response {
        StatusCode::OK.into_response()
    }
}

/// The binary's composition: these routes, the uniform 404 fallback for
/// unknown paths and methods, the 5 s handler deadline.
pub(crate) fn compose(work: Arc<Work>) -> Router {
    http::uniform(router(work))
}

const APP: &str = "00000000-0000-4000-8000-000000000065";
const WORK: &str = "00000000-0000-4000-8000-0000000000f2";

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, headers, body.to_vec())
}
fn uniform_404(reply: &(StatusCode, HeaderMap, Vec<u8>)) -> bool {
    reply.0 == StatusCode::NOT_FOUND
        && reply.2 == b"{\"code\":\"not_found\",\"status\":404}"
        && reply.1[header::CONTENT_TYPE] == "application/problem+json"
        && reply.1[header::CACHE_CONTROL] == "no-store"
        && reply.1.get(header::SET_COOKIE).is_none()
}

#[tokio::test]
async fn anonymous_cross_site_and_unknown_session_requests_never_reach_the_engine() {
    let (_guard, log) = crate::query::tests::capture_log();
    let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
    let counting = Arc::new(Counting::default());
    let pages = Arc::new(RecordingPages::default());
    let app = compose(Arc::new(Work::new(
        auth,
        Some(counting.clone()),
        pages.clone(),
    )));
    let query = format!("/v1/work/{APP}/{WORK}/queries/total");
    let same = ("sec-fetch-site", "same-origin");
    let window = br#"{"parameters":{"window":{"start":"2026-10-22T10:00:00Z","end":"2026-10-22T10:10:00Z"}}}"#.to_vec();
    // Fetch metadata first: a cross-site POST is 403 before anything else.
    let cross = send(
        &app,
        "POST",
        &query,
        &[("sec-fetch-site", "cross-site")],
        window.clone(),
    )
    .await;
    assert_eq!(cross.0, StatusCode::FORBIDDEN);
    assert_eq!(cross.2, b"{\"code\":\"cross_site\",\"status\":403}");
    // Anonymous, a forged session cookie, a pre-access cookie presented as a
    // session, and a filter in the URL are all the same uniform 404.
    for headers in [
        vec![same],
        vec![same, ("cookie", "__Host-genui_session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
        vec![same, ("cookie", "__Host-genui_pre=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
    ] {
        assert!(uniform_404(&send(&app, "POST", &query, &headers, window.clone()).await));
        assert!(uniform_404(&send(&app, "POST", &format!("{query}?destination=198.51.100.7"), &headers, window.clone()).await));
        assert!(uniform_404(&send(&app, "GET", &format!("/w/{APP}/{WORK}"), &headers, vec![]).await));
        assert!(uniform_404(&send(&app, "POST", &format!("/w/{APP}/{WORK}"), &headers, b"csrf=x".to_vec()).await));
    }
    // The bounded body (64 KiB) is judged before the session.
    let oversize = send(
        &app,
        "POST",
        &query,
        &[same],
        vec![b' '; QUERY_BODY_LIMIT + 1],
    )
    .await;
    assert_eq!(oversize.0, StatusCode::PAYLOAD_TOO_LARGE);
    let page_oversize = send(
        &app,
        "POST",
        &format!("/w/{APP}/{WORK}"),
        &[same],
        vec![b'a'; http::FORM_BODY_LIMIT + 1],
    )
    .await;
    assert_eq!(page_oversize.0, StatusCode::PAYLOAD_TOO_LARGE);
    // Other methods and paths are the fallback's uniform 404.
    assert!(uniform_404(
        &send(&app, "GET", &query, &[same], vec![]).await
    ));
    assert!(uniform_404(
        &send(&app, "PUT", &format!("/w/{APP}/{WORK}"), &[same], vec![]).await
    ));
    assert!(uniform_404(
        &send(
            &app,
            "POST",
            &format!("/v1/work/{APP}/{WORK}/queries"),
            &[same],
            window.clone()
        )
        .await
    ));
    // An undecodable path segment is the same uniform 404, never a 400.
    assert!(uniform_404(
        &send(
            &app,
            "POST",
            "/v1/work/%FF/x/queries/total",
            &[same],
            window.clone()
        )
        .await
    ));
    assert!(uniform_404(
        &send(&app, "GET", "/w/%FF/x", &[same], vec![]).await
    ));
    assert_eq!(counting.calls(), 0, "no request reached the engine");
    assert!(pages.last.lock().unwrap().is_none(), "no page was rendered");
    // APP-076 G6: the service log carries no dispatch line for any of them.
    assert!(crate::query::tests::log_lines(&log, "query.dispatch").is_empty());
}

/// Design §§6.3 and 9 with APP-012 review m2: a full or expired queue is
/// 429 (nothing ran); a query past its budget (interrupted, killed) and one
/// over a page are 422 `narrow_the_query`; only an engine or protocol failure
/// is 503 `query_failed`.
#[tokio::test]
async fn refusal_mapping_matches_design_section_nine() {
    let cases = [
        (engine_refusal(&QueryError::Busy), 429, "too_many_requests"),
        (
            engine_refusal(&QueryError::Failed(Failure::QueueExpired)),
            429,
            "too_many_requests",
        ),
        (
            engine_refusal(&QueryError::NarrowTheQuery),
            422,
            "narrow_the_query",
        ),
        (
            engine_refusal(&QueryError::Failed(Failure::Deadline)),
            422,
            "narrow_the_query",
        ),
        (
            engine_refusal(&QueryError::Failed(Failure::Killed)),
            422,
            "narrow_the_query",
        ),
        (
            engine_refusal(&QueryError::Refused("operator_not_qualified".into())),
            422,
            "query_refused",
        ),
        (
            engine_refusal(&QueryError::SourceUnavailable),
            503,
            "source_unavailable",
        ),
        (
            engine_refusal(&QueryError::Failed(Failure::Engine)),
            503,
            "query_failed",
        ),
        (storage_refusal(&Error::Denied), 404, "not_found"),
        (storage_refusal(&Error::Unavailable), 503, "unavailable"),
        (storage_refusal(&Error::Deadline), 503, "unavailable"),
        (storage_refusal(&Error::Configuration), 503, "unavailable"),
        (storage_refusal(&Error::UnknownOutcome), 503, "unavailable"),
        (invalid_parameters(), 422, "invalid_parameters"),
    ];
    for (response, status, code) in cases {
        assert_eq!(response.status().as_u16(), status, "{code}");
        let headers = response.headers().clone();
        assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            body,
            format!("{{\"code\":\"{code}\",\"status\":{status}}}").as_bytes()
        );
        if status == 429 {
            assert_eq!(headers[header::RETRY_AFTER], "2");
        } else {
            assert!(headers.get(header::RETRY_AFTER).is_none());
        }
    }
}

#[test]
fn filters_travel_in_a_closed_body() {
    let window =
        json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}});
    assert_eq!(body_parameters(b""), Some(json!({})));
    assert_eq!(body_parameters(b"{}"), Some(json!({})));
    let body = serde_json::to_vec(&json!({"parameters": window})).unwrap();
    assert_eq!(body_parameters(&body), Some(window.clone()));
    for refused in [
        &b"[]"[..],
        b"not json",
        br#"{"filters":{}}"#,
        br#"{"parameters":{"sql":"SELECT 1"}}"#,
        br#"{"parameters":{"destination":""}}"#,
        br#"{"parameters":{"amount_cents":{"type":"money","minor_units":"20000"}}}"#,
        br#"{"parameters":{"window":{"start":"2026-10-22T10:10:00Z","end":"2026-10-22T10:00:00Z"}}}"#,
        br#"{"parameters":{},"organization_id":"00000000-0000-4000-8000-000000000002"}"#,
    ] {
        assert_eq!(body_parameters(refused), None, "{}", String::from_utf8_lossy(refused));
    }
}

#[test]
fn the_page_requires_the_window_and_reads_the_form_body_only() {
    let fields = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    let (parameters, echo) = form_parameters(&fields(&[
        ("csrf", "t"),
        ("window_start", "2026-10-22T10:00"),
        ("window_end", "2026-10-22T10:10"),
        ("destination", ""),
    ]))
    .unwrap();
    assert_eq!(
        parameters,
        json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}})
    );
    assert_eq!(echo.window_start, "2026-10-22T10:00");
    let (filtered, _) = form_parameters(&fields(&[
        ("csrf", "t"),
        ("window_start", "2026-10-22T10:00:00Z"),
        ("window_end", "2026-10-22T10:10:00"),
        ("destination", "198.51.100.7"),
    ]))
    .unwrap();
    assert_eq!(filtered["destination"], "198.51.100.7");
    assert_eq!(filtered["window"]["end"], "2026-10-22T10:10:00Z");
    // No window, half a window, a reversed or malformed one, a duplicate or
    // an unknown field: refused, and the submitted values are echoed back.
    for refused in [
        fields(&[("csrf", "t"), ("destination", "198.51.100.7")]),
        fields(&[
            ("csrf", "t"),
            ("window_start", "2026-10-22T10:00"),
            ("window_end", ""),
        ]),
        fields(&[
            ("csrf", "t"),
            ("window_start", "2026-10-22T10:10"),
            ("window_end", "2026-10-22T10:00"),
        ]),
        fields(&[
            ("csrf", "t"),
            ("window_start", "yesterday"),
            ("window_end", "2026-10-22T10:00"),
        ]),
        fields(&[
            ("csrf", "t"),
            ("window_start", "2026-10-22T10:00"),
            ("window_end", "2026-10-22T10:10"),
            ("window_end", "2026-10-22T10:20"),
        ]),
        fields(&[
            ("csrf", "t"),
            ("window_start", "2026-10-22T10:00"),
            ("window_end", "2026-10-22T10:10"),
            ("organization_id", "x"),
        ]),
    ] {
        assert!(form_parameters(&refused).is_err(), "{refused:?}");
    }
    assert_eq!(
        form_timestamp("2026-10-22T10:00"),
        Some("2026-10-22T10:00:00Z".into())
    );
    assert_eq!(
        form_timestamp("2026-10-22 10:00"),
        Some("2026-10-22 10:00:00Z".into())
    );
    assert!(parse_parameters(
        &json!({"window": {"start": "2026-10-22 10:00:00Z", "end": "2026-10-22T10:10:00Z"}})
    )
    .is_none());
    // An impossible day is refused by the form too (review M2), and the
    // page's destination is bounded so the run cookie always fits.
    assert!(form_parameters(&fields(&[
        ("csrf", "t"),
        ("window_start", "2026-02-31T10:00"),
        ("window_end", "2026-03-01T10:00"),
    ]))
    .is_err());
    assert!(form_parameters(&fields(&[
        ("csrf", "t"),
        ("window_start", "2026-10-22T10:00"),
        ("window_end", "2026-10-22T10:10"),
        ("destination", &"d".repeat(FORM_DESTINATION_CHARACTERS)),
    ]))
    .is_ok());
    assert!(form_parameters(&fields(&[
        ("csrf", "t"),
        ("window_start", "2026-10-22T10:00"),
        ("window_end", "2026-10-22T10:10"),
        ("destination", &"d".repeat(FORM_DESTINATION_CHARACTERS + 1)),
    ]))
    .is_err());
}

/// Reload restores (ADR-010 §1 act 4): the run cookie carries exactly the
/// normalized parameters, tagged to the work path and the session; anything
/// else is `Invalid` (ignored and cleared), and no cookie is `Absent`.
#[test]
fn the_run_cookie_restores_only_this_sessions_run_of_this_work() {
    let auth = Auth::new(None, None, Arc::new(NoPages)).unwrap();
    let parameters = json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}, "destination": "198.51.100.7"});
    let value = run_cookie_value(&auth, APP, WORK, "session-a", &parameters).unwrap();
    assert!(value.len() <= cookies::RUN_MAX_VALUE);
    assert!(value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    let headers = |cookie: &str| {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("{}={cookie}", cookies::RUN)).unwrap(),
        );
        headers
    };
    assert_eq!(
        restore(&auth, &headers(&value), APP, WORK, "session-a"),
        Restore::Parameters(parameters.clone())
    );
    assert_eq!(
        restore(&auth, &HeaderMap::new(), APP, WORK, "session-a"),
        Restore::Absent
    );
    // Another session, another work or app, a tampered byte, a short value,
    // a foreign key and non-canonical parameters are all ignored.
    assert_eq!(
        restore(&auth, &headers(&value), APP, WORK, "session-b"),
        Restore::Invalid
    );
    assert_eq!(
        restore(
            &auth,
            &headers(&value),
            APP,
            "00000000-0000-4000-8000-0000000000f3",
            "session-a"
        ),
        Restore::Invalid
    );
    assert_eq!(
        restore(&auth, &headers(&value), WORK, WORK, "session-a"),
        Restore::Invalid
    );
    let mut tampered = value.clone().into_bytes();
    let last = tampered.len() - 1;
    tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
    assert_eq!(
        restore(
            &auth,
            &headers(&String::from_utf8(tampered).unwrap()),
            APP,
            WORK,
            "session-a"
        ),
        Restore::Invalid
    );
    assert_eq!(
        restore(&auth, &headers(&value[..40]), APP, WORK, "session-a"),
        Restore::Invalid
    );
    let other = Auth::new(None, None, Arc::new(NoPages)).unwrap();
    assert_eq!(
        restore(&other, &headers(&value), APP, WORK, "session-a"),
        Restore::Invalid
    );
    // Parameters the form would refuse never become a cookie the page runs:
    // a tag over them verifies, but the parameters themselves do not parse.
    let forged = {
        let bad =
            json!({"window": {"start": "2026-02-31T00:00:00Z", "end": "2026-03-01T00:00:00Z"}});
        let canonical = crate::admission::encode(&bad).unwrap();
        format!(
            "{}{}",
            auth.csrf.run_tag(APP, WORK, "session-a", &canonical),
            base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                &canonical
            )
        )
    };
    assert_eq!(
        restore(&auth, &headers(&forged), APP, WORK, "session-a"),
        Restore::Invalid
    );
    // The restored echo spells the window as the datetime-local control does.
    let echo = echo_of(&parameters);
    assert_eq!(
        (
            echo.window_start.as_str(),
            echo.window_end.as_str(),
            echo.destination.as_str()
        ),
        ("2026-10-22T10:00:00", "2026-10-22T10:10:00", "198.51.100.7")
    );
}

fn view(id: &str, kind: &str) -> WorkView {
    WorkView {
        id: id.into(),
        title: id.into(),
        kind: kind.into(),
        integer_columns: Vec::new(),
        column_types: Default::default(),
    }
}
/// A result whose integer columns are the byte counts (`total_bytes`,
/// `bytes`), as the seeded views' typed queries make them.
pub(crate) fn result(columns: &[&str], rows: Vec<Vec<Value>>) -> WorkerResult {
    WorkerResult {
        columns: columns.iter().map(|c| (*c).to_owned()).collect(),
        integer_columns: columns
            .iter()
            .filter(|c| matches!(**c, "total_bytes" | "bytes"))
            .map(|c| (*c).to_owned())
            .collect(),
        column_types: Default::default(),
        result_digest: result_digest(&rows).unwrap(),
        rows,
        truncated_page: false,
        engine_elapsed_ms: 0,
        wall: Duration::ZERO,
        peak_rss_bytes: 0,
    }
}
fn receipt_for(result: &WorkerResult) -> QueryReceipt {
    let document = json!({"kind": "query_receipt", "result_digest": result.result_digest, "digest": "sha256:feed",
        "source_revisions": [{"input_id": "events", "source_id": "s", "source_revision_id": "r", "source_digest": "sha256:d"}],
        "parameters": [{"field_id": "window_end", "value": {"type": "timestamp", "value": "2026-10-22T10:10:00Z"}}],
        "provenance": {"query_digest": "sha256:q", "predicate_digest": "sha256:p"}, "membership_epoch": 1, "partial": false});
    QueryReceipt {
        digest: "sha256:feed".into(),
        document,
    }
}
fn shows_no_number(card: &CardProps) -> bool {
    !card.ready
        && !card.has_summary
        && card.summary.is_empty()
        && !card.has_table
        && card.rows.is_empty()
        && card.columns.is_empty()
        && !card.message.bytes().any(|b| b.is_ascii_digit())
}

#[test]
fn view_states_loading_empty_failed_denied_never_zero() {
    let total = view("total", "summary");
    let idle = card(&total, Outcome::Idle);
    let loading = card(&total, Outcome::Loading);
    let denied = card(&total, Outcome::Denied);
    for (card, state) in [(&idle, "idle"), (&loading, "loading"), (&denied, "denied")] {
        assert_eq!(card.state, state);
        assert!(
            shows_no_number(card) && !card.has_receipt && card.receipt_json.is_empty(),
            "{state}"
        );
    }
    assert!(idle.idle && loading.loading && denied.denied);
    // The badge names the state, except that nothing is loading on a no-JS
    // page: the queued-out card is labelled busy (review m7).
    assert_eq!(
        (idle.badge.as_str(), denied.badge.as_str()),
        ("idle", "denied")
    );
    assert_eq!(loading.badge, "busy");
    assert!(loading.message.starts_with("Busy"));
    for reason in [
        Reason::Deadline,
        Reason::Engine,
        Reason::SourceUnavailable,
        Reason::NotQualified,
        Reason::NarrowTheQuery,
        Reason::Store,
    ] {
        let failed = card(&total, Outcome::Failed(reason));
        assert_eq!(failed.state, "failed");
        assert!(failed.failed && shows_no_number(&failed) && !failed.has_receipt);
        assert!(failed.message.contains("No number is shown"), "{reason:?}");
    }
    // An aggregate over no matching input answers one unknown cell: empty,
    // with its receipt, and never a zero.
    let nothing = result(&["total_bytes"], vec![vec![Value::Null]]);
    let empty = card(&total, Outcome::Released(&nothing, &receipt_for(&nothing)));
    assert_eq!(empty.state, "empty");
    assert!(empty.empty && shows_no_number(&empty) && empty.has_receipt);
    let none = result(&["minute", "destination", "bytes"], vec![]);
    let empty_timeline = card(
        &view("timeline", "timeline"),
        Outcome::Released(&none, &receipt_for(&none)),
    );
    assert!(empty_timeline.empty && shows_no_number(&empty_timeline));
    // Ready: the exact engine value, grouped; the receipt inspector shows it.
    let exact = result(&["total_bytes"], vec![vec![json!("1140000")]]);
    let receipt = receipt_for(&exact);
    let ready = card(&total, Outcome::Released(&exact, &receipt));
    assert_eq!(ready.state, "ready");
    assert!(ready.ready && ready.has_summary && !ready.has_table);
    assert_eq!(
        (
            ready.summary[0].label.as_str(),
            ready.summary[0].value.as_str()
        ),
        ("total_bytes", "1,140,000")
    );
    assert_eq!(ready.receipt_digest, "sha256:feed");
    assert!(ready
        .receipt
        .iter()
        .any(|p| p.label == "Result digest" && p.value == exact.result_digest));
    assert!(ready
        .receipt
        .iter()
        .any(|p| p.label == "Parameter window_end" && p.value == "2026-10-22T10:10:00Z"));
    assert!(ready.receipt_json.contains(&exact.result_digest));
    let buckets = result(
        &["minute", "destination", "bytes"],
        vec![
            vec![
                json!("2026-10-22T10:00:00Z"),
                json!("203.0.113.9"),
                json!("60000"),
            ],
            vec![json!("2026-10-22T10:01:00Z"), Value::Null, json!("120000")],
            // A digit-only text cell (an identifier) is text, not a number.
            vec![json!("2026-10-22T10:02:00Z"), json!("1234567"), json!("1")],
        ],
    );
    let timeline = card(
        &view("timeline", "timeline"),
        Outcome::Released(&buckets, &receipt_for(&buckets)),
    );
    assert!(timeline.ready && timeline.has_table && !timeline.has_summary);
    assert_eq!(
        timeline
            .columns
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["minute", "destination", "bytes"]
    );
    assert_eq!(timeline.rows[0].cells[2].text, "60,000");
    assert_eq!(
        timeline.rows[1].cells[1].text, "unknown",
        "an unknown cell is never a zero"
    );
    assert_eq!(
        timeline.rows[2].cells[1].text, "1234567",
        "a text column is never digit-grouped (review m11)"
    );
    assert_eq!(timeline.rows[2].cells[0].text, "2026-10-22T10:02:00Z");
    assert_eq!(ready.badge, "ready");
}

#[test]
fn display_groups_exact_decimals_only() {
    assert_eq!(display("1140000"), "1,140,000");
    assert_eq!(display("900000"), "900,000");
    assert_eq!(display("999"), "999");
    assert_eq!(display("-1234"), "-1,234");
    assert_eq!(display("0"), "0");
    assert_eq!(display("2026-10-22T10:00:00Z"), "2026-10-22T10:00:00Z");
    assert_eq!(display("12.5"), "12.5");
    assert_eq!(display(""), "");
}

/// APP-093 review M4: values are displayed by their column's value type:
/// money minor units as currency at the exponent, a decimal coefficient at
/// its scale, an integer grouped, and a text column verbatim even when every
/// character is a digit. Nothing that is not an exact integer string is
/// reinterpreted.
#[test]
fn values_format_by_their_column_type() {
    use crate::query::{page::format_value, worker::ColumnType};
    let usd = ColumnType::Money {
        currency: "USD".into(),
        minor_unit_exponent: 2,
    };
    assert_eq!(format_value("26262004", &usd), "$262,620.04");
    assert_eq!(format_value("477866", &usd), "$4,778.66");
    assert_eq!(format_value("5", &usd), "$0.05");
    assert_eq!(format_value("0", &usd), "$0.00");
    assert_eq!(format_value("-150", &usd), "-$1.50");
    let yen = ColumnType::Money {
        currency: "JPY".into(),
        minor_unit_exponent: 0,
    };
    assert_eq!(format_value("1234567", &yen), "JPY 1,234,567");
    let rate = ColumnType::Decimal { scale: 3 };
    assert_eq!(format_value("1234567", &rate), "1,234.567");
    assert_eq!(format_value("-7", &rate), "-0.007");
    assert_eq!(format_value("1140000", &ColumnType::Integer), "1,140,000");
    for verbatim in [
        ColumnType::Text,
        ColumnType::Date,
        ColumnType::Timestamp,
        ColumnType::Boolean,
    ] {
        assert_eq!(format_value("100200300", &verbatim), "100200300");
    }
    // Not an exact integer string: never reinterpreted.
    for odd in ["12.5", "", "-", "1e3", "١٢٣", "+5"] {
        assert_eq!(format_value(odd, &usd), odd);
        assert_eq!(format_value(odd, &ColumnType::Integer), odd);
    }
    // A released card: the shown text is formatted, the exact string kept.
    let mut released = result(
        &["claims", "total_amount"],
        vec![vec![json!("240"), json!("26262004")]],
    );
    released.column_types = [
        ("claims".to_owned(), ColumnType::Integer),
        ("total_amount".to_owned(), usd.clone()),
    ]
    .into();
    let receipt = receipt_for(&released);
    let summary = card(
        &view("totals", "summary"),
        Outcome::Released(&released, &receipt),
    );
    assert_eq!(
        summary
            .summary
            .iter()
            .map(|p| (p.value.as_str(), p.raw.as_str()))
            .collect::<Vec<_>>(),
        [("240", "240"), ("$262,620.04", "26262004")]
    );
    // A digit-only text cell beside a money cell is never grouped.
    let mut table = result(
        &["expense_id", "amount"],
        vec![vec![json!("100200300"), json!("123456")]],
    );
    table.column_types = [
        ("expense_id".to_owned(), ColumnType::Text),
        ("amount".to_owned(), usd),
    ]
    .into();
    let receipt = receipt_for(&table);
    let shown = card(&view("queue", "table"), Outcome::Released(&table, &receipt));
    assert_eq!(
        shown.rows[0]
            .cells
            .iter()
            .map(|c| (c.text.as_str(), c.raw.as_str()))
            .collect::<Vec<_>>(),
        [("100200300", "100200300"), ("$1,234.56", "123456")]
    );
}

/// APP-093 review m5: `GET /v1/library` at the route. Without a live
/// session (none, a forged session cookie, a pre-access cookie presented as
/// a session) it is the uniform 404, as is every other method; nothing is
/// listed and no engine runs. The listing of a live member (only the apps it
/// may open, a foreign organization's member seeing none of them, 503 on a
/// store outage) runs over real sessions in
/// `storage::query_qualification::native_query_campaign`.
#[tokio::test]
async fn the_library_route_is_the_uniform_404_without_a_live_session() {
    let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
    let counting = Arc::new(Counting::default());
    let app = compose(Arc::new(Work::new(
        auth,
        Some(counting.clone()),
        Arc::new(RecordingPages::default()),
    )));
    let same = ("sec-fetch-site", "same-origin");
    for headers in [
        vec![],
        vec![same],
        vec![same, ("cookie", "__Host-genui_session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
        vec![same, ("cookie", "__Host-genui_pre=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
    ] {
        assert!(uniform_404(&send(&app, "GET", "/v1/library", &headers, vec![]).await));
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            assert!(uniform_404(
                &send(&app, method, "/v1/library", &headers, b"{}".to_vec()).await
            ), "{method}");
        }
        assert!(uniform_404(&send(&app, "GET", "/v1/library/x", &headers, vec![]).await));
    }
    assert_eq!(counting.calls(), 0);
}
