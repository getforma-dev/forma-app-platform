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
/// APP-115: the People (time off) and Security (access requests) library apps.
const TIME_OFF_APP: &str = "00000000-0000-4000-8000-000000001701";
const TIME_OFF_WORK: &str = "00000000-0000-4000-8000-000000001704";
const TIME_OFF_SOURCE: &str = "00000000-0000-4000-8000-00000000301a";
const ACCESS_APP: &str = "00000000-0000-4000-8000-000000001881";
const ACCESS_SOURCE: &str = "00000000-0000-4000-8000-000000003020";

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
            .merge(mcp::tokens::router(auth.clone()))
            .merge(mcp::governance::router(auth.clone())),
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

    // --- APP-115: scoped agent tokens, the access log, the timeline ---
    for (app_id, source) in [(TIME_OFF_APP, TIME_OFF_SOURCE), (ACCESS_APP, ACCESS_SOURCE)] {
        for permission in ["discover", "run"] {
            grant(&fixture, EMPLOYEE, permission, &["--app", app_id]);
        }
        grant(&fixture, EMPLOYEE, "source_read", &["--source", source]);
    }
    let mint_scoped = |cookie: &str, body: Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        async move {
            send(
                &app,
                "POST",
                "/v1/agent-tokens",
                Some(&cookie),
                &body.to_string(),
                &SAME_ORIGIN,
            )
            .await
        }
    };
    for bad in [
        json!({"label": "Sec", "scope": {"spaces": ["hr"]}}),
        json!({"label": "Sec", "scope": {"spaces": []}}),
        json!({"label": "Sec", "actions": ["read", "admin"]}),
        json!({"label": "Sec", "expires_in_days": 31}),
        json!({"label": "Sec", "grants": ["admin"]}),
    ] {
        let refused = mint_scoped(&diego, bad.clone()).await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{bad}");
        assert!(
            refused.body.contains("malformed_scope"),
            "{bad}: {}",
            refused.body
        );
    }
    let minted = mint_scoped(
        &diego,
        json!({"label": "Security bot", "scope": {"spaces": ["security"]}, "actions": ["read", "draft"], "expires_in_days": 7}),
    )
    .await;
    assert_eq!(minted.status, StatusCode::CREATED, "{}", minted.body);
    let minted = minted.json();
    assert_eq!(
        minted["scope"],
        json!({"kind": "selected", "spaces": ["security"], "apps": [], "actions": ["read", "draft"]})
    );
    let security = minted["token"].as_str().unwrap().to_owned();
    let security_id = minted["token_id"].as_str().unwrap().to_owned();
    let lifetime = scalar(&fixture, &format!("SELECT extract(epoch FROM expires_at-created_at)::bigint FROM genui.agent_tokens WHERE token_id='{security_id}'")).await;
    assert_eq!(lifetime, 7 * 86_400);
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM genui.agent_tokens WHERE scope_kind='all' AND cardinality(scope_actions)=6").await,
        scalar(&fixture, "SELECT count(*) FROM genui.agent_tokens WHERE scope_kind='all'").await,
        "existing and unscoped tokens keep every action"
    );
    // In scope: the security app is listed and drafted; the People and
    // Finance apps are not listed.
    let listed = client
        .call(&security, mcp::tools::LIST_APPS, json!({}))
        .await;
    let listed = ok(&listed).clone();
    let library: Vec<&str> = listed["library"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["app_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        library,
        [ACCESS_APP],
        "only the in-scope library app is listed"
    );
    assert!(
        !listed.to_string().contains(EXPENSE_APP) && !listed.to_string().contains(TIME_OFF_APP)
    );
    // Out of scope: People (time off) by hint, by id and by a pending record.
    let frozen = footprint(&fixture).await;
    let nothing = client
        .call(
            &security,
            mcp::tools::DRAFT_RECORD,
            json!({"app_hint": FOREIGN_APP, "values": {"amount": "1"}}),
        )
        .await;
    assert_eq!(error_code(&nothing), "not_found");
    let started = Instant::now();
    let time_off = client
        .call(&security, mcp::tools::DRAFT_RECORD, json!({"app_hint": "Time off", "values": {"leave_type": "vacation", "start": "2026-11-10", "end": "2026-11-12", "days": 3}}))
        .await;
    timings.insert("scope_refusal_ms".into(), json!(ms(started)));
    assert_eq!(
        time_off, nothing,
        "a scope refusal is the uniform tool error"
    );
    let run = client
        .call(
            &security,
            mcp::tools::RUN_VIEW,
            json!({"app_id": TIME_OFF_APP, "work_id": TIME_OFF_WORK}),
        )
        .await;
    assert_eq!(run, nothing);
    let decide = client
        .call(
            &security,
            mcp::tools::DRAFT_DECISION,
            json!({"target_hint": "Nobu dinner", "decision": "approve"}),
        )
        .await;
    assert_eq!(decide, nothing);
    let compose = client
        .call(
            &security,
            mcp::tools::COMPOSE_APP,
            json!({"description": "time off requests"}),
        )
        .await;
    assert_eq!(compose, nothing);
    let shared = client
        .call(
            &security,
            mcp::tools::SHARE,
            json!({"app_id": app_id, "work_id": work_id, "member_name": "Sarah"}),
        )
        .await;
    assert_eq!(shared, nothing);
    assert_eq!(
        footprint(&fixture).await,
        frozen,
        "no scope refusal wrote anything"
    );
    // In scope: an access request drafts; its commit is refused (read and draft only).
    let drafted = client
        .call(&security, mcp::tools::DRAFT_RECORD, json!({"app_hint": "Access requests", "values": {"system": "GitHub", "role": "read", "reason": "on-call rotation", "expires_on": "2026-11-30"}}))
        .await;
    let drafted = ok(&drafted).clone();
    assert_eq!(drafted["app_id"], ACCESS_APP);
    let frozen = footprint(&fixture).await;
    let commit = client
        .call(
            &security,
            mcp::tools::COMMIT,
            json!({"draft_id": drafted["draft_id"]}),
        )
        .await;
    assert_eq!(commit, nothing);
    assert_eq!(
        footprint(&fixture).await,
        frozen,
        "a refused commit writes nothing"
    );
    check(
        &mut checks,
        &format!("APP-115 scoped tokens: a mint with an unknown space, an empty selection, an unknown action, 31 days or an extra member is 400 malformed_scope; \"Security bot\" (Security, read and draft, 7 days) lists only the in-scope Access requests app; drafting in Time off (People) by its name ({} ms), running its view by id, drafting a decision on the Nobu claim (decide not allowed), composing (narrowed scope) and sharing (share not allowed) are each the uniform not_found, byte-identical to a nonexistent app, and write nothing; an in-scope access request drafts and its commit is refused (commit not allowed) with nothing written", timings["scope_refusal_ms"]),
    );

    // The access log and the member's timeline.
    let agents = send(&app, "GET", "/v1/governance/agents", Some(&diego), "", &[]).await;
    assert_eq!(agents.status, StatusCode::OK, "{}", agents.body);
    let agents = agents.json();
    assert_eq!(agents["organization_wide"], false);
    let bot = agents["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["token_id"] == security_id.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (bot["label"].as_str(), bot["status"].as_str()),
        (Some("Security bot"), Some("live"))
    );
    assert_eq!(bot["scope"]["space_titles"], json!(["Security"]));
    assert_eq!(
        (bot["calls"].as_i64(), bot["refused"].as_i64()),
        (Some(9), Some(7))
    );
    assert!(
        !agents.to_string().contains(&security[5..]),
        "never a token"
    );
    let claude_agent = agents["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["label"] == "Claude")
        .unwrap()
        .clone();
    assert!(
        claude_agent["calls"].as_i64().unwrap() >= 8
            && claude_agent["refused"].as_i64().unwrap() >= 4,
        "{claude_agent}"
    );
    assert_eq!(claude_agent["scope"]["kind"], "all");
    let activity = send(
        &app,
        "GET",
        &format!("/v1/governance/agents/{security_id}/activity"),
        Some(&diego),
        "",
        &[],
    )
    .await;
    assert_eq!(activity.status, StatusCode::OK, "{}", activity.body);
    let activity = activity.json();
    let entries = activity["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 9);
    assert_eq!(entries[0]["tool"], mcp::tools::COMMIT, "newest first");
    assert_eq!(
        (
            entries[0]["reason"].as_str(),
            entries[0]["reason_title"].as_str()
        ),
        (Some("scope_action"), Some("action not allowed"))
    );
    let time_off_entry = entries
        .iter()
        .find(|e| {
            e["tool"] == mcp::tools::DRAFT_RECORD
                && e["outcome"] == "refused"
                && e["reason"] == "scope_space"
        })
        .unwrap();
    assert_eq!(time_off_entry["space_title"], "People");
    assert_eq!(time_off_entry["resource_label"], "Time off");
    assert_eq!(time_off_entry["reason_title"], "outside scope");
    assert_eq!(time_off_entry["touched"]["apps"], json!([TIME_OFF_APP]));
    let run_entry = entries
        .iter()
        .find(|e| e["tool"] == mcp::tools::RUN_VIEW)
        .unwrap();
    assert_eq!(
        (run_entry["reason"].as_str(), run_entry["space"].as_str()),
        (Some("scope_space"), Some("people"))
    );
    assert_eq!(run_entry["touched"]["works"], json!([TIME_OFF_WORK]));
    let allowed = entries
        .iter()
        .find(|e| e["tool"] == mcp::tools::DRAFT_RECORD && e["outcome"] == "allowed")
        .unwrap();
    assert_eq!(
        (
            allowed["space"].as_str(),
            allowed["resource_label"].as_str()
        ),
        (Some("security"), Some("Access requests"))
    );
    assert!(entries
        .iter()
        .all(|e| e["duration_ms"].as_i64().unwrap() >= 0));
    let foreign = send(
        &app,
        "GET",
        &format!("/v1/governance/agents/{security_id}/activity"),
        Some(&sarah),
        "",
        &[],
    )
    .await;
    assert_eq!(
        (foreign.status, foreign.body.as_str()),
        (StatusCode::NOT_FOUND, UNIFORM)
    );
    let anonymous = send(&app, "GET", "/v1/governance/agents", None, "", &[]).await;
    assert_eq!(anonymous.body, UNIFORM);
    assert_eq!(scalar(&fixture, "SELECT count(*) FROM genui.agent_access_log WHERE resource_label LIKE '%fmat_%' OR agent_label LIKE '%fmat_%'").await, 0);
    let member = stores
        .workspace
        .resolve_agent_token(&security)
        .await
        .unwrap()
        .unwrap()
        .request;
    let (verifies, rows) = stores
        .workspace
        .agent_access_chain_verifies(&member)
        .await
        .unwrap();
    assert!(verifies && rows >= 20, "{rows}");
    let edit = |duration: &str| {
        format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); UPDATE genui.agent_access_log SET duration_ms={duration} WHERE organization_id='{ORG}' AND chain_sequence=3; COMMIT;")
    };
    let original = scalar(&fixture, &format!("SELECT duration_ms::bigint FROM genui.agent_access_log WHERE organization_id='{ORG}' AND chain_sequence=3")).await;
    fixture.sql(&edit(&(original + 1).to_string()), "genui");
    assert_eq!(
        stores
            .workspace
            .agent_access_chain_verifies(&member)
            .await
            .unwrap(),
        (false, 3)
    );
    fixture.sql(&edit(&original.to_string()), "genui");
    assert!(
        stores
            .workspace
            .agent_access_chain_verifies(&member)
            .await
            .unwrap()
            .0
    );
    let workspace_edit = scalar(&fixture, "SELECT count(*) FROM information_schema.role_table_grants WHERE table_schema='genui' AND table_name='agent_access_log' AND grantee='genui_workspace' AND privilege_type IN ('UPDATE','DELETE')").await;
    assert_eq!(workspace_edit, 0);
    // Revoke from the governance panel's route: the next call is the uniform 404.
    let temporary = mint_scoped(
        &diego,
        json!({"label": "Temp bot", "scope": {"apps": [ACCESS_APP]}}),
    )
    .await
    .json();
    let temporary_token = temporary["token"].as_str().unwrap().to_owned();
    let temporary_id = temporary["token_id"].as_str().unwrap().to_owned();
    ok(&client
        .call(&temporary_token, mcp::tools::LIST_APPS, json!({}))
        .await);
    let revoked = send(
        &app,
        "DELETE",
        &format!("/v1/agent-tokens/{temporary_id}"),
        Some(&diego),
        "",
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT);
    let after = client
        .raw(
            &[
                ("authorization", &format!("Bearer {temporary_token}")),
                ("content-type", "application/json"),
            ],
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
        )
        .await;
    assert_eq!(
        (after.status, after.body.as_str()),
        (StatusCode::NOT_FOUND, UNIFORM)
    );
    let listed = send(&app, "GET", "/v1/governance/agents", Some(&diego), "", &[])
        .await
        .json();
    let temporary = listed["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["token_id"] == temporary_id.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (temporary["status"].as_str(), temporary["calls"].as_i64()),
        (Some("revoked"), Some(1))
    );
    check(
        &mut checks,
        &format!("APP-115 access log: every tools/call is one row (token, label, member, tool, touched ids, outcome and reason, space, duration); GET /v1/governance/agents lists Diego's agents with scope, status and counts (Security bot: 9 calls, 7 refused); its timeline is newest first and shows \"Draft a record — refused: outside scope (People), Time off\" with the time-off app id, the view refusal with its work id, and the allowed access-request draft; Sarah reading Diego's agent and an anonymous read are the uniform 404; no token in the log; the {rows}-row organization chain verifies, editing row 3's duration as the owner breaks it at row 3 and restoring it verifies again; genui_workspace holds no UPDATE or DELETE; a token revoked through DELETE /v1/agent-tokens answers the uniform 404 on its next call and lists as revoked"),
    );

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
                    "security_token": security,
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
