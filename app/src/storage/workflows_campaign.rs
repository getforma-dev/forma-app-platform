//! APP-107 workflows campaign: the actions campaign extended to the five
//! workflow apps (time off, purchase requests, vendor invoices, access
//! requests, deal desk), end to end over real rows. Isolated test executable
//! only: local Docker PostgreSQL, the locked Python environments (the
//! operator seeds the library), a loopback Messages stub named by
//! `GENUI_COMPOSER_ENDPOINT` for the unrehearsed sentences, minted sessions
//! over the composed HTTP routes, the event streams read as bodies, and the
//! MCP route with an agent token.
//!
//! Per app: the rehearsed sentence (demo organization, no model call) answers
//! a typed card; Confirm files one audited record; the work is shared with
//! the approver(s) (`app.shared` on their stream); the approver decides with
//! the records card's structured decision and the filer's stream receives
//! `record.committed`. The declared rules are the server's: a purchase over
//! $5,000 needs the manager then finance (a finance approval first is
//! refused by storage), under $5,000 one approval completes it; a deal desk
//! discount of 10% or less is approved on create (one audited command) and a
//! create claiming "approved" for 15% is refused; an invoice naming a PO
//! that does not match the purchase request is flagged (display only). An
//! unrehearsed sentence per app goes through the model stub, whose status
//! words the server overrides. An MCP agent token decides as the approver
//! and the record carries its label.
//!
//! With `GENUI_WORKFLOWS_STAND_SECONDS` the campaign then serves the composed
//! routes on 127.0.0.1 and writes the origin, the three synthetic session
//! cookies and Sarah's agent token to `GENUI_WORKFLOWS_STAND_FILE` (scratch
//! only, never evidence) for the browser walk and the Haiku agent loop
//! (`app/scripts/mcp_haiku_agent.py`); a Figma seat purchase is left pending
//! for Sarah and a $7,200 one at its finance step for Priya.
use super::{
    actions_campaign::{ms, open_stream, value_of, NoCanvas, Stream, SAME_ORIGIN},
    admission_qualification::operator,
    command_qualification::{json_headers, send, vault_token, TestPages},
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    actions::{self, routes::Act, Parser},
    auth::{cookies, Auth, Pages},
    commands::{routes::Commands, RecordPage},
    composer::{
        model::{ENDPOINT_ENV, KEY_ENV},
        Composer,
    },
    demo, directory, events, http, mcp,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        routes::tests::RecordingPages,
        Work,
    },
    sharing,
};
use axum::{http::StatusCode, Router};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

const EMPLOYEE: &str = "00000000-0000-4000-8000-00000000006f";
const TIME_OFF: (&str, &str) = (
    "00000000-0000-4000-8000-000000001701",
    "00000000-0000-4000-8000-00000000301a",
);
const PURCHASE: (&str, &str) = (
    "00000000-0000-4000-8000-000000001781",
    "00000000-0000-4000-8000-00000000301c",
);
const INVOICE: (&str, &str) = (
    "00000000-0000-4000-8000-000000001801",
    "00000000-0000-4000-8000-00000000301e",
);
const ACCESS: (&str, &str) = (
    "00000000-0000-4000-8000-000000001881",
    "00000000-0000-4000-8000-000000003020",
);
const DEAL_DESK: (&str, &str) = (
    "00000000-0000-4000-8000-000000001901",
    "00000000-0000-4000-8000-000000003022",
);

/// The loopback model's reply to an unrehearsed sentence: one per app, each
/// naming a status the server must ignore.
fn stub_reply(utterance: &str) -> String {
    let parse = |app: &str, values: Value, approver: Option<&str>| {
        json!({"action": "create", "app_hint": app, "values": values, "approver_name": approver, "target_record_hint": null}).to_string()
    };
    let text = if utterance.contains("Out sick") {
        parse(
            TIME_OFF.0,
            json!({"leave_type": "sick", "start": "2026-10-31", "end": "2026-10-31", "days": "1", "status": "approved"}),
            Some("Sarah"),
        )
    } else if utterance.contains("label printer") {
        parse(
            PURCHASE.0,
            json!({"item": "Label printer", "vendor": "Atlas Office Supply", "amount": "320", "cost_center": "operations", "status": "approved"}),
            Some("Sarah"),
        )
    } else if utterance.contains("bill A-99") {
        parse(
            INVOICE.0,
            json!({"vendor": "Atlas Office Supply", "invoice_number": "A-99", "amount": "120", "due_date": "2026-11-30", "status": "paid"}),
            None,
        )
    } else if utterance.contains("read rights") {
        parse(
            ACCESS.0,
            json!({"system": "hr-records", "role": "read", "reason": "benefits audit", "expires_on": "2026-12-01", "status": "granted"}),
            Some("Sarah"),
        )
    } else if utterance.contains("25 off") {
        parse(
            DEAL_DESK.0,
            json!({"account": "Brightwater Foods", "deal_amount": "90000", "discount_pct": "25%", "reason": "renewal", "status": "approved"}),
            Some("Sarah"),
        )
    } else {
        json!({"action": "place", "app_hint": "", "values": {}, "approver_name": null, "target_record_hint": null}).to_string()
    };
    json!({"id": "msg", "model": "stub-haiku", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 900, "output_tokens": 80}}).to_string()
}
async fn stub_server() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = Router::new().route(
        "/v1/messages",
        axum::routing::post(move |body: axum::body::Bytes| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let body: Value = serde_json::from_slice(&body).unwrap();
                let user = body["messages"][0]["content"].as_str().unwrap_or("");
                let utterance = serde_json::from_str::<Value>(user)
                    .ok()
                    .and_then(|v| v["utterance"].as_str().map(str::to_owned))
                    .unwrap_or_default();
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    stub_reply(&utterance),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (endpoint, calls)
}

/// Reads `stream` until the event `kind` for this work arrives (other
/// events, which share and decision order may interleave, are skipped).
async fn until(stream: &mut Stream, kind: &str, app: &str, work: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        let Some(frame) = stream.frame(Duration::from_secs(8)).await else {
            break;
        };
        let data = frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .and_then(|d| serde_json::from_str::<Value>(d).ok())
            .unwrap_or(Value::Null);
        if data["kind"] == kind && data["app_id"] == app && data["work_id"] == work {
            return;
        }
    }
    panic!("no {kind} for {app}/{work}");
}

fn display_of(draft: &Value, field: &str) -> String {
    draft["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["field_id"] == field)
        .map(|v| v["display"].as_str().unwrap().to_owned())
        .unwrap_or_default()
}
fn status_of(record: &Value) -> String {
    value_of(record, "status")["value"]
        .as_str()
        .unwrap_or("")
        .to_owned()
}

#[test]
#[ignore = "requires local Docker and the locked migration/contract Python environments"]
fn native_workflows_campaign() {
    std::thread::Builder::new()
        .name("workflows-campaign".into())
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

/// The operator's grant; a grant the persona already holds (Priya authored
/// the library) is an unchanged repeat.
fn grant(fixture: &Fixture, principal: &str, permission: &str, resource: &[&str]) {
    let mut args = vec![
        "grant-permission",
        "--organization",
        ORG,
        "--principal",
        principal,
        "--permission",
        permission,
    ];
    args.extend_from_slice(resource);
    operator(fixture, &args);
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
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
    let seeded = operator(&fixture, &["seed-library", "--organization", ORG]);
    let titles: Vec<&str> = seeded["apps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["title"].as_str())
        .collect();
    for title in [
        "Time off",
        "Purchase requests",
        "Vendor invoices",
        "Access requests",
        "Deal desk",
    ] {
        assert!(titles.contains(&title), "{title} seeded");
    }
    // The operator grants each persona discover and run on the five apps and
    // source_read on their history sources (ADR-007 D7.3); nothing else.
    for member in [EMPLOYEE, REVIEWER, PRINCIPAL] {
        for (app, source) in [TIME_OFF, PURCHASE, INVOICE, ACCESS, DEAL_DESK] {
            for permission in ["discover", "run"] {
                grant(&fixture, member, permission, &["--app", app]);
            }
            grant(&fixture, member, "source_read", &["--source", source]);
        }
    }
    directory::set_for_tests(&[
        (EMPLOYEE, "Diego Ramos"),
        (REVIEWER, "Sarah Chen"),
        (PRINCIPAL, "Priya Natarajan"),
    ]);
    demo::configure_for_tests(true, &[ORG]);
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let employee = super::definition_qualification::request(&stores, ORG, EMPLOYEE, 1);
    let context = stores.workspace.composer_context(&employee).await.unwrap();
    let catalog = actions::prompt::catalog(&context.library);
    assert_eq!(catalog.len(), 5, "the five workflow apps, nothing else");
    check(
        &mut checks,
        "seed-library adds the five workflow apps (19 in all); the operator's grants put exactly the five in Diego's action catalog, each with a filing create and a decide command",
    );

    let (endpoint, model_calls) = stub_server().await;
    std::env::set_var(KEY_ENV, "sk-ant-synthetic-not-a-real-key");
    std::env::set_var(ENDPOINT_ENV, &endpoint);
    let parser = Parser::from_env();

    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let mint = |principal: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            stores
                .workspace
                .mint_session_for_tests(ORG, principal, &reference)
                .await
                .unwrap()
        }
    };
    let diego = mint(EMPLOYEE).await;
    let sarah = mint(REVIEWER).await;
    let priya = mint(PRINCIPAL).await;
    let record_page: Arc<dyn RecordPage> = pages;
    let stand = std::env::var("GENUI_WORKFLOWS_STAND_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let work = Arc::new(Work::new(
        auth.clone(),
        None,
        Arc::new(RecordingPages::default()),
    ));
    let canvas_pages: Arc<dyn canvas::CanvasPages> = if stand.is_some() {
        let mut manifest =
            forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
        manifest.routes.retain(|route, _| route == "/canvas");
        let modules = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
        let pages = Arc::new(SsrCanvasPages {
            page: modules.bind::<CanvasProps>("canvas").unwrap(),
        });
        modules.finalize().unwrap();
        pages
    } else {
        Arc::new(NoCanvas)
    };
    let composer =
        Composer::new(None, "router", "generator").with_demo_organizations([ORG.to_owned()]);
    let app = http::uniform_with_many(
        Router::new()
            .route(
                "/_assets/{*filename}",
                axum::routing::get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(crate::auth::routes::router(auth.clone()))
            .merge(sharing::router(auth.clone()))
            .merge(events::routes::router(auth.clone()))
            .merge(crate::commands::routes::router(Arc::new(Commands {
                auth: auth.clone(),
                page: record_page,
            })))
            .merge(actions::routes::records_router(auth.clone()))
            .merge(crate::composer::commands::router(auth.clone()))
            .merge(crate::query::routes::router(work.clone()))
            .merge(mcp::tokens::router(auth.clone()))
            .merge(canvas::router(Arc::new(canvas::Canvas {
                work: work.clone(),
                pages: canvas_pages,
                live_events: "/v1/events".into(),
                demo_organizations: [ORG.to_owned()].into_iter().collect(),
            }))),
        vec![
            (
                actions::PATH,
                actions::routes::router(Arc::new(Act::new(auth.clone(), parser))),
            ),
            (
                mcp::PATH,
                mcp::router(Arc::new(mcp::Mcp::new(work.clone(), composer))),
            ),
        ],
    );
    let act = |cookie: &str, body: Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        async move {
            send(
                &app,
                "POST",
                actions::PATH,
                Some(&cookie),
                &body.to_string(),
                &SAME_ORIGIN,
            )
            .await
        }
    };
    let say = |cookie: &str, text: &str| act(cookie, json!({"utterance": text}));
    let decide = |cookie: &str, draft: &Value, record: &str, status: &str| {
        act(
            cookie,
            json!({"decide": {"app_id": draft["app_id"], "work_id": draft["work_id"], "record_id": record, "status": status}}),
        )
    };
    let post_command =
        |cookie: &str, app_id: &str, work_id: &str, command: &str, request: Value| {
            let app = app.clone();
            let cookie = cookie.to_owned();
            let uri = format!("/v1/work/{app_id}/{work_id}/commands/{command}");
            async move {
                let key = request["idempotency_key"].as_str().unwrap().to_owned();
                send(
                    &app,
                    "POST",
                    &uri,
                    Some(&cookie),
                    &request.to_string(),
                    &json_headers(&key),
                )
                .await
            }
        };
    let confirm = |cookie: &str, draft: Value| {
        let app_id = draft["app_id"].as_str().unwrap().to_owned();
        let work_id = draft["work_id"].as_str().unwrap().to_owned();
        let command = draft["command_id"].as_str().unwrap().to_owned();
        post_command(
            cookie,
            &app_id,
            &work_id,
            &command,
            draft["command_request"].clone(),
        )
    };
    let share = |cookie: &str, draft: &Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = format!(
            "/v1/work/{}/{}/share",
            draft["app_id"].as_str().unwrap(),
            draft["work_id"].as_str().unwrap()
        );
        let approvers: Vec<String> = draft["approvers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["principal_id"].as_str().unwrap().to_owned())
            .collect();
        async move {
            for principal in approvers {
                let body =
                    json!({"grantee_principal_id": principal, "record_write": true}).to_string();
                let reply = send(&app, "POST", &uri, Some(&cookie), &body, &SAME_ORIGIN).await;
                assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
            }
        }
    };
    let records = |cookie: &str, app_id: &str, work_id: &str| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = format!("/v1/work/{app_id}/{work_id}/records");
        async move {
            let reply = send(&app, "GET", &uri, Some(&cookie), "", &[]).await;
            assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
            reply.json()
        }
    };
    let row = |listing: &Value, record: &str| -> Value {
        listing["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["record_id"] == record)
            .cloned()
            .unwrap_or(Value::Null)
    };

    let mut diego_stream = open_stream(&app, &diego).await;
    let mut sarah_stream = open_stream(&app, &sarah).await;
    let mut priya_stream = open_stream(&app, &priya).await;

    // --- One rehearsed request per app, filed, shared, decided, seen live ---
    struct Flow {
        name: &'static str,
        sentence: &'static str,
        schema: &'static str,
        start: &'static str,
        decision: &'static str,
        expect: &'static str,
    }
    let flows = [
        Flow { name: "time_off", sentence: "Book me off Nov 24–26 for Thanksgiving, Sarah approves", schema: "leave_request", start: "submitted", decision: "approved", expect: "approved" },
        Flow { name: "purchase_small", sentence: "Buy a monitor arm from Atlas Office Supply for $180, cost center engineering, Sarah approves", schema: "purchase_request", start: "submitted", decision: "approved", expect: "approved" },
        Flow { name: "access", sentence: "I need admin access to the production database until Nov 30 for the billing migration, Sarah approves", schema: "access_request", start: "requested", decision: "granted", expect: "granted" },
        Flow { name: "discount_deep", sentence: "Request a 15% discount for Kestrel Logistics on a $120,000 deal, competitive pressure, Sarah approves", schema: "discount_request", start: "submitted", decision: "approved", expect: "approved" },
    ];
    let mut works = std::collections::BTreeMap::new();
    for flow in &flows {
        let started = Instant::now();
        let reply = say(&diego, flow.sentence).await;
        timings.insert(format!("{}_card_ms", flow.name), json!(ms(started)));
        assert_eq!(
            reply.status,
            StatusCode::OK,
            "{}: {}",
            flow.name,
            reply.body
        );
        let draft = reply.json();
        assert_eq!(
            (draft["action"].as_str(), draft["schema_id"].as_str()),
            (Some("create"), Some(flow.schema)),
            "{}",
            flow.name
        );
        assert_eq!(
            display_of(&draft, "status"),
            flow.start,
            "{}: the declared initial value",
            flow.name
        );
        assert_eq!(draft["approvers"][0]["principal_id"], REVIEWER);
        let (app_id, work_id) = (
            draft["app_id"].as_str().unwrap().to_owned(),
            draft["work_id"].as_str().unwrap().to_owned(),
        );
        if draft["placed"] == true {
            until(&mut diego_stream, "app.placed", &app_id, &work_id).await;
        }
        let started = Instant::now();
        let committed = confirm(&diego, draft.clone()).await;
        timings.insert(format!("{}_confirm_ms", flow.name), json!(ms(started)));
        assert_eq!(committed.status, StatusCode::OK, "{}", committed.body);
        let record = committed.json()["record_id"].as_str().unwrap().to_owned();
        until(&mut diego_stream, "record.committed", &app_id, &work_id).await;
        share(&diego, &draft).await;
        if draft["placed"] == true {
            until(&mut sarah_stream, "app.shared", &app_id, &work_id).await;
        }
        let listing = records(&sarah, &app_id, &work_id).await;
        let pending = row(&listing, &record);
        assert_eq!(status_of(&pending), flow.start);
        assert!(
            pending["next"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["to"] == flow.expect),
            "{pending}"
        );
        let started = Instant::now();
        let decision = decide(&sarah, &draft, &record, flow.decision).await;
        assert_eq!(decision.status, StatusCode::OK, "{}", decision.body);
        let decision = decision.json();
        assert_eq!(display_of(&decision, "status"), flow.expect);
        let decided = confirm(&sarah, decision).await;
        timings.insert(format!("{}_decide_ms", flow.name), json!(ms(started)));
        assert_eq!(decided.status, StatusCode::OK, "{}", decided.body);
        assert_eq!(decided.json()["record_version"], 2);
        until(&mut diego_stream, "record.committed", &app_id, &work_id).await;
        until(&mut sarah_stream, "record.committed", &app_id, &work_id).await;
        let mine = records(&diego, &app_id, &work_id).await;
        assert_eq!(status_of(&row(&mine, &record)), flow.expect);
        assert!(row(&mine, &record)["next"].as_array().unwrap().is_empty());
        works.insert(flow.name, (app_id, work_id, record));
    }
    // The time off record is typed as the sentence said.
    let (pto_app, pto_work, pto_record) = works["time_off"].clone();
    let pto = row(&records(&diego, &pto_app, &pto_work).await, &pto_record);
    assert_eq!(value_of(&pto, "start")["value"], "2026-11-24");
    assert_eq!(value_of(&pto, "days")["value"], "3");
    assert_eq!(
        model_calls.load(Ordering::SeqCst),
        0,
        "rehearsed sentences never call the model"
    );
    check(
        &mut checks,
        &format!(
            "time off, a $180 purchase, an admin access request and a 15% discount: each rehearsed sentence answers a typed card (no model call; status at the declared initial value), Confirm files one audited record (record.committed on Diego's stream), the share reaches Sarah (app.shared on hers), her listing offers exactly the allowed transition, her structured decision commits version 2 (approved / approved in one step under $5,000 / granted / approved) and Diego's listing shows it live; card {} / {} / {} / {} ms",
            timings["time_off_card_ms"], timings["purchase_small_card_ms"], timings["access_card_ms"], timings["discount_deep_card_ms"]
        ),
    );

    // --- Purchase over $5,000: manager, then finance; finance first is refused ---
    let reply = say(&diego, "Buy 12 Figma Enterprise seats from Figma for $7,200, cost center design, Sarah and Priya approve").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let big = reply.json();
    let approvers: Vec<&str> = big["approvers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["label"].as_str().unwrap())
        .collect();
    assert_eq!(approvers, ["Sarah Chen", "Priya Natarajan"]);
    assert_eq!(display_of(&big, "amount"), "$7,200.00");
    let (buy_app, buy_work) = (
        big["app_id"].as_str().unwrap().to_owned(),
        big["work_id"].as_str().unwrap().to_owned(),
    );
    let committed = confirm(&diego, big.clone()).await;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.body);
    let big_record = committed.json()["record_id"].as_str().unwrap().to_owned();
    until(&mut diego_stream, "record.committed", &buy_app, &buy_work).await;
    share(&diego, &big).await;
    until(&mut priya_stream, "app.shared", &buy_app, &buy_work).await;
    let seen = records(&priya, &buy_app, &buy_work).await;
    let pending = row(&seen, &big_record);
    let next: Vec<&str> = pending["next"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["to"].as_str().unwrap())
        .collect();
    assert_eq!(
        next,
        ["manager_approved", "rejected"],
        "over $5,000 the manager's step comes first"
    );
    assert_eq!(
        pending["reference"],
        actions::workflows::reference(&big_record)
    );
    // Finance approving first: a command straight to "approved" is refused by storage.
    let mut early = big["command_request"].clone();
    early["command_id"] = json!("decide");
    early["expected_record_version"] = json!(1);
    early["values"] =
        json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]);
    early["idempotency_key"] = json!("0000000000000000000000000000e107");
    early["record_id"] = json!(big_record);
    let refused = post_command(&priya, &buy_app, &buy_work, "decide", early).await;
    assert_ne!(refused.status, StatusCode::OK, "{}", refused.body);
    assert_eq!(
        status_of(&row(
            &records(&priya, &buy_app, &buy_work).await,
            &big_record
        )),
        "submitted"
    );
    let started = Instant::now();
    let manager = decide(&sarah, &big, &big_record, "approved").await;
    assert_eq!(manager.status, StatusCode::OK, "{}", manager.body);
    let manager = manager.json();
    assert_eq!(display_of(&manager, "status"), "manager_approved");
    assert_eq!(
        manager["rule"],
        "Manager approved by the app's declared rule: amount > $5,000.00"
    );
    assert_eq!(confirm(&sarah, manager).await.json()["record_version"], 2);
    until(&mut priya_stream, "record.committed", &buy_app, &buy_work).await;
    let seen = row(&records(&priya, &buy_app, &buy_work).await, &big_record);
    let next: Vec<&str> = seen["next"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["to"].as_str().unwrap())
        .collect();
    assert_eq!(next, ["approved", "rejected"]);
    let finance = decide(&priya, &big, &big_record, "approved").await.json();
    assert_eq!(display_of(&finance, "status"), "approved");
    assert_eq!(confirm(&priya, finance).await.json()["record_version"], 3);
    timings.insert("two_step_ms".into(), json!(ms(started)));
    until(&mut diego_stream, "record.committed", &buy_app, &buy_work).await;
    let final_row = row(&records(&diego, &buy_app, &buy_work).await, &big_record);
    assert_eq!(status_of(&final_row), "approved");
    assert_eq!(final_row["updater"]["label"], "Priya Natarajan");
    // Under $5,000 the manager-only step is refused (its guard fails).
    let (small_app, small_work, small_record) = works["purchase_small"].clone();
    let small = row(
        &records(&diego, &small_app, &small_work).await,
        &small_record,
    );
    assert_eq!(small["version"], 2, "one approval completed it");
    check(
        &mut checks,
        &format!("a $7,200 purchase names two approvers (Sarah Chen, then Priya Natarajan) and is shared with both; at submitted it offers only manager_approved or rejected; finance posting \"approved\" first is refused by storage and nothing changes; Sarah's Approve drafts manager_approved with the declared rule shown (amount > $5,000.00), then Priya's Approve commits approved (version 3, updater Priya) and Diego sees it live; the $180 one completed in one approval; two steps {} ms", timings["two_step_ms"]),
    );

    // --- Vendor invoice: the PO match, computed by the server, display only ---
    let po = actions::workflows::reference(&big_record);
    let reply = say(
        &priya,
        &format!("Log invoice FIG-2207 from Figma for $7,450, {po}, due Nov 30, Sarah approves"),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    let invoice = reply.json();
    assert_eq!(display_of(&invoice, "status"), "received");
    assert_eq!(invoice["flags"][0]["state"], "mismatch", "{invoice}");
    assert_eq!(
        invoice["flags"][0]["label"],
        format!("PO mismatch: {po} is $7,200.00, this invoice is $7,450.00")
    );
    let matching = say(
        &priya,
        &format!("Log invoice FIG-2208 from Figma for $7,200, {po}, due Nov 30"),
    )
    .await
    .json();
    assert_eq!(matching["flags"][0]["state"], "match");
    let committed = confirm(&priya, invoice.clone()).await;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.body);
    let invoice_record = committed.json()["record_id"].as_str().unwrap().to_owned();
    let (inv_app, inv_work) = (
        invoice["app_id"].as_str().unwrap().to_owned(),
        invoice["work_id"].as_str().unwrap().to_owned(),
    );
    until(&mut priya_stream, "record.committed", &inv_app, &inv_work).await;
    share(&priya, &invoice).await;
    let listed = row(&records(&sarah, &inv_app, &inv_work).await, &invoice_record);
    // Sarah sees the purchase too (shared), so the flag is the same for her.
    assert_eq!(listed["flags"][0]["state"], "mismatch");
    let approve = decide(&sarah, &invoice, &invoice_record, "approved")
        .await
        .json();
    assert_eq!(
        confirm(&sarah, approve).await.json()["record_version"],
        2,
        "the mismatch never blocks"
    );
    let pay = decide(&sarah, &invoice, &invoice_record, "paid")
        .await
        .json();
    assert_eq!(display_of(&pay, "status"), "paid");
    assert_eq!(confirm(&sarah, pay).await.json()["record_version"], 3);
    check(
        &mut checks,
        &format!("Priya logs invoice FIG-2207 for $7,450 naming {po}: the card carries the server's flag \"PO mismatch: {po} is $7,200.00, this invoice is $7,450.00\" (a $7,200 one: match), status received by the declared initial value; after the share Sarah's listing carries the same flag, and she approves and marks it paid (versions 2 and 3): the flag never blocks"),
    );

    // --- Deal desk: 10% or less approves on create, by the declared rule ---
    let audit_before = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'"),
    )
    .await;
    let started = Instant::now();
    let auto = say(
        &diego,
        "Request an 8% discount for Quill Education on a $48,000 renewal, multi-year commitment",
    )
    .await
    .json();
    assert_eq!(display_of(&auto, "status"), "approved");
    assert_eq!(display_of(&auto, "discount_pct"), "8%");
    assert_eq!(
        auto["rule"],
        "Approved by the app's declared rule: discount pct ≤ 10%"
    );
    let committed = confirm(&diego, auto.clone()).await;
    timings.insert("auto_approve_ms".into(), json!(ms(started)));
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.body);
    let auto_outcome = committed.json();
    let (deal_app, deal_work) = (
        auto["app_id"].as_str().unwrap().to_owned(),
        auto["work_id"].as_str().unwrap().to_owned(),
    );
    until(&mut diego_stream, "record.committed", &deal_app, &deal_work).await;
    let audit_after = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'"),
    )
    .await;
    assert_eq!(audit_after, audit_before + 1, "one audited command");
    let event = scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND event_id='{}'", auto_outcome["audit_event_id"].as_str().unwrap())).await;
    assert_eq!(event, 1);
    // A create claiming "approved" for 15% is refused: the status is the rule's.
    let mut forged = auto["command_request"].clone();
    forged["record_id"] = json!("00000000-0000-4000-8000-00000000e107");
    forged["idempotency_key"] = json!("0000000000000000000000000000e108");
    for item in forged["values"].as_array_mut().unwrap() {
        if item["field_id"] == "discount_pct" {
            item["value"]["value"] = json!("15");
        }
    }
    let refused = post_command(&diego, &deal_app, &deal_work, "submit", forged).await;
    assert_ne!(refused.status, StatusCode::OK, "{}", refused.body);
    check(
        &mut checks,
        &format!("deal desk: an 8% discount drafts status approved with the rule \"discount pct ≤ 10%\" and commits as one audited command (exactly one new audit event, its id the outcome's) in {} ms; a create claiming approved for 15% is refused by storage; the 15% Kestrel request waited for Sarah (above)", timings["auto_approve_ms"]),
    );

    // --- One unrehearsed sentence per app: the model (stub) parses, the server types ---
    for (sentence, schema, status) in [
        (
            "Out sick tomorrow, Sarah knows",
            "leave_request",
            "submitted",
        ),
        (
            "We need a new label printer, about 320 dollars from Atlas",
            "purchase_request",
            "submitted",
        ),
        (
            "Atlas sent bill A-99 for 120 dollars due end of November",
            "vendor_invoice",
            "received",
        ),
        (
            "Give me read rights on hr-records till December",
            "access_request",
            "requested",
        ),
        (
            "Brightwater wants 25 off on their 90k renewal",
            "discount_request",
            "submitted",
        ),
    ] {
        let reply = say(&diego, sentence).await;
        assert_eq!(reply.status, StatusCode::OK, "{sentence}: {}", reply.body);
        let draft = reply.json();
        assert_eq!(draft["schema_id"], schema, "{sentence}");
        assert_eq!(
            display_of(&draft, "status"),
            status,
            "{sentence}: the model's status word is ignored"
        );
    }
    assert_eq!(model_calls.load(Ordering::SeqCst), 5);
    check(
        &mut checks,
        "one unrehearsed sentence per app goes to the model (loopback stub, 5 calls) and drafts in the right app; every status the model named (approved, paid, granted) is replaced by the declared initial value (submitted, received, requested; 25% stays submitted)",
    );

    // --- An MCP agent decides as the approver; the commit is marked via its label ---
    let minted = send(
        &app,
        "POST",
        "/v1/agent-tokens",
        Some(&sarah),
        &json!({"label": "Haiku"}).to_string(),
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(minted.status, StatusCode::CREATED, "{}", minted.body);
    let token = minted.json()["token"].as_str().unwrap().to_owned();
    let next_id = AtomicUsize::new(1);
    let rpc = |name: &str, arguments: Value| {
        let app = app.clone();
        let bearer = format!("Bearer {token}");
        let id = next_id.fetch_add(1, Ordering::SeqCst);
        let body = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).to_string();
        async move {
            let reply = send(
                &app,
                "POST",
                mcp::PATH,
                None,
                &body,
                &[
                    ("authorization", &bearer),
                    ("content-type", "application/json"),
                ],
            )
            .await;
            assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
            reply.json()["result"].clone()
        }
    };
    let pto2 = say(
        &diego,
        "Book me off Dec 21–23 for the holidays, Sarah approves",
    )
    .await
    .json();
    let pto2_outcome = confirm(&diego, pto2.clone()).await.json();
    share(&diego, &pto2).await;
    let pending = rpc("forma_list_pending", json!({})).await;
    assert_eq!(pending["isError"], false, "{pending}");
    assert!(
        pending["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("holidays"),
        "{pending}"
    );
    let started = Instant::now();
    let drafted = rpc(
        "forma_draft_decision",
        json!({"target_hint": "holidays time off", "decision": "approve"}),
    )
    .await;
    assert_eq!(drafted["isError"], false, "{drafted}");
    let draft_id = drafted["structuredContent"]["draft_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let committed = rpc("forma_commit", json!({"draft_id": draft_id})).await;
    timings.insert("mcp_decide_ms".into(), json!(ms(started)));
    assert_eq!(committed["isError"], false, "{committed}");
    let decided = row(
        &records(&diego, &pto_app, &pto_work).await,
        pto2_outcome["record_id"].as_str().unwrap(),
    );
    assert_eq!(status_of(&decided), "approved");
    assert_eq!(decided["via"], "Haiku");
    check(
        &mut checks,
        &format!("MCP: Sarah's agent token \"Haiku\" lists the pending holidays time off (forma_list_pending), drafts the decision across her decidable works (forma_draft_decision, no app named) and commits it (forma_commit) in {} ms; Diego's listing shows approved, via Haiku", timings["mcp_decide_ms"]),
    );

    // --- The stand: the browser walk and the Haiku agent loop ---
    if let Some(seconds) = stand {
        // Left pending for the Haiku agent (Sarah), and at its finance step for Priya.
        let seat = say(
            &diego,
            "Buy a Figma seat from Figma for $480, cost center design, Sarah approves",
        )
        .await
        .json();
        confirm(&diego, seat.clone()).await;
        share(&diego, &seat).await;
        let mid = say(&diego, "Buy 10 standing desks from Atlas Office Supply for $6,400, cost center operations, Sarah and Priya approve").await.json();
        let mid_record = confirm(&diego, mid.clone()).await.json()["record_id"]
            .as_str()
            .unwrap()
            .to_owned();
        share(&diego, &mid).await;
        let step = decide(&sarah, &mid, &mid_record, "approved").await.json();
        confirm(&sarah, step).await;
        let file = std::env::var("GENUI_WORKFLOWS_STAND_FILE").unwrap_or_default();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        if !file.is_empty() {
            std::fs::write(
                &file,
                serde_json::to_vec(&json!({
                    "origin": origin, "cookie": cookies::SESSION,
                    "diego": diego, "sarah": sarah, "priya": priya, "sarah_token": token,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        println!("WORKFLOWS_STAND {origin}");
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
        "campaign": "APP-107: the workflow library end to end (synthetic identities, Docker, loopback model stub)",
        "passed": true,
        "timings": timings,
        "checks": checks,
    });
    if let Some(path) = std::env::var_os("GENUI_WORKFLOWS_CAMPAIGN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    demo::configure_for_tests(false, &[]);
    directory::set_for_tests(&[]);
    drop((diego_stream, sarah_stream, priya_stream));
    stores.close().await.unwrap();
}
