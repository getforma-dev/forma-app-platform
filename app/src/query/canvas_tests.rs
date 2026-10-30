//! Pitch canvas route and model tests without Docker (ADR-011 §3 I1, I2,
//! I3): every request without a live workspace session, and any query
//! string, is the uniform 404 on both canvas routes and reaches neither the
//! engine nor the page; a card shows a number only with its receipt, and its
//! key receipt identities are always present. The authorized paths and the
//! foreign-organization refusal over real sessions run in
//! `storage::query_qualification::native_query_campaign` (Docker); the SSR
//! rendering is the binary's `canvas_page_renders_groups_cards_and_receipts_without_javascript`.
use super::*;
use crate::{
    auth::{Auth, DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView},
    query::{
        page::{card, Outcome, Reason},
        receipt::QueryReceipt,
        routes::tests::{result, Counting, RecordingPages},
        Executor,
    },
    storage::WorkView,
};
use axum::body::{to_bytes, Body};
use serde_json::json;
use std::sync::Mutex;
use tower::ServiceExt;

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

#[derive(Default)]
struct RecordingCanvas {
    last: Mutex<Option<CanvasProps>>,
}
impl CanvasPages for RecordingCanvas {
    fn canvas(&self, props: &CanvasProps) -> Response {
        *self.last.lock().unwrap() = Some(props.clone());
        StatusCode::OK.into_response()
    }
}

const APP: &str = "00000000-0000-4000-8000-000000000065";
const WORK: &str = "00000000-0000-4000-8000-0000000000f2";

async fn send(
    app: &Router,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method("GET").uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, headers, body.to_vec())
}
use axum::http::HeaderMap;
fn uniform_404(reply: &(StatusCode, HeaderMap, Vec<u8>)) -> bool {
    reply.0 == StatusCode::NOT_FOUND
        && reply.2 == b"{\"code\":\"not_found\",\"status\":404}"
        && reply.1[header::CONTENT_TYPE] == "application/problem+json"
        && reply.1[header::CACHE_CONTROL] == "no-store"
        && reply.1.get(header::SET_COOKIE).is_none()
}

#[tokio::test]
async fn anonymous_forged_and_pre_access_requests_get_the_uniform_404_and_run_nothing() {
    let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
    let counting = Arc::new(Counting::default());
    let work = Arc::new(Work::new(
        auth,
        Some(counting.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    let pages = Arc::new(RecordingCanvas::default());
    let app = http::uniform(router(Arc::new(Canvas {
        work,
        pages: pages.clone(),
        live_events: String::new(),
        demo: false,
    })));
    let group = format!("/v1/canvas/works/{APP}/{WORK}");
    for headers in [
        vec![],
        vec![("cookie", "__Host-genui_session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
        vec![("cookie", "__Host-genui_pre=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
    ] {
        assert!(uniform_404(&send(&app, "/canvas", &headers).await));
        assert!(uniform_404(&send(&app, "/canvas?intent=spend", &headers).await));
        assert!(uniform_404(&send(&app, &group, &headers).await));
        assert!(uniform_404(&send(&app, "/v1/canvas/works/%FF/x", &headers).await));
    }
    // Other methods are the fallback's uniform 404 too.
    let post = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/canvas")
                .body(Body::from("intent=x"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::NOT_FOUND);
    assert_eq!(counting.calls(), 0, "no request reached the engine");
    assert!(pages.last.lock().unwrap().is_none(), "no page was rendered");
}

fn view(id: &str, kind: &str, integers: &[&str]) -> WorkView {
    WorkView {
        id: id.into(),
        title: id.into(),
        kind: kind.into(),
        integer_columns: integers.iter().map(|c| (*c).to_owned()).collect(),
    }
}

#[test]
fn a_card_shows_a_number_only_with_its_receipt_compact_and_complete() {
    let total = result(&["total_bytes"], vec![vec![json!("1140000")]]);
    let receipt = QueryReceipt {
        digest: format!("sha256:{}", "ab12cd34".repeat(8)),
        document: json!({"digest": "sha256:receipt", "result_digest": total.result_digest,
            "mode": "published", "principal_id": "p", "membership_epoch": 3, "policy_epoch": 1,
            "source_revisions": [{"input_id": "events", "source_id": "s", "source_revision_id": "r", "source_digest": "sha256:s"}],
            "parameters": [{"field_id": "window", "value": {"value": "x"}}], "provenance": {}}),
    };
    let total_view = view("total", "summary", &["total_bytes"]);
    let ready = canvas_card(
        APP,
        WORK,
        &total_view,
        card(&total_view, Outcome::Released(&total, &receipt)),
    );
    assert_eq!(ready.id, format!("c-{WORK}-total"));
    assert_eq!(ready.title, "Total");
    assert_eq!(ready.query, format!("/v1/work/{APP}/{WORK}/queries/total"));
    assert_eq!(ready.integer_columns, "total_bytes");
    // One insight: compact with its unit, the exact value beside it.
    assert!(ready.has_insight && !ready.has_stats);
    assert_eq!(ready.insight_label, "Total bytes");
    assert_eq!(ready.insight, "1.14 MB");
    assert_eq!(ready.insight_exact, "1,140,000 bytes");
    // The receipt: compact on the card, every row kept for the inspector.
    assert!(ready.has_receipt && ready.receipt_digest == receipt.digest);
    assert_eq!(ready.receipt_short, "ab12cd34");
    assert_eq!(ready.receipt_sources, "1 source");
    assert!(
        ready.receipt_time.ends_with(" UTC"),
        "{}",
        ready.receipt_time
    );
    let ids: Vec<&str> = ready.receipt.iter().map(|row| row.id.as_str()).collect();
    for id in [
        "digest",
        "result",
        "mode",
        "source-0",
        "parameter-0",
        "principal",
        "membership",
        "policy",
        "query",
        "truncated",
    ] {
        assert!(ids.contains(&id), "{id} is in the receipt");
    }
    assert!(ready
        .receipt
        .iter()
        .any(|row| row.value == total.result_digest));
    for outcome in [
        Outcome::Idle,
        Outcome::Loading,
        Outcome::Denied,
        Outcome::Failed(Reason::Deadline),
        Outcome::Failed(Reason::Store),
    ] {
        let empty = canvas_card(APP, WORK, &total_view, card(&total_view, outcome));
        assert!(!empty.has_insight && !empty.has_chart && !empty.has_table && !empty.has_receipt);
        assert!(empty.insight.is_empty() && empty.rows.is_empty() && empty.receipt.is_empty());
        assert!(empty.receipt_time.is_empty());
        assert!(
            !empty.message.chars().any(|c| c.is_ascii_digit()),
            "{}",
            empty.message
        );
    }
}

#[test]
fn a_timeline_card_is_a_chart_with_its_rows_folded_beneath() {
    let rows = vec![
        vec![
            json!("2026-10-22T10:00:00Z"),
            json!("203.0.113.10"),
            json!("60000"),
        ],
        vec![
            json!("2026-10-22T10:10:00Z"),
            json!("198.51.100.99"),
            json!("900000"),
        ],
    ];
    let buckets = result(&["minute", "destination", "bytes"], rows);
    let receipt = QueryReceipt {
        digest: "sha256:timeline".into(),
        document: json!({"digest": "sha256:timeline", "result_digest": buckets.result_digest,
            "source_revisions": [{}, {}], "parameters": [], "provenance": {}}),
    };
    let timeline = view("timeline", "timeline", &["bytes"]);
    let shown = canvas_card(
        APP,
        WORK,
        &timeline,
        card(&timeline, Outcome::Released(&buckets, &receipt)),
    );
    assert!(shown.has_chart && shown.chart.has_bars && shown.chart.bars.len() == 2);
    assert!(shown.has_table && shown.has_folded_table && !shown.has_open_table);
    assert_eq!(shown.rows_label, "2 rows");
    assert_eq!(
        shown
            .columns
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["Minute", "Destination", "Bytes"]
    );
    let first = &shown.rows[0].cells;
    assert_eq!(
        (first[0].text.as_str(), first[0].exact.as_str()),
        ("Oct 22, 10:00", "2026-10-22T10:00:00Z")
    );
    assert_eq!(
        (first[2].text.as_str(), first[2].exact.as_str()),
        ("60 KB", "60,000 bytes")
    );
    assert_eq!(first[2].class, "value num");
    assert_eq!(shown.receipt_sources, "2 sources");
}

#[test]
fn a_form_card_offers_the_writable_fields_and_enum_choices_only_to_a_writer() {
    let field = |id: &str, kind: &str, options: &str, writable: bool| commands::FieldRow {
        id: id.into(),
        kind: kind.into(),
        value: String::new(),
        input: format!("value.{id}"),
        spec: format!("{{\"type\":\"{kind}\"}}"),
        spec_input: format!("spec.{id}"),
        options: options.into(),
        nullable: false,
        writable,
    };
    let mut record = commands::RecordView {
        app: APP.into(),
        work: WORK.into(),
        collection: "00000000-0000-4000-8000-0000000000ce".into(),
        record_id: "00000000-0000-4000-8000-0000000013b1".into(),
        schema: "decision".into(),
        schema_version: "1".into(),
        publication: "00000000-0000-4000-8000-0000000000cc".into(),
        work_version: "1".into(),
        version: "0".into(),
        exists: false,
        fields: vec![
            field("status", "enum", "needs_context | reviewed", true),
            field("note", "text", "", true),
            field("expense_id", "text", "", false),
        ],
        command: "record_decision".into(),
        action: format!("/v1/work/{APP}/{WORK}/commands/record_decision"),
        can_write: true,
        csrf: "C".repeat(43),
        idempotency_key: "0123456789abcdef0123456789abcdef".into(),
        notice: None,
    };
    let form = form_card(WORK, &record).unwrap();
    assert_eq!(form.title, "New decision record");
    assert_eq!(form.record_version, "0");
    let status = &form.fields[0];
    assert!(status.has_choices);
    assert_eq!(
        status
            .choices
            .iter()
            .map(|c| c.value.as_str())
            .collect::<Vec<_>>(),
        ["needs_context", "reviewed"]
    );
    assert!(!form.fields[1].has_choices && form.fields[1].writable);
    assert!(!form.fields[2].writable && !form.fields[2].has_choices);
    assert_eq!(form.fields[2].label, "expense id");
    record.can_write = false;
    assert!(form_card(WORK, &record).is_none());
}

#[test]
fn the_receipt_line_and_the_top_bar_identity() {
    assert_eq!(
        digest_prefix(&format!("sha256:{}", "0123456789abcdef".repeat(4))),
        "01234567"
    );
    assert_eq!(digest_prefix("sha256:ab"), "ab");
    let rows = |n: usize| -> Vec<PairProps> {
        (0..n)
            .map(|i| PairProps {
                id: format!("source-{i}"),
                ..PairProps::default()
            })
            .collect()
    };
    assert_eq!(sources_label(&rows(1)), "1 source");
    assert_eq!(sources_label(&rows(0)), "0 sources");
    assert_eq!(sources_label(&rows(3)), "3 sources");
    assert_eq!(initials("ba92f4c5-0000-4000-8000-00000000a119"), "BA");
    assert_eq!(seeded_origins()[0].text, "Retrieved from library");
    assert_eq!(seeded_origins()[1].kind, "seeded");
}

#[test]
fn the_summary_counts_apps_and_cards() {
    let group = |cards: usize, forms: usize| GroupProps {
        cards: vec![CanvasCardProps::default(); cards],
        forms: vec![FormCardProps::default(); forms],
        ..GroupProps::default()
    };
    assert_eq!(summary(&[]), "0 apps · 0 cards");
    assert_eq!(summary(&[group(1, 0)]), "1 app · 1 card");
    assert_eq!(summary(&[group(2, 0), group(0, 1)]), "2 apps · 3 cards");
}
