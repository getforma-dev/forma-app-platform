//! APP-099 campaign: Forma as an MCP server, the two-member pitch story
//! driven through `POST /v1/mcp` over real rows. Isolated test executable
//! only: local Docker PostgreSQL, the locked Python environments (the
//! operator seeds the library), agent tokens minted through the canvas's
//! `POST /v1/agent-tokens` with minted workspace sessions, the event streams
//! read as bodies, and a deterministic stub engine behind the query route
//! (the same `Executor` serves the canvas's JSON query route and the MCP
//! tool, so the parity check compares the two paths, not two engines).
//!
//! The story: Diego's agent ("Claude") lists his apps, drafts "$86 at Nobu,
//! Sarah approves" with `forma_draft_record` (the expense app is placed for
//! him: `app.placed`), and commits it after confirmation with
//! `forma_commit`: one audited record, shared with Sarah (`app.shared` on
//! her stream, `record.committed` on his), idempotent under a repeated
//! commit, the records route showing "via Claude". Sarah's agent ("OpenAI")
//! lists what is pending, drafts the approval and commits it: version 2
//! approved, `record.committed` on Diego's stream, "via OpenAI". The audit
//! chain verifies. Then the isolation and hostile cases: no, malformed,
//! forged, revoked, foreign-organization and epoch-bumped tokens and a
//! cookie without a bearer are the same uniform 404 (timings recorded); a
//! foreign Origin is 403; a foreign app id and another member's work are the
//! same `not_found` tool error; another member's or an unknown draft id is
//! refused; hostile values are refused or typed as inert text; nothing is
//! written by any of them. Finally `forma_compose_app` retrieves a library
//! app (the demo organization's rehearsed routing, no model) and places it
//! live.
//!
//! With `GENUI_MCP_STAND_SECONDS` the campaign then serves the composed
//! routes (canvas page and assets included) on 127.0.0.1 for that long and
//! writes the origin, both synthetic session cookies and two fresh synthetic
//! agent tokens to `GENUI_MCP_STAND_FILE` (scratch only, never evidence), so
//! a local browser and real MCP clients (Claude Code, the OpenAI smoke) can
//! run the pitch scenario against it.
use super::{
    admission_qualification::operator,
    command_qualification::{send, vault_token, TestPages},
    definition_qualification::grant,
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    auth::{cookies, Auth, Pages},
    commands::{routes::Commands, RecordPage},
    composer::Composer,
    demo, directory, events, http, mcp,
    query::{
        routes::{tests::RecordingPages, Execution},
        Executor, Work, WorkerResult,
    },
    sharing,
};
use axum::{
    body::{Body, BodyDataStream},
    extract::ConnectInfo,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tower::ServiceExt;

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

struct NoCanvas;
impl crate::query::canvas::CanvasPages for NoCanvas {
    fn canvas(&self, _: &crate::query::canvas::CanvasProps) -> axum::response::Response {
        use axum::response::IntoResponse;
        (StatusCode::OK, "canvas").into_response()
    }
}

/// The employee (tenant_seed actor 0x6f), granted discover and run on the
/// expense claims app and source_read on its claims source.
const EMPLOYEE: &str = "00000000-0000-4000-8000-00000000006f";
/// A member of the canary organization only.
const OUTSIDER: &str = "00000000-0000-4000-8000-000000000ab0";
const EXPENSE_APP: &str = "00000000-0000-4000-8000-000000001681";
const EXPENSE_SOURCE: &str = "00000000-0000-4000-8000-000000003000";
/// The canary organization's seeded app (a foreign app id).
const FOREIGN_APP: &str = "00000000-0000-4000-8000-000000000065";
const FOREIGN_WORK: &str = "00000000-0000-4000-8000-000000000066";
const UNIFORM: &str = "{\"code\":\"not_found\",\"status\":404}";

#[test]
#[ignore = "requires local Docker and the locked migration/contract Python environments"]
fn native_mcp_campaign() {
    std::thread::Builder::new()
        .name("mcp-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// Every row the campaign's writes can touch in either organization.
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in [
        "internal_records",
        "internal_record_revisions",
        "command_outcomes",
        "saved_work",
        "app_permissions",
        "work_permissions",
        "source_permissions",
        "audit_events",
        "agent_attributions",
    ] {
        counts.push(scalar(fixture, &format!("SELECT count(*) FROM genui.{table}")).await);
    }
    counts
}

/// The deterministic engine stand-in: one fixed result for any run.
struct StubEngine {
    calls: AtomicUsize,
}
impl Executor for StubEngine {
    fn execute<'a>(&'a self, _run: &'a QueryRun, _deadline: Duration) -> Execution<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(WorkerResult {
                columns: vec![
                    "claims".into(),
                    "total_amount".into(),
                    "largest_claim".into(),
                ],
                integer_columns: BTreeSet::new(),
                column_types: BTreeMap::new(),
                rows: vec![vec![json!("3"), json!("21300"), json!("8600")]],
                truncated_page: false,
                result_digest: format!("sha256:{}", "ab".repeat(32)),
                engine_elapsed_ms: 1,
                wall: Duration::from_millis(1),
                peak_rss_bytes: 0,
            })
        })
    }
}

// --- The event stream as a body ---

struct Stream {
    body: BodyDataStream,
}
impl Stream {
    async fn frame(&mut self, within: Duration) -> Option<String> {
        loop {
            let next = timeout(
                within,
                std::future::poll_fn(|cx| {
                    futures_core::Stream::poll_next(Pin::new(&mut self.body), cx)
                }),
            )
            .await
            .ok()??;
            let text = String::from_utf8(next.unwrap().to_vec()).unwrap();
            if text.starts_with(':') {
                continue;
            }
            return Some(text);
        }
    }
    /// The next event of `kind` for the work within five seconds, skipping
    /// other events of the same work (an announcement judged after a later
    /// share may also reach the teammate).
    async fn expect(&mut self, kind: &str, app: &str, work: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let frame = self
                .frame(left)
                .await
                .unwrap_or_else(|| panic!("no {kind} event"));
            let data = frame
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap();
            let data: Value = serde_json::from_str(data).unwrap();
            assert_eq!(data["app_id"], app, "{frame}");
            assert_eq!(data["work_id"], work, "{frame}");
            if data["kind"] == kind {
                return;
            }
        }
    }
    async fn silent(&mut self, within: Duration) {
        if let Some(frame) = self.frame(within).await {
            panic!("unexpected frame: {frame}");
        }
    }
}
async fn open_stream(app: &Router, cookie: &str) -> Stream {
    let mut request = Request::builder()
        .uri("/v1/events")
        .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.40:4000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    Stream {
        body: response.into_body().into_data_stream(),
    }
}

const SAME_ORIGIN: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/json"),
];

fn ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn us(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// The value of `field` in a records-route record.
fn value_of<'a>(record: &'a Value, field: &str) -> &'a Value {
    record["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["field_id"] == field)
        .map(|v| &v["value"])
        .unwrap_or(&Value::Null)
}

struct Client {
    app: Router,
    next: AtomicUsize,
}
impl Client {
    /// One raw `POST /v1/mcp` with these headers.
    async fn raw(&self, headers: &[(&str, &str)], body: &Value) -> command_qualification::Reply {
        send(
            &self.app,
            "POST",
            mcp::PATH,
            None,
            &body.to_string(),
            headers,
        )
        .await
    }
    /// One JSON-RPC request with a bearer token; the JSON-RPC answer.
    async fn rpc(&self, token: &str, method: &str, params: Value) -> Value {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let bearer = format!("Bearer {token}");
        let reply = self
            .raw(
                &[
                    ("authorization", &bearer),
                    ("content-type", "application/json"),
                ],
                &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        let answer = reply.json();
        assert_eq!(answer["id"], id);
        answer
    }
    /// `tools/call`; the tool result.
    async fn call(&self, token: &str, name: &str, arguments: Value) -> Value {
        let answer = self
            .rpc(
                token,
                "tools/call",
                json!({"name": name, "arguments": arguments}),
            )
            .await;
        answer["result"].clone()
    }
}
fn ok(result: &Value) -> &Value {
    assert_eq!(result["isError"], false, "{result}");
    &result["structuredContent"]
}
fn error_code(result: &Value) -> String {
    assert_eq!(result["isError"], true, "{result}");
    result["structuredContent"]["error"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn campaign() {
    let _ = tracing_subscriber::fmt().with_ansi(false).try_init();
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    let mut timings = serde_json::Map::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    operator(&fixture, &["seed-library", "--organization", ORG]);
    for member in [EMPLOYEE, REVIEWER] {
        for permission in ["discover", "run"] {
            grant(&fixture, member, permission, &["--app", EXPENSE_APP]);
        }
        grant(
            &fixture,
            member,
            "source_read",
            &["--source", EXPENSE_SOURCE],
        );
    }
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{OTHER}',true); INSERT INTO genui.principals VALUES ('{OTHER}','{OUTSIDER}','app008-fixture-only','ten_{OTHER}','usr_{OUTSIDER}'); INSERT INTO genui.memberships VALUES ('{OTHER}','{OUTSIDER}','active',1); INSERT INTO genui.entitlements VALUES ('{OTHER}','{OUTSIDER}','0000e000-0000-4000-8000-000000000ab0','active','member_invite','2026-01-01T00:00:00Z','2027-01-01T00:00:00Z',NULL); COMMIT;"), "genui");
    directory::set_for_tests(&[
        (EMPLOYEE, "Diego Ramos"),
        (REVIEWER, "Sarah Chen"),
        (PRINCIPAL, "Priya Natarajan"),
    ]);
    demo::configure_for_tests(true, &[ORG]);

    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let mint = |principal: &'static str, org: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            stores
                .workspace
                .mint_session_for_tests(org, principal, &reference)
                .await
                .unwrap()
        }
    };
    let diego = mint(EMPLOYEE, ORG).await;
    let sarah = mint(REVIEWER, ORG).await;
    let priya = mint(PRINCIPAL, ORG).await;
    let outsider = mint(OUTSIDER, OTHER).await;
    let record_page: Arc<dyn RecordPage> = pages;
    let engine = Arc::new(StubEngine {
        calls: AtomicUsize::new(0),
    });
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(engine.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    // No model key: the demo organization's rehearsed routing retrieves.
    let composer =
        Composer::new(None, "router", "generator").with_demo_organizations([ORG.to_owned()]);
    let stand = std::env::var("GENUI_MCP_STAND_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let canvas_pages: Arc<dyn crate::query::canvas::CanvasPages> = if stand.is_some() {
        let mut manifest =
            forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
        manifest.routes.retain(|route, _| route == "/canvas");
        let modules = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
        let pages = Arc::new(crate::query::canvas::SsrCanvasPages {
            page: modules
                .bind::<crate::query::canvas::CanvasProps>("canvas")
                .unwrap(),
        });
        modules.finalize().unwrap();
        pages
    } else {
        Arc::new(NoCanvas)
    };
    let app = http::uniform_with_many(
        Router::new()
            .route(
                "/_assets/{*filename}",
                axum::routing::get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(crate::auth::routes::router(auth.clone()))
            .merge(crate::query::canvas::router(Arc::new(
                crate::query::canvas::Canvas {
                    work: work.clone(),
                    pages: canvas_pages,
                    live_events: "/v1/events".into(),
                    demo_organizations: [ORG.to_owned()].into_iter().collect(),
                },
            )))
            .merge(sharing::router(auth.clone()))
            .merge(events::routes::router(auth.clone()))
            .merge(crate::commands::routes::router(Arc::new(Commands {
                auth: auth.clone(),
                page: record_page,
            })))
            .merge(crate::actions::routes::records_router(auth.clone()))
            .merge(crate::query::routes::router(work.clone()))
            .merge(mcp::tokens::router(auth.clone())),
        vec![(
            mcp::PATH,
            mcp::router(Arc::new(mcp::Mcp::new(work.clone(), composer))),
        )],
    );
    let client = Client {
        app: app.clone(),
        next: AtomicUsize::new(1),
    };
    let records = |cookie: &str, app_id: &str, work_id: &str| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = format!("/v1/work/{app_id}/{work_id}/records");
        async move { send(&app, "GET", &uri, Some(&cookie), "", &[]).await }
    };
    let mint_token = |cookie: &str, label: &str| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let body = json!({"label": label}).to_string();
        async move {
            let reply = send(
                &app,
                "POST",
                "/v1/agent-tokens",
                Some(&cookie),
                &body,
                &SAME_ORIGIN,
            )
            .await;
            assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
            let minted = reply.json();
            (
                minted["token"].as_str().unwrap().to_owned(),
                minted["token_id"].as_str().unwrap().to_owned(),
            )
        }
    };

    // --- Connect an agent: tokens minted from the canvas session ---
    let (claude, _claude_id) = mint_token(&diego, "Claude").await;
    let (openai, _) = mint_token(&sarah, "OpenAI").await;
    let (cursor, cursor_id) = mint_token(&diego, "Cursor").await;
    let (priya_token, _) = mint_token(&priya, "Claude").await;
    let (outsider_token, _) = mint_token(&outsider, "Claude").await;
    assert!(claude.starts_with(AGENT_TOKEN_PREFIX) && claude.len() == 69);
    let listed = send(&app, "GET", "/v1/agent-tokens", Some(&diego), "", &[])
        .await
        .json();
    let labels: Vec<&str> = listed["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.contains(&"Claude") && labels.contains(&"Cursor"));
    assert!(
        !listed.to_string().contains(&claude[5..]),
        "a listing never shows the token"
    );
    let stored = scalar(&fixture, "SELECT count(*) FROM genui.agent_tokens WHERE token_digest LIKE 'sha256:%' AND label IN ('Claude','OpenAI','Cursor')").await;
    assert_eq!(stored, 5);
    let raw_stored = scalar(&fixture, &format!("SELECT count(*) FROM genui.agent_tokens WHERE token_digest='{claude}' OR label='{claude}'")).await;
    assert_eq!(raw_stored, 0);
    // Sarah cannot revoke Diego's token; a cross-site revoke is refused.
    let foreign_revoke = send(
        &app,
        "DELETE",
        &format!("/v1/agent-tokens/{cursor_id}"),
        Some(&sarah),
        "",
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(
        (foreign_revoke.status, foreign_revoke.body.as_str()),
        (StatusCode::NOT_FOUND, UNIFORM)
    );
    let cross = send(
        &app,
        "DELETE",
        &format!("/v1/agent-tokens/{cursor_id}"),
        Some(&diego),
        "",
        &[("sec-fetch-site", "cross-site")],
    )
    .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let anonymous_mint = send(
        &app,
        "POST",
        "/v1/agent-tokens",
        None,
        "{\"label\":\"x\"}",
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(anonymous_mint.body, UNIFORM);
    check(
        &mut checks,
        "connect an agent: members mint labelled tokens through POST /v1/agent-tokens from their canvas session (fmat_ + 64 characters, shown once); only SHA-256 digests are stored; the listing shows labels, never a token; another member's revoke and an anonymous mint are the uniform 404, a cross-site revoke 403",
    );

    // --- The protocol: initialize, tools/list, ping, unknown method ---
    let initialized = client
        .rpc(&claude, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "campaign", "version": "1"}}))
        .await;
    assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(initialized["result"]["serverInfo"]["name"], "forma");
    let notified = client
        .raw(
            &[
                ("authorization", &format!("Bearer {claude}")),
                ("content-type", "application/json"),
            ],
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
        .await;
    assert_eq!(notified.status, StatusCode::ACCEPTED);
    assert!(
        notified.headers.get("mcp-session-id").is_none(),
        "stateless"
    );
    let listed = client.rpc(&claude, "tools/list", json!({})).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, mcp::tools::NAMES.to_vec());
    assert_eq!(
        client.rpc(&claude, "ping", json!({})).await["result"],
        json!({})
    );
    assert_eq!(
        client.rpc(&claude, "resources/list", json!({})).await["error"]["code"],
        mcp::METHOD_NOT_FOUND
    );
    assert_eq!(
        client
            .rpc(&claude, "tools/call", json!({"name": "forma_drop_tables"}))
            .await["error"]["code"],
        mcp::INVALID_PARAMS
    );
    let get = send(&app, "GET", mcp::PATH, None, "", &[]).await;
    assert_eq!(get.status, StatusCode::METHOD_NOT_ALLOWED);
    check(
        &mut checks,
        &format!("protocol: initialize negotiates 2025-06-18, notifications answer 202 with no session id, tools/list is exactly the {} tools, ping answers, an unknown method is -32601 and an unknown tool -32602, GET /v1/mcp is 405", names.len()),
    );

    // --- Scenario: Diego's agent files the Nobu dinner ---
    let mut diego_stream = open_stream(&app, &diego).await;
    let mut sarah_stream = open_stream(&app, &sarah).await;
    let apps = client.call(&claude, mcp::tools::LIST_APPS, json!({})).await;
    let apps = ok(&apps).clone();
    assert_eq!(apps["synthetic"], true);
    assert_eq!(apps["library"].as_array().unwrap().len(), 1);
    assert_eq!(apps["library"][0]["app_id"], EXPENSE_APP);
    assert_eq!(apps["library"][0]["files_records"], true);
    let fields: Vec<&str> = apps["library"][0]["record_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    assert!(fields.contains(&"amount") && !fields.contains(&"status"));
    assert!(apps["works"].as_array().unwrap().is_empty());
    let before = footprint(&fixture).await;
    let started = Instant::now();
    let drafted = client
        .call(&claude, mcp::tools::DRAFT_RECORD, json!({
            "app_hint": "Expense claims",
            "values": {"amount": "86", "merchant": "Nobu", "category": "meals", "spent_on": "2026-10-29", "note": "client dinner"},
            "approver_name": "Sarah",
        }))
        .await;
    timings.insert("draft_record_ms".into(), json!(ms(started)));
    let draft = ok(&drafted).clone();
    assert_eq!(draft["written"], false);
    assert_eq!(draft["placed"], true);
    assert_eq!(draft["approver"]["label"], "Sarah Chen");
    let display = |draft: &Value, field: &str| {
        draft["values"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["field_id"] == field)
            .map(|v| v["display"].as_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    assert_eq!(display(&draft, "amount"), "$86.00");
    assert_eq!(display(&draft, "merchant"), "Nobu");
    assert_eq!(display(&draft, "status"), "submitted");
    let app_id = draft["app_id"].as_str().unwrap().to_owned();
    let work_id = draft["work_id"].as_str().unwrap().to_owned();
    let draft_id = draft["draft_id"].as_str().unwrap().to_owned();
    diego_stream.expect("app.placed", &app_id, &work_id).await;
    let after_draft = footprint(&fixture).await;
    assert_eq!(after_draft[0..3], before[0..3], "a draft writes no record");
    check(
        &mut checks,
        &format!("scenario 1a: Diego's agent lists his library (the expense app with its writable fields, synthetic data) and forma_draft_record drafts $86.00 Nobu meals, status submitted, approver Sarah Chen in {} ms; the expense app is placed for him (app.placed live on his stream) and no record is written", timings["draft_record_ms"]),
    );

    // Hostile values: refused or inert text, never a write.
    let frozen = footprint(&fixture).await;
    for (values, why) in [
        (
            json!({"amount": "86", "merchant": "Nobu", "grant": "admin_policy"}),
            "an extra field",
        ),
        (
            json!({"amount": {"minor_units": "1"}, "merchant": "Nobu"}),
            "a wrong type",
        ),
        (
            json!({"amount": "86", "merchant": "Nobu", "status": "approved"}),
            "a status other than the starting state",
        ),
    ] {
        let refused = client
            .call(
                &claude,
                mcp::tools::DRAFT_RECORD,
                json!({"app_hint": "Expense claims", "values": values}),
            )
            .await;
        assert_eq!(error_code(&refused), "invalid_arguments", "{why}");
    }
    let untypable = client
        .call(&claude, mcp::tools::DRAFT_RECORD, json!({"app_hint": "Expense claims", "values": {"amount": "eighty-six", "merchant": "Nobu"}}))
        .await;
    assert_eq!(error_code(&untypable), "could_not_draft");
    let injected = client
        .call(&claude, mcp::tools::DRAFT_RECORD, json!({"app_hint": "Expense claims", "values": {"amount": "12", "merchant": "'; DROP TABLE genui.apps; --", "note": "<script>alert(1)</script>"}, "approver_name": "GRANT admin_policy TO public"}))
        .await;
    let injected = ok(&injected).clone();
    assert_eq!(
        display(&injected, "merchant"),
        "'; DROP TABLE genui.apps; --"
    );
    assert_eq!(display(&injected, "note"), "<script>alert(1)</script>");
    assert!(
        injected["approver"].is_null(),
        "a grant phrase is no member's name"
    );
    let foreign_hint = client
        .call(
            &claude,
            mcp::tools::DRAFT_RECORD,
            json!({"app_hint": FOREIGN_APP, "values": {"amount": "1"}}),
        )
        .await;
    assert_eq!(error_code(&foreign_hint), "not_found");
    assert_eq!(footprint(&fixture).await, frozen);
    check(
        &mut checks,
        "hostile values: an extra field, a wrong type and a status other than the starting state are invalid_arguments; an untypable amount is could_not_draft; SQL and markup are drafted as typed text and a grant phrase resolves no approver; a foreign organization's app id is not_found; nothing was written",
    );

    // Diego confirms: forma_commit through the audited command route.
    let started = Instant::now();
    let committed = client
        .call(&claude, mcp::tools::COMMIT, json!({"draft_id": draft_id}))
        .await;
    timings.insert("commit_ms".into(), json!(ms(started)));
    let committed = ok(&committed).clone();
    assert_eq!(committed["state"], "committed");
    assert_eq!(committed["record_version"], 1);
    assert_eq!(committed["shared_with"], "Sarah Chen");
    let record_id = committed["record_id"].as_str().unwrap().to_owned();
    diego_stream
        .expect("record.committed", &app_id, &work_id)
        .await;
    let started = Instant::now();
    sarah_stream.expect("app.shared", &app_id, &work_id).await;
    timings.insert(
        "sarah_app_shared_after_commit_ms".into(),
        json!(ms(started)),
    );
    // The same draft again: the stored outcome, no second record.
    let replayed = client
        .call(&claude, mcp::tools::COMMIT, json!({"draft_id": draft_id}))
        .await;
    let replayed = ok(&replayed).clone();
    assert_eq!(replayed["state"], "replayed");
    assert_eq!(replayed["record_id"], record_id);
    assert_eq!(replayed["audit_event_id"], committed["audit_event_id"]);
    let after = footprint(&fixture).await;
    assert_eq!(after[0], before[0] + 1, "exactly one record");
    assert_eq!(after[2], before[2] + 1, "exactly one outcome");
    // Another member's draft id and an unknown one: the same refusal.
    let foreign_draft = client
        .call(&openai, mcp::tools::COMMIT, json!({"draft_id": draft_id}))
        .await;
    let unknown_draft = client
        .call(
            &openai,
            mcp::tools::COMMIT,
            json!({"draft_id": "0".repeat(32)}),
        )
        .await;
    assert_eq!(error_code(&foreign_draft), "draft_not_found");
    assert_eq!(foreign_draft, unknown_draft);
    let hers = records(&sarah, &app_id, &work_id).await;
    assert_eq!(hers.status, StatusCode::OK, "{}", hers.body);
    let hers = hers.json();
    assert_eq!(hers["records"][0]["record_id"], record_id);
    assert_eq!(hers["records"][0]["via"], "Claude");
    assert_eq!(hers["records"][0]["creator"]["label"], "Diego Ramos");
    check(
        &mut checks,
        &format!("scenario 1b: forma_commit files one audited record v1 through the command route ({} ms) and shares it with Sarah Chen: record.committed on Diego's stream, app.shared on Sarah's ({} ms); a repeated forma_commit replays the same outcome (one record, one outcome); Sarah's agent cannot commit Diego's draft (same refusal as an unknown id); Sarah's canvas records list shows the claim \"via Claude\"", timings["commit_ms"], timings["sarah_app_shared_after_commit_ms"]),
    );

    // --- Scenario: Sarah's agent approves it ---
    let pending = client
        .call(&openai, mcp::tools::LIST_PENDING, json!({}))
        .await;
    let pending = ok(&pending).clone();
    assert_eq!(pending["pending"].as_array().unwrap().len(), 1);
    assert_eq!(pending["pending"][0]["record_id"], record_id);
    assert_eq!(pending["pending"][0]["filed_by"], "Diego Ramos");
    let diego_pending = client
        .call(&claude, mcp::tools::LIST_PENDING, json!({}))
        .await;
    assert!(
        ok(&diego_pending)["pending"].as_array().unwrap().is_empty(),
        "his own claim is not his to decide"
    );
    let started = Instant::now();
    let decision = client
        .call(
            &openai,
            mcp::tools::DRAFT_DECISION,
            json!({"target_hint": "Nobu dinner", "decision": "approve"}),
        )
        .await;
    let decision = ok(&decision).clone();
    assert_eq!(decision["target"]["record_id"], record_id);
    assert_eq!(display(&decision, "status"), "approved");
    let approved = client
        .call(
            &openai,
            mcp::tools::COMMIT,
            json!({"draft_id": decision["draft_id"]}),
        )
        .await;
    timings.insert("sarah_decide_and_commit_ms".into(), json!(ms(started)));
    let approved = ok(&approved).clone();
    assert_eq!(approved["record_version"], 2);
    let started = Instant::now();
    diego_stream
        .expect("record.committed", &app_id, &work_id)
        .await;
    timings.insert("diego_record_committed_event_ms".into(), json!(ms(started)));
    let his = records(&diego, &app_id, &work_id).await.json();
    assert_eq!(his["records"][0]["version"], 2);
    assert_eq!(value_of(&his["records"][0], "status")["value"], "approved");
    assert_eq!(his["records"][0]["updater"]["label"], "Sarah Chen");
    assert_eq!(his["records"][0]["via"], "OpenAI");
    let again = client
        .call(
            &openai,
            mcp::tools::DRAFT_DECISION,
            json!({"target_hint": "Nobu dinner", "decision": "approve"}),
        )
        .await;
    assert_eq!(error_code(&again), "could_not_draft");
    check(
        &mut checks,
        &format!("scenario 2: Sarah's agent lists one pending claim filed by Diego Ramos (Diego's own list is empty), drafts and commits the approval ({} ms): version 2 approved; record.committed reaches Diego's stream ({} ms) and his records list shows it approved by Sarah Chen \"via OpenAI\"; a second approval has no transition and is refused before any write", timings["sarah_decide_and_commit_ms"], timings["diego_record_committed_event_ms"]),
    );

    // --- forma_run_view: the canvas's rows and receipt ---
    let canvas = send(
        &app,
        "POST",
        &format!("/v1/work/{app_id}/{work_id}/queries/totals"),
        Some(&diego),
        "{}",
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(canvas.status, StatusCode::OK, "{}", canvas.body);
    let canvas = canvas.json();
    let started = Instant::now();
    let viewed = client
        .call(
            &claude,
            mcp::tools::RUN_VIEW,
            json!({"app_id": app_id, "work_id": work_id, "view": "totals"}),
        )
        .await;
    timings.insert("run_view_ms".into(), json!(ms(started)));
    let viewed = ok(&viewed).clone();
    assert_eq!(viewed["rows"], canvas["rows"]);
    assert_eq!(viewed["receipt_id"], canvas["receipt"]["digest"]);
    assert_eq!(viewed["receipt"], canvas["receipt"]);
    assert_eq!(viewed["synthetic"], true);
    let calls = engine.calls.load(Ordering::SeqCst);
    let sarah_view = client
        .call(
            &openai,
            mcp::tools::RUN_VIEW,
            json!({"app_id": app_id, "work_id": work_id}),
        )
        .await;
    assert_eq!(
        ok(&sarah_view)["view_id"],
        "totals",
        "the first view by default"
    );
    assert_eq!(engine.calls.load(Ordering::SeqCst), calls + 1);
    assert_ne!(
        ok(&sarah_view)["receipt_id"],
        viewed["receipt_id"],
        "her receipt names her"
    );
    check(
        &mut checks,
        &format!("forma_run_view returns exactly the canvas query route's rows and receipt (digest {}) for the same member, view and parameters ({} ms); Sarah's run (first view by default) carries her own receipt", viewed["receipt_id"].as_str().unwrap(), timings["run_view_ms"]),
    );

    // --- The audit chain ---
    let verified = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(verified["verified"], true, "{verified}");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='data' AND kind='command.committed' AND work_id='{work_id}'")).await,
        2
    );
    check(
        &mut checks,
        "the audit chains verify; exactly two command.committed events name the work (Diego's claim through Claude, Sarah's approval through OpenAI), the actor being the member each time",
    );

    // --- Isolation: every credential failure is the same uniform 404 ---
    // Revoke Diego's second token from the canvas; bump Priya's epoch by signing out.
    let revoked = send(
        &app,
        "DELETE",
        &format!("/v1/agent-tokens/{cursor_id}"),
        Some(&diego),
        "",
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    let alive = client.rpc(&priya_token, "ping", json!({})).await;
    assert_eq!(
        alive["result"],
        json!({}),
        "her token works before the epoch moves"
    );
    assert!(stores.workspace.sign_out(&priya).await.unwrap().is_some());
    let frozen = footprint(&fixture).await;
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let forged = format!("{AGENT_TOKEN_PREFIX}{}", "A".repeat(64));
    let unknown = {
        let mut t = claude.clone();
        t.replace_range(40..44, if &t[40..44] == "AAAA" { "BBBB" } else { "AAAA" });
        t
    };
    let diego_cookie = format!("{}={diego}", cookies::SESSION);
    let mut uniform_us = serde_json::Map::new();
    for (name, headers) in [
        (
            "no_credential",
            vec![("content-type", "application/json".to_owned())],
        ),
        (
            "malformed",
            vec![
                ("content-type", "application/json".into()),
                ("authorization", "Bearer not-a-token".into()),
            ],
        ),
        (
            "forged",
            vec![
                ("content-type", "application/json".into()),
                ("authorization", format!("Bearer {forged}")),
            ],
        ),
        (
            "unknown_secret",
            vec![
                ("content-type", "application/json".into()),
                ("authorization", format!("Bearer {unknown}")),
            ],
        ),
        (
            "revoked",
            vec![
                ("content-type", "application/json".into()),
                ("authorization", format!("Bearer {cursor}")),
            ],
        ),
        (
            "epoch_bumped",
            vec![
                ("content-type", "application/json".into()),
                ("authorization", format!("Bearer {priya_token}")),
            ],
        ),
        (
            "session_cookie_only",
            vec![
                ("content-type", "application/json".into()),
                ("cookie", diego_cookie.clone()),
            ],
        ),
    ] {
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let started = Instant::now();
        let reply = client.raw(&headers, &list).await;
        uniform_us.insert(name.into(), json!(us(started)));
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (StatusCode::NOT_FOUND, UNIFORM),
            "{name}"
        );
    }
    timings.insert("uniform_refusal_us".into(), Value::Object(uniform_us));
    let bearer = format!("Bearer {claude}");
    let foreign_origin = client
        .raw(
            &[
                ("content-type", "application/json"),
                ("authorization", &bearer),
                ("origin", "https://evil.example"),
            ],
            &list,
        )
        .await;
    assert_eq!(foreign_origin.status, StatusCode::FORBIDDEN);
    // The foreign organization's token reaches nothing of Northstar's.
    let missing = client
        .call(
            &claude,
            mcp::tools::RUN_VIEW,
            json!({"app_id": app_id, "work_id": "00000000-0000-4000-8000-00000000dead"}),
        )
        .await;
    assert_eq!(error_code(&missing), "not_found");
    let foreign_cases = [
        (
            &outsider_token,
            mcp::tools::RUN_VIEW,
            json!({"app_id": app_id, "work_id": work_id}),
        ),
        (
            &claude,
            mcp::tools::RUN_VIEW,
            json!({"app_id": FOREIGN_APP, "work_id": FOREIGN_WORK}),
        ),
        (
            &outsider_token,
            mcp::tools::SHARE,
            json!({"app_id": app_id, "work_id": work_id, "member_name": "Diego"}),
        ),
        (
            &outsider_token,
            mcp::tools::DRAFT_RECORD,
            json!({"app_hint": EXPENSE_APP, "values": {"amount": "1"}}),
        ),
        (
            &outsider_token,
            mcp::tools::DRAFT_DECISION,
            json!({"target_hint": "Nobu dinner", "decision": "approve"}),
        ),
        (
            &openai,
            mcp::tools::SHARE,
            json!({"app_id": app_id, "work_id": work_id, "member_name": "Priya"}),
        ),
    ];
    for (token, tool, arguments) in foreign_cases {
        let result = client.call(token, tool, arguments.clone()).await;
        assert_eq!(result, missing, "{tool} {arguments}");
    }
    let outsider_commit = client
        .call(
            &outsider_token,
            mcp::tools::COMMIT,
            json!({"draft_id": draft_id}),
        )
        .await;
    assert_eq!(error_code(&outsider_commit), "draft_not_found");
    let outsider_apps = client
        .call(&outsider_token, mcp::tools::LIST_APPS, json!({}))
        .await;
    let outsider_apps = ok(&outsider_apps);
    assert!(!outsider_apps.to_string().contains(EXPENSE_APP));
    assert_eq!(
        footprint(&fixture).await,
        frozen,
        "no refusal wrote anything"
    );
    diego_stream.silent(Duration::from_millis(500)).await;
    check(
        &mut checks,
        "isolation: no bearer, a malformed, forged or unknown token, a token revoked from the canvas, a token whose member's epoch moved (sign-out) and a session cookie without a bearer are all the byte-identical uniform 404 (timings in the report); a foreign Origin is 403; a canary-organization token running Diego's view, sharing his work, drafting on his app or deciding his claim, Diego's token on the canary's app, and Sarah sharing Diego's work onward all answer the same not_found tool result as a nonexistent work; a foreign token cannot commit his draft and lists nothing of Northstar's; nothing was written",
    );

    // --- forma_compose_app: retrieved from the library and placed live ---
    let started = Instant::now();
    let composed = client
        .call(
            &claude,
            mcp::tools::COMPOSE_APP,
            json!({"description": "expense claims for the offsite team dinner"}),
        )
        .await;
    timings.insert("compose_ms".into(), json!(ms(started)));
    let composed = ok(&composed).clone();
    assert_eq!(composed["origin"], "retrieved");
    assert_eq!(composed["app_id"], EXPENSE_APP);
    let composed_work = composed["work_id"].as_str().unwrap().to_owned();
    assert_ne!(composed_work, work_id);
    diego_stream
        .expect("app.placed", EXPENSE_APP, &composed_work)
        .await;
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.agent_attributions WHERE work_id='{composed_work}' AND record_id IS NULL")).await,
        1
    );
    // Outside the demo organization and without a model key: the instant
    // matcher runs in every organization (APP-101), and a member with nothing
    // composable gets the uniform not_found; nothing is written.
    let refused = client
        .call(
            &outsider_token,
            mcp::tools::COMPOSE_APP,
            json!({"description": "expense claims"}),
        )
        .await;
    assert_eq!(error_code(&refused), "not_found");
    check(
        &mut checks,
        &format!("forma_compose_app runs the composer route's compose_for (demo organization, rehearsed routing, no model): the expense app retrieved as a new saved work in {} ms, app.placed live on Diego's stream, the work attributed to his token; an outsider with nothing composable gets the uniform not_found and nothing is written", timings["compose_ms"]),
    );
    drop(diego_stream);

    // --- The local stand for the browser and the real MCP clients ---
    if let Some(seconds) = stand {
        let file = std::env::var("GENUI_MCP_STAND_FILE").unwrap_or_default();
        let (diego_token, _) = mint_token(&diego, "Claude Code").await;
        let (sarah_token, _) = mint_token(&sarah, "Claude Code").await;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        if !file.is_empty() {
            std::fs::write(
                &file,
                serde_json::to_vec(&json!({
                    "origin": origin, "cookie": cookies::SESSION, "employee": diego, "approver": sarah,
                    "employee_token": diego_token, "approver_token": sarah_token,
                    "app": app_id, "work": work_id,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        println!("MCP_STAND {origin}");
        let served = app.clone();
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                served.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let stop = format!("{file}.stop");
        while Instant::now() < deadline
            && !(file.is_empty() || std::path::Path::new(&stop).exists())
        {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        server.abort();
    }

    let report = json!({
        "campaign": "APP-099: Forma as an MCP server, the two-member story through POST /v1/mcp (synthetic identities, Docker)",
        "passed": true,
        "engine": "deterministic stub Executor shared by the canvas query route and the MCP tool",
        "timings": timings,
        "checks": checks,
    });
    if let Some(path) = std::env::var_os("GENUI_MCP_CAMPAIGN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    demo::configure_for_tests(false, &[]);
    directory::set_for_tests(&[]);
    drop(sarah_stream);
    stores.close().await.unwrap();
}
