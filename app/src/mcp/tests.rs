//! APP-099 unit tests: the JSON-RPC surface, the refusal order before any
//! storage, the tool definitions, the held drafts and each tool's argument
//! refusals. The two-member story over real rows is
//! `storage::mcp_campaign` (Docker).
use super::{
    drafts::{draft_id_shaped, Drafts, DRAFT_TTL},
    tools::{self, Output},
    *,
};
use crate::{
    actions::{
        prompt::{CatalogApp, CatalogCommand},
        resolve::Draft,
        Kind,
    },
    auth::Auth,
    composer::Composer,
    query::routes::tests::RecordingPages,
    storage::{AgentCaller, CommandRequest},
};
use axum::{
    body::{to_bytes, Body},
    http::Request as HttpRequest,
};
use std::time::{Duration, Instant};
use tower::ServiceExt as _;

const ORG: &str = "00000000-0000-4000-8000-000000000001";
const DIEGO: &str = "00000000-0000-4000-8000-00000000006f";
const SARAH: &str = "00000000-0000-4000-8000-00000000006d";
const TOKEN_A: &str = "00000000-0000-4000-8000-0000000000a1";
const TOKEN_B: &str = "00000000-0000-4000-8000-0000000000b2";

fn expense_app() -> CatalogApp {
    CatalogApp {
        app: "00000000-0000-4000-8000-000000001681".into(),
        title: "Expense claims".into(),
        purpose: "File and approve expense claims".into(),
        schema: "expense_claim".into(),
        fields: vec![
            ("amount".into(), json!({"type": "money", "currency": "USD"})),
            ("merchant".into(), json!({"type": "text"})),
            (
                "category".into(),
                json!({"type": "enum", "values": ["meals", "travel", "software", "office", "other"]}),
            ),
            ("spent_on".into(), json!({"type": "date"})),
            ("note".into(), json!({"type": "text"})),
            (
                "status".into(),
                json!({"type": "enum", "values": ["submitted", "approved", "rejected"]}),
            ),
        ],
        commands: vec![
            CatalogCommand {
                id: "submit".into(),
                operation: "create".into(),
                writable: [
                    "amount", "merchant", "category", "spent_on", "note", "status",
                ]
                .map(String::from)
                .to_vec(),
                transitions: vec![],
                rules: Default::default(),
            },
            CatalogCommand {
                id: "decide".into(),
                operation: "update".into(),
                writable: ["status", "note"].map(String::from).to_vec(),
                transitions: vec![
                    ("status".into(), "submitted".into(), "approved".into()),
                    ("status".into(), "submitted".into(), "rejected".into()),
                ],
                rules: Default::default(),
            },
        ],
    }
}

fn draft() -> Draft {
    Draft {
        kind: Kind::Create,
        app: expense_app().app,
        work: "00000000-0000-4000-8000-0000000000f7".into(),
        title: "Expense claims".into(),
        placed: false,
        collection: "claims".into(),
        request: CommandRequest {
            app: expense_app().app,
            work: "00000000-0000-4000-8000-0000000000f7".into(),
            publication: "00000000-0000-4000-8000-0000000000f8".into(),
            command: "submit".into(),
            schema: "expense_claim".into(),
            schema_version_expected: 1,
            record: "00000000-0000-4000-8000-0000000000f9".into(),
            expected_record_version: 0,
            expected_work_version: 1,
            values: json!([]),
            idempotency_key: "0123456789abcdef0123456789abcdef".into(),
        },
        values: vec![],
        approver: None,
        approvers: vec![],
        target: None,
        confirm_token: "x".into(),
        rule: None,
        flags: vec![],
    }
}

#[test]
fn tools_list_is_the_closed_set_with_schemas_and_annotations() {
    let definitions = tools::definitions();
    let names: Vec<&str> = definitions
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, tools::NAMES.to_vec());
    for definition in &definitions {
        let schema = &definition["inputSchema"];
        assert_eq!(schema["type"], "object", "{definition}");
        assert_eq!(schema["additionalProperties"], false, "{definition}");
        let annotations = &definition["annotations"];
        assert_eq!(annotations["destructiveHint"], false, "{definition}");
        assert_eq!(annotations["openWorldHint"], false);
        assert!(definition["description"].as_str().unwrap().len() > 40);
        for required in schema["required"].as_array().into_iter().flatten() {
            assert!(schema["properties"][required.as_str().unwrap()].is_object());
        }
    }
    let read_only: Vec<&str> = definitions
        .iter()
        .filter(|d| d["annotations"]["readOnlyHint"] == true)
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        read_only,
        ["forma_list_apps", "forma_run_view", "forma_list_pending"]
    );
}

#[test]
fn json_rpc_messages_are_judged_strictly() {
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#),
        Message::Request {
            id: json!(1),
            method: "tools/list".into(),
            params: json!({})
        }
    );
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":"a","method":"ping","params":{}}"#),
        Message::Request {
            id: json!("a"),
            method: "ping".into(),
            params: json!({})
        }
    );
    // Notifications and client responses are accepted and ignored.
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
        Message::Ignored
    );
    assert_eq!(
        parse(br#"{"jsonrpc":"2.0","id":7,"result":{}}"#),
        Message::Ignored
    );
    let code = |bytes: &[u8]| match parse(bytes) {
        Message::Invalid(error) => error["error"]["code"].as_i64().unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!(code(b"not json"), PARSE_ERROR);
    assert_eq!(
        code(br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#),
        INVALID_REQUEST
    );
    assert_eq!(
        code(br#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#),
        INVALID_REQUEST
    );
    assert_eq!(code(br#"{"jsonrpc":"2.0","id":1}"#), INVALID_REQUEST);
    assert_eq!(
        code(br#"{"jsonrpc":"2.0","id":1,"method":5}"#),
        INVALID_REQUEST
    );
    assert_eq!(
        code(br#"{"jsonrpc":"2.0","id":{"x":1},"method":"ping"}"#),
        INVALID_REQUEST
    );
    assert_eq!(
        code(br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#),
        INVALID_PARAMS
    );
    assert_eq!(code(br#""string""#), INVALID_REQUEST);
}

#[test]
fn initialize_negotiates_the_protocol_version() {
    for version in PROTOCOL_VERSIONS {
        assert_eq!(negotiate(&json!({"protocolVersion": version})), version);
    }
    assert_eq!(
        negotiate(&json!({"protocolVersion": "2024-11-05"})),
        PROTOCOL_VERSIONS[0]
    );
    assert_eq!(negotiate(&json!({})), PROTOCOL_VERSIONS[0]);
    // A newer revision than ours (Claude Code's 2026-07-28) is answered with
    // our latest; an older unsupported one or garbage is refused.
    assert_eq!(
        negotiate(&json!({"protocolVersion": "2026-07-28"})),
        PROTOCOL_VERSIONS[0]
    );
    assert!(header_version_accepted("2026-07-28") && header_version_accepted("2025-06-18"));
    for bad in [
        "2024-11-05",
        "2025-01-01",
        "latest",
        "2026-7-28",
        "2026-07-28x",
        "",
    ] {
        assert!(!header_version_accepted(bad), "{bad}");
    }
}

#[test]
fn only_one_bearer_and_only_the_own_origin() {
    let mut headers = HeaderMap::new();
    assert_eq!(bearer(&headers), None);
    headers.insert(header::AUTHORIZATION, "Bearer fmat_abc".parse().unwrap());
    assert_eq!(bearer(&headers), Some("fmat_abc"));
    headers.insert(header::AUTHORIZATION, "bearer fmat_abc".parse().unwrap());
    assert_eq!(bearer(&headers), Some("fmat_abc"));
    for bad in [
        "Basic abc",
        "Bearer",
        "Bearer ",
        "fmat_abc",
        &format!("Bearer {}", "a".repeat(129)),
    ] {
        headers.insert(header::AUTHORIZATION, bad.parse().unwrap());
        assert_eq!(bearer(&headers), None, "{bad}");
    }
    headers.insert(header::AUTHORIZATION, "Bearer a".parse().unwrap());
    headers.append(header::AUTHORIZATION, "Bearer b".parse().unwrap());
    assert_eq!(bearer(&headers), None, "two Authorization headers");

    let origin = "https://beta.example.test";
    let mut headers = HeaderMap::new();
    assert!(
        origin_allowed(&headers, origin),
        "no Origin: a non-browser client"
    );
    headers.insert(header::ORIGIN, origin.parse().unwrap());
    assert!(origin_allowed(&headers, origin));
    assert!(
        !origin_allowed(&headers, ""),
        "an unconfigured origin matches nothing"
    );
    headers.insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    assert!(!origin_allowed(&headers, origin));
    headers.insert(header::ORIGIN, "null".parse().unwrap());
    assert!(!origin_allowed(&headers, origin));
}

fn app() -> Router {
    let auth = Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
    let work = Arc::new(Work::new(auth, None, Arc::new(RecordingPages::default())));
    let state = Arc::new(Mcp::new(work, Composer::new(None, "router", "generator")));
    http::uniform_with_many(Router::new(), vec![(PATH, router(state))])
}

async fn send(method: &str, headers: &[(&str, &str)], body: &str) -> (StatusCode, String) {
    let mut request = HttpRequest::builder().method(method).uri(PATH);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app()
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    assert!(response.headers().get(header::ALLOW).is_none());
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Before any storage: a foreign `Origin` is 403; every missing, malformed,
/// cookie-only or unresolvable credential is the same uniform 404, byte for
/// byte; `GET` and `DELETE` are 405.
#[tokio::test]
async fn refusals_before_storage_are_uniform() {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let uniform = (
        StatusCode::NOT_FOUND,
        "{\"code\":\"not_found\",\"status\":404}".to_owned(),
    );
    let token = format!("Bearer fmat_{}", "A".repeat(64));
    let cookie = format!("{}=AAAA", crate::auth::cookies::SESSION);
    for headers in [
        vec![("content-type", "application/json")],
        vec![
            ("content-type", "application/json"),
            ("authorization", "Bearer not-a-token"),
        ],
        vec![
            ("content-type", "application/json"),
            ("authorization", token.as_str()),
        ],
        vec![
            ("content-type", "application/json"),
            ("cookie", cookie.as_str()),
        ],
        vec![
            ("content-type", "application/json"),
            ("authorization", "Basic YTpi"),
        ],
    ] {
        assert_eq!(send("POST", &headers, body).await, uniform, "{headers:?}");
    }
    let (status, text) = send(
        "POST",
        &[
            ("content-type", "application/json"),
            ("origin", "https://evil.example"),
            ("authorization", token.as_str()),
        ],
        body,
    )
    .await;
    assert_eq!(
        (status, text.as_str()),
        (
            StatusCode::FORBIDDEN,
            "{\"code\":\"cross_site\",\"status\":403}"
        )
    );
    for method in ["GET", "DELETE"] {
        assert_eq!(
            send(method, &[], "").await.0,
            StatusCode::METHOD_NOT_ALLOWED,
            "{method}"
        );
    }
    assert_eq!(
        send("PUT", &[], "").await,
        uniform,
        "other methods are the uniform 404"
    );
}

fn caller(principal: &str, token: &str) -> AgentCaller {
    AgentCaller::for_tests(ORG, principal, token)
}

#[test]
fn drafts_are_the_callers_own_and_expire() {
    let drafts = Drafts::default();
    let diego = caller(DIEGO, TOKEN_A);
    let now = Instant::now();
    let id = drafts.hold_at(&diego, draft(), now).unwrap();
    assert!(draft_id_shaped(&id));
    assert!(drafts.get_at(&diego, &id, now).is_some_and(|h| !h.spent));
    // Another member, another token of the same member, a wrong shape.
    assert!(drafts.get_at(&caller(SARAH, TOKEN_A), &id, now).is_none());
    assert!(drafts.get_at(&caller(DIEGO, TOKEN_B), &id, now).is_none());
    assert!(drafts.get_at(&diego, &id.to_uppercase(), now).is_none());
    assert!(drafts.get_at(&diego, "00", now).is_none());
    drafts.spend(&caller(SARAH, TOKEN_A), &id);
    assert!(
        !drafts.get_at(&diego, &id, now).unwrap().spent,
        "only its owner spends it"
    );
    drafts.spend(&diego, &id);
    assert!(drafts.get_at(&diego, &id, now).unwrap().spent);
    // Expired after the TTL, and pruned.
    let later = now + DRAFT_TTL + Duration::from_secs(1);
    assert!(drafts.get_at(&diego, &id, later).is_none());
    assert_eq!(drafts.len(), 0);
    // At most twenty live drafts per member.
    for _ in 0..20 {
        assert!(drafts.hold_at(&diego, draft(), now).is_some());
    }
    assert!(drafts.hold_at(&diego, draft(), now).is_none());
    assert!(drafts
        .hold_at(&caller(SARAH, TOKEN_B), draft(), now)
        .is_some());
}

#[test]
fn a_tool_answer_carries_text_and_structured_content() {
    let ok = Output {
        code: "ok",
        text: "Two rows.".into(),
        structured: json!({"rows": [1, 2]}),
        is_error: false,
    }
    .result();
    assert_eq!(ok["isError"], false);
    assert_eq!(ok["structuredContent"]["rows"][1], 2);
    let text = ok["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("Two rows.") && text.contains("{\"rows\":[1,2]}"));
}

#[test]
fn draft_values_are_typed_hints_or_refused() {
    let app = expense_app();
    let catalog = vec![app.clone()];
    // The hint names the app by id, title or schema; never a foreign id.
    for hint in [
        app.app.as_str(),
        "Expense claims",
        "expense claims",
        "expense_claim",
        "Expense",
    ] {
        assert!(
            tools::app_for_hint_for_tests(&catalog, hint).is_some(),
            "{hint}"
        );
    }
    for hint in ["00000000-0000-4000-8000-000000000065", "Payroll", ""] {
        assert!(
            tools::app_for_hint_for_tests(&catalog, hint).is_none(),
            "{hint}"
        );
    }
    let ok = tools::create_hints_for_tests(
        &app,
        &json!({"amount": 86, "merchant": "'; DROP TABLE genui.apps; --", "note": "<script>alert(1)</script>", "status": "submitted"}),
    )
    .unwrap();
    assert_eq!(ok["amount"], "86");
    assert_eq!(
        ok["merchant"], "'; DROP TABLE genui.apps; --",
        "SQL is text, typed later under the schema"
    );
    assert_eq!(
        ok["note"], "<script>alert(1)</script>",
        "markup is inert text"
    );
    for (bad, why) in [
        (json!({"grant": "admin_policy"}), "an extra field"),
        (json!({"amount": {"minor_units": "1"}}), "a wrong type"),
        (json!({"amount": [1]}), "a wrong type"),
        (
            json!({"status": "approved"}),
            "a status other than the starting state",
        ),
        (json!({"note": "line\nbreak"}), "a control character"),
        (json!({"note": "x".repeat(501)}), "too long"),
        (json!("amount=86"), "not an object"),
        (json!({"Amount": "86"}), "not a field id"),
    ] {
        let refused = tools::create_hints_for_tests(&app, &bad).unwrap_err();
        assert_eq!(refused.code, "invalid_arguments", "{why}: {bad}");
        assert!(refused.is_error);
    }
}

#[test]
fn agent_token_labels_are_closed_bodies() {
    assert_eq!(
        tokens::parse_label(br#"{"label":"Claude"}"#).as_deref(),
        Some("Claude")
    );
    assert_eq!(
        tokens::parse_label(br#"{"label":" OpenAI "}"#).as_deref(),
        Some("OpenAI")
    );
    for bad in [
        &br#"{"label":"<b>x</b>"}"#[..],
        br#"{"label":""}"#,
        br#"{"label":"Claude","principal_id":"x"}"#,
        br#"{}"#,
        b"label=Claude",
    ] {
        assert_eq!(tokens::parse_label(bad), None);
    }
}
