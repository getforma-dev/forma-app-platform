//! ADR-011 pitch integration campaign (APP-096; the seams of APP-092–095
//! joined). Isolated test executable only, as the canvas campaign it
//! extends: local Docker PostgreSQL, the locked Python environments
//! (`GENUI_NATIVE_FIXTURE_PYTHON`, DuckDB 2.0.0) and the real confined engine
//! over the library's normalized revisions.
//!
//! The binary's route modules (auth, query, canvas, commands, sharing,
//! events and the composer under its own deadline, composed as `main.rs`
//! composes them) are served on a real loopback TCP socket; every request
//! below travels over HTTP (reqwest), the event streams are read as bodies.
//! The model is a loopback Messages stub reached exactly as the deployment
//! reaches the real endpoint: `Composer::from_env()` with `ANTHROPIC_API_KEY`
//! and `GENUI_COMPOSER_ENDPOINT` set in this process for the campaign only.
//!
//! The story, in order: three real members sign in through the APP-009
//! routes (Maya and a teammate in Northstar, an outsider in the canary
//! organization); the operator grants Maya the library and an owned authoring
//! app (`grant-library-access`); Maya reads `GET /v1/library`; her
//! `POST /v1/compose "review expenses over $200"` is routed by the stub to
//! the expense-review library app (`retrieved`); her canvas lists the new
//! work and `GET /v1/canvas/works/{app}/{work}` places it; its views run
//! through the governed query route with contract-valid receipts; her own
//! stream saw `app.placed`; she shares it with the teammate, whose stream
//! receives `app.shared` and whose group route then answers; she commits a
//! decision through the audited command route and the teammate's stream
//! receives `record.committed`; the stub then returns a valid typed
//! definition and a `generated` app is drafted on her dedicated authoring
//! app (never a library app), published, saved and runs; a hostile
//! generation is refused by admission on every attempt and places nothing;
//! the outsider, an anonymous caller and a forged session get the uniform
//! 404 everywhere and the outsider's stream stays silent. Timings
//! (compose→200, compose→placed, query p50) are recorded in the report.
use super::*;
use crate::{
    auth::{
        cookies,
        invite::Invitations,
        synthetic_issuer::{Issuer, ISSUER as GATEWASM},
        tests::{self as harness, Campaign, CANARY_HANDLE, NORTHSTAR_HANDLE},
        Auth,
    },
    commands::{self as record_commands, RecordPage, RecordView},
    composer::{
        self,
        model::{ENDPOINT_ENV, KEY_ENV},
        routes::Compose,
    },
    events,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        routes::tests::{Counting, RecordingPages},
        Executor, Work,
    },
    sharing,
    storage::admission_qualification::FakeProvider,
};
use axum::http::StatusCode;
use std::{collections::VecDeque, sync::Mutex};

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

/// The expense-review library app (contracts/fixtures/library/catalog.json).
const EXPENSE_APP: &str = "00000000-0000-4000-8000-000000001001";
const EXPENSE_WORK: &str = "00000000-0000-4000-8000-000000001004";
const EXPENSE_FIXTURE: &str = "contracts/fixtures/library/expense-review.json";
/// Maya's dedicated authoring app (created by `grant-library-access`).
const AUTHORING_APP: &str = "00000000-0000-4000-8000-00000000e0a1";
const INTENT: &str = "review expenses over $200";
const WAIT: Duration = Duration::from_secs(10);

#[test]
#[ignore = "requires Docker and GENUI_NATIVE_FIXTURE_PYTHON (DuckDB 2.0.0)"]
fn native_pitch_campaign() {
    std::thread::Builder::new()
        .name("pitch-integration-campaign".into())
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

/// The loopback Messages stub: scripted reply texts in order, every request
/// body and api key header kept for the checks.
struct Stub {
    replies: Mutex<VecDeque<String>>,
    bodies: Mutex<Vec<Value>>,
    keys: Mutex<Vec<String>>,
}
impl Stub {
    fn script(&self, replies: Vec<String>) {
        *self.replies.lock().unwrap() = replies.into();
    }
    fn calls(&self) -> usize {
        self.bodies.lock().unwrap().len()
    }
}
async fn stub_server() -> (String, Arc<Stub>) {
    let stub = Arc::new(Stub {
        replies: Mutex::new(VecDeque::new()),
        bodies: Mutex::new(Vec::new()),
        keys: Mutex::new(Vec::new()),
    });
    let state = stub.clone();
    let app = axum::Router::new().route(
        "/v1/messages",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                let state = state.clone();
                async move {
                    if let Some(key) = headers.get("x-api-key") {
                        state
                            .keys
                            .lock()
                            .unwrap()
                            .push(key.to_str().unwrap_or_default().to_owned());
                    }
                    state
                        .bodies
                        .lock()
                        .unwrap()
                        .push(serde_json::from_slice(&body).unwrap_or(Value::Null));
                    let text = state
                        .replies
                        .lock()
                        .unwrap()
                        .pop_front()
                        .unwrap_or_else(|| r#"{"choice":"none","confidence":"low"}"#.into());
                    let reply = json!({"id": "msg", "model": "stub-model",
                        "content": [{"type": "text", "text": text}],
                        "usage": {"input_tokens": 5, "output_tokens": 7}});
                    (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        reply.to_string(),
                    )
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (endpoint, stub)
}

fn router_reply(choice: &str, confidence: &str) -> String {
    json!({"choice": choice, "confidence": confidence, "reason": "campaign", "title": "ignored"})
        .to_string()
}
fn expense_content() -> Value {
    let bundle: Value =
        serde_json::from_slice(&std::fs::read(repository().join(EXPENSE_FIXTURE)).unwrap())
            .unwrap();
    bundle["definition"]["content"].clone()
}
/// What a good model returns for the generated path: the expense-review
/// definition's runnable views under a new title, with `amount > $200`
/// written directly into every data view (APP-101: the "direct run with that
/// filter" the live transform's adapted views must match).
fn generated_reply() -> String {
    let mut content = composer::prompt::runnable_example(expense_content());
    content["title"] = json!("Expenses over $200 (generated)");
    content["description"] = json!("Synthetic generated example for the pitch campaign.");
    let filter = json!({"type": "filter", "field": {"input_id": "expense_claims", "field_id": "amount"},
        "comparison": "gt", "value": {"type": "money", "minor_units": "20000", "currency": "USD", "minor_unit_exponent": 2}});
    for view in content["views"].as_array_mut().unwrap() {
        if let Some(operations) = view
            .get_mut("query")
            .and_then(|query| query.get_mut("operations"))
            .and_then(Value::as_array_mut)
        {
            operations.insert(0, filter.clone());
        }
    }
    serde_json::to_string(&content).unwrap()
}
/// APP-101: what the fast model returns for "review expenses over $200".
const TRANSFORM_REPLY: &str = r#"{"filters":[{"field":"amount","op":"gt","value":"200"}]}"#;

/// The next `app.adapted` on a stream (others skipped): the adapted ids and
/// the standard work it came from, ids and time only.
async fn expect_adapted(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Value>,
    from_work: &str,
) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("no app.adapted within {WAIT:?}"))
            .expect("stream closed");
        if event["kind"] == "app.adapted" && event["from_work_id"] == from_work {
            assert_eq!(event.as_object().unwrap().len(), 6, "ids and time only");
            return event;
        }
    }
}

/// One HTTP answer over the socket.
pub(crate) struct Answer {
    pub(crate) status: StatusCode,
    pub(crate) headers: reqwest::header::HeaderMap,
    pub(crate) body: Vec<u8>,
    pub(crate) elapsed: Duration,
}
impl Answer {
    pub(crate) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    /// The uniform 404 (`http::not_found`), byte for byte.
    pub(crate) fn uniform_404(&self) -> bool {
        self.status == StatusCode::NOT_FOUND
            && self.body == br#"{"code":"not_found","status":404}"#
            && self.headers[reqwest::header::CACHE_CONTROL] == "no-store"
    }
}
pub(crate) struct Stand {
    pub(crate) origin: String,
    pub(crate) client: reqwest::Client,
}
impl Stand {
    pub(crate) async fn call(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Answer {
        let mut request = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
            format!("{}{path}", self.origin),
        );
        if let Some(cookie) = cookie {
            request = request.header(
                reqwest::header::COOKIE,
                format!("{}={cookie}", cookies::SESSION),
            );
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if let Some(body) = body {
            request = request
                .header("sec-fetch-site", "same-origin")
                .header("content-type", "application/json")
                .body(body.to_owned());
        }
        let started = Instant::now();
        let response = request.send().await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await.unwrap().to_vec();
        Answer {
            status,
            headers,
            body,
            elapsed: started.elapsed(),
        }
    }
    pub(crate) async fn get(&self, path: &str, cookie: Option<&str>) -> Answer {
        self.call("GET", path, cookie, None, &[]).await
    }
    pub(crate) async fn post(&self, path: &str, cookie: Option<&str>, body: &str) -> Answer {
        self.call("POST", path, cookie, Some(body), &[]).await
    }
    /// Opens `GET /v1/events` for a session and parses its frames into
    /// documents; the task ends with the stream.
    pub(crate) async fn stream(&self, cookie: &str) -> tokio::sync::mpsc::UnboundedReceiver<Value> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let response = self
            .client
            .get(format!("{}/v1/events", self.origin))
            .header(
                reqwest::header::COOKIE,
                format!("{}={cookie}", cookies::SESSION),
            )
            .header("accept", "text/event-stream")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "event stream open");
        assert_eq!(
            response.headers()[reqwest::header::CONTENT_TYPE],
            "text/event-stream"
        );
        tokio::spawn(async move {
            let mut response = response;
            let mut buffer = String::new();
            while let Ok(Some(chunk)) = response.chunk().await {
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(end) = buffer.find('\n') {
                    let line = buffer[..end].trim_end_matches('\r').to_owned();
                    buffer.drain(..=end);
                    if let Some(data) = line.strip_prefix("data:") {
                        if let Ok(document) = serde_json::from_str::<Value>(data.trim()) {
                            if tx.send(document).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        });
        rx
    }
}
/// The next event of a kind (others of the same stream are skipped), within
/// `WAIT`.
pub(crate) async fn expect_event(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Value>,
    kind: &str,
    app: &str,
    work: &str,
) -> Value {
    let deadline = Instant::now() + WAIT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("no {kind} event within {WAIT:?}"))
            .expect("stream closed");
        if event["kind"] == kind {
            assert_eq!(event["app_id"], app);
            assert_eq!(event["work_id"], work);
            assert!(event["at"].as_str().is_some_and(|at| at.ends_with('Z')));
            assert_eq!(event.as_object().unwrap().len(), 4, "ids and time only");
            return event;
        }
    }
}
/// Nothing arrives on a stream for `quiet`.
async fn expect_silence(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Value>, quiet: Duration) {
    if let Ok(Some(event)) = tokio::time::timeout(quiet, rx.recv()).await {
        panic!("unexpected event {event}");
    }
}
fn ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn p50(samples: &mut [u64]) -> u64 {
    samples.sort_unstable();
    samples[samples.len() / 2]
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
    // The library: seeded into Northstar, its revisions normalized into the
    // engine cache under the pinned digests (as the query campaign).
    let seeded = operator(&fixture, &["seed-library", "--organization", ORG]);
    assert_eq!(seeded["apps"].as_array().unwrap().len(), 20);
    let python = fixture_python();
    let root = Directory::new("pitch");
    let built = Command::new(&python)
        .current_dir(repository().join("app/runtime"))
        .args([
            "-c",
            "import json,sys,library_sources as l; print(json.dumps(l.build(sys.argv[1], sys.argv[2])))",
            ORG,
            root.0.join("cache").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let engine = Arc::new(real_engine(&python, &root.0));

    // Sign-in: handles, bindings, the loopback issuer and the fake providers.
    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    fixture.sql(&format!("INSERT INTO genui.provider_tenant_bindings VALUES ('{ORG}','{GATEWASM}','{}',true),('{OTHER}','{GATEWASM}','{}',true)", harness::NORTHSTAR_TENANT, harness::CANARY_TENANT), "genui");
    fixture.write_registry(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000a13","bindings":[
        {"issuer":GATEWASM,"provider_tenant_id":harness::NORTHSTAR_TENANT,"organization_id":ORG},
        {"issuer":GATEWASM,"provider_tenant_id":harness::CANARY_TENANT,"organization_id":OTHER}]}));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let issuer = Issuer::start(harness::tenants()).await;
    let provider = FakeProvider::start_at(
        &fixture,
        harness::NORTHSTAR_TENANT,
        "pitch-northstar",
        None,
        None,
    );
    let canary_provider = FakeProvider::start_at(
        &fixture,
        harness::CANARY_TENANT,
        "pitch-canary",
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

    // The model: the real client against the loopback stub, configured as
    // the deployment configures it (environment only; this process, this
    // campaign). Northstar is not a demo organization: every intent reaches
    // the (stub) model.
    let (endpoint, stub) = stub_server().await;
    std::env::set_var(KEY_ENV, "sk-ant-synthetic-pitch-campaign-key");
    std::env::set_var(ENDPOINT_ENV, &endpoint);
    std::env::remove_var(composer::DEMO_ORGANIZATIONS_ENV);
    let composer = composer::Composer::from_env();
    std::env::remove_var(KEY_ENV);
    std::env::remove_var(ENDPOINT_ENV);
    assert!(composer.enabled());
    assert!(!composer.demo_for(ORG));

    // The binary's routes, composed as main.rs composes them.
    let counting = Arc::new(Counting {
        engine: Some(engine.clone() as Arc<dyn Executor>),
        ..Counting::default()
    });
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(counting.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    let mut manifest =
        forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
    manifest.routes.retain(|route, _| route == "/canvas");
    let pages = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
    let canvas_pages = Arc::new(SsrCanvasPages {
        page: pages.bind::<CanvasProps>("canvas").unwrap(),
    });
    pages.finalize().unwrap();
    // APP-103 widgets on the stand too (the weather fixture, no model), so
    // the canvas's `/` list and `/weather` can be walked here (APP-101).
    let widgets = crate::widgets::routes::router(Arc::new(crate::widgets::routes::Widgets::new(
        auth.clone(),
        crate::widgets::customize::Customizer::new(None, "none"),
        crate::widgets::weather::OpenMeteo::new(),
    )));
    let routes = crate::http::uniform_with_many(
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
            .merge(sharing::router(auth.clone()))
            .merge(composer::commands::router(auth.clone()))
            .merge(events::routes::router(auth.clone()))
            // APP-109: the records listing and the member's notes work.
            .merge(crate::actions::routes::records_router(auth.clone()))
            .merge(canvas::router(Arc::new(canvas::Canvas {
                work: work.clone(),
                pages: canvas_pages,
                live_events: "/v1/events".into(),
                demo_organizations: Default::default(),
            }))),
        vec![
            (
                composer::routes::PATH,
                composer::routes::router(Arc::new(Compose::new(auth.clone(), composer))),
            ),
            (crate::widgets::WEATHER_PATH, widgets.clone()),
            (crate::widgets::CUSTOMIZE_PATH, widgets),
        ],
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let stand = Stand {
        origin: format!("http://{}", listener.local_addr().unwrap()),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    };
    let served = routes.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            served.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });
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

    // --- Three real members sign in ---
    let (maya_subject, maya) = c.member("00000000e001").await;
    let (mate_subject, mate) = c.member("00000000e002").await;
    let (_, outsider) = c.member_in(OTHER, "00000000e003").await;
    let maya_id = c.principal(&maya_subject).await;
    let mate_id = c.principal(&mate_subject).await;
    let maya_cookie = maya.cookie(cookies::SESSION).unwrap();
    let mate_cookie = mate.cookie(cookies::SESSION).unwrap();
    let outsider_cookie = outsider.cookie(cookies::SESSION).unwrap();

    // Before any grant: Maya may compose nothing (the uniform 404, no model
    // call) and her library is empty.
    let nothing = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": INTENT}).to_string(),
        )
        .await;
    assert!(nothing.uniform_404(), "{}", nothing.text());
    assert_eq!(stub.calls(), 0);
    let empty = stand.get("/v1/library", Some(&maya_cookie)).await;
    assert_eq!(empty.status, StatusCode::OK);
    assert_eq!(empty.json()["apps"], json!([]));

    // --- The operator: Maya gets the library and her own authoring app ---
    let granted = operator(
        &c.fixture,
        &[
            "grant-library-access",
            "--organization",
            ORG,
            "--principal",
            &maya_id,
            "--record-write",
            "--authoring-app",
            AUTHORING_APP,
        ],
    );
    assert_eq!(granted["changed"], true);
    assert_eq!(granted["library_apps"].as_array().unwrap().len(), 20);
    assert_eq!(granted["authoring_app"]["created"], true);
    let library = stand.get("/v1/library", Some(&maya_cookie)).await;
    assert_eq!(library.status, StatusCode::OK, "{}", library.text());
    let apps = library.json()["apps"].as_array().unwrap().clone();
    assert_eq!(apps.len(), 20);
    let expense = apps
        .iter()
        .find(|app| app["library_id"] == "expense-review")
        .expect("expense-review listed");
    assert_eq!(expense["app_id"], EXPENSE_APP);
    assert_eq!(expense["work_id"], EXPENSE_WORK);
    assert_eq!(expense["title"], "Expense review queue");
    assert_eq!(expense["can_write"], true);
    // The composer's context prefers her unpublished shell and never a
    // library app (review M2), with or without library ownership.
    // Maya as storage sees her: the identity the loopback issuer signed in.
    let maya_request = TrustedRequest {
        organization: ORG.into(),
        principal: maya_id.clone(),
        epoch: 1,
        issuer: GATEWASM.into(),
        provider_tenant: harness::NORTHSTAR_TENANT.into(),
        provider_subject: maya_subject.clone(),
        registry_sha256: stores.registry_identity().sha256.clone(),
    };
    let context = stores
        .workspace
        .composer_context(&maya_request)
        .await
        .unwrap();
    assert_eq!(
        context.authoring.as_ref().map(|a| a.app.as_str()),
        Some(AUTHORING_APP)
    );
    assert_eq!(context.library.len(), 20);
    c.fixture.sql(&format!("UPDATE genui.apps SET owner_principal_id='{maya_id}' WHERE organization_id='{ORG}' AND app_id='{EXPENSE_APP}'"), "genui");
    crate::storage::definition_qualification::grant(
        &c.fixture,
        &maya_id,
        "edit_definition",
        &["--app", EXPENSE_APP],
    );
    let context = stores
        .workspace
        .composer_context(&maya_request)
        .await
        .unwrap();
    assert_eq!(
        context.authoring.as_ref().map(|a| a.app.as_str()),
        Some(AUTHORING_APP),
        "the dedicated shell, not the library app she now owns"
    );
    c.fixture.sql(&format!("DELETE FROM genui.app_permissions WHERE organization_id='{ORG}' AND app_id='{AUTHORING_APP}' AND principal_id='{maya_id}'"), "genui");
    let context = stores
        .workspace
        .composer_context(&maya_request)
        .await
        .unwrap();
    assert_eq!(
        context.authoring, None,
        "owning only a library app: nothing to draft on"
    );
    c.fixture.sql(&format!("INSERT INTO genui.app_permissions VALUES ('{ORG}','{maya_id}','{AUTHORING_APP}','discover'),('{ORG}','{maya_id}','{AUTHORING_APP}','edit_definition'),('{ORG}','{maya_id}','{AUTHORING_APP}','run')"), "genui");
    c.fixture.sql(&format!("UPDATE genui.apps SET owner_principal_id='00000000-0000-4000-8000-000000000066' WHERE organization_id='{ORG}' AND app_id='{EXPENSE_APP}'"), "genui");
    c.fixture.sql(&format!("DELETE FROM genui.app_permissions WHERE organization_id='{ORG}' AND app_id='{EXPENSE_APP}' AND principal_id='{maya_id}' AND permission='edit_definition'"), "genui");
    check(
        &mut checks,
        "membership: before the grant Maya composes nothing (uniform 404, no model call) and her library is empty; grant-library-access gives her all 20 library apps (GET /v1/library lists them with can_write) and a dedicated authoring app, which the composer context chooses over a library app she owns and never replaces with one (review M2)",
    );

    // --- Streams open: Maya's, the teammate's, the outsider's ---
    let mut maya_stream = stand.stream(&maya_cookie).await;
    let mut mate_stream = stand.stream(&mate_cookie).await;
    let mut outsider_stream = stand.stream(&outsider_cookie).await;

    // --- Compose: retrieved at once by the instant matcher, then adapted ---
    stub.script(vec![TRANSFORM_REPLY.into()]);
    let compose_sent = Instant::now();
    let composed = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": INTENT}).to_string(),
        )
        .await;
    assert_eq!(composed.status, StatusCode::OK, "{}", composed.text());
    let composed_at = Instant::now();
    let answer = composed.json();
    assert_eq!(answer["origin"], "retrieved");
    assert_eq!(answer["app_id"], EXPENSE_APP);
    assert_eq!(answer["title"], "Expense review queue");
    assert_eq!(answer["tailoring"], "pending");
    assert_eq!(
        answer.as_object().unwrap().len(),
        6,
        "app_id, work_id, origin, title, tailoring, duration_ms"
    );
    assert!(
        answer["duration_ms"].is_u64(),
        "APP-116: the route's duration"
    );
    let work_id = answer["work_id"].as_str().unwrap().to_owned();
    assert_ne!(
        work_id, EXPENSE_WORK,
        "a new saved work, not the seeded one"
    );
    // The live transform follows on Maya's stream: placed, then adapted.
    expect_event(&mut maya_stream, "app.placed", EXPENSE_APP, &work_id).await;
    let adapted_event = expect_adapted(&mut maya_stream, &work_id).await;
    let reshaped = compose_sent.elapsed();
    assert_eq!(adapted_event["from_app_id"], EXPENSE_APP);
    assert_eq!(
        adapted_event["app_id"], AUTHORING_APP,
        "drafted on her own app"
    );
    let adapted_work = adapted_event["work_id"].as_str().unwrap().to_owned();
    assert_eq!(stub.calls(), 1, "no router call; the transform alone");
    assert_eq!(
        stub.keys.lock().unwrap().as_slice(),
        ["sk-ant-synthetic-pitch-campaign-key"]
    );
    let transform_body = stub.bodies.lock().unwrap()[0].clone();
    assert_eq!(
        transform_body["system"].as_str().unwrap_or_default(),
        composer::transform::TRANSFORM_SYSTEM
    );
    assert!(transform_body["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains(INTENT));
    assert!(
        !transform_body.to_string().contains("sk-ant"),
        "the key never travels in a body"
    );
    timings.insert("compose_retrieved_ms".into(), json!(ms(composed.elapsed)));
    timings.insert("compose_to_reshape_ms".into(), json!(ms(reshaped)));
    let group_path = format!("/v1/canvas/works/{EXPENSE_APP}/{work_id}");
    let group = stand.get(&group_path, Some(&maya_cookie)).await;
    assert_eq!(group.status, StatusCode::OK, "{}", group.text());
    timings.insert(
        "compose_to_placed_ms".into(),
        json!(ms(composed.elapsed + composed_at.elapsed())),
    );
    let group = group.json();
    assert_eq!(group["work"], work_id);
    assert_eq!(group["app"], EXPENSE_APP);
    let views: Vec<String> = group["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["view"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(views, ["totals", "queue", "by_category", "by_status"]);
    assert_eq!(group["forms"][0]["command"], "record_decision");
    let page = stand.get("/canvas", Some(&maya_cookie)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.text().contains(&format!("id=\"lib-{work_id}\"")),
        "openable_works lists the composed work"
    );
    assert!(
        page.text().contains("data-live-events=\"/v1/events\""),
        "live updates on"
    );
    check(
        &mut checks,
        &format!("compose (APP-101 instant pick, retrieved): POST /v1/compose answered {{app_id, work_id, origin: retrieved, title, tailoring: pending}} in {} ms with no router call (the local matcher picked the expense review app); the group route placed it {} ms after the intent was sent; GET /canvas lists the new work; Maya's stream received app.placed, then app.adapted {} ms after the intent was sent, after exactly one transform call through the real client (key as the header only, the intent in the user message)",
            ms(composed.elapsed), timings["compose_to_placed_ms"], ms(reshaped)),
    );

    // --- The views run with receipts ---
    let mut samples = Vec::new();
    let mut receipts = serde_json::Map::new();
    for (index, view) in views.iter().enumerate() {
        let path = format!("/v1/work/{EXPENSE_APP}/{work_id}/queries/{view}");
        let runs = if index == 0 { 9 } else { 1 };
        for _ in 0..runs {
            let run = stand
                .post(
                    &path,
                    Some(&maya_cookie),
                    &json!({"parameters": {}}).to_string(),
                )
                .await;
            assert_eq!(run.status, StatusCode::OK, "{view}: {}", run.text());
            let (rows, receipt) = receipted(&run.text());
            assert!(receipt["digest"]
                .as_str()
                .is_some_and(|d| d.starts_with("sha256:")));
            assert!(receipt["result_digest"].as_str().is_some());
            assert_eq!(receipt["work_id"], work_id);
            if index == 0 {
                samples.push(ms(run.elapsed));
                let cells = rows[0].as_array().unwrap();
                assert!(
                    cells.contains(&json!("240")) && cells.contains(&json!("26262004")),
                    "count 240 and sum 26,262,004 minor units of the synthetic claims: {rows}"
                );
            }
            receipts.insert(view.clone(), receipt);
        }
    }
    timings.insert("query_totals_p50_ms".into(), json!(p50(&mut samples)));
    timings.insert("query_totals_samples".into(), json!(samples.len()));
    check(
        &mut checks,
        &format!("governed data: the four views of the placed app ran through POST /v1/work/{{app}}/{{work}}/queries/{{view}} on the confined engine over the normalized library revision, each 200 carrying a contract-valid receipt with digest and result_digest; totals p50 {} ms over {} runs",
            timings["query_totals_p50_ms"], samples.len()),
    );

    // --- APP-101: the adapted work: chips, filtered views, receipts ---
    let adapted_path = format!("/v1/canvas/works/{AUTHORING_APP}/{adapted_work}");
    let adapted_group = stand.get(&adapted_path, Some(&maya_cookie)).await;
    assert_eq!(
        adapted_group.status,
        StatusCode::OK,
        "{}",
        adapted_group.text()
    );
    let adapted_group = adapted_group.json();
    let chips: Vec<&str> = adapted_group["origins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|origin| origin["text"].as_str().unwrap())
        .collect();
    assert_eq!(chips, ["Adapted to your request", "Amount > $200"]);
    let adapted_views: Vec<String> = adapted_group["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["view"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(adapted_views, views, "the same views, reshaped");
    let adapted_run = stand
        .post(
            &format!("/v1/work/{AUTHORING_APP}/{adapted_work}/queries/totals"),
            Some(&maya_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(adapted_run.status, StatusCode::OK, "{}", adapted_run.text());
    let (adapted_rows, adapted_receipt) = receipted(&adapted_run.text());
    assert!(
        !adapted_rows[0].as_array().unwrap().contains(&json!("240")),
        "fewer claims than the unfiltered 240: {adapted_rows}"
    );
    // The standard work stays openable ("back to standard"), and the
    // adapted revision is a new publication of her own app.
    let standard_again = stand.get(&group_path, Some(&maya_cookie)).await;
    assert_eq!(standard_again.status, StatusCode::OK);
    let own_publications = scalar(
        &c.fixture,
        &format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{AUTHORING_APP}'"),
    )
    .await;
    assert_eq!(own_publications, 1);
    check(
        &mut checks,
        "live transform: the adapted work (drafted on Maya's own app, owner-published, admitted whole) carries the chips Adapted to your request · Amount > $200 on its group route, runs the same four views with the filter and a contract-valid receipt (fewer than the 240 claims), and the standard work stays openable for back to standard",
    );

    // --- APP-101: the near-tie chooser and the pinned compose ---
    let calls_before = stub.calls();
    let works_before_choice = scalar(
        &c.fixture,
        &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{ORG}'"),
    )
    .await;
    let chooser = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": "expenses"}).to_string(),
        )
        .await;
    assert_eq!(chooser.status, StatusCode::OK, "{}", chooser.text());
    let choices = chooser.json()["choices"].as_array().unwrap().clone();
    assert!((2..=3).contains(&choices.len()), "{choices:?}");
    let offered: Vec<&str> = choices
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert!(
        offered.contains(&"Expense review queue") && offered.contains(&"Expense claims"),
        "{offered:?}"
    );
    assert_eq!(stub.calls(), calls_before, "no model call for a chooser");
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{ORG}'")
        )
        .await,
        works_before_choice,
        "nothing written until the member picks"
    );
    let picked = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": "expenses", "app_id": EXPENSE_APP}).to_string(),
        )
        .await;
    assert_eq!(picked.status, StatusCode::OK, "{}", picked.text());
    assert_eq!(picked.json()["app_id"], EXPENSE_APP);
    assert_eq!(picked.json()["origin"], "retrieved");
    assert_eq!(picked.json()["tailoring"], "none");
    assert_eq!(stub.calls(), calls_before, "a pinned app needs no model");
    let foreign_pin = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": "expenses", "app_id": "00000000-0000-4000-8000-0000000fffff"})
                .to_string(),
        )
        .await;
    assert!(
        foreign_pin.uniform_404(),
        "an app outside her library is the uniform 404"
    );
    timings.insert("chooser_ms".into(), json!(ms(chooser.elapsed)));
    timings.insert("pinned_compose_ms".into(), json!(ms(picked.elapsed)));
    check(
        &mut checks,
        &format!("chooser: an ambiguous sentence (\"expenses\") answered {} choices ({}) in {} ms with no model call and no row written; the click (a pinned app_id) placed the chosen app in {} ms with no model call; a pin outside her library is the uniform 404",
            choices.len(), offered.join(", "), ms(chooser.elapsed), ms(picked.elapsed)),
    );

    // --- APP-101: slash commands and "save as /name" ---
    let listed = stand
        .get(composer::commands::COMMANDS_PATH, Some(&maya_cookie))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text());
    let commands = listed.json()["commands"].as_array().unwrap().clone();
    let slug_of = |app: &str| {
        commands
            .iter()
            .find(|c| c["app_id"] == app && c["kind"] == "library")
            .map(|c| c["slug"].as_str().unwrap().to_owned())
    };
    assert_eq!(slug_of(EXPENSE_APP).as_deref(), Some("expense-review"));
    assert_eq!(
        commands.iter().filter(|c| c["kind"] == "library").count(),
        20
    );
    let named = stand
        .post(
            &format!("/v1/work/{AUTHORING_APP}/{adapted_work}/name"),
            Some(&maya_cookie),
            &json!({"slug": "big-claims"}).to_string(),
        )
        .await;
    assert_eq!(named.status, StatusCode::OK, "{}", named.text());
    let named_work = named.json()["work_id"].as_str().unwrap().to_owned();
    assert_ne!(named_work, adapted_work);
    let listed = stand
        .get(composer::commands::COMMANDS_PATH, Some(&maya_cookie))
        .await;
    let saved: Vec<Value> = listed.json()["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "saved")
        .cloned()
        .collect();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0]["slug"], "big-claims");
    assert_eq!(saved[0]["work_id"], named_work);
    for (slug, code) in [
        ("big-claims", "name_taken"),
        ("expense", "name_taken"),
        ("apps", "name_taken"),
        ("Big Claims", "invalid_name"),
    ] {
        let again = stand
            .post(
                &format!("/v1/work/{AUTHORING_APP}/{adapted_work}/name"),
                Some(&maya_cookie),
                &json!({"slug": slug}).to_string(),
            )
            .await;
        assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY, "{slug}");
        assert_eq!(again.json()["code"], code, "{slug}");
    }
    let mate_commands = stand
        .get(composer::commands::COMMANDS_PATH, Some(&mate_cookie))
        .await;
    assert!(
        !mate_commands.text().contains("big-claims"),
        "a named work is listed only to members who can open it"
    );
    let named_run = stand
        .post(
            &format!("/v1/work/{AUTHORING_APP}/{named_work}/queries/totals"),
            Some(&maya_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(named_run.status, StatusCode::OK, "{}", named_run.text());
    assert_eq!(
        receipted(&named_run.text()).0,
        adapted_rows,
        "the named copy runs the same views"
    );
    check(
        &mut checks,
        "slash commands: GET /v1/commands lists the 20 library apps she may compose with their slugs (/expense-review for the expense review queue); POST /v1/work/{app}/{work}/name saved the adapted app as /big-claims (a new saved work of the same publication, slug on its context, no migration), listed as her saved command and running the same views; a taken name, a library slug, the reserved /apps and a malformed name are refused (422); the teammate, who cannot open it, does not see it",
    );

    // --- Share with the teammate ---
    let unshared = stand.get(&group_path, Some(&mate_cookie)).await;
    assert!(unshared.uniform_404());
    let share = stand
        .post(
            &format!("/v1/work/{EXPENSE_APP}/{work_id}/share"),
            Some(&maya_cookie),
            &json!({"grantee_principal_id": mate_id, "record_write": true}).to_string(),
        )
        .await;
    assert_eq!(share.status, StatusCode::OK, "{}", share.text());
    let shared = share.json();
    assert_eq!(shared["grantee_principal_id"], mate_id);
    assert!(shared["granted"].as_array().unwrap().len() >= 4);
    expect_event(&mut mate_stream, "app.shared", EXPENSE_APP, &work_id).await;
    // APP-101: the composing member's share of the standard work carries its
    // adaptation, so the teammate's canvas reshapes live.
    let mate_adapted = expect_adapted(&mut mate_stream, &work_id).await;
    assert_eq!(mate_adapted["work_id"], adapted_work);
    let mate_view = stand.get(&adapted_path, Some(&mate_cookie)).await;
    assert_eq!(mate_view.status, StatusCode::OK, "{}", mate_view.text());
    let placed = stand.get(&group_path, Some(&mate_cookie)).await;
    assert_eq!(placed.status, StatusCode::OK, "{}", placed.text());
    assert_eq!(placed.json()["work"], work_id);
    let mate_page = stand.get("/canvas", Some(&mate_cookie)).await;
    assert!(mate_page.text().contains(&format!("id=\"lib-{work_id}\"")));
    let mate_run = stand
        .post(
            &format!("/v1/work/{EXPENSE_APP}/{work_id}/queries/totals"),
            Some(&mate_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(mate_run.status, StatusCode::OK, "{}", mate_run.text());
    receipted(&mate_run.text());
    check(
        &mut checks,
        "sharing: the teammate's group route was the uniform 404 before the share; POST /v1/work/{app}/{work}/share wrote the sharer's subset, the teammate's stream received app.shared with the ids and then app.adapted naming the adapted work (shared too, through the same share path under Maya's authority), and their group routes (standard and adapted), canvas and totals view (with a receipt) answer under their own grants",
    );

    // --- Commit a decision; the teammate sees record.committed ---
    let form = group["forms"][0].clone();
    let document = json!({
        "kind": "command_request",
        "schema_version": 1,
        "app_id": EXPENSE_APP,
        "work_id": work_id,
        "publication_id": form["publication"],
        "command_id": "record_decision",
        "schema_id": form["schema"],
        "schema_version_expected": form["schema_version"].as_str().unwrap().parse::<i64>().unwrap(),
        "record_id": form["record"],
        "expected_record_version": form["record_version"].as_str().unwrap().parse::<i64>().unwrap(),
        "expected_work_version": form["work_version"].as_str().unwrap().parse::<i64>().unwrap(),
        "values": [
            {"field_id": "status", "value": {"type": "enum", "value": "needs_context"}},
            {"field_id": "note", "value": {"type": "text", "value": "Pitch campaign: synthetic decision"}},
        ],
    });
    let key = form["idempotency_key"].as_str().unwrap().to_owned();
    let committed = stand
        .call(
            "POST",
            form["action"].as_str().unwrap(),
            Some(&maya_cookie),
            Some(&document.to_string()),
            &[("idempotency-key", key.as_str())],
        )
        .await;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.text());
    let outcome = committed.json();
    assert_eq!(outcome["state"], "committed", "{outcome}");
    assert_eq!(outcome["record_version"], 1);
    assert!(outcome["audit_event_id"].as_str().is_some());
    expect_event(&mut mate_stream, "record.committed", EXPENSE_APP, &work_id).await;
    expect_event(&mut maya_stream, "record.committed", EXPENSE_APP, &work_id).await;
    let replayed = stand
        .call(
            "POST",
            form["action"].as_str().unwrap(),
            Some(&maya_cookie),
            Some(&document.to_string()),
            &[("idempotency-key", key.as_str())],
        )
        .await;
    assert_eq!(replayed.status, StatusCode::OK);
    assert_eq!(
        replayed.json(),
        outcome,
        "the same outcome (audit event, record version): replayed, not duplicated"
    );
    timings.insert("command_commit_ms".into(), json!(ms(committed.elapsed)));
    check(
        &mut checks,
        &format!("commands: Maya's decision committed once through the audited command route ({} ms) with the group's server-minted key; the identical retry is replayed, not duplicated; the teammate's and Maya's streams received record.committed",
            ms(committed.elapsed)),
    );

    // --- Generated: a valid typed definition on her dedicated app ---
    let works_before = scalar(
        &c.fixture,
        &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{ORG}'"),
    )
    .await;
    stub.script(vec![generated_reply()]);
    let calls_before = stub.calls();
    let generated = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": "a queue of expense claims over $200 with totals and a decision form", "generate": true})
                .to_string(),
        )
        .await;
    assert_eq!(generated.status, StatusCode::OK, "{}", generated.text());
    let generated_answer = generated.json();
    assert_eq!(generated_answer["origin"], "generated");
    assert_eq!(
        generated_answer["app_id"], AUTHORING_APP,
        "drafted on the dedicated app"
    );
    assert_eq!(generated_answer["title"], "Expenses over $200 (generated)");
    assert_eq!(
        stub.calls(),
        calls_before + 1,
        "\"build something new\": one generation, no router"
    );
    let generated_work = generated_answer["work_id"].as_str().unwrap().to_owned();
    let generated_group = stand
        .get(
            &format!("/v1/canvas/works/{AUTHORING_APP}/{generated_work}"),
            Some(&maya_cookie),
        )
        .await;
    assert_eq!(
        generated_group.status,
        StatusCode::OK,
        "{}",
        generated_group.text()
    );
    expect_event(
        &mut maya_stream,
        "app.placed",
        AUTHORING_APP,
        &generated_work,
    )
    .await;
    let generated_run = stand
        .post(
            &format!("/v1/work/{AUTHORING_APP}/{generated_work}/queries/totals"),
            Some(&maya_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(
        generated_run.status,
        StatusCode::OK,
        "{}",
        generated_run.text()
    );
    let (rows, direct_receipt) = receipted(&generated_run.text());
    // APP-101: the live transform's filtered view equals a direct run with
    // that filter: the same rows, result digest, query and source revisions.
    assert_eq!(rows, adapted_rows);
    assert_eq!(
        direct_receipt["result_digest"],
        adapted_receipt["result_digest"]
    );
    assert_eq!(
        direct_receipt["provenance"]["query_digest"],
        adapted_receipt["provenance"]["query_digest"]
    );
    assert_eq!(
        direct_receipt["source_revisions"],
        adapted_receipt["source_revisions"]
    );
    // The library app's newest publication is untouched (review M2).
    let newest = text(&c.fixture, &format!("SELECT definition_digest::text FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{EXPENSE_APP}' ORDER BY publication_sequence DESC LIMIT 1")).await;
    assert_eq!(newest, expense["definition_digest"]);
    timings.insert("compose_generated_ms".into(), json!(ms(generated.elapsed)));
    check(
        &mut checks,
        &format!("compose (generated, \"build something new\"): with the stub returning a valid typed definition with amount > $200 written directly, the composition was admitted, drafted on Maya's dedicated authoring app, owner-published and saved as origin generated in {} ms (one generation call, no router); its group route places it, app.placed reached her stream, and its totals view's rows, result digest, query digest and source revisions equal the live transform's adapted totals (the transform's filter is the direct filter); the library app's newest publication is unchanged",
            ms(generated.elapsed)),
    );

    // --- Hostile generation: refused on every attempt, nothing placed ---
    let hostile = r#"{"title":"<script>alert(1)</script>","description":"x","inputs":[{"id":"expense_claims","kind":"source"}],"views":[{"id":"v","kind":"html","title":"<img onerror=x>","markup":"<b>"}],"grants":["admin"]}"#;
    stub.script(vec![
        router_reply("none", "low"),
        hostile.into(),
        hostile.into(),
        hostile.into(),
    ]);
    let calls_before = stub.calls();
    let works_after_generated = scalar(
        &c.fixture,
        &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{ORG}'"),
    )
    .await;
    assert_eq!(works_after_generated, works_before + 1);
    let refused = stand
        .post(
            composer::routes::PATH,
            Some(&maya_cookie),
            &json!({"intent": "something the library does not have"}).to_string(),
        )
        .await;
    assert_eq!(
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.text()
    );
    assert_eq!(refused.json()["code"], "could_not_compose");
    assert_eq!(
        stub.calls(),
        calls_before + 1 + composer::MAX_GENERATION_ATTEMPTS
    );
    assert!(
        !refused.text().contains("script"),
        "nothing of the refused output is disclosed"
    );
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{ORG}'")
        )
        .await,
        works_after_generated
    );
    check(
        &mut checks,
        "compose (hostile): a model output with markup, an executable view and a grant member is refused by admission on each of the three attempts, answered 422 could_not_compose without disclosing it, and no work is saved",
    );

    // --- APP-101: with no model, the instant pick works and the transform is kept ---
    let keyless = composer::Composer::new(None, "r", "g");
    let kept = composer::routes::compose_for(&stores, &keyless, &maya_request, INTENT)
        .await
        .unwrap();
    assert_eq!(kept.origin, composer::Origin::Retrieved);
    assert_eq!(kept.app, EXPENSE_APP);
    assert_eq!(kept.tailoring, composer::routes::Tailoring::Kept);
    check(
        &mut checks,
        "model unavailable: with no key, the same sentence is still picked and retrieved at once (no model call) and the transform is skipped with the neutral kept state",
    );

    // --- Isolation: the outsider, anonymous and forged callers ---
    let before_dispatch = counting.calls();
    let before_model = stub.calls();
    let mut denials = 0;
    let outsider_requests: Vec<(&str, String, Option<String>)> = vec![
        ("GET", group_path.clone(), None),
        (
            "GET",
            format!("/v1/canvas/works/{AUTHORING_APP}/{generated_work}"),
            None,
        ),
        (
            "POST",
            composer::routes::PATH.to_owned(),
            Some(json!({"intent": INTENT}).to_string()),
        ),
        (
            "POST",
            format!("/v1/work/{EXPENSE_APP}/{work_id}/queries/totals"),
            Some(json!({"parameters": {}}).to_string()),
        ),
        (
            "POST",
            format!("/v1/work/{EXPENSE_APP}/{work_id}/share"),
            Some(json!({"grantee_principal_id": mate_id}).to_string()),
        ),
        (
            "POST",
            format!("/v1/work/{AUTHORING_APP}/{adapted_work}/name"),
            Some(json!({"slug": "outsider-copy"}).to_string()),
        ),
        (
            "POST",
            form["action"].as_str().unwrap().to_owned(),
            Some(document.to_string()),
        ),
    ];
    for (method, path, body) in &outsider_requests {
        let reply = stand
            .call(
                method,
                path,
                Some(&outsider_cookie),
                body.as_deref(),
                &[("idempotency-key", key.as_str())],
            )
            .await;
        assert!(
            reply.uniform_404(),
            "outsider {method} {path}: {} {}",
            reply.status,
            reply.text()
        );
        denials += 1;
    }
    let outsider_library = stand.get("/v1/library", Some(&outsider_cookie)).await;
    assert_eq!(outsider_library.status, StatusCode::OK);
    assert_eq!(
        outsider_library.json()["apps"],
        json!([]),
        "nothing of Northstar's library"
    );
    let outsider_page = stand.get("/canvas", Some(&outsider_cookie)).await;
    assert_eq!(outsider_page.status, StatusCode::OK);
    assert!(
        !outsider_page.text().contains(&work_id) && !outsider_page.text().contains(&generated_work)
    );
    for cookie in [
        None,
        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
    ] {
        for path in [
            "/canvas",
            group_path.as_str(),
            "/v1/library",
            "/v1/events",
            "/v1/members",
            composer::commands::COMMANDS_PATH,
        ] {
            let reply = stand.get(path, cookie).await;
            assert!(reply.uniform_404(), "{cookie:?} {path}: {}", reply.status);
            denials += 1;
        }
        let reply = stand
            .post(
                composer::routes::PATH,
                cookie,
                &json!({"intent": INTENT}).to_string(),
            )
            .await;
        assert!(reply.uniform_404());
        denials += 1;
    }
    assert_eq!(
        counting.calls(),
        before_dispatch,
        "no denial reached the engine"
    );
    assert_eq!(stub.calls(), before_model, "no denial reached the model");
    expect_silence(&mut outsider_stream, Duration::from_millis(1500)).await;
    check(
        &mut checks,
        &format!("isolation: {denials} requests by the canary member (both groups, compose, the query route, share and the command route), anonymous and forged sessions (canvas, group, library, events, members, compose) got the uniform 404; the engine and the model saw none; the outsider's library is empty, their canvas shows neither work and their event stream stayed silent through every Northstar event"),
    );

    // --- The local stand for the pitch screenshots (scratch only) ---
    // With `GENUI_PITCH_STAND_SECONDS`, the composed routes stay served on the
    // socket for that long (or until `<stand file>.stop` exists), and the
    // origin with Maya's synthetic session cookie is written to
    // `GENUI_PITCH_STAND_FILE` (never evidence), so a local browser can show
    // her canvas with the retrieved and generated apps and their receipts.
    if let Some(seconds) = std::env::var("GENUI_PITCH_STAND_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let file = std::env::var("GENUI_PITCH_STAND_FILE").unwrap_or_default();
        if !file.is_empty() {
            std::fs::write(
                &file,
                serde_json::to_vec(&json!({
                    "origin": stand.origin, "cookie": cookies::SESSION, "value": maya_cookie,
                    "app": EXPENSE_APP, "work": work_id, "generated_app": AUTHORING_APP, "generated_work": generated_work,
                }))
                .unwrap(),
            )
            .unwrap();
        }
        // APP-101: the stand's transform answers (scratch screenshots only).
        stub.script(vec![TRANSFORM_REPLY.into(); 16]);
        println!("PITCH_STAND {}", stand.origin);
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let stopped = || !file.is_empty() && std::path::Path::new(&format!("{file}.stop")).exists();
        while Instant::now() < deadline && !stopped() {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    // --- APP-113: the canvas page's reads, batched ---
    // Maya now holds the whole library, her authoring app and her
    // compositions; the teammate holds what was shared with her.
    let mate_request = TrustedRequest {
        organization: ORG.into(),
        principal: mate_id.clone(),
        epoch: 1,
        issuer: GATEWASM.into(),
        provider_tenant: harness::NORTHSTAR_TENANT.into(),
        provider_subject: mate_subject.clone(),
        registry_sha256: stores.registry_identity().sha256.clone(),
    };
    let compared = super::canvas_campaign::equivalent_reads(&stores, &maya_request).await;
    let mate_compared = super::canvas_campaign::equivalent_reads(&stores, &mate_request).await;
    let hooks = stores.hooks();
    let delta = |before: (usize, usize)| {
        let after = hooks.counts();
        (after.0 - before.0, after.1 - before.1)
    };
    // Before: the listing judged one work at a time, then each work's
    // outline and records listing in their own transactions.
    let (before, started) = (hooks.counts(), Instant::now());
    let listed = stores
        .data
        .openable_works_serial(&maya_request)
        .await
        .unwrap();
    for work in &listed {
        stores
            .data
            .work_outline(&maya_request, &work.app, &work.work)
            .await
            .unwrap();
        if !work.collections.is_empty() {
            let _ = stores
                .data
                .list_records(&maya_request, &work.app, &work.work)
                .await;
        }
    }
    let (serial_ms, (serial_transactions, serial_statements)) =
        (ms(started.elapsed()), delta(before));
    // After: the batched listing and the batched reads.
    let (before, started) = (hooks.counts(), Instant::now());
    let batched = stores.data.openable_works(&maya_request).await.unwrap();
    let works: Vec<(&str, &str, bool)> = batched
        .iter()
        .map(|w| (w.app.as_str(), w.work.as_str(), !w.collections.is_empty()))
        .collect();
    let reads = stores.data.work_reads(&maya_request, &works).await.unwrap();
    let (batched_ms, (batched_transactions, batched_statements)) =
        (ms(started.elapsed()), delta(before));
    assert_eq!(batched, listed);
    assert!(reads.iter().all(|read| read.outline.is_ok()));
    assert!(
        listed.len() >= 24,
        "the whole library and her own works: {}",
        listed.len()
    );
    assert_eq!(
        batched_transactions, 2,
        "one listing and one read transaction"
    );
    assert!(
        batched_statements <= 40,
        "a fixed number of statements: {batched_statements}"
    );
    // The page itself, end to end over the stand.
    let mut page_samples = Vec::new();
    let before = hooks.counts();
    const PAGES: usize = 5;
    for _ in 0..PAGES {
        let started = Instant::now();
        let page = stand.get("/canvas", Some(&maya_cookie)).await;
        page_samples.push(ms(started.elapsed()));
        assert_eq!(page.status, StatusCode::OK);
        assert!(page.text().contains(&format!("id=\"lib-{work_id}\"")));
    }
    let (page_transactions, page_statements) = delta(before);
    let page_ms = p50(&mut page_samples);
    timings.insert("canvas_serial_reads_ms".into(), json!(serial_ms));
    timings.insert("canvas_batched_reads_ms".into(), json!(batched_ms));
    timings.insert("canvas_page_p50_ms".into(), json!(page_ms));
    println!(
        "CANVAS_READS works={} serial: {serial_transactions} transactions, {serial_statements} statements, {serial_ms} ms; batched: {batched_transactions} transactions, {batched_statements} statements, {batched_ms} ms; GET /canvas per page: {} transactions, {} statements, p50 {page_ms} ms",
        listed.len(),
        page_transactions / PAGES,
        page_statements / PAGES,
    );
    assert!(page_ms < 1000, "GET /canvas p50 {page_ms} ms");
    check(
        &mut checks,
        &format!("batched canvas reads (APP-113): for Maya ({compared} works compared) and the teammate ({mate_compared}), the batched listing and library catalog equal the serial ones, and the batched outline and records reads equal work_outline and list_records per work (an unknown work and a malformed id refused alike); for Maya's {} works the listing and reads took {batched_transactions} transactions and {batched_statements} statements in {batched_ms} ms (one work at a time: {serial_transactions} transactions, {serial_statements} statements, {serial_ms} ms); GET /canvas took {} transactions and {} statements per page, p50 {page_ms} ms over {PAGES}",
            listed.len(), page_transactions / PAGES, page_statements / PAGES),
    );

    // --- Chain integrity ---
    let verified = operator(&c.fixture, &["verify", "--organization", ORG]);
    assert_eq!(verified["verified"], true);
    check(
        &mut checks,
        "audit: every Northstar chain recomputes to its head after the grants, the compositions, the share and the commit",
    );

    let report = json!({
        "scope": "ADR-011 pitch integration (APP-096), local Docker campaign over a loopback TCP stand of the binary's routes, synthetic identities and the synthetic library, a loopback model stub",
        "timings_ms": timings,
        "receipt_digests": receipts.iter().map(|(view, receipt)| (view.clone(), receipt["digest"].clone())).collect::<serde_json::Map<_, _>>(),
        "checks": checks,
    });
    if let Some(path) = std::env::var_os("GENUI_PITCH_CAMPAIGN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    server.abort();
    drop(c);
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn text(fixture: &Fixture, sql: &str) -> String {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
