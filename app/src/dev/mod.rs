//! The local dev binary (ADR-012 D12.1): the product on 127.0.0.1 over this
//! checkout's Compose PostgreSQL, for iterating in a browser. Compiled only
//! with the `dev-stand` feature (`genui-dev`, `src/bin/genui-dev.rs`); the
//! Dockerfile never enables it, so no release build contains this module
//! (the ADR-007 D7.13 rule the qualification harness follows).
//!
//! It composes the binary's routes through `crate::product::routes` (the
//! function `main.rs` calls) with the binary's pages, over the real confined
//! query engine, a local object store and the sealed parser when `just
//! parser` built it. The model is the real Anthropic client when
//! `ANTHROPIC_API_KEY` is in this process's environment (`scripts/dev/
//! server.sh` reads it from the clone's `.env`); Slack mounts only with
//! `SLACK_BOT_TOKEN` and `SLACK_SIGNING_SECRET`.
//!
//! Two profiles, each its own instance (database volume, state directory,
//! ports), started by `scripts/dev/dev.sh` under process-compose:
//!
//! - `pitch`: Northstar (the demo organization, demo mode on) with the
//!   20-app library and its sources; Diego Ramos, Sarah Chen and Priya
//!   Natarajan, each with the library and an authoring app; Diego's expense
//!   claims and time off (one approved by Sarah, two waiting for her) and his
//!   "Q3 board pack" folder. Seeded once per database volume.
//! - `clean`: a new customer, Alex Rivera, in a separate (synthetic)
//!   organization: no library, no records, no folders, demo mode off. Only an
//!   authoring app, the app the composer drafts on (with no sources a
//!   generated draft is refused `not_runnable` today). Templates are an
//!   operator install on demand: `genui-dev seed-library` (`just dev clean
//!   seed-library`) runs the audited `seed-library` and
//!   `grant-library-access` operator commands.
//!
//! Sessions are minted directly (no sign-in provider; a deliberate,
//! labeled difference from beta): one loopback listener per persona adds
//! that persona's session cookie to each request (`personas`), each browser
//! on its own session. The data lives in the Compose volume and survives a
//! restart; sessions are minted again at every start (the vault is in
//! memory). When `app/admin/dist/manifest.json` changes (the admin watcher
//! rebuilt the front end) the process exits with code 75 and the supervisor
//! starts it again with the new assets.
//!
//! Commands: `genui-dev [serve]`, `genui-dev seed-library`. Configuration is
//! the environment `scripts/dev/env.sh` sets; nothing here prints a secret.
mod db;
mod personas;
mod seed;

use crate::{
    actions,
    auth::{Auth, Pages},
    boxes,
    commands::RecordPage,
    composer::Composer,
    imports::sandbox::ParserIdentity,
    pages::{
        DeniedProps, InviteProps, PendingProps, RecordProps, SigninProps, SsrPages, WorkspaceProps,
    },
    product,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        page::{SsrWorkPages, WorkProps},
        worker, Engine, EngineSettings, Executor, Work,
    },
    slack,
    storage::Stores,
    widgets,
};
use axum::{http::StatusCode, Router};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

/// Northstar, the demo organization (tenant_seed's first fixture organization).
pub(crate) const ORG: &str = "00000000-0000-4000-8000-000000000001";
/// The second fixture organization: the clean profile's customer.
pub(crate) const OTHER: &str = "00000000-0000-4000-8000-000000000002";
/// The fixture space the library and authoring apps live in.
pub(crate) const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const DIEGO: &str = "00000000-0000-4000-8000-00000000006f";
const SARAH: &str = "00000000-0000-4000-8000-00000000006d";
const PRIYA: &str = "00000000-0000-4000-8000-000000000066";
/// Sessions per persona: each browser gets its own (a browser keeps one live
/// stream per session; two browsers on one session replaced each other's).
pub(crate) const BROWSER_SLOTS: usize = 8;
/// The exit code that asks the supervisor for a restart (EX_TEMPFAIL).
const RELOAD: i32 = 75;

/// A persona: its name in the URLs and logs, principal, display name and
/// authoring app (`grant-library-access --authoring-app` / `create-app`).
pub(crate) struct Persona {
    pub key: &'static str,
    pub principal: &'static str,
    pub name: &'static str,
    pub authoring: &'static str,
}
const PITCH: [Persona; 3] = [
    Persona {
        key: "diego",
        principal: DIEGO,
        name: "Diego Ramos",
        authoring: "00000000-0000-4000-8000-00000000d0a1",
    },
    Persona {
        key: "sarah",
        principal: SARAH,
        name: "Sarah Chen",
        authoring: "00000000-0000-4000-8000-00000000d0a2",
    },
    Persona {
        key: "priya",
        principal: PRIYA,
        name: "Priya Natarajan",
        authoring: "00000000-0000-4000-8000-00000000d0a3",
    },
];
/// The clean profile's one persona: a new customer (an active member of the
/// second fixture organization with no app grants), under a new name.
const CLEAN: [Persona; 1] = [Persona {
    key: "alex",
    principal: DIEGO,
    name: "Alex Rivera",
    authoring: "00000000-0000-4000-8000-00000000d0a4",
}];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Profile {
    Pitch,
    Clean,
}
impl Profile {
    pub fn organization(self) -> &'static str {
        match self {
            Self::Pitch => ORG,
            Self::Clean => OTHER,
        }
    }
    pub fn personas(self) -> &'static [Persona] {
        match self {
            Self::Pitch => &PITCH,
            Self::Clean => &CLEAN,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Pitch => "pitch",
            Self::Clean => "clean",
        }
    }
}

/// The environment `scripts/dev/env.sh` sets for one instance.
pub(crate) struct Env {
    pub profile: Profile,
    /// `<checkout>/.cache/dev/<profile>` (0700): TLS material, credentials,
    /// registry, the revision cache, objects and scratch.
    pub state: PathBuf,
    pub db_port: u16,
    pub db_container: String,
    /// One loopback port per persona, in persona order.
    pub persona_ports: Vec<u16>,
    pub python: PathBuf,
}

pub(crate) fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app/ has a parent")
        .to_owned()
}

fn variable(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} is required (scripts/dev/dev.sh sets it)"))
}

impl Env {
    fn from_process() -> Result<Self, String> {
        let profile = match std::env::var("GENUI_DEV_PROFILE").as_deref() {
            Ok("pitch") | Err(_) => Profile::Pitch,
            Ok("clean") => Profile::Clean,
            Ok(_) => return Err("GENUI_DEV_PROFILE must be pitch or clean".into()),
        };
        let state = PathBuf::from(variable("GENUI_DEV_STATE")?);
        if !state.is_absolute() {
            return Err("GENUI_DEV_STATE must be an absolute path".into());
        }
        let db_port = variable("GENUI_DEV_DB_PORT")?
            .parse()
            .map_err(|_| "GENUI_DEV_DB_PORT must be a port")?;
        let db_container = variable("GENUI_DEV_DB_CONTAINER")?;
        if db_container.is_empty()
            || !db_container
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err("GENUI_DEV_DB_CONTAINER refused".into());
        }
        let persona_ports = variable("GENUI_DEV_PERSONA_PORTS")?
            .split(',')
            .map(|p| p.trim().parse::<u16>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "GENUI_DEV_PERSONA_PORTS must be a comma-separated port list")?;
        if persona_ports.len() != profile.personas().len() {
            return Err(format!(
                "GENUI_DEV_PERSONA_PORTS names {} ports; the {} profile has {} personas",
                persona_ports.len(),
                profile.name(),
                profile.personas().len()
            ));
        }
        let python = std::env::var_os("GENUI_DEV_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|| repository().join("app/runtime/.venv/bin/python"));
        if !python.exists() {
            return Err(format!("{} is missing: run `just setup`", python.display()));
        }
        Ok(Self {
            profile,
            state,
            db_port,
            db_container,
            persona_ports,
            python,
        })
    }
}

/// The binary's entry point (`src/bin/genui-dev.rs`).
pub fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    let result = Env::from_process().and_then(|env| match command.as_str() {
        "serve" => serve(env),
        "seed-library" => seed_library(env),
        _ => Err("usage: genui-dev [serve|seed-library]".into()),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("genui-dev: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))
}

/// `just dev clean seed-library`: install the template apps into the clean
/// customer's organization through the audited operator commands, then
/// build their sources into this instance's revision cache.
fn seed_library(env: Env) -> Result<(), String> {
    let db = db::Database::new(&env)?;
    db.prepare_files()?;
    let org = env.profile.organization();
    let seeded = db.operator(&["seed-library", "--organization", org])?;
    let apps = seeded["apps"].as_array().map(Vec::len).unwrap_or(0);
    db.library_sources(org)?;
    for persona in env.profile.personas() {
        db.operator(&[
            "grant-library-access",
            "--organization",
            org,
            "--principal",
            persona.principal,
            "--record-write",
            "--authoring-app",
            persona.authoring,
        ])?;
    }
    db.mark("library", "seeded")?;
    println!(
        "genui-dev: {apps} template apps installed in the {} organization (seed-library, grant-library-access; changed: {})",
        env.profile.name(),
        seeded["changed"]
    );
    Ok(())
}

fn serve(env: Env) -> Result<(), String> {
    let started = Instant::now();
    let db = db::Database::new(&env)?;
    db.bootstrap()?;
    db.profile_setup(env.profile)?;
    let epoch = db.epoch()?;
    // The demo switches and display names, from the environment as a
    // deployment sets them (read once, before any request).
    let organization = env.profile.organization();
    if env.profile == Profile::Pitch {
        std::env::set_var(crate::demo::MODE_ENV, "1");
        std::env::set_var(crate::demo::ORGANIZATIONS_ENV, organization);
    } else {
        std::env::remove_var(crate::demo::MODE_ENV);
        std::env::remove_var(crate::demo::ORGANIZATIONS_ENV);
    }
    let names: Vec<_> = env
        .profile
        .personas()
        .iter()
        .map(|p| serde_json::json!({"principal_id": p.principal, "display_name": p.name}))
        .collect();
    std::env::remove_var(crate::directory::FILE_ENV);
    std::env::set_var(
        crate::directory::JSON_ENV,
        serde_json::json!({ "members": names }).to_string(),
    );
    runtime()?.block_on(run(env, db, epoch, started))
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The sealed parser `just parser` builds into this checkout
/// (`GENUI_BOX_PARSER_DIR` overrides it); without it folders keep files but
/// parse no CSV/XLSX.
fn parser_identity() -> Option<ParserIdentity> {
    let dir = std::env::var_os("GENUI_BOX_PARSER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository().join(".cache/parser-target"));
    let host = dir.join("release/parser-host");
    let wasm = dir.join("wasm32-unknown-unknown/release/parser_guest.wasm");
    let (Ok(host_bytes), Ok(wasm_bytes)) = (std::fs::read(&host), std::fs::read(&wasm)) else {
        println!(
            "genui-dev: no sealed parser at {} (`just parser` builds it): CSV/XLSX are not parsed",
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

/// The served router: the binary's product routes plus its assets, the
/// retiring service worker and liveness, and `/` opening the canvas.
fn served(product_routes: Router, long: Vec<(&'static str, Router)>) -> Router {
    use axum::routing::get;
    crate::http::uniform_with_many(
        Router::new()
            .route(
                "/",
                get(|| async { axum::response::Redirect::to("/canvas") }),
            )
            .route("/health/live", get(|| async { StatusCode::OK }))
            .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
            .route("/sw.js", get(forma_server::sw::serve_sw::<Assets>))
            .route(
                "/_assets/{*filename}",
                get(forma_server::assets::serve_asset::<Assets>),
            )
            .merge(product_routes),
        long,
    )
}

fn private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{}: {e}", path.display()))
}

async fn run(env: Env, db: db::Database, epoch: String, started: Instant) -> Result<(), String> {
    let _ = tracing_subscriber::fmt().with_ansi(false).try_init();
    let organization = env.profile.organization();
    let stores = Arc::new(Stores::new(db.settings()).map_err(|e| format!("stores: {e}"))?);
    stores
        .readiness()
        .await
        .map_err(|e| format!("database readiness: {e}"))?;

    // --- The binary's pages ---
    let mut manifest = forma_server::assets::try_load_manifest::<Assets>()
        .map_err(|e| format!("app/admin/dist/manifest.json: {e} (the admin watcher builds it)"))?;
    manifest
        .routes
        .retain(|route, _| route != "/" && route != "/about");
    let modules = forma_server::load_ir_modules::<Assets>(&manifest)
        .map_err(|errors| format!("FMIR modules: {} errors", errors.len()))?;
    let bind_error = |errors: Vec<forma_server::PropsError>| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    };
    let ssr_pages = Arc::new(SsrPages {
        auth_origin: None,
        invite: modules.bind::<InviteProps>("invite").map_err(bind_error)?,
        signin: modules.bind::<SigninProps>("signin").map_err(bind_error)?,
        pending: modules
            .bind::<PendingProps>("pending")
            .map_err(bind_error)?,
        denied: modules.bind::<DeniedProps>("denied").map_err(bind_error)?,
        workspace: modules
            .bind::<WorkspaceProps>("workspace")
            .map_err(bind_error)?,
        record: modules.bind::<RecordProps>("record").map_err(bind_error)?,
    });
    let work_pages = Arc::new(SsrWorkPages {
        page: modules.bind::<WorkProps>("work").map_err(bind_error)?,
    });
    let canvas_pages = Arc::new(SsrCanvasPages {
        page: modules.bind::<CanvasProps>("canvas").map_err(bind_error)?,
    });
    modules.finalize().map_err(bind_error)?;
    forma_server::sw::configure(forma_server::sw::ServiceWorker::off())
        .map_err(|_| "service worker configuration".to_string())?;

    // --- The product ---
    let auth_pages: Arc<dyn Pages> = ssr_pages.clone();
    let auth = Arc::new(
        Auth::new(None, Some(stores.clone()), auth_pages).map_err(|e| format!("auth: {e}"))?,
    );
    for dir in ["cache", "scratch", "objects", "box-scratch"] {
        private_dir(&env.state.join(dir))?;
    }
    let runtime_py = repository()
        .join("app/runtime/runtime.py")
        .to_string_lossy()
        .into_owned();
    let engine = Engine::new(EngineSettings {
        program: env.python.clone(),
        args: vec!["-I".into(), runtime_py.clone(), "query".into()],
        cache_root: env.state.join("cache"),
        scratch_root: env.state.join("scratch"),
        fetch: None,
        backstop: worker::BACKSTOP,
        rss_limit_bytes: worker::RSS_LIMIT_BYTES,
    })
    .map_err(|_| "query engine directories refused".to_string())?;
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(Arc::new(engine) as Arc<dyn Executor>),
        work_pages,
    ));
    let composer = Composer::from_env();
    println!(
        "genui-dev: model {}",
        if composer.enabled() {
            "Anthropic (ANTHROPIC_API_KEY from the clone's .env)"
        } else {
            "none (no ANTHROPIC_API_KEY): rehearsed sentences only"
        }
    );
    let box_state = Arc::new(boxes::routes::Boxes {
        auth: auth.clone(),
        work: work.clone(),
        composer: composer.clone(),
        store: boxes::store::ObjectStore::Local {
            root: env.state.join("objects"),
        },
        normalizer: boxes::register::Normalizer {
            program: env.python.clone(),
            args: vec!["-I".into(), runtime_py, "normalize".into()],
            cache_root: env.state.join("cache"),
            scratch_root: env.state.join("box-scratch"),
        },
        parser: parser_identity(),
    });
    boxes::intent::install(box_state.clone());
    let slack = slack::Slack::from_env(auth.clone());
    if let Some(slack) = &slack {
        slack::notify::spawn(slack.clone());
        println!("genui-dev: slack routes mounted");
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

    // --- Sessions, and the example data once per database ---
    let minter = personas::Minter {
        auth: auth.clone(),
        stores: stores.clone(),
        organization,
    };
    let mut sessions = Vec::new();
    for persona in env.profile.personas() {
        let mut slots = Vec::with_capacity(BROWSER_SLOTS);
        for _ in 0..BROWSER_SLOTS {
            slots.push(minter.mint(persona.principal).await?);
        }
        sessions.push(slots);
    }
    if env.profile == Profile::Pitch && db.marked("examples")?.is_none() {
        seed::examples(&app, &sessions[0][0], &sessions[1][0]).await?;
        db.mark("examples", "seeded")?;
    }

    // ADR-006 expiry sweep, as the binary runs it: sessions from an earlier
    // start (their vault entries are gone) are removed.
    let sweeper = auth.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            sweeper.sweep().await;
        }
    });

    let clear_on_reset = (env.profile == Profile::Clean).then_some(epoch);
    let urls = personas::serve(
        app,
        minter,
        env.profile.personas(),
        &env.persona_ports,
        sessions,
        clear_on_reset,
    )
    .await?;
    println!(
        "genui-dev: {} ready in {} s",
        env.profile.name(),
        started.elapsed().as_secs()
    );
    for (persona, url) in env.profile.personas().iter().zip(urls) {
        println!("  {:<6} {:<16} {url}/canvas", persona.key, persona.name);
    }
    watch_manifest().await
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The front end is read at start (forma-server's manifest and FMIR
/// modules): when the admin watcher writes a new manifest, exit with 75 so
/// the supervisor starts this process again (no Rust rebuild, data kept).
async fn watch_manifest() -> Result<(), String> {
    let path = repository().join("app/admin/dist/manifest.json");
    let at_start = modified(&path);
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let now = modified(&path);
        if now.is_some() && now != at_start {
            // Let the watcher finish writing the modules it names.
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("genui-dev: the front end was rebuilt; restarting (exit {RELOAD})");
            std::process::exit(RELOAD);
        }
    }
}
