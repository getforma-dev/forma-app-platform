//! The local dev stand (ADR-011 pitch): the whole product on 127.0.0.1 over a
//! throwaway Docker PostgreSQL, for iterating in a browser instead of
//! deploying to beta. Isolated test executable only (`#[ignore]`); started
//! by `app/scripts/dev_stand.sh`, never by `cargo test` on its own.
//!
//! It composes the binary's routes through `crate::product::routes` (the
//! function `main.rs` calls) with the binary's pages (`crate::pages`), over
//! the real confined query engine and the library's normalized revisions,
//! a local object store and the sealed parser when it is built. The model
//! is the real Anthropic client when `ANTHROPIC_API_KEY` is in this
//! process's environment (otherwise none: the model routes answer
//! unavailable, as on a deployment without a key); Slack routes mount only
//! when `SLACK_BOT_TOKEN` and `SLACK_SIGNING_SECRET` are set.
//!
//! Seeded: Northstar (the demo organization, demo mode on, so rehearsed
//! sentences parse without a model) with the 20-app library and its
//! sources; personas Diego Ramos, Sarah Chen and Priya Natarajan, each with
//! the whole library (`grant-library-access --record-write`) and an
//! authoring app; Diego's expense claims (one approved by Sarah, one waiting
//! for her) and a time off request waiting for her; a "Q3 board pack"
//! folder of Diego's with `docs/pitch/folder-fixture`.
//!
//! The fresh profile (`GENUI_DEV_STAND_PROFILE=fresh`, `app/scripts/fresh_stand.sh`)
//! is a new customer instead: the same 20-app library, one persona (Alex
//! Rivera), no records, no folder and demo mode off (so nothing rehearsed:
//! sentences go to the model, as for any organization that is not the demo
//! one). Its proxy clears the browser's storage for its origin after each
//! reset, so the canvas starts empty too.
//!
//! Sessions are minted directly (no sign-in provider): the stand writes the
//! origin and one session cookie per persona to `GENUI_DEV_STAND_FILE`
//! (mode 0600, scratch only, never printed), keeps the sessions alive, and
//! re-mints one that ended (a sign-out) and rewrites the file. It serves
//! until `GENUI_DEV_STAND_SECONDS` (default 8 hours) pass or
//! `<file>.stop` exists; on the way out the container and the private
//! directories are removed.
use super::{
    actions_campaign::SAME_ORIGIN,
    admission_qualification::operator,
    command_qualification::{json_headers, send, vault_token, Reply},
    qualification::{fixture_python, Fixture},
    qualification_cases::{ORG, OTHER, PRINCIPAL, REVIEWER},
    Stores,
};
use crate::{
    actions,
    auth::{cookies, Auth, Pages},
    boxes,
    commands::RecordPage,
    composer::Composer,
    demo, directory,
    imports::sandbox::ParserIdentity,
    pages::{
        DeniedProps, InviteProps, PendingProps, RecordProps, SigninProps, SsrPages, WorkspaceProps,
    },
    product,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        page::{SsrWorkPages, WorkProps},
        tests::{real_engine, repository, Directory},
        Executor, Work,
    },
    slack, widgets,
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tower::ServiceExt;

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

/// The employee (tenant_seed actor 0x6f).
const DIEGO: &str = "00000000-0000-4000-8000-00000000006f";
/// One authoring app per persona (`grant-library-access --authoring-app`).
const AUTHORING: [(&str, &str); 3] = [
    (DIEGO, "00000000-0000-4000-8000-00000000d0a1"),
    (REVIEWER, "00000000-0000-4000-8000-00000000d0a2"),
    (PRINCIPAL, "00000000-0000-4000-8000-00000000d0a3"),
];
const FOLDER_FIXTURE: &str = "docs/pitch/folder-fixture";
const DEFAULT_PARSER: &str = "/private/tmp/claude-501/-Users-victorvillacis-dev/df0e31da-4f04-46a7-8043-30cdac91f9c2/scratchpad/parser-target";
/// Sessions per persona: the proxy gives each browser its own (a browser
/// keeps one live stream per session, so two browsers on one session
/// replaced each other's stream; production gives every browser its own).
const BROWSER_SLOTS: usize = 8;
/// How often the stand touches each persona's session (idle limit: 1 hour).
const KEEPALIVE: Duration = Duration::from_secs(60);

/// A persona: key (the proxy's name for it), principal, display name.
struct Persona {
    key: &'static str,
    principal: &'static str,
    name: &'static str,
}
const PERSONAS: [Persona; 3] = [
    Persona {
        key: "diego",
        principal: DIEGO,
        name: "Diego Ramos",
    },
    Persona {
        key: "sarah",
        principal: REVIEWER,
        name: "Sarah Chen",
    },
    Persona {
        key: "priya",
        principal: PRINCIPAL,
        name: "Priya Natarajan",
    },
];
/// The fresh profile's one persona: a new customer (Diego's principal, which
/// holds the library grants, under a new customer's name).
const FRESH: [Persona; 1] = [Persona {
    key: "alex",
    principal: DIEGO,
    name: "Alex Rivera",
}];

/// `GENUI_DEV_STAND_PROFILE=fresh`: a new customer's empty workspace.
fn fresh() -> bool {
    std::env::var("GENUI_DEV_STAND_PROFILE").as_deref() == Ok("fresh")
}

fn personas() -> &'static [Persona] {
    if fresh() {
        &FRESH
    } else {
        &PERSONAS
    }
}

#[test]
#[ignore = "the local dev stand: Docker, the locked Python environment; run by app/scripts/dev_stand.sh"]
fn native_dev_stand() {
    std::thread::Builder::new()
        .name("dev-stand".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(stand());
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

/// The sealed parser built from `parser/` (`GENUI_BOX_PARSER_DIR`), when it
/// is there; without it folders keep files but parse no CSV/XLSX.
fn parser_identity() -> Option<ParserIdentity> {
    let dir = PathBuf::from(
        std::env::var("GENUI_BOX_PARSER_DIR").unwrap_or_else(|_| DEFAULT_PARSER.into()),
    );
    let host = dir.join("release/parser-host");
    let wasm = dir.join("wasm32-unknown-unknown/release/parser_guest.wasm");
    let (Ok(host_bytes), Ok(wasm_bytes)) = (std::fs::read(&host), std::fs::read(&wasm)) else {
        println!(
            "DEV_STAND no sealed parser at {}: CSV/XLSX are not parsed",
            dir.display()
        );
        return None;
    };
    Some(ParserIdentity {
        host,
        host_sha256: hex(&host_bytes),
        wasm,
        wasm_sha256: hex(&wasm_bytes),
        manifest_sha256: hex(b"dev-stand"),
    })
}

/// Writes `bytes` to `path` atomically, readable by its owner only.
fn private_write(path: &Path, bytes: &[u8]) {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let temporary = path.with_extension("new");
    let _ = std::fs::remove_file(&temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    std::fs::rename(temporary, path).unwrap();
}

/// One multipart upload (one `file` part) through the router.
async fn upload(app: &Router, cookie: &str, box_id: &str, name: &str, bytes: &[u8]) -> Reply {
    let boundary = "----formadevstandboundary";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/v1/boxes/{box_id}/files"))
        .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION))
        .header("sec-fetch-site", "same-origin")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:4000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 22).await.unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

/// The served router: the binary's product routes plus its assets and
/// liveness, and `/` opening the canvas.
fn served(product_routes: Router, long: Vec<(&'static str, Router)>) -> Router {
    crate::http::uniform_with_many(
        Router::new()
            .route(
                "/",
                axum::routing::get(|| async { axum::response::Redirect::to("/canvas") }),
            )
            .route(
                "/health/live",
                axum::routing::get(|| async { StatusCode::OK }),
            )
            .route(
                "/favicon.ico",
                axum::routing::get(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/_assets/{*filename}",
                axum::routing::get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(product_routes),
        long,
    )
}

async fn stand() {
    let _ = tracing_subscriber::fmt().with_ansi(false).try_init();
    let started = Instant::now();
    let file = PathBuf::from(
        std::env::var("GENUI_DEV_STAND_FILE").expect("GENUI_DEV_STAND_FILE names the stand file"),
    );
    let seconds = std::env::var("GENUI_DEV_STAND_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8 * 3600);
    let stop = PathBuf::from(format!("{}.stop", file.display()));
    let _ = std::fs::remove_file(&stop);

    // --- The database, the library and its sources ---
    let fixture = Fixture::new();
    println!("DEV_STAND container {}", fixture.name);
    println!("DEV_STAND scratch {}", fixture.directory.0.display());
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    let seeded = operator(&fixture, &["seed-library", "--organization", ORG]);
    let apps = seeded["apps"].as_array().map(Vec::len).unwrap_or(0);
    assert_eq!(apps, 20, "the 20-app library is seeded");
    let python = fixture_python();
    let root = Directory::new("dev-stand");
    println!("DEV_STAND scratch {}", root.0.display());
    let built = std::process::Command::new(&python)
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
    for (principal, authoring) in AUTHORING {
        operator(
            &fixture,
            &[
                "grant-library-access",
                "--organization",
                ORG,
                "--principal",
                principal,
                "--record-write",
                "--authoring-app",
                authoring,
            ],
        );
    }

    // --- Demo organization and personas (in process, as the deployment's
    // environment sets them) ---
    let fresh = fresh();
    if fresh {
        // A new customer is not the demo organization: nothing rehearsed.
        std::env::remove_var(demo::MODE_ENV);
        std::env::remove_var(demo::ORGANIZATIONS_ENV);
        demo::configure_for_tests(false, &[]);
    } else {
        std::env::set_var(demo::MODE_ENV, "1");
        std::env::set_var(demo::ORGANIZATIONS_ENV, ORG);
        demo::configure_for_tests(true, &[ORG]);
    }
    directory::set_for_tests(
        &personas()
            .iter()
            .map(|p| (p.principal, p.name))
            .collect::<Vec<_>>(),
    );
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();

    // --- The binary's pages and routes ---
    let mut manifest =
        forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
    manifest
        .routes
        .retain(|route, _| route != "/" && route != "/about");
    let modules = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
    let ssr_pages = Arc::new(SsrPages {
        auth_origin: None,
        invite: modules.bind::<InviteProps>("invite").unwrap(),
        signin: modules.bind::<SigninProps>("signin").unwrap(),
        pending: modules.bind::<PendingProps>("pending").unwrap(),
        denied: modules.bind::<DeniedProps>("denied").unwrap(),
        workspace: modules.bind::<WorkspaceProps>("workspace").unwrap(),
        record: modules.bind::<RecordProps>("record").unwrap(),
    });
    let work_pages = Arc::new(SsrWorkPages {
        page: modules.bind::<WorkProps>("work").unwrap(),
    });
    let canvas_pages = Arc::new(SsrCanvasPages {
        page: modules.bind::<CanvasProps>("canvas").unwrap(),
    });
    modules.finalize().unwrap();
    let auth_pages: Arc<dyn Pages> = ssr_pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(engine as Arc<dyn Executor>),
        work_pages,
    ));
    let composer = Composer::from_env();
    println!(
        "DEV_STAND model {}",
        if composer.enabled() {
            "Anthropic (ANTHROPIC_API_KEY from the environment)"
        } else {
            "none (no ANTHROPIC_API_KEY): rehearsed sentences only"
        }
    );
    let box_state = Arc::new(boxes::routes::Boxes {
        auth: auth.clone(),
        work: work.clone(),
        composer: composer.clone(),
        store: boxes::store::ObjectStore::Local {
            root: root.0.join("objects"),
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
        parser: parser_identity(),
    });
    boxes::intent::install(box_state.clone());
    let slack = slack::Slack::from_env(auth.clone());
    if let Some(slack) = &slack {
        slack::notify::spawn(slack.clone());
        println!("DEV_STAND slack routes mounted");
    }
    let record_page: Arc<dyn RecordPage> = ssr_pages;
    let (product_routes, long) = product::routes(product::Product {
        auth: auth.clone(),
        work,
        record_page,
        canvas_pages,
        live_events: "/v1/events".into(),
        demo_organizations: canvas::demo_organizations(),
        composer,
        parser: actions::Parser::from_env(),
        slack,
        customizer: widgets::customize::Customizer::from_env(),
        weather: widgets::weather::OpenMeteo::new(),
        boxes: box_state,
    });
    let app = served(product_routes, long);

    // --- Sessions ---
    let mint = |principal: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth
                .vault
                .insert("northstar", &vault_token())
                .expect("vault capacity");
            stores
                .workspace
                .mint_session_for_tests(ORG, principal, &reference)
                .await
                .expect("dev stand session (are the stand credentials current?)")
        }
    };
    let mut sessions = Vec::new();
    for persona in personas() {
        sessions.push(mint(persona.principal).await);
    }
    if !fresh {
        seed_examples(&app, sessions[0].clone(), sessions[1].clone()).await;
    } else {
        println!("DEV_STAND fresh: a new customer, no records, no folder, demo mode off");
    }

    // --- Serve ---
    serve(
        &file, seconds, &stop, started, app, &fixture, &mint, sessions,
    )
    .await;
    demo::configure_for_tests(false, &[]);
    directory::set_for_tests(&[]);
    stores.close().await.unwrap();
    drop(fixture);
}

/// The example data of the pitch's stand: Diego's records (one approved by
/// Sarah, two waiting for her) and his "Q3 board pack" folder.
async fn seed_examples(app: &Router, diego: String, sarah: String) {
    let app = app.clone();

    // --- A few existing records: one approved, two waiting for Sarah ---
    let act = |cookie: &str, body: Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        async move {
            let reply = send(
                &app,
                "POST",
                actions::PATH,
                Some(&cookie),
                &body.to_string(),
                &SAME_ORIGIN,
            )
            .await;
            assert_eq!(reply.status, StatusCode::OK, "{body}: {}", reply.body);
            reply.json()
        }
    };
    let confirm = |cookie: &str, draft: Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        async move {
            let request = &draft["command_request"];
            let key = request["idempotency_key"].as_str().unwrap().to_owned();
            let uri = format!(
                "/v1/work/{}/{}/commands/{}",
                draft["app_id"].as_str().unwrap(),
                draft["work_id"].as_str().unwrap(),
                draft["command_id"].as_str().unwrap()
            );
            let reply = send(
                &app,
                "POST",
                &uri,
                Some(&cookie),
                &request.to_string(),
                &json_headers(&key),
            )
            .await;
            assert_eq!(reply.status, StatusCode::OK, "{uri}: {}", reply.body);
            reply.json()["record_id"].as_str().unwrap().to_owned()
        }
    };
    let share = |cookie: &str, draft: Value| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        async move {
            let uri = format!(
                "/v1/work/{}/{}/share",
                draft["app_id"].as_str().unwrap(),
                draft["work_id"].as_str().unwrap()
            );
            let mut approvers: Vec<String> = draft["approvers"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|a| a["principal_id"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            if approvers.is_empty() {
                approvers.push(REVIEWER.to_owned());
            }
            for principal in approvers {
                let body =
                    json!({"grantee_principal_id": principal, "record_write": true}).to_string();
                // A work the filer did not create (the library's seeded work,
                // which every persona holds by `grant-library-access`) is not
                // his to share: the approver already reads it.
                let reply = send(&app, "POST", &uri, Some(&cookie), &body, &SAME_ORIGIN).await;
                assert!(
                    reply.status == StatusCode::OK || reply.status == StatusCode::NOT_FOUND,
                    "{uri}: {}",
                    reply.body
                );
            }
        }
    };
    let file_and_share = |sentence: &'static str| {
        let (act, confirm, share, diego) = (&act, &confirm, &share, diego.clone());
        async move {
            let draft = act(&diego, json!({"utterance": sentence})).await;
            let record = confirm(&diego, draft.clone()).await;
            share(&diego, draft.clone()).await;
            (draft, record)
        }
    };
    let (lunch, lunch_record) = file_and_share(
        "I spent $42 on a team lunch at Sweetgreen yesterday, Sarah approves my expenses",
    )
    .await;
    let decision = act(
        &sarah,
        json!({"decide": {"app_id": lunch["app_id"], "work_id": lunch["work_id"], "record_id": lunch_record, "status": "approved"}}),
    )
    .await;
    confirm(&sarah, decision).await;
    file_and_share("I spent $389 on a flight at Delta yesterday, Sarah approves my expenses").await;
    file_and_share("Book me off Nov 24–26 for Thanksgiving, Sarah approves").await;
    println!("DEV_STAND records: one approved expense, an expense and a time off request waiting for Sarah");

    // --- A sample folder with the pitch's folder fixture ---
    let created = send(
        &app,
        "POST",
        "/v1/boxes",
        Some(&diego),
        &json!({"name": "Q3 board pack"}).to_string(),
        &SAME_ORIGIN,
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let box_id = created.json()["box"]["box_id"].as_str().unwrap().to_owned();
    let fixture_dir = repository().join(FOLDER_FIXTURE);
    for name in ["q3_metrics.csv", "q3_pipeline.csv", "q3_metrics_brief.pdf"] {
        let bytes = std::fs::read(fixture_dir.join(name)).unwrap();
        let reply = upload(&app, &diego, &box_id, name, &bytes).await;
        assert!(
            reply.status.is_success(),
            "{name}: {} {}",
            reply.status,
            reply.body
        );
    }
    println!("DEV_STAND folder \"Q3 board pack\" (Diego) with {FOLDER_FIXTURE}");
}

/// Serves until the deadline or the stop file, keeping the sessions alive.
#[allow(clippy::too_many_arguments)]
async fn serve<M, F>(
    file: &Path,
    seconds: u64,
    stop: &Path,
    started: Instant,
    app: Router,
    fixture: &Fixture,
    mint: &M,
    sessions: Vec<String>,
) where
    M: Fn(&'static str) -> F,
    F: std::future::Future<Output = String>,
{
    let listener = tokio::net::TcpListener::bind((
        "127.0.0.1",
        std::env::var("GENUI_DEV_STAND_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(0),
    ))
    .await
    .unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    // Each persona's first session is the one the examples were seeded with;
    // the others are for more browsers.
    let mut slots: Vec<Vec<String>> = Vec::new();
    for (persona, first) in personas().iter().zip(sessions) {
        let mut slot = vec![first];
        for _ in 1..BROWSER_SLOTS {
            slot.push(mint(persona.principal).await);
        }
        slots.push(slot);
    }
    let write_file = |slots: &[Vec<String>]| {
        let personas: serde_json::Map<String, Value> = personas()
            .iter()
            .zip(slots)
            .map(|(p, sessions)| {
                (
                    p.key.to_owned(),
                    json!({"name": p.name, "principal_id": p.principal, "session": sessions[0], "sessions": sessions}),
                )
            })
            .collect();
        private_write(
            file,
            &serde_json::to_vec_pretty(&json!({
                "origin": origin,
                "cookie": cookies::SESSION,
                "organization_id": ORG,
                "container": fixture.name,
                "pid": std::process::id(),
                "personas": personas,
            }))
            .unwrap(),
        );
    };
    write_file(&slots);
    let server_app = app.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            server_app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await;
    });
    println!(
        "DEV_STAND ready {origin} in {} s; serving for {seconds} s",
        started.elapsed().as_secs()
    );
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut touched = Instant::now();
    while Instant::now() < deadline && !stop.exists() {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if touched.elapsed() < KEEPALIVE {
            continue;
        }
        touched = Instant::now();
        // Production's sidecar rewrites the short-lived credentials file; the
        // stand does too, or it refuses everything ten minutes in (2026-10-31).
        fixture.refresh_credentials();
        // Keep each session inside its idle limit; one that ended (a
        // sign-out in the browser) is minted again and the file rewritten.
        let mut changed = false;
        for (persona, sessions) in personas().iter().zip(slots.iter_mut()) {
            for session in sessions.iter_mut() {
                let reply = send(&app, "GET", "/v1/members", Some(session), "", &[]).await;
                if reply.status != StatusCode::OK {
                    *session = mint(persona.principal).await;
                    changed = true;
                    println!("DEV_STAND session for {} minted again", persona.name);
                }
            }
        }
        if changed {
            write_file(&slots);
        }
    }
    println!("DEV_STAND stopping");
    server.abort();
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_file(stop);
}
