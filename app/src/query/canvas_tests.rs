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
        column_types: Default::default(),
    }
}

#[test]
fn a_card_shows_a_number_only_with_its_receipt_and_key_identities() {
    let total = result(&["total_bytes"], vec![vec![json!("1140000")]]);
    let receipt = QueryReceipt {
        digest: "sha256:receipt".into(),
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
    assert_eq!(ready.query, format!("/v1/work/{APP}/{WORK}/queries/total"));
    assert_eq!(ready.integer_columns, "total_bytes");
    assert!(ready.has_summary && ready.summary[0].value == "1,140,000");
    assert!(ready.has_receipt && ready.receipt_digest == "sha256:receipt");
    let key: Vec<&str> = ready
        .receipt_key
        .iter()
        .map(|row| row.id.as_str())
        .collect();
    for id in [
        "result",
        "mode",
        "source-0",
        "parameter-0",
        "principal",
        "membership",
        "policy",
    ] {
        assert!(key.contains(&id), "{id} is always visible");
    }
    assert!(ready.receipt.len() > ready.receipt_key.len());
    for outcome in [
        Outcome::Idle,
        Outcome::Loading,
        Outcome::Denied,
        Outcome::Failed(Reason::Deadline),
        Outcome::Failed(Reason::Store),
    ] {
        let empty = canvas_card(APP, WORK, &total_view, card(&total_view, outcome));
        assert!(!empty.has_summary && !empty.has_table && !empty.has_receipt);
        assert!(empty.summary.is_empty() && empty.rows.is_empty() && empty.receipt_key.is_empty());
        assert!(
            !empty.message.chars().any(|c| c.is_ascii_digit()),
            "{}",
            empty.message
        );
    }
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
fn key_receipt_values_shorten_digests_and_ids_only() {
    let digest = format!("sha256:{}", "0123456789abcdef".repeat(4));
    let id = "00000000-0000-4000-8000-00000000006e";
    assert_eq!(
        abbreviate(&format!("source {id} revision {id} digest {digest}")),
        "source 00000000…006e revision 00000000…006e digest sha256:0123456789ab…abcdef"
    );
    assert_eq!(abbreviate("published"), "published");
    assert_eq!(abbreviate("2026-10-22T10:00:00Z"), "2026-10-22T10:00:00Z");
    assert_eq!(abbreviate("sha256:short"), "sha256:short");
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
