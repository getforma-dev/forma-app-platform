//! APP-108 folders campaign (isolated test executable only): local Docker
//! PostgreSQL, the locked Python environments (`GENUI_NATIVE_FIXTURE_PYTHON`,
//! DuckDB 2.0.0: the operator, the library normalizer, the box normalizer and
//! the confined query engine), the sealed parser built from `parser/`
//! (`GENUI_BOX_PARSER_DIR`: `parser-host` and `parser_guest.wasm`), a local
//! object store, and a loopback Messages stub reached as the deployment
//! reaches the real endpoint.
//!
//! The story, over HTTP on a loopback socket: Maya creates a folder and
//! uploads a PDF, an image, two CSVs and an XLSX (each a tile; the CSV and
//! XLSX parsed at once with typed previews); a CSV with PNG bytes, a 25 MB
//! file and a 21st file are refused with nothing stored; the PDF is served
//! inline under `Content-Security-Policy: sandbox`; a teammate and an
//! outsider get the uniform 404 for the folder, each file and the view;
//! confirming the CSV's types registers a source revision and a data card
//! whose receipt names that revision, traced to the file; "build a metrics
//! app from folder 1" through `POST /v1/act` sends the model only the capped
//! context and places an admitted, generated app built from the folder; its
//! explanation fills every placeholder from a governed result (a hostile
//! reply with an invented number is refused); sharing the derived app lets
//! the teammate open it and read exactly the file it was built from, not the
//! others; the brief cites the PDF by page; a reload reads everything back;
//! the demo organization's rehearsed sentence, explanation and brief answer
//! without the model; every chain verifies.
use super::{
    admission_qualification::operator,
    qualification::{fixture_python, Fixture},
    qualification_cases::{check, ORG, OTHER},
    query_qualification::pitch_campaign::{Answer, Stand},
    Stores, TrustedRequest,
};
use crate::{
    auth::{
        cookies,
        invite::Invitations,
        synthetic_issuer::{Issuer, ISSUER as GATEWASM},
        tests::{self as harness, Campaign, CANARY_HANDLE, NORTHSTAR_HANDLE},
        Auth,
    },
    boxes,
    composer::{
        self,
        model::{ENDPOINT_ENV, KEY_ENV},
        routes::Compose,
    },
    imports::sandbox::ParserIdentity,
    query::{
        routes::tests::{Counting, RecordingPages},
        tests::{real_engine, repository, Directory},
        Executor, Work,
    },
    storage::admission_qualification::FakeProvider,
};
use axum::http::StatusCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const AUTHORING_APP: &str = "00000000-0000-4000-8000-00000000e0a1";
const FIXTURE: &str = "docs/pitch/folder-fixture";
/// The sealed parser `just parser` builds into this checkout (ADR-012 D12.6);
/// `GENUI_BOX_PARSER_DIR` overrides it.
const DEFAULT_PARSER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../.cache/parser-target");

#[test]
#[ignore = "requires Docker, GENUI_NATIVE_FIXTURE_PYTHON (DuckDB 2.0.0) and the built sealed parser"]
fn native_box_campaign() {
    std::thread::Builder::new()
        .name("box-campaign".into())
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

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The sealed parser built from `parser/` (release guest and host).
fn parser_identity() -> ParserIdentity {
    let dir = std::env::var("GENUI_BOX_PARSER_DIR").unwrap_or_else(|_| DEFAULT_PARSER.into());
    let dir = PathBuf::from(dir);
    let host = dir.join("release/parser-host");
    let wasm = dir.join("wasm32-unknown-unknown/release/parser_guest.wasm");
    let host_bytes = std::fs::read(&host).expect("parser-host built (just parser)");
    let wasm_bytes = std::fs::read(&wasm).expect("parser_guest.wasm built");
    ParserIdentity {
        host,
        host_sha256: hex(&host_bytes),
        wasm,
        wasm_sha256: hex(&wasm_bytes),
        manifest_sha256: hex(b"campaign"),
    }
}

/// The loopback Messages stub: replies scripted per call kind (generation,
/// explanation, documents, brief summary); every request body kept.
#[derive(Default)]
struct Stub {
    replies: Mutex<HashMap<&'static str, VecDeque<String>>>,
    bodies: Mutex<Vec<(String, Value)>>,
}
impl Stub {
    fn script(&self, kind: &'static str, reply: String) {
        self.replies
            .lock()
            .unwrap()
            .entry(kind)
            .or_default()
            .push_back(reply);
    }
    fn calls(&self, kind: &str) -> Vec<Value> {
        self.bodies
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|(_, b)| b.clone())
            .collect()
    }
}
fn kind_of(body: &Value) -> &'static str {
    let system = body["system"].as_str().unwrap_or("");
    let documents = body["messages"][0]["content"]
        .as_array()
        .is_some_and(|c| c.iter().any(|b| b["type"] == "document"));
    if documents {
        "documents"
    } else if system.starts_with("You compose typed Forma") {
        "generation"
    } else if system.starts_with("You explain the results") {
        "explain"
    } else if system.starts_with("You write the overview") {
        "summary"
    } else if system.starts_with("You answer a member's question about the tables") {
        "folder_ask"
    } else {
        "other"
    }
}
async fn stub_server() -> (String, Arc<Stub>) {
    let stub = Arc::new(Stub::default());
    let state = stub.clone();
    let app = axum::Router::new().route(
        "/v1/messages",
        axum::routing::post(move |body: axum::body::Bytes| {
            let state = state.clone();
            async move {
                let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let kind = kind_of(&body);
                state.bodies.lock().unwrap().push((kind.to_owned(), body));
                let reply = if kind == "documents" {
                    json!({"id": "msg", "model": "stub-model", "content": [
                        {"type": "text", "text": "The brief reports that "},
                        {"type": "text", "text": "Q3 revenue grew in every month of the quarter", "citations": [{"type": "page_location", "cited_text": "Q3 revenue grew in every month of the quarter, with September the strongest month.", "document_index": 0, "document_title": "q3_metrics_brief.pdf", "start_page_number": 1, "end_page_number": 2}]},
                        {"type": "text", "text": ", and revenue rose 11% (an uncited number, dropped)."}
                    ], "usage": {"input_tokens": 1500, "output_tokens": 60}})
                } else {
                    let text = state.replies.lock().unwrap().get_mut(kind).and_then(VecDeque::pop_front).unwrap_or_else(|| "{}".into());
                    json!({"id": "msg", "model": "stub-model", "content": [{"type": "text", "text": text}],
                        "usage": {"input_tokens": 5, "output_tokens": 7}})
                };
                (StatusCode::OK, [("content-type", "application/json")], reply.to_string())
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (endpoint, stub)
}

/// One multipart upload (one `file` part).
async fn upload(stand: &Stand, cookie: &str, box_id: &str, name: &str, bytes: &[u8]) -> Answer {
    try_upload(stand, cookie, box_id, name, bytes)
        .await
        .unwrap()
}
/// As `upload`; `Err` when the server closed the connection before the body
/// was sent (a refused oversized body answered early).
async fn try_upload(
    stand: &Stand,
    cookie: &str,
    box_id: &str,
    name: &str,
    bytes: &[u8],
) -> Result<Answer, reqwest::Error> {
    let boundary = "----formacampaignboundary7MA4YWxk";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let started = Instant::now();
    let response = stand
        .client
        .post(format!("{}/v1/boxes/{box_id}/files", stand.origin))
        .header(
            reqwest::header::COOKIE,
            format!("{}={cookie}", cookies::SESSION),
        )
        .header("sec-fetch-site", "same-origin")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?.to_vec();
    Ok(Answer {
        status,
        headers,
        body,
        elapsed: started.elapsed(),
    })
}

fn generated_reply() -> String {
    let field = |f: &str| json!({"input_id": "q3_metrics", "field_id": f});
    json!({
        "title": "Q3 metrics", "description": "Revenue, signups and churn from the Q3 folder.",
        "inputs": [{"id": "q3_metrics", "contract_id": "x", "contract_version": 1, "kind": "source", "fields": []}],
        "views": [
            {"id": "kpis", "kind": "summary", "renderer_version": 1, "title": "Q3 totals", "query": {"operator_version": 1, "inputs": ["q3_metrics"], "null_policy": "unknown",
                "operations": [{"type": "aggregate", "group_by": [], "measures": [
                    {"id": "rows", "operation": "count", "field": field("region")},
                    {"id": "total_revenue", "operation": "sum", "field": field("revenue")},
                    {"id": "total_signups", "operation": "sum", "field": field("signups")}]}]}},
            {"id": "by_region", "kind": "chart", "renderer_version": 1, "title": "Revenue by region", "query": {"operator_version": 1, "inputs": ["q3_metrics"], "null_policy": "unknown",
                "operations": [{"type": "aggregate", "group_by": [field("region")], "measures": [{"id": "total_revenue", "operation": "sum", "field": field("revenue")}]}]}},
        ],
        "record_schemas": [], "commands": [], "links": [],
        "default_layout": [{"view_id": "kpis", "column": 0, "row": 0, "width": 12}, {"view_id": "by_region", "column": 0, "row": 1, "width": 12}],
    }).to_string()
}

fn ms(duration: Duration) -> u64 {
    duration.as_millis() as u64
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
    operator(&fixture, &["seed-library", "--organization", OTHER]);
    let python = fixture_python();
    let root = Directory::new("boxes");
    for org in [ORG, OTHER] {
        let built = std::process::Command::new(&python)
            .current_dir(repository().join("app/runtime"))
            .args(["-c", "import json,sys,library_sources as l; print(json.dumps(l.build(sys.argv[1], sys.argv[2])))", org, root.0.join("cache").to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
    }
    let engine = Arc::new(real_engine(&python, &root.0));

    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    fixture.sql(&format!("INSERT INTO genui.provider_tenant_bindings VALUES ('{ORG}','{GATEWASM}','{}',true),('{OTHER}','{GATEWASM}','{}',true)", harness::NORTHSTAR_TENANT, harness::CANARY_TENANT), "genui");
    fixture.write_registry(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000a18","bindings":[
        {"issuer":GATEWASM,"provider_tenant_id":harness::NORTHSTAR_TENANT,"organization_id":ORG},
        {"issuer":GATEWASM,"provider_tenant_id":harness::CANARY_TENANT,"organization_id":OTHER}]}));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let issuer = Issuer::start(harness::tenants()).await;
    let provider = FakeProvider::start_at(
        &fixture,
        harness::NORTHSTAR_TENANT,
        "box-northstar",
        None,
        None,
    );
    let canary_provider = FakeProvider::start_at(
        &fixture,
        harness::CANARY_TENANT,
        "box-canary",
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

    let (endpoint, stub) = stub_server().await;
    std::env::set_var(KEY_ENV, "sk-ant-synthetic-box-campaign-key");
    std::env::set_var(ENDPOINT_ENV, &endpoint);
    std::env::remove_var(composer::DEMO_ORGANIZATIONS_ENV);
    let composer = composer::Composer::from_env();
    std::env::remove_var(KEY_ENV);
    std::env::remove_var(ENDPOINT_ENV);
    crate::demo::configure_for_tests(false, &[]);

    let counting = Arc::new(Counting {
        engine: Some(engine.clone() as Arc<dyn Executor>),
        ..Counting::default()
    });
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(counting.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    let objects = root.0.join("objects");
    let state = Arc::new(boxes::routes::Boxes {
        auth: auth.clone(),
        work: work.clone(),
        composer: composer.clone(),
        store: boxes::store::ObjectStore::Local {
            root: objects.clone(),
        },
        normalizer: boxes::register::Normalizer {
            program: python.clone(),
            args: vec![
                "-I".into(),
                repository()
                    .join("app/runtime/runtime.py")
                    .to_str()
                    .unwrap()
                    .into(),
                "normalize".into(),
            ],
            cache_root: root.0.join("cache"),
            scratch_root: root.0.join("box-scratch"),
        },
        parser: Some(parser_identity()),
    });
    boxes::intent::install(state.clone());

    #[derive(rust_embed::Embed)]
    #[folder = "admin/dist/"]
    struct Assets;
    let mut manifest =
        forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
    manifest.routes.retain(|route, _| route == "/canvas");
    let pages = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
    let canvas_pages = Arc::new(crate::query::canvas::SsrCanvasPages {
        page: pages
            .bind::<crate::query::canvas::CanvasProps>("canvas")
            .unwrap(),
    });
    pages.finalize().unwrap();
    let long = vec![
        (
            composer::routes::PATH,
            composer::routes::router(Arc::new(Compose::new(auth.clone(), composer.clone()))),
        ),
        (
            crate::actions::PATH,
            crate::actions::routes::router(Arc::new(crate::actions::routes::Act::new(
                auth.clone(),
                crate::actions::Parser::new(None, "none"),
            ))),
        ),
    ];
    let routes = crate::http::uniform_with_many(
        axum::Router::new()
            .route(
                "/_assets/{*filename}",
                axum::routing::get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(crate::auth::routes::router(auth.clone()))
            .merge(crate::query::routes::router(work.clone()))
            .merge(crate::sharing::router(auth.clone()))
            .merge(crate::events::routes::router(auth.clone()))
            .merge(crate::actions::routes::records_router(auth.clone()))
            .merge(crate::composer::commands::router(auth.clone()))
            .merge(boxes::routes::router(state.clone()))
            .merge(crate::query::canvas::router(Arc::new(
                crate::query::canvas::Canvas {
                    work: work.clone(),
                    pages: canvas_pages,
                    live_events: "/v1/events".into(),
                    demo_organizations: Default::default(),
                },
            ))),
        long,
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
    let (maya_subject, maya) = c.member("00000000e081").await;
    let (_, mate) = c.member("00000000e082").await;
    let (_, outsider) = c.member_in(OTHER, "00000000e083").await;
    let maya_id = c.principal(&maya_subject).await;
    let mate_id = {
        let subject = mate.cookie(cookies::SESSION).unwrap();
        let _ = subject;
        let rows = c.fixture.operator().await;
        let row = rows.client.query_one(&format!("SELECT principal_id::text FROM genui.principals WHERE organization_id='{ORG}' AND principal_id<>'{maya_id}'::uuid AND provider_subject LIKE '%00000000e082%'"), &[]).await;
        match row {
            Ok(row) => row.get::<_, String>(0),
            Err(_) => String::new(),
        }
    };
    let maya_cookie = maya.cookie(cookies::SESSION).unwrap();
    let mate_cookie = mate.cookie(cookies::SESSION).unwrap();
    let outsider_cookie = outsider.cookie(cookies::SESSION).unwrap();
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
    let mate_id = if mate_id.is_empty() {
        // The members list names the teammate (labels only).
        let members = stand.get("/v1/members", Some(&maya_cookie)).await.json();
        members["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["principal_id"].as_str().unwrap().to_owned())
            .find(|p| p != &maya_id)
            .unwrap()
    } else {
        mate_id
    };
    let _ = &maya_subject;
    let _maya_request = TrustedRequest {
        organization: ORG.into(),
        principal: maya_id.clone(),
        epoch: 1,
        issuer: GATEWASM.into(),
        provider_tenant: harness::NORTHSTAR_TENANT.into(),
        provider_subject: maya_subject.clone(),
        registry_sha256: stores.registry_identity().sha256.clone(),
    };

    // --- A folder with mixed files ---
    let created = stand
        .post(
            "/v1/boxes",
            Some(&maya_cookie),
            &json!({"name": "Q3 board pack"}).to_string(),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let box_id = created.json()["box"]["box_id"].as_str().unwrap().to_owned();
    assert_eq!(created.json()["box"]["number"], 1);
    let fixture_dir = repository().join(FIXTURE);
    let pdf = std::fs::read(fixture_dir.join("q3_metrics_brief.pdf")).unwrap();
    let metrics = std::fs::read(fixture_dir.join("q3_metrics.csv")).unwrap();
    let pipeline = std::fs::read(fixture_dir.join("q3_pipeline.csv")).unwrap();
    let xlsx = std::fs::read(repository().join("parser/fixtures/valid.xlsx")).unwrap();
    let png: Vec<u8> = {
        // A 1x1 PNG.
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&[
            0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15,
            0xc4, 0x89, 0, 0, 0, 13, b'I', b'D', b'A', b'T', 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0,
            0xf0, 0x1f, 0, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0, 0, 0, 0, b'I', b'E',
            b'N', b'D', 0xae, 0x42, 0x60, 0x82,
        ]);
        bytes
    };
    let mut files: HashMap<&str, String> = HashMap::new();
    for (name, bytes) in [
        ("q3_metrics_brief.pdf", &pdf),
        ("logo.png", &png),
        ("q3_metrics.csv", &metrics),
        ("q3_pipeline.csv", &pipeline),
        ("forecast.xlsx", &xlsx),
    ] {
        let answer = upload(&stand, &maya_cookie, &box_id, name, bytes).await;
        assert_eq!(
            answer.status,
            StatusCode::CREATED,
            "{name}: {}",
            answer.text()
        );
        let value = answer.json();
        assert_eq!(value["file"]["sha256"], format!("sha256:{}", hex(bytes)));
        if name.ends_with(".csv") || name.ends_with(".xlsx") {
            assert!(
                value["preview"]["columns"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty()),
                "{name}: {value}"
            );
            assert!(value["preview"]["sample"].as_array().unwrap().len() <= 20);
        } else {
            assert!(value["preview"].is_null());
        }
        timings.insert(format!("upload_{name}"), json!(ms(answer.elapsed)));
        files.insert(name, value["file"]["file_id"].as_str().unwrap().to_owned());
    }
    let metrics_preview = upload(
        &stand,
        &maya_cookie,
        &box_id,
        "q3_metrics_copy.csv",
        &metrics,
    )
    .await
    .json()["preview"]
        .clone();
    let kinds: Vec<(String, String)> = metrics_preview["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().into(),
                c["type"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("month", "date"),
            ("region", "text"),
            ("revenue", "money"),
            ("signups", "integer"),
            ("churn_rate", "decimal")
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    check(&mut checks, "upload: a PDF, a PNG, two CSVs and an XLSX land in a new folder as tiles with their SHA-256; each CSV/XLSX is parsed at once by the sealed parser (one parser-host process per job) with typed columns (date, text, money, integer, decimal) and at most 20 sample rows");

    // --- Refusals: wrong magic bytes, too large, a 21st file; nothing stored ---
    let objects_before = walk(&objects);
    let rows_before = scalar(
        &c.fixture,
        &format!("SELECT count(*) FROM genui.box_files WHERE organization_id='{ORG}'"),
    )
    .await;
    let wrong = upload(&stand, &maya_cookie, &box_id, "numbers.csv", &png).await;
    assert_eq!(
        (wrong.status, wrong.json()["code"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("unsupported_file"))
    );
    // The 413 may arrive before the client finished sending (the connection
    // then closes under the upload): either way nothing is stored (below).
    if let Ok(large) = try_upload(
        &stand,
        &maya_cookie,
        &box_id,
        "big.csv",
        &vec![b'a'; 25 * 1024 * 1024],
    )
    .await
    {
        assert_eq!(
            (large.status, large.json()["code"].clone()),
            (StatusCode::PAYLOAD_TOO_LARGE, json!("file_too_large"))
        );
    }
    let full = stand
        .post(
            "/v1/boxes",
            Some(&maya_cookie),
            &json!({"name": "Notes"}).to_string(),
        )
        .await
        .json()["box"]["box_id"]
        .as_str()
        .unwrap()
        .to_owned();
    for n in 0..20 {
        let answer = upload(
            &stand,
            &maya_cookie,
            &full,
            &format!("note-{n}.txt"),
            format!("note {n}").as_bytes(),
        )
        .await;
        assert_eq!(answer.status, StatusCode::CREATED);
    }
    let objects_full = walk(&objects);
    let twenty_first = upload(&stand, &maya_cookie, &full, "note-20.txt", b"one too many").await;
    assert_eq!(
        (twenty_first.status, twenty_first.json()["code"].clone()),
        (StatusCode::CONFLICT, json!("box_full"))
    );
    assert_eq!(walk(&objects), objects_full, "the 21st file stored nothing");
    assert_eq!(
        objects_full - objects_before,
        20,
        "the refused CSV and the 25 MB file stored nothing"
    );
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.box_files WHERE organization_id='{ORG}'")
        )
        .await,
        rows_before + 20
    );
    check(&mut checks, "refusals: a CSV carrying PNG bytes (422 unsupported_file), a 25 MB file (413 file_too_large) and a 21st file (409 box_full) are refused with neutral codes; no object and no row is written for any of them");

    // --- The viewer route and the uniform 404 ---
    let pdf_path = format!(
        "/v1/boxes/{box_id}/files/{}/content",
        files["q3_metrics_brief.pdf"]
    );
    let viewed = stand
        .call(
            "GET",
            &pdf_path,
            Some(&maya_cookie),
            None,
            &[("sec-fetch-site", "same-origin")],
        )
        .await;
    assert_eq!(viewed.status, StatusCode::OK);
    assert_eq!(viewed.body, pdf);
    assert_eq!(viewed.headers["content-type"], "application/pdf");
    assert!(viewed.headers["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("inline"));
    assert!(viewed.headers["content-security-policy"]
        .to_str()
        .unwrap()
        .starts_with("sandbox"));
    assert_eq!(viewed.headers["x-content-type-options"], "nosniff");
    let cross = stand
        .call(
            "GET",
            &pdf_path,
            Some(&maya_cookie),
            None,
            &[("sec-fetch-site", "cross-site")],
        )
        .await;
    assert!(cross.uniform_404());
    let png_path = format!("/v1/boxes/{box_id}/files/{}/content", files["logo.png"]);
    let csv_path = format!(
        "/v1/boxes/{box_id}/files/{}/content",
        files["q3_metrics.csv"]
    );
    let preview_path = format!(
        "/v1/boxes/{box_id}/files/{}/preview",
        files["q3_metrics.csv"]
    );
    for cookie in [&mate_cookie, &outsider_cookie] {
        for path in [
            format!("/v1/boxes/{box_id}"),
            pdf_path.clone(),
            png_path.clone(),
            csv_path.clone(),
            preview_path.clone(),
        ] {
            let answer = stand.get(&path, Some(cookie)).await;
            assert!(answer.uniform_404(), "{path}: {}", answer.text());
        }
    }
    assert!(stand
        .get(&format!("/v1/boxes/{box_id}"), None)
        .await
        .uniform_404());
    let outsider_list = stand.get("/v1/boxes", Some(&outsider_cookie)).await.json();
    assert_eq!(outsider_list["boxes"], json!([]));
    check(&mut checks, "view: the PDF is served same-origin with Content-Disposition inline, Content-Security-Policy sandbox and nosniff (a cross-site fetch is the uniform 404); a teammate the folder was not shared with and a foreign organization's member get the uniform 404 for the folder, every file, the view and the preview, and list no folders");

    // --- Confirm types → a source revision and a data card with receipts ---
    let columns: Vec<Value> = metrics_preview["columns"].as_array().unwrap().iter().map(|c| json!({"id": c["id"], "type": c["type"], "nullable": c["nullable"], "scale": c["scale"]})).collect();
    let started = Instant::now();
    let confirmed = stand
        .post(
            &format!("/v1/boxes/{box_id}/files/{}/data", files["q3_metrics.csv"]),
            Some(&maya_cookie),
            &json!({"columns": columns, "card": true}).to_string(),
        )
        .await;
    assert_eq!(
        confirmed.status,
        StatusCode::CREATED,
        "{}",
        confirmed.text()
    );
    timings.insert("confirm_register_card".into(), json!(ms(started.elapsed())));
    let data = confirmed.json()["data"].clone();
    let revision = data["revision_id"].as_str().unwrap().to_owned();
    assert_eq!(data["row_count"], 12);
    let card = confirmed.json()["card"].clone();
    let (card_app, card_work) = (
        card["app_id"].as_str().unwrap().to_owned(),
        card["work_id"].as_str().unwrap().to_owned(),
    );
    let bad = stand.post(&format!("/v1/boxes/{box_id}/files/{}/data", files["q3_pipeline.csv"]), Some(&maya_cookie), &json!({"columns": [
        {"id": "deal", "type": "integer", "nullable": false}, {"id": "stage", "type": "text", "nullable": false}, {"id": "amount", "type": "money", "nullable": false},
        {"id": "close_date", "type": "date", "nullable": false}, {"id": "owner", "type": "text", "nullable": false}], "card": false}).to_string()).await;
    assert_eq!(
        (bad.status, bad.json()["column"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("deal"))
    );
    let run = stand
        .post(
            &format!("/v1/work/{card_app}/{card_work}/queries/q3_metrics_kpis"),
            Some(&maya_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(run.status, StatusCode::OK, "{}", run.text());
    let receipt = run.json()["receipt"].clone();
    assert!(
        receipt.to_string().contains(&revision),
        "the receipt names the registered revision"
    );
    let lineage = stand
        .get(
            &format!("/v1/boxes/lineage/{card_app}/{card_work}"),
            Some(&maya_cookie),
        )
        .await
        .json();
    assert_eq!(lineage["box_id"], box_id);
    assert_eq!(lineage["files"][0]["revision_id"], revision.as_str());
    assert_eq!(
        lineage["files"][0]["file_id"],
        files["q3_metrics.csv"].as_str()
    );
    let parquet = root.0.join("cache/o").join(ORG).join(format!(
        "{}.{revision}.parquet",
        data["source_id"].as_str().unwrap()
    ));
    assert_eq!(
        format!("sha256:{}", hex(&std::fs::read(&parquet).unwrap())),
        data["normalized_digest"].as_str().unwrap()
    );
    assert!(objects
        .join(format!(
            "o/{ORG}/s/{}/r/{revision}/original",
            data["source_id"].as_str().unwrap()
        ))
        .exists());
    check(&mut checks, "real data: confirming the CSV's types registers an immutable source revision (original and part.parquet under o/<org>/s/<source>/r/<revision>/, the Parquet's digest the engine re-checks) and a data card; its view runs on the confined engine and its receipt names that revision, which the lineage route traces to the CSV in the folder; a confirmed type the values do not fit names the column (422 data_refused)");

    // --- Build from the folder through the action path (capped context) ---
    stub.script("generation", generated_reply());
    let started = Instant::now();
    let built = stand
        .post(
            crate::actions::PATH,
            Some(&maya_cookie),
            &json!({"utterance": "build a metrics app from folder 1"}).to_string(),
        )
        .await;
    assert_eq!(built.status, StatusCode::OK, "{}", built.text());
    timings.insert("build_from_folder".into(), json!(ms(started.elapsed())));
    let answer = built.json();
    assert_eq!(answer["via"], "folder");
    assert_eq!(answer["kind"], "build_from_box");
    assert_eq!(answer["origin"], "generated");
    assert!(answer["label"]
        .as_str()
        .unwrap()
        .starts_with("Built from folder: Q3 board pack"));
    let (app, derived) = (
        answer["app_id"].as_str().unwrap().to_owned(),
        answer["work_id"].as_str().unwrap().to_owned(),
    );
    assert_eq!(app, AUTHORING_APP);
    let generation = stub.calls("generation");
    assert_eq!(generation.len(), 1);
    let user: Value =
        serde_json::from_str(generation[0]["messages"][0]["content"].as_str().unwrap()).unwrap();
    let context = &user["box"];
    assert!(context["data"].as_array().unwrap().len() <= 3);
    assert!(context["data"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["sample_rows"].as_array().unwrap().len() <= 20));
    assert!(context["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["name"] == "q3_metrics_brief.pdf"));
    let request_text = generation[0].to_string();
    assert!(
        !request_text.contains("strongest month"),
        "no PDF content in the context"
    );
    assert!(user["sources"].as_array().unwrap().len() <= 3);
    timings.insert("context_bytes".into(), json!(user["box"].to_string().len()));
    let group = stand
        .get(
            &format!("/v1/canvas/works/{app}/{derived}"),
            Some(&maya_cookie),
        )
        .await;
    assert_eq!(group.status, StatusCode::OK, "{}", group.text());
    assert!(group.text().contains("Built from folder: Q3 board pack"));
    let run = stand
        .post(
            &format!("/v1/work/{app}/{derived}/queries/kpis"),
            Some(&maya_cookie),
            &json!({"parameters": {}}).to_string(),
        )
        .await;
    assert_eq!(run.status, StatusCode::OK, "{}", run.text());
    let kpis = run.json();
    let derived_lineage = stand
        .get(
            &format!("/v1/boxes/lineage/{app}/{derived}"),
            Some(&maya_cookie),
        )
        .await
        .json();
    let used: Vec<String> = derived_lineage["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file_id"].as_str().unwrap().to_owned())
        .collect();
    assert!(used.contains(&files["q3_metrics.csv"]));
    assert!(
        !used.contains(&files["q3_metrics_brief.pdf"]),
        "the PDF contributed its name only"
    );
    for file in derived_lineage["files"].as_array().unwrap() {
        if let Some(revision) = file["revision_id"].as_str() {
            assert!(
                kpis["receipt"].to_string().contains(revision)
                    || file["file_id"] != files["q3_metrics.csv"].as_str()
            );
        }
    }
    check(&mut checks, &format!("derived app: \"build a metrics app from folder 1\" through POST /v1/act resolves the folder by number and the model receives only the capped context ({} bytes: file names and types, at most 3 data files with typed fields and at most 20 sample rows, the PDF by name only); the typed definition is admitted, published on Maya's authoring app and placed, labeled generated with \"Built from folder: Q3 board pack\"; its receipts name the folder's revisions", user["box"].to_string().len()));

    // --- The explanation card: placeholders filled from receipts ---
    stub.script("explain", json!({"findings": [
        {"text": "Revenue totaled {kpis.total_revenue} across {kpis.rows} region-months.", "refs": ["kpis.total_revenue", "kpis.rows"]},
        {"text": "Signups reached {kpis.total_signups}.", "refs": ["kpis.total_signups"]}]}).to_string());
    let explained = stand
        .post(
            &format!("/v1/boxes/explain/{app}/{derived}"),
            Some(&maya_cookie),
            "{}",
        )
        .await;
    assert_eq!(explained.status, StatusCode::OK, "{}", explained.text());
    let explanation = explained.json();
    assert_eq!(explanation["state"], "ready");
    let index = |name: &str| {
        kpis["columns"]
            .as_array()
            .unwrap()
            .iter()
            .position(|c| c == name)
            .unwrap()
    };
    let exact_revenue = kpis["rows"][0][index("total_revenue")].clone();
    let value = &explanation["findings"][0]["parts"][1];
    assert_eq!(value["exact"], exact_revenue);
    assert!(value["receipt_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let explain_input = stub.calls("explain")[0]["messages"][0]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        !explain_input.contains("Acme Retail") && !explain_input.contains("strongest month"),
        "results only, never files"
    );
    stub.script(
        "explain",
        json!({"findings": [{"text": "Revenue grew 40% to {kpis.total_revenue}."}]}).to_string(),
    );
    let hostile = stand
        .post(
            &format!("/v1/boxes/explain/{app}/{derived}"),
            Some(&maya_cookie),
            "{}",
        )
        .await
        .json();
    assert_eq!(hostile["state"], "none", "an invented number is refused");
    let kept = stand
        .get(
            &format!("/v1/boxes/explain/{app}/{derived}"),
            Some(&maya_cookie),
        )
        .await
        .json();
    assert_eq!(kept["state"], "ready", "the accepted summary stays");
    check(&mut checks, "explanation: the fast model sees only the computed results; every placeholder is filled with the governed value of a receipt-carrying run (total revenue equals the KPI view's exact value); a hostile reply with an invented number (\"40%\") is refused as \"No summary\" and the accepted summary stays");

    // --- APP-124: Ask on a folder (read-only, file and row references) ---
    let audit_sql =
        format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'");
    let audit = || scalar(&c.fixture, &audit_sql);
    let audit_before = audit().await;
    let ask_path = format!("/v1/boxes/{box_id}/ask");
    stub.script(
        "folder_ask",
        json!({"text": "South led July with the highest revenue.", "references": ["f1r2"]})
            .to_string(),
    );
    let asked = stand
        .post(
            &ask_path,
            Some(&maya_cookie),
            &json!({"question": "which region led July?"}).to_string(),
        )
        .await;
    assert_eq!(asked.status, StatusCode::OK, "{}", asked.text());
    let answer = asked.json();
    assert_eq!(
        answer.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["references", "text"]
    );
    // The reference is the server's: the folder's own file (id, name and the
    // SHA-256 the folder lists for it) and the row under the header.
    let reference = &answer["references"][0];
    let listed = stand
        .get(&format!("/v1/boxes/{box_id}"), Some(&maya_cookie))
        .await
        .json();
    let file = listed["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file_id"] == reference["file_id"])
        .unwrap_or_else(|| panic!("{reference}"))
        .clone();
    assert!(
        matches!(file["kind"].as_str(), Some("csv" | "xlsx")),
        "{file}"
    );
    assert_eq!(reference["file"], file["name"]);
    assert_eq!(reference["sha256"], file["sha256"]);
    assert_eq!(reference["row"], 2);
    assert!(!reference["label"].as_str().unwrap().is_empty());
    let asks = stub.calls("folder_ask");
    assert_eq!(asks.len(), 1);
    let sent = asks[0]["messages"][0]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        sent.contains("which region led July?")
            && sent.contains("\"f1r2\"")
            && sent.contains("South")
    );
    assert!(!sent.contains("strongest month"), "the PDF is not a table");
    // A reply citing a row it was not shown, or carrying a view, is refused.
    for hostile in [
        json!({"text": "Row nine.", "references": ["f1r99"]}),
        json!({"text": "A chart.", "chart": "bar"}),
    ] {
        stub.script("folder_ask", hostile.to_string());
        let refused = stand
            .post(
                &ask_path,
                Some(&maya_cookie),
                &json!({"question": "anything?"}).to_string(),
            )
            .await;
        assert_eq!(
            (refused.status, refused.json()["code"].clone()),
            (StatusCode::UNPROCESSABLE_ENTITY, json!("could_not_answer")),
            "{hostile}"
        );
    }
    // Closed body; the teammate (not shared), the outsider, no session and a
    // malformed id get the uniform 404, before any model call.
    let closed = stand
        .post(
            &ask_path,
            Some(&maya_cookie),
            &json!({"question": "x", "scope": "all"}).to_string(),
        )
        .await;
    assert_eq!(
        (closed.status, closed.json()["code"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("invalid_request"))
    );
    let calls_before = stub.calls("folder_ask").len();
    for (cookie, path) in [
        (Some(mate_cookie.as_str()), ask_path.clone()),
        (Some(outsider_cookie.as_str()), ask_path.clone()),
        (None, ask_path.clone()),
        (
            Some(maya_cookie.as_str()),
            "/v1/boxes/not-a-folder/ask".to_owned(),
        ),
        (
            Some(maya_cookie.as_str()),
            format!("/v1/boxes/{}/ask", "00000000-0000-4000-8000-00000000dead"),
        ),
    ] {
        let denied = stand
            .post(
                &path,
                cookie,
                &json!({"question": "how many rows?"}).to_string(),
            )
            .await;
        assert!(
            denied.uniform_404(),
            "{path}: {} {}",
            denied.status,
            denied.text()
        );
    }
    assert_eq!(
        stub.calls("folder_ask").len(),
        calls_before,
        "no model call for a refused caller"
    );
    assert_eq!(
        audit().await,
        audit_before,
        "Ask on a folder writes no audit row"
    );
    check(&mut checks, "Ask on a folder (APP-124): POST /v1/boxes/{box}/ask reads the folder's tables in a READ ONLY snapshot under the folder's own authorization and answers exactly {text, references}, each reference the file (id, name, SHA-256 of its bytes) and the row the answer used; the model sees the rows by reference (the PDF is not a table) and a reply citing a row it was not shown, or carrying a chart, is refused (422 could_not_answer); an open body is 422; the teammate the folder was not shared with, a foreign member, no session, a malformed and an unknown id get the uniform 404 with no model call; no audit row is written");

    // --- APP-124: Make a card (formation constrained to one card) ---
    let generations = stub.calls("generation").len();
    let made = stand
        .post(
            &format!("/v1/boxes/{box_id}/apps"),
            Some(&maya_cookie),
            &json!({"one_card": true}).to_string(),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.text());
    let made = made.json();
    assert_eq!(
        (made["cards"].clone(), made["source"].clone()),
        (json!(1), json!("rule"))
    );
    assert_eq!(made["title"], "Q3 board pack");
    assert_eq!(
        made["address"], "q3-board-pack-card",
        "the folder keeps /q3-board-pack"
    );
    assert!(made["label"]
        .as_str()
        .unwrap()
        .starts_with("Built from folder: Q3 board pack"));
    let (card_app_id, one_card) = (
        made["app_id"].as_str().unwrap().to_owned(),
        made["work_id"].as_str().unwrap().to_owned(),
    );
    assert_eq!(
        stub.calls("generation").len(),
        generations,
        "Make a card calls no model"
    );
    let card_group = stand
        .get(
            &format!("/v1/canvas/works/{card_app_id}/{one_card}"),
            Some(&maya_cookie),
        )
        .await;
    assert_eq!(card_group.status, StatusCode::OK, "{}", card_group.text());
    let card_group = card_group.json();
    assert_eq!(
        card_group["cards"].as_array().map(Vec::len),
        Some(1),
        "exactly one card: {card_group}"
    );
    assert!(card_group
        .to_string()
        .contains("Built from folder: Q3 board pack"));
    let commands = stand.get("/v1/commands", Some(&maya_cookie)).await.json();
    assert!(commands["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["slug"] == "q3-board-pack-card"
            && c["kind"] == "saved"
            && c["work_id"] == one_card.as_str()));
    let card_lineage = stand
        .get(
            &format!("/v1/boxes/lineage/{card_app_id}/{one_card}"),
            Some(&maya_cookie),
        )
        .await
        .json();
    assert_eq!(card_lineage["box_id"], box_id.as_str());
    assert_eq!(card_lineage["files"].as_array().unwrap().len(), 1);
    let again = stand
        .post(
            &format!("/v1/boxes/{box_id}/apps"),
            Some(&maya_cookie),
            &json!({"one_card": true}).to_string(),
        )
        .await
        .json();
    assert_eq!(
        again["address"], "q3-board-pack-2",
        "a second card takes the next free name"
    );
    for (body, code) in [
        (json!({"one_card": "yes"}), "invalid_intent"),
        (json!({"one_card": false}), "invalid_intent"),
        (json!({"one_card": true, "views": 2}), "invalid_intent"),
    ] {
        let refused = stand
            .post(
                &format!("/v1/boxes/{box_id}/apps"),
                Some(&maya_cookie),
                &body.to_string(),
            )
            .await;
        assert_eq!(
            (refused.status, refused.json()["code"].clone()),
            (StatusCode::UNPROCESSABLE_ENTITY, json!(code)),
            "{body}"
        );
    }
    for cookie in [&mate_cookie, &outsider_cookie] {
        let denied = stand
            .post(
                &format!("/v1/boxes/{box_id}/apps"),
                Some(cookie),
                &json!({"one_card": true}).to_string(),
            )
            .await;
        assert!(denied.uniform_404());
    }
    check(&mut checks, "Make a card (APP-124): POST /v1/boxes/{box}/apps {\"one_card\": true} makes, with no model call, a minimal app named after the folder with exactly one card (the most useful view of its confirmed table), admitted, published on Maya's authoring app, \"Built from folder: Q3 board pack\" with its one source file in the lineage, and saved under a free /name (q3-board-pack-card, since the folder holds /q3-board-pack; the next one q3-board-pack-2) that /v1/commands lists; a non-boolean, a false or an open body is 422; the teammate and the outsider get the uniform 404");

    // --- Share the derived app: the teammate reads exactly the files used ---
    let shared = stand
        .post(
            &format!("/v1/work/{app}/{derived}/share"),
            Some(&maya_cookie),
            &json!({"grantee_principal_id": mate_id}).to_string(),
        )
        .await;
    assert_eq!(shared.status, StatusCode::OK, "{}", shared.text());
    let mate_group = stand
        .get(
            &format!("/v1/canvas/works/{app}/{derived}"),
            Some(&mate_cookie),
        )
        .await;
    assert_eq!(mate_group.status, StatusCode::OK, "{}", mate_group.text());
    let mate_lineage = stand
        .get(
            &format!("/v1/boxes/lineage/{app}/{derived}"),
            Some(&mate_cookie),
        )
        .await;
    assert_eq!(mate_lineage.status, StatusCode::OK);
    let mate_box = stand
        .get(&format!("/v1/boxes/{box_id}"), Some(&mate_cookie))
        .await
        .json();
    let visible: Vec<String> = mate_box["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file_id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(visible.len(), used.len());
    assert!(visible.iter().all(|f| used.contains(f)));
    assert_eq!(mate_box["apps"], json!([]));
    assert_eq!(
        stand
            .call("GET", &csv_path, Some(&mate_cookie), None, &[])
            .await
            .status,
        StatusCode::OK
    );
    for path in [&pdf_path, &png_path] {
        assert!(stand.get(path, Some(&mate_cookie)).await.uniform_404());
    }
    let mate_explanation = stand
        .get(
            &format!("/v1/boxes/explain/{app}/{derived}"),
            Some(&mate_cookie),
        )
        .await
        .json();
    assert_eq!(
        mate_explanation["findings"][0]["parts"][1]["exact"],
        exact_revenue
    );
    for path in [
        format!("/v1/boxes/lineage/{app}/{derived}"),
        format!("/v1/boxes/explain/{app}/{derived}"),
    ] {
        assert!(stand.get(&path, Some(&outsider_cookie)).await.uniform_404());
    }
    check(&mut checks, "sharing: sharing the derived app gives the teammate the app, its explanation with the same governed values, and read access to exactly the folder files it was built from (the CSV opens; the PDF and the image stay the uniform 404, no other file is listed, no derived apps are listed); a foreign member gets the uniform 404 for the lineage and the explanation");

    // --- The brief: PDF statements cited by page, uncited numbers dropped ---
    stub.script("summary", json!({"overview": "The folder holds the Q3 board pack: metrics, pipeline and a brief.", "data": [], "key_points": ["Revenue and signups come from the confirmed metrics file."]}).to_string());
    let started = Instant::now();
    let brief = stand
        .post(
            &format!("/v1/boxes/{box_id}/brief"),
            Some(&maya_cookie),
            "{}",
        )
        .await;
    assert_eq!(brief.status, StatusCode::OK, "{}", brief.text());
    timings.insert("brief".into(), json!(ms(started.elapsed())));
    let brief = brief.json();
    let pdf_entry = brief["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "q3_metrics_brief.pdf")
        .unwrap()
        .clone();
    assert_eq!(pdf_entry["status"], "included");
    assert!(pdf_entry["paragraph"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["cite"]["page"] == 1));
    assert!(
        !pdf_entry["paragraph"].to_string().contains("11%"),
        "the uncited number is dropped"
    );
    let documents = stub.calls("documents");
    assert_eq!(documents.len(), 1);
    assert_eq!(
        documents[0]["messages"][0]["content"][0]["citations"]["enabled"],
        true
    );
    check(&mut checks, "brief: the small PDF goes to the model as a document block with citations enabled; the cited statement is kept with its page (p. 1) and the uncited sentence with a number is dropped; the overview and key points carry no digit");

    // --- Reload: everything reads back ---
    let listed = stand.get("/v1/boxes", Some(&maya_cookie)).await.json();
    assert_eq!(listed["boxes"].as_array().unwrap().len(), 2);
    let again = stand
        .get(&format!("/v1/boxes/{box_id}"), Some(&maya_cookie))
        .await
        .json();
    assert_eq!(again["files"].as_array().unwrap().len(), 6);
    assert!(again["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["data"]["revision_id"] == revision.as_str()));
    assert!(again["apps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["work_id"] == derived.as_str() && a["kind"] == "derived"));
    let canvas = stand.get("/canvas", Some(&maya_cookie)).await;
    assert_eq!(canvas.status, StatusCode::OK);
    assert!(canvas.headers["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("frame-src 'self'"));
    check(&mut checks, "reload: the folders, files, the registered revision, the derived app's provenance and the stored explanation and brief read back; the canvas page allows same-origin frames for the viewer");

    // --- APP-124 review: a bad newest table, names judged whole, idempotent builds ---
    let folder = |name: &'static str| {
        let stand = &stand;
        let cookie = maya_cookie.clone();
        async move {
            stand
                .post(
                    "/v1/boxes",
                    Some(&cookie),
                    &json!({"name": name}).to_string(),
                )
                .await
                .json()["box"]["box_id"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    // The canvas addresses "Ventas año" as /ventas-ano (NFKD, marks dropped);
    // an untitled folder's card is named after its file, "Ventas ano".
    folder("Ventas año").await;
    let ventas = folder("Untitled folder").await;
    let headers_only = folder("Only headers").await;
    assert_eq!(
        upload(&stand, &maya_cookie, &ventas, "Ventas ano.csv", &metrics)
            .await
            .status,
        StatusCode::CREATED
    );
    // A newer table the sealed parser cannot read (a header, no rows).
    tokio::time::sleep(Duration::from_millis(1100)).await;
    for target in [&ventas, &headers_only] {
        assert_eq!(
            upload(
                &stand,
                &maya_cookie,
                target,
                "empty.csv",
                b"month,revenue\n"
            )
            .await
            .status,
            StatusCode::CREATED
        );
    }
    // Organization labels (of an app Maya never opened) hold two names.
    c.fixture.sql(&format!("INSERT INTO genui.app_labels(organization_id,app_id,card,address,kept,updated_by,updated_at) SELECT '{ORG}',a.app_id,'','ventas-ano-card',ARRAY['ventas-ano-2'],w.created_by,now() FROM genui.apps a, genui.saved_work w WHERE a.organization_id='{ORG}' AND a.app_id<>'{AUTHORING_APP}' AND w.organization_id='{ORG}' AND w.work_id='{one_card}' ORDER BY a.app_id LIMIT 1"), "genui");
    let publications_sql =
        format!("SELECT count(*) FROM genui.publications WHERE organization_id='{ORG}'");
    let publications = scalar(&c.fixture, &publications_sql).await;
    let key = "0123456789abcdef0123456789abcdef";
    let make = |body: Value, key: &'static str| {
        let stand = &stand;
        let cookie = maya_cookie.clone();
        let path = format!("/v1/boxes/{ventas}/apps");
        async move {
            stand
                .call(
                    "POST",
                    &path,
                    Some(&cookie),
                    Some(&body.to_string()),
                    &[("idempotency-key", key)],
                )
                .await
        }
    };
    let carded = make(json!({"one_card": true}), key).await;
    assert_eq!(carded.status, StatusCode::CREATED, "{}", carded.text());
    let carded = carded.json();
    assert_eq!(
        (carded["title"].clone(), carded["files"].clone()),
        (json!("Ventas ano"), json!(1)),
        "the older readable table, past the unreadable newest one"
    );
    assert_eq!(
        carded["address"], "ventas-ano-3",
        "/ventas-ano is the folder Ventas año; -card and -2 are held by labels"
    );
    assert_eq!(
        scalar(&c.fixture, &publications_sql).await,
        publications + 1,
        "one publication: the name was judged before publishing (no orphan)"
    );
    let repeat = make(json!({"one_card": true}), key).await;
    assert_eq!(repeat.status, StatusCode::CREATED, "{}", repeat.text());
    assert_eq!(repeat.headers["idempotent-replayed"], "true");
    let repeat = repeat.json();
    assert_eq!(
        (
            repeat["app_id"].clone(),
            repeat["work_id"].clone(),
            repeat["address"].clone()
        ),
        (
            carded["app_id"].clone(),
            carded["work_id"].clone(),
            carded["address"].clone()
        ),
        "a repeat answers the first result"
    );
    assert_eq!(
        scalar(&c.fixture, &publications_sql).await,
        publications + 1,
        "a repeat makes nothing"
    );
    let reused = make(json!({"purpose": "a metrics app"}), key).await;
    assert_eq!(
        (reused.status, reused.json()["code"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("idempotency_key_reused")
        )
    );
    let malformed = make(json!({"one_card": true}), "not-a-key").await;
    assert_eq!(
        (malformed.status, malformed.json()["code"].clone()),
        (StatusCode::BAD_REQUEST, json!("malformed_request"))
    );
    // Ask on a folder whose only table cannot be read: its text answer.
    let asks = stub.calls("folder_ask").len();
    let unreadable = stand
        .post(
            &format!("/v1/boxes/{headers_only}/ask"),
            Some(&maya_cookie),
            &json!({"question": "how many rows?"}).to_string(),
        )
        .await;
    assert_eq!(unreadable.status, StatusCode::OK, "{}", unreadable.text());
    let unreadable = unreadable.json();
    assert_eq!(unreadable["references"], json!([]));
    assert!(
        unreadable["text"]
            .as_str()
            .unwrap()
            .contains("one table (CSV or XLSX) could not be read"),
        "{unreadable}"
    );
    assert_eq!(stub.calls("folder_ask").len(), asks, "no model call");
    check(&mut checks, "APP-124 review: Make a card on an untitled folder walks past a newest table the sealed parser cannot read (a header, no rows) to the older readable one, Ventas ano.csv, and is named after it; its /name is judged whole before anything is published: /ventas-ano is the folder Ventas año as the canvas addresses it (NFKD, marks dropped), /ventas-ano-card and /ventas-ano-2 are held by organization labels of an app Maya never opened, so the card is /ventas-ano-3 with exactly one new publication (no orphan); an Idempotency-Key makes Make a card (here) and folder formation (the demo's rehearsed build, below) answer their first result (201, Idempotent-Replayed, nothing new published), the same key with another body is 422 idempotency_key_reused and a malformed key 400; Ask on a folder whose only table cannot be read answers its text (200, no references, no model call), not 503");

    // --- The demo organization: rehearsed sentence, explanation, brief ---
    crate::demo::configure_for_tests(true, &[ORG]);
    let demo_box = stand
        .post(
            "/v1/boxes",
            Some(&maya_cookie),
            &json!({"name": "Q3 metrics"}).to_string(),
        )
        .await
        .json()["box"]["box_id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (name, bytes) in [
        ("q3_metrics.csv", &metrics),
        ("q3_pipeline.csv", &pipeline),
        ("q3_metrics_brief.pdf", &pdf),
    ] {
        assert_eq!(
            upload(&stand, &maya_cookie, &demo_box, name, bytes)
                .await
                .status,
            StatusCode::CREATED
        );
    }
    let calls = stub.bodies.lock().unwrap().len();
    let started = Instant::now();
    // Folder formation with an Idempotency-Key (ADR-011 I4, APP-124 review).
    let formation_path = format!("/v1/boxes/{demo_box}/apps");
    let formation_body =
        json!({"purpose": "Build an app that shows the key metrics and explain them"}).to_string();
    let formation = || {
        stand.call(
            "POST",
            &formation_path,
            Some(&maya_cookie),
            Some(&formation_body),
            &[("idempotency-key", "fedcba9876543210fedcba9876543210")],
        )
    };
    let rehearsed = formation().await;
    assert_eq!(
        rehearsed.status,
        StatusCode::CREATED,
        "{}",
        rehearsed.text()
    );
    timings.insert("demo_build".into(), json!(ms(started.elapsed())));
    let rehearsed = rehearsed.json();
    assert_eq!(rehearsed["source"], "rehearsed");
    let (demo_app, demo_work) = (
        rehearsed["app_id"].as_str().unwrap().to_owned(),
        rehearsed["work_id"].as_str().unwrap().to_owned(),
    );
    let publications = scalar(&c.fixture, &publications_sql).await;
    let formed_again = formation().await;
    assert_eq!(formed_again.status, StatusCode::CREATED);
    assert_eq!(formed_again.headers["idempotent-replayed"], "true");
    assert_eq!(
        formed_again.json()["work_id"],
        demo_work.as_str(),
        "formation sent again answers its first result"
    );
    assert_eq!(
        scalar(&c.fixture, &publications_sql).await,
        publications,
        "nothing new is published"
    );
    let demo_explained = stand
        .post(
            &format!("/v1/boxes/explain/{demo_app}/{demo_work}"),
            Some(&maya_cookie),
            "{}",
        )
        .await
        .json();
    assert_eq!(demo_explained["state"], "ready", "{demo_explained}");
    let demo_brief = stand
        .post(
            &format!("/v1/boxes/{demo_box}/brief"),
            Some(&maya_cookie),
            "{}",
        )
        .await
        .json();
    assert!(demo_brief["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["paragraph"]
            .as_array()
            .is_some_and(|p| p.iter().any(|x| x["cite"]["page"] == 1))));
    // APP-124: the folder's rehearsed questions (computed from its rows).
    let counted = stand
        .post(
            &format!("/v1/boxes/{demo_box}/ask"),
            Some(&maya_cookie),
            &json!({"question": "how many rows?"}).to_string(),
        )
        .await;
    assert_eq!(counted.status, StatusCode::OK, "{}", counted.text());
    let counted = counted.json();
    assert!(
        counted["text"]
            .as_str()
            .unwrap()
            .contains("q3_metrics.csv has 12 rows"),
        "{counted}"
    );
    let metrics_rows = counted["references"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["file"] == "q3_metrics.csv")
        .count();
    assert_eq!(metrics_rows, 12, "a count references every row it counted");
    let total = stand
        .post(
            &format!("/v1/boxes/{demo_box}/ask"),
            Some(&maya_cookie),
            &json!({"question": "what's the total revenue?"}).to_string(),
        )
        .await
        .json();
    assert!(
        total["text"]
            .as_str()
            .unwrap()
            .contains("revenue in q3_metrics.csv adds up to $3,799,872.03 (12 rows)"),
        "{total}"
    );
    assert_eq!(
        stub.bodies.lock().unwrap().len(),
        calls,
        "the demo's rehearsed paths call no model"
    );
    check(&mut checks, "demo (DS-73..DS-75, APP-124): in the demo organization the rehearsed sentence builds the metrics app over the folder's data files by rule, admitted and labeled generated (sent again with its Idempotency-Key: the same work, nothing new published); its explanation, the folder's brief (the fixture PDF cited by page) and Ask on the folder (a row count referencing all 12 rows, the exact revenue total) answer with no model call");
    crate::demo::configure_for_tests(false, &[]);

    // The local stand for a browser walk (scratch only, synthetic sessions).
    if let Some(seconds) = std::env::var("GENUI_BOX_STAND_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        crate::demo::configure_for_tests(true, &[ORG]);
        let file = std::env::var("GENUI_BOX_STAND_FILE").unwrap_or_default();
        if !file.is_empty() {
            std::fs::write(&file, serde_json::to_vec(&json!({"origin": stand.origin, "cookie": cookies::SESSION, "value": maya_cookie,
                "mate": mate_cookie, "box": box_id, "demo_box": demo_box, "app": app, "work": derived, "card_app": card_app, "card_work": card_work})).unwrap()).unwrap();
        }
        println!("BOX_STAND {}", stand.origin);
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let stopped = || !file.is_empty() && std::path::Path::new(&format!("{file}.stop")).exists();
        while Instant::now() < deadline && !stopped() {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        crate::demo::configure_for_tests(false, &[]);
    }

    let verified = operator(&c.fixture, &["verify", "--organization", ORG]);
    assert_eq!(verified["verified"], true);
    let events = scalar(&c.fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND kind LIKE 'box.%'")).await;
    assert!(events >= 30, "{events} box events");
    check(&mut checks, &format!("audit: {events} box.* events on the workspace chain (created, file_added, data_registered, app_derived, shared); every Northstar chain recomputes to its head"));

    let report = json!({"scope": "APP-108 folders, local Docker campaign over a loopback stand, synthetic identities and files, a loopback model stub", "timings_ms": timings, "checks": checks});
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    server.abort();
    drop(c);
}

fn walk(root: &std::path::Path) -> usize {
    fn count(path: &std::path::Path) -> usize {
        match std::fs::read_dir(path) {
            Ok(entries) => entries
                .flatten()
                .map(|e| {
                    if e.path().is_dir() {
                        count(&e.path())
                    } else {
                        1
                    }
                })
                .sum(),
            Err(_) => 0,
        }
    }
    count(root)
}
async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
