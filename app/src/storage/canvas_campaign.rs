//! ADR-011 pitch canvas campaign (APP-092, APP-094). Isolated test
//! executable only, as the APP-012 query campaign it extends: local Docker,
//! the locked Python environment with DuckDB (`GENUI_NATIVE_FIXTURE_PYTHON`)
//! and the real confined engine over the uploaded Northstar revision.
//!
//! Real members signed in through the APP-009 routes: Maya (Northstar) with
//! a saved work bound to the uploaded events and `record_read`/
//! `record_write` on Spend Review; a canary member of the other
//! organization. Checks: `GET /canvas` is Maya's two apps with their cards
//! and record form; the group JSON answers her works only; the canary, an
//! anonymous caller and a forged session get the uniform 404 on her group
//! and on her views' query route, with no engine dispatch; the canvas's
//! query route results carry contract-valid receipts.
//!
//! With `GENUI_CANVAS_STAND_SECONDS` the campaign also serves the composed
//! routes (canvas page, assets, query, command and auth routes) on
//! 127.0.0.1 for that long (with the event stream), writing the origin and
//! Maya's and the canary member's synthetic session cookies to
//! `GENUI_CANVAS_STAND_FILE` (scratch only, never evidence), so a
//! local browser can take the pitch screenshots against real results.
use super::*;
use crate::{
    auth::{
        invite::Invitations,
        synthetic_issuer::{Issuer, ISSUER as GATEWASM},
        tests::{self as harness, Campaign, CANARY_HANDLE, NORTHSTAR_HANDLE},
        Auth,
    },
    commands::{self as record_commands, RecordPage, RecordView},
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        routes::tests::{Counting, RecordingPages},
        Executor, Work,
    },
    storage::{
        admission_qualification::FakeProvider,
        definition_qualification::{grant, spend_review_s05, EXPENSES, SPEND_APP},
    },
};
use axum::http::StatusCode;

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

struct NoRecordPage;
impl RecordPage for NoRecordPage {
    fn record(&self, _: &RecordView) -> axum::response::Response {
        use axum::response::IntoResponse;
        (StatusCode::OK, "record").into_response()
    }
}

const CANVAS_WORK: &str = "00000000-0000-4000-8000-0000000000f7";

/// `GENUI_CANVAS_STAND_SECONDS`: serve the local pitch stand that long.
fn stand_seconds() -> Option<u64> {
    std::env::var("GENUI_CANVAS_STAND_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
}

#[test]
#[ignore = "requires Docker and GENUI_NATIVE_FIXTURE_PYTHON (DuckDB 2.0.0)"]
fn native_canvas_campaign() {
    std::thread::Builder::new()
        .name("pitch-canvas-campaign".into())
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

async fn campaign() {
    let _ = tracing_subscriber::fmt().with_ansi(false).try_init();
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    let uploaded = json!({
        "organization_id": ORG, "source_id": SOURCE, "revision_id": REVISION,
        "content_digest": CONTENT, "normalized_digest": NORMALIZED, "object_digest": NORMALIZED,
        "row_count": 6, "contract_digest": CONTRACT,
    });
    let path = record(&fixture, "record.json", &uploaded);
    operator(
        &fixture,
        &[
            "register-source-revision",
            "--organization",
            ORG,
            "--record",
            &path,
        ],
    );
    let python = fixture_python();
    let root = Directory::new("canvas");
    assert_eq!(
        cache_northstar(&python, &root.0.join("cache"), ORG, REVISION),
        NORMALIZED
    );
    let engine = Arc::new(real_engine(&python, &root.0));

    // Spend Review (S05) is created by its author under the fixture's own
    // registry, before the loopback issuer's registry replaces it.
    let seeding = Stores::new(fixture.settings()).unwrap();
    seeding.readiness().await.unwrap();
    let spend = spend_review_s05(&fixture, &seeding).await;
    drop(seeding);
    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    fixture.sql(&format!("INSERT INTO genui.provider_tenant_bindings VALUES ('{ORG}','{GATEWASM}','{}',true),('{OTHER}','{GATEWASM}','{}',true)", harness::NORTHSTAR_TENANT, harness::CANARY_TENANT), "genui");
    fixture.write_registry(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000a12","bindings":[
        {"issuer":GATEWASM,"provider_tenant_id":harness::NORTHSTAR_TENANT,"organization_id":ORG},
        {"issuer":GATEWASM,"provider_tenant_id":harness::CANARY_TENANT,"organization_id":OTHER}]}));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let issuer = Issuer::start(harness::tenants()).await;
    let provider = FakeProvider::start_at(
        &fixture,
        harness::NORTHSTAR_TENANT,
        "canvas-northstar",
        None,
        None,
    );
    let canary_provider = FakeProvider::start_at(
        &fixture,
        harness::CANARY_TENANT,
        "canvas-canary",
        Some(&provider.capabilities),
        Some(&provider.secret),
    );
    let auth = Arc::new(
        Auth::loopback(
            harness::config(&issuer.origin),
            Some(stores.clone()),
            Arc::new(harness::TestPages),
        )
        .with_invitations(Some(Invitations::open(&provider.capabilities).unwrap())),
    );
    let counting = Arc::new(Counting {
        engine: Some(engine.clone() as Arc<dyn Executor>),
        ..Counting::default()
    });
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(counting.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    // The binary's canvas page, from the built admin/dist.
    // Only the canvas route's module is loaded: its sibling pages' props are
    // the binary's (main.rs), which binds and finalizes all of them.
    let mut manifest =
        forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
    manifest.routes.retain(|route, _| route == "/canvas");
    let pages = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
    let canvas_pages = Arc::new(SsrCanvasPages {
        page: pages.bind::<CanvasProps>("canvas").unwrap(),
    });
    pages.finalize().unwrap();
    let routes = crate::http::uniform(
        axum::Router::new()
            .route(
                "/_assets/{*filename}",
                axum::routing::get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(crate::auth::routes::router(auth.clone()))
            .merge(record_commands::routes::router(Arc::new(
                record_commands::routes::Commands {
                    auth: auth.clone(),
                    page: Arc::new(NoRecordPage),
                },
            )))
            .merge(crate::query::routes::router(work.clone()))
            // The stand also serves the session-gated event stream, so the
            // canvas shows its live state and re-runs on a committed record.
            .merge(crate::events::routes::router(auth.clone()))
            .merge(canvas::router(Arc::new(canvas::Canvas {
                work: work.clone(),
                pages: canvas_pages,
                live_events: if stand_seconds().is_some() {
                    "/v1/events".to_owned()
                } else {
                    String::new()
                },
                demo_organizations: canvas::demo_organizations(),
            }))),
    );
    let c = Campaign {
        app: harness::app(auth.clone()),
        fixture,
        stores: stores.clone(),
        issuer,
        auth: auth.clone(),
        log: Arc::default(),
        checks: Vec::new(),
        provider,
        canary: Some(canary_provider),
        codes: Default::default(),
    };

    // Maya: a saved work bound to the uploaded Northstar events, as creator.
    let (maya_subject, mut maya) = c.member("0000000013c1").await;
    let member = c.principal(&maya_subject).await;
    for source in [SOURCE, INVENTORY] {
        grant(&c.fixture, &member, "source_read", &["--source", source]);
    }
    operator(
        &c.fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            CANVAS_WORK,
            "--creator",
            &member,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );
    let (_, mut canary) = c.member_in(OTHER, "0000000013c2").await;
    let group_path = format!("/v1/canvas/works/{APP}/{CANVAS_WORK}");
    let spend_path = format!("/v1/canvas/works/{SPEND_APP}/{}", spend.work);

    // --- Maya's canvas before Spend Review is shared with her ---
    let page = maya.get(&routes, "/canvas").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.headers[axum::http::header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("no-store"));
    let body = &page.body;
    assert!(body.contains(&format!("id=\"g-{CANVAS_WORK}\"")), "{body}");
    assert!(!body.contains(&spend.work), "not shared yet");
    assert!(body.contains(&format!(
        "data-query=\"/v1/work/{APP}/{CANVAS_WORK}/queries/total\""
    )));
    assert!(body.contains(&format!(
        "data-query=\"/v1/work/{APP}/{CANVAS_WORK}/queries/timeline\""
    )));
    assert!(!body.contains("class=\"value\""), "nothing ran on GET");
    assert_eq!(counting.calls(), 0);
    let group = maya.get(&routes, &group_path).await;
    assert_eq!(group.status, StatusCode::OK, "{}", group.body);
    let group: Value = serde_json::from_str(&group.body).unwrap();
    assert_eq!(group["work"], CANVAS_WORK);
    assert_eq!(
        group["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["view"].clone())
            .collect::<Vec<_>>(),
        [json!("total"), json!("timeline")]
    );
    assert_eq!(group["cards"][0]["integer_columns"], "total_bytes");
    let unshared = maya.get(&routes, &spend_path).await;
    assert_eq!(unshared.status, StatusCode::NOT_FOUND);
    let total = maya
        .post(
            &routes,
            &format!("/v1/work/{APP}/{CANVAS_WORK}/queries/total"),
            &query_body(&window()),
        )
        .await;
    assert_eq!(total.status, StatusCode::OK, "{}", total.body);
    let (rows, receipt_document) = receipted(&total.body);
    assert_eq!(rows, json!([["1140000"]]));
    assert_eq!(receipt_document["work_id"], CANVAS_WORK);
    check(
        &mut checks,
        "canvas: GET /canvas renders the member's saved work (Northstar egress bound to the uploaded events) with its view cards idle and nothing run on GET; the group JSON lists the views with their integer columns; an app not shared with her (Spend Review) is absent and its group is the uniform 404; the canvas card's query route answers 1,140,000 with a contract-valid receipt",
    );

    // --- Sharing: Spend Review is shared with Maya and appears ---
    let share = || {
        for permission in ["discover", "run"] {
            grant(&c.fixture, &member, permission, &["--app", SPEND_APP]);
        }
        grant(&c.fixture, &member, "source_read", &["--source", EXPENSES]);
        for permission in ["record_read", "record_write"] {
            grant(
                &c.fixture,
                &member,
                permission,
                &["--app", SPEND_APP, "--work", &spend.work],
            );
        }
    };
    let stand = stand_seconds();
    let mut shared = false;
    if let Some(seconds) = stand {
        // The local stand for the pitch screenshots: the composed routes on
        // 127.0.0.1; `<stand file>.share` shares Spend Review with Maya (the
        // placement demo), `<stand file>.stop` ends it.
        let file = std::env::var("GENUI_CANVAS_STAND_FILE").unwrap_or_default();
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let session = maya
            .jar
            .get(crate::auth::cookies::SESSION)
            .cloned()
            .expect("Maya's session");
        let canary_session = canary
            .jar
            .get(crate::auth::cookies::SESSION)
            .cloned()
            .unwrap_or_default();
        if !file.is_empty() {
            std::fs::write(
                &file,
                serde_json::to_vec(&json!({
                    "origin": origin, "cookie": crate::auth::cookies::SESSION, "value": session,
                    "app": APP, "northstar": CANVAS_WORK, "spend_app": SPEND_APP, "spend": spend.work,
                    "canary": canary_session,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        println!("CANVAS_STAND {origin}");
        let served = routes.clone();
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                served.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
        let exists = |suffix: &str| {
            !file.is_empty() && std::path::Path::new(&format!("{file}.{suffix}")).exists()
        };
        while std::time::Instant::now() < deadline && !exists("stop") {
            if !shared && exists("share") {
                share();
                shared = true;
                println!("CANVAS_SHARED");
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        server.abort();
    }
    if !shared {
        share();
    }
    let page = maya.get(&routes, "/canvas").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains(&format!("id=\"g-{}\"", spend.work)));
    assert!(page.body.contains(&format!(
        "action=\"/v1/work/{SPEND_APP}/{}/commands/record_decision\"",
        spend.work
    )));
    let placed = maya.get(&routes, &spend_path).await;
    assert_eq!(placed.status, StatusCode::OK, "{}", placed.body);
    let placed: Value = serde_json::from_str(&placed.body).unwrap();
    assert_eq!(placed["forms"][0]["command"], "record_decision");
    check(
        &mut checks,
        "sharing: once Spend Review is shared with the member (discover, run, source_read, record_read, record_write), her canvas renders it with its record form posting to the audited command route, and its group JSON (the island's placement seam) answers with the form",
    );

    // --- Isolation: foreign, anonymous and forged callers ---
    let before = counting.calls();
    let foreign_page = canary.get(&routes, "/canvas").await;
    assert_eq!(foreign_page.status, StatusCode::OK);
    assert!(!foreign_page.body.contains(CANVAS_WORK) && !foreign_page.body.contains(&spend.work));
    assert!(foreign_page.body.contains("What do you need?"));
    let mut denials = 0;
    for path in [&group_path, &spend_path] {
        let reply = canary.get(&routes, path).await;
        assert!(
            raw_404(&(
                reply.status,
                reply.headers.clone(),
                reply.body.clone().into_bytes()
            )),
            "{path}"
        );
        denials += 1;
    }
    let foreign_query = canary
        .post(
            &routes,
            &format!("/v1/work/{APP}/{CANVAS_WORK}/queries/total"),
            &query_body(&window()),
        )
        .await;
    assert_eq!(foreign_query.status, StatusCode::NOT_FOUND);
    denials += 1;
    for cookies in [
        String::new(),
        "__Host-genui_session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            .to_owned(),
    ] {
        for path in ["/canvas", group_path.as_str(), spend_path.as_str()] {
            let reply = send_raw(
                routes.clone(),
                raw_request("GET", path, &cookies, Vec::new()),
            )
            .await;
            assert!(raw_404(&reply), "{path}");
            denials += 1;
        }
    }
    // Maya's own group with a query string is the uniform 404 too.
    let filtered = maya
        .get(&routes, &format!("{group_path}?destination=x"))
        .await;
    assert_eq!(filtered.status, StatusCode::NOT_FOUND);
    assert_eq!(counting.calls(), before, "no denial reached the engine");
    check(
        &mut checks,
        &format!("canvas isolation: the canary member's canvas shows none of Northstar's works; {denials} requests (the canary member on both Northstar groups and on the canvas view's query route; anonymous and forged sessions on /canvas and both groups) get the uniform 404, and the dispatch counter did not move"),
    );

    let report = json!({
        "scope": "ADR-011 pitch canvas, local Docker campaign (synthetic identities, uploaded Northstar events)",
        "checks": checks,
    });
    if let Some(path) = std::env::var_os("GENUI_CANVAS_CAMPAIGN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    drop(c);
}
