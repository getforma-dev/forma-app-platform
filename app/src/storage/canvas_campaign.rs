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
/// APP-169: a well-formed work id no row names.
const UNKNOWN_WORK: &str = "00000000-0000-4000-8000-00000000dead";
const CARD: &str = r#"{"kind":"card"}"#;
const APP_KIND: &str = r#"{"kind":"app"}"#;
const NOT_FOUND_BODY: &str = r#"{"code":"not_found","status":404}"#;

fn kind_path(app: &str, work: &str) -> String {
    format!("/v1/library/{app}/{work}/kind")
}

/// The start tag of the library row of `work` on a canvas page, or empty.
fn library_row(body: &str, work: &str) -> String {
    body.find(&format!("id=\"lib-{work}\""))
        .map(|start| {
            let rest = &body[start..];
            rest[..rest.find('>').unwrap_or(rest.len())].to_owned()
        })
        .unwrap_or_default()
}

/// APP-113's deferred batching: the canvas page's batched reads equal the
/// one-work-at-a-time reads they replaced, for `request`: the listing
/// (`openable_works` against the verbatim serial `openable_works_serial`),
/// the library catalog (the same), and per listed work, plus an unknown work
/// and a malformed identifier, `work_reads` against `work_outline` and then
/// `list_records` (asked for when the work has collections, as the page
/// asks). Returns the number of works compared.
pub(super) async fn equivalent_reads(stores: &Stores, request: &TrustedRequest) -> usize {
    let listed = stores.data.openable_works(request).await;
    assert_eq!(
        listed,
        stores.data.openable_works_serial(request).await,
        "the batched listing equals the serial listing"
    );
    assert_eq!(
        stores.data.library_catalog(request).await,
        stores.data.library_catalog_serial(request).await,
        "the batched library catalog equals the serial one"
    );
    let listed = listed.unwrap_or_default();
    let unknown = "00000000-0000-4000-8000-00000000dead";
    let mut works: Vec<(&str, &str, bool)> = listed
        .iter()
        .map(|w| (w.app.as_str(), w.work.as_str(), !w.collections.is_empty()))
        .collect();
    if let Some(first) = listed.first() {
        works.push((first.app.as_str(), unknown, true));
    }
    works.push(("not-a-uuid", unknown, true));
    let reads = stores.data.work_reads(request, &works).await.unwrap();
    assert_eq!(reads.len(), works.len());
    for (&(app, work, records), read) in works.iter().zip(&reads) {
        let outline = stores.data.work_outline(request, app, work).await;
        assert_eq!(read.outline, outline, "{app}/{work}: outline");
        let listing = if records && outline.is_ok() {
            Some(stores.data.list_records(request, app, work).await)
        } else {
            None
        };
        assert_eq!(read.records, listing, "{app}/{work}: records listing");
    }
    works.len()
}

/// Both outline paths for one work. `Err` is the shared refusal.
pub(super) async fn agreed_outline(
    stores: &Stores,
    request: &TrustedRequest,
    app: &str,
    work: &str,
) -> std::result::Result<crate::storage::WorkOutline, Error> {
    let batched = stores
        .data
        .work_reads(request, &[(app, work, true)])
        .await
        .unwrap();
    assert_eq!(batched.len(), 1);
    let serial = stores.data.work_outline(request, app, work).await;
    assert_eq!(batched[0].outline, serial, "{app}/{work}");
    let quiet = stores
        .data
        .work_reads(request, &[(app, work, false)])
        .await
        .unwrap();
    assert_eq!(
        quiet[0].outline, serial,
        "{app}/{work}: outline without a listing"
    );
    assert!(quiet[0].records.is_none());
    serial
}

/// The member as storage sees her (the identity the loopback issuer signed
/// in).
fn maya_request(c: &Campaign, subject: &str, principal: &str) -> TrustedRequest {
    TrustedRequest {
        organization: ORG.into(),
        principal: principal.into(),
        epoch: 1,
        issuer: GATEWASM.into(),
        provider_tenant: harness::NORTHSTAR_TENANT.into(),
        provider_subject: subject.into(),
        registry_sha256: c.stores.registry_identity().sha256.clone(),
    }
}

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
    let (canary_subject, mut canary) = c.member_in(OTHER, "0000000013c2").await;
    let canary_principal = c.principal(&canary_subject).await;
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
    // APP-104: the library pane lists the work and its cards (metadata only).
    assert!(
        body.contains(&format!("id=\"lib-{CANVAS_WORK}\"")),
        "{body}"
    );
    assert!(!body.contains(&spend.work), "not shared yet");
    assert!(body.contains(&format!("data-card=\"c-{CANVAS_WORK}-total\"")));
    assert!(body.contains(&format!("data-card=\"c-{CANVAS_WORK}-timeline\"")));
    assert!(!body.contains("class=\"value\""), "nothing ran on GET");
    assert_eq!(counting.calls(), 0);
    equivalent_reads(&stores, &maya_request(&c, &maya_subject, &member)).await;
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
    // APP-169: saving a kind on a work she holds no grant on is the same
    // uniform 404 as a work that does not exist.
    let ungranted = maya
        .post(&routes, &kind_path(SPEND_APP, &spend.work), CARD)
        .await;
    let missing = maya
        .post(&routes, &kind_path(APP, UNKNOWN_WORK), CARD)
        .await;
    for reply in [&ungranted, &missing] {
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{}", reply.body);
        assert_eq!(reply.body, NOT_FOUND_BODY);
    }
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
    assert!(page.body.contains(&format!("id=\"lib-{}\"", spend.work)));
    let placed = maya.get(&routes, &spend_path).await;
    assert_eq!(placed.status, StatusCode::OK, "{}", placed.body);
    let placed: Value = serde_json::from_str(&placed.body).unwrap();
    assert_eq!(placed["forms"][0]["command"], "record_decision");
    assert_eq!(
        placed["forms"][0]["action"],
        format!(
            "/v1/work/{SPEND_APP}/{}/commands/record_decision",
            spend.work
        )
    );
    let compared = equivalent_reads(&stores, &maya_request(&c, &maya_subject, &member)).await;
    check(
        &mut checks,
        &format!("batched reads (APP-113): for the member, after the share, the batched listing and library catalog equal the serial ones, and the canvas page's batched outline and records reads equal work_outline and list_records for each of {compared} works (an unknown work and a malformed id included, refused alike)"),
    );
    check(
        &mut checks,
        "sharing: once Spend Review is shared with the member (discover, run, source_read, record_read, record_write), her canvas renders it with its record form posting to the audited command route, and its group JSON (the island's placement seam) answers with the form",
    );

    // The two outline paths agree, and write is the grant rather than a
    // consequence of being able to see the collections.
    let maya_storage = maya_request(&c, &maya_subject, &member);
    let source = agreed_outline(&stores, &maya_storage, APP, CANVAS_WORK)
        .await
        .expect("the creator can open the source-query work");
    assert!(
        !source.views.is_empty(),
        "the source-query work keeps its runnable views"
    );
    assert!(
        source.can_write,
        "the creator's record_write is the outline's can_write"
    );
    let writer = agreed_outline(&stores, &maya_storage, SPEND_APP, &spend.work)
        .await
        .expect("the grantee can open Spend Review");
    assert!(
        !writer.collections.is_empty(),
        "record collections stay visible with record_read"
    );
    assert!(writer.can_write, "record_write is granted");
    let (reader_subject, mut reader_browser) = c.member("0000000013c3").await;
    let reader_id = c.principal(&reader_subject).await;
    for permission in ["discover", "run"] {
        grant(&c.fixture, &reader_id, permission, &["--app", SPEND_APP]);
    }
    grant(
        &c.fixture,
        &reader_id,
        "source_read",
        &["--source", EXPENSES],
    );
    grant(
        &c.fixture,
        &reader_id,
        "record_read",
        &["--app", SPEND_APP, "--work", &spend.work],
    );
    let reader = maya_request(&c, &reader_subject, &reader_id);
    let read_only = agreed_outline(&stores, &reader, SPEND_APP, &spend.work)
        .await
        .expect("record_read without record_write still opens the work");
    assert!(!read_only.can_write, "no record_write grant");
    assert_eq!(
        read_only.collections, writer.collections,
        "the same collections are visible without write"
    );
    let stranger = TrustedRequest {
        organization: OTHER.into(),
        principal: canary_principal,
        epoch: 1,
        issuer: GATEWASM.into(),
        provider_tenant: harness::CANARY_TENANT.into(),
        provider_subject: canary_subject,
        registry_sha256: c.stores.registry_identity().sha256.clone(),
    };
    assert!(
        agreed_outline(&stores, &stranger, APP, CANVAS_WORK)
            .await
            .is_err(),
        "another organization's member is refused"
    );
    check(
        &mut checks,
        "outline authority: the batched outline matches work_outline for the source-query work and for Spend Review; the creator and the record_write grantee have can_write, a record_read grantee sees the same collections with can_write false, and the canary member is refused",
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

    // --- APP-169: the member's saved Card or App kind ---
    let page = maya.get(&routes, "/canvas").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("data-artifact-kinds=\"true\""));
    assert!(
        !page.body.contains("data-artifact-kind=\""),
        "nothing saved yet: no row carries a kind"
    );
    let northstar = kind_path(APP, CANVAS_WORK);
    let spend_kind = kind_path(SPEND_APP, &spend.work);
    // `app` before any row exists is stored (an upsert), then `card` replaces it.
    for (body, kind) in [(APP_KIND, "app"), (CARD, "card")] {
        let saved = maya.post(&routes, &northstar, body).await;
        assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
        assert_eq!(saved.body, format!("{{\"kind\":\"{kind}\"}}"));
        let page = maya.get(&routes, "/canvas").await;
        assert!(
            library_row(&page.body, CANVAS_WORK)
                .contains(&format!("data-artifact-kind=\"{kind}\"")),
            "{kind}"
        );
    }
    // D7.8 (APP-169 review): each change of kind is one `work.kind_set` workspace
    // event in the same transaction; the same choice again writes nothing.
    let kind_events = || {
        c.fixture.sql(
            &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='workspace' AND kind='work.kind_set' AND app_id='{APP}' AND work_id='{CANVAS_WORK}'"),
            "genui",
        )
    };
    assert_eq!(kind_events().trim(), "2", "app then card");
    let again = maya.post(&routes, &northstar, CARD).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    assert_eq!(
        kind_events().trim(),
        "2",
        "the same kind again is not audited"
    );
    // `if_absent` (the canvas's first-load upload) never replaces a saved
    // choice: it answers with the kind already saved and writes nothing.
    let kept = maya
        .post(&routes, &northstar, r#"{"kind":"app","if_absent":true}"#)
        .await;
    assert_eq!(kept.status, StatusCode::OK, "{}", kept.body);
    assert_eq!(kept.body, r#"{"kind":"card"}"#);
    assert_eq!(kind_events().trim(), "2", "if_absent over a saved choice");
    // Where nothing is saved, `if_absent` stores the kind and audits it once.
    let saved = maya
        .post(&routes, &spend_kind, r#"{"kind":"card","if_absent":true}"#)
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body, r#"{"kind":"card"}"#);
    assert_eq!(
        c.fixture
            .sql(
                &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND kind='work.kind_set' AND work_id='{}'", spend.work),
                "genui",
            )
            .trim(),
        "1"
    );
    // D7.8: `verify` recomputes the workspace chain, work.kind_set events included.
    assert_eq!(
        operator(&c.fixture, &["verify", "--organization", ORG])["verified"],
        true
    );
    // A closed body naming card or app; anything else is 422 and stores nothing.
    for body in [
        r#"{"kind":"stack"}"#,
        r#"{"kind":"card","app_id":"x"}"#,
        r#"{"kind":"card","if_absent":false}"#,
        r#"{"kind":"card","if_absent":"true"}"#,
        r#"{}"#,
        "kind=card",
    ] {
        let refused = maya.post(&routes, &northstar, body).await;
        assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert!(refused.body.contains("invalid_kind"), "{}", refused.body);
    }
    // Cross-site with her live session is 403 before anything else.
    let cross = maya
        .send(
            &routes,
            "POST",
            &northstar,
            APP_KIND,
            &[("sec-fetch-site", "cross-site")],
        )
        .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    // Anonymous and forged sessions are the uniform 404.
    for cookies in [
        String::new(),
        "__Host-genui_session=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            .to_owned(),
    ] {
        let reply = send_raw(
            routes.clone(),
            raw_request("POST", &northstar, &cookies, APP_KIND.as_bytes().to_vec()),
        )
        .await;
        assert!(raw_404(&reply));
    }
    // Another organization's member: the uniform 404, and her own canvas
    // and kinds show nothing of Maya's.
    for path in [&northstar, &spend_kind] {
        let foreign = canary.post(&routes, path, APP_KIND).await;
        assert_eq!(foreign.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(foreign.body, NOT_FOUND_BODY);
    }
    // A teammate who opens Spend Review sees no kind on it: the row is Maya's.
    let teammate = reader_browser.get(&routes, "/canvas").await;
    assert_eq!(teammate.status, StatusCode::OK);
    assert!(!library_row(&teammate.body, &spend.work).contains("data-artifact-kind"));
    // Storage: her rows only; the teammate and the canary read none and
    // cannot write where they hold no grant.
    let kinds = stores
        .workspace
        .artifact_kinds(&maya_storage)
        .await
        .unwrap();
    assert_eq!(
        kinds,
        std::collections::BTreeMap::from([
            ((APP.to_owned(), CANVAS_WORK.to_owned()), "card"),
            ((SPEND_APP.to_owned(), spend.work.clone()), "card"),
        ])
    );
    for other in [&reader, &stranger] {
        assert!(stores
            .workspace
            .artifact_kinds(other)
            .await
            .unwrap()
            .is_empty());
        for if_absent in [false, true] {
            assert_eq!(
                stores
                    .workspace
                    .set_artifact_kind(other, APP, CANVAS_WORK, "card", if_absent)
                    .await,
                Err(Error::Denied)
            );
        }
    }
    assert_eq!(
        stores
            .workspace
            .set_artifact_kind(&maya_storage, APP, CANVAS_WORK, "stack", false)
            .await,
        Err(Error::Denied),
        "storage refuses a kind outside card and app"
    );
    // The member write window: an exhausted window is 429 and stores nothing.
    while auth.limits.admit_member_write(ORG, &member).is_ok() {}
    let limited = maya.post(&routes, &northstar, APP_KIND).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(limited
        .headers
        .contains_key(axum::http::header::RETRY_AFTER));
    auth.limits.clear();
    let page = maya.get(&routes, "/canvas").await;
    assert!(library_row(&page.body, CANVAS_WORK).contains("data-artifact-kind=\"card\""));
    // A revoked grant hides the saved card: the work is not listed, so its
    // kind is not rendered, and a write is the uniform 404.
    crate::storage::definition_qualification::revoke(
        &c.fixture,
        &member,
        "record_read",
        &["--app", SPEND_APP, "--work", &spend.work],
    );
    let revoked = maya.get(&routes, "/canvas").await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert!(!revoked.body.contains(&spend.work));
    assert!(library_row(&revoked.body, CANVAS_WORK).contains("data-artifact-kind=\"card\""));
    let refused = maya.post(&routes, &spend_kind, APP_KIND).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    assert_eq!(refused.body, NOT_FOUND_BODY);
    check(
        &mut checks,
        "artifact kind (APP-169): with no grant on a work, saving its kind is the same uniform 404 as a missing work; the canvas says the kinds were read and a work with nothing saved carries no kind; an app before any row is stored and a card replaces it, each rendered on the next load and each one work.kind_set workspace event, while the same kind again writes none; an if_absent write over a saved choice answers with that choice and writes nothing, and where nothing is saved stores the kind with one event; operator verify recomputes the workspace chain with these events; a body other than card or app is 422 invalid_kind; cross-site is 403; anonymous, forged and another organization's member get the uniform 404; a teammate on the same work sees no kind; storage returns only her rows and refuses the teammate, the canary and an unknown kind; an exhausted member write window is 429; once record_read is revoked the saved card is not rendered and the write is the uniform 404",
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
