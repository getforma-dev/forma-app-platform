use axum::{extract::State, http::StatusCode, response::Response, routing::get, Router};
use forma_ir::PageProps;
use forma_server::{
    assets, load_ir_modules, render_browser_shell, sw, PageConfig, PageHandle, RenderCtx, RouteMode,
};
use genui::auth::{self, Auth, AuthConfig};
use genui::pages::{
    respond, DeniedProps, InviteProps, PendingProps, RecordProps, SigninProps, SsrPages,
    WorkspaceProps,
};
use rust_embed::Embed;
use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Embed)]
#[folder = "admin/dist/"]
struct Assets;

#[derive(PageProps)]
struct HomeProps {
    count: u32,
}
#[derive(PageProps)]
struct AboutProps {}

struct AppState {
    manifest: forma_server::AssetManifest,
    readiness: Arc<genui::readiness::Readiness>,
    home: Option<PageHandle<HomeProps>>,
    about: Option<PageHandle<AboutProps>>,
}

fn refuse(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
fn bind_errors<T>(errors: Vec<forma_server::PropsError>) -> T {
    for error in errors {
        eprintln!("{error}");
    }
    std::process::exit(1);
}

/// ADR-007 D7.13: the operator qualification harness exists only in the
/// isolated qualification executable (`--features qualification-harness`);
/// a release binary refuses its configuration instead of starting.
#[cfg(not(feature = "qualification-harness"))]
fn qualification_router() -> Option<Router> {
    if std::env::var_os("GENUI_AUTH_QUALIFICATION_CONFIG_FILE").is_some() {
        refuse("auth qualification harness is not part of this binary (ADR-007 D7.13)");
    }
    None
}
#[cfg(feature = "qualification-harness")]
fn qualification_router() -> Option<Router> {
    std::env::var_os("GENUI_AUTH_QUALIFICATION_CONFIG_FILE").map(|_| {
        genui::auth_qualification::from_environment().unwrap_or_else(|reason| {
            refuse(format!(
                "auth qualification configuration refused: {reason}"
            ))
        })
    })
}

#[tokio::main]
async fn main() {
    // The image names its parser bytes in parser-manifest.json and sets this
    // variable; startup refuses if the guest or host on disk differ (ADR-009
    // D9.1, integrity only). Without it no parse can run.
    if std::env::var_os(genui::imports::sandbox::MANIFEST_ENV).is_some() {
        if let Err(reason) = genui::imports::sandbox::verify_manifest() {
            refuse(format!("parser manifest refused: {reason}"));
        }
    }
    let credential_path =
        std::env::var("GENUI_RUNTIME_CONFIG_FILE").expect("GENUI_RUNTIME_CONFIG_FILE is required");
    let config = genui::config::Config::load(std::path::Path::new(&credential_path))
        .unwrap_or_else(|reason| refuse(format!("configuration refused: {reason}")));
    // A missing registry permits the inert public shell, but never readiness or
    // private data. The managed launcher supplies an explicit empty deny-all
    // registry until real provider bindings are provisioned and reviewed.
    let stores = match std::env::var("GENUI_IDENTITY_REGISTRY_FILE") {
        Ok(registry_path) => {
            let settings = genui::storage::Settings::new(
                credential_path.clone().into(),
                registry_path.into(),
                std::env::var("GENUI_RDS_CA_FILE")
                    .unwrap_or_else(|_| "/etc/ssl/certs/rds-global-bundle.pem".into())
                    .into(),
                config.database_host.clone(),
            );
            Some(Arc::new(
                settings
                    .and_then(genui::storage::Stores::new)
                    .unwrap_or_else(|_| refuse("tenant storage configuration refused")),
            ))
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => refuse("tenant storage configuration refused"),
    };
    drop(config);
    // ADR-006 provider routing; absent: sign-in fails neutrally and every
    // protected route stays the uniform 404.
    let auth_config = match std::env::var(auth::CONFIG_ENV) {
        Ok(path) => Some(
            AuthConfig::load(std::path::Path::new(&path)).unwrap_or_else(|reason| refuse(reason)),
        ),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => refuse("auth configuration file refused"),
    };
    // ADR-007 D7.14: the load balancer's peers; APP-073's private provider
    // invitation capabilities (absent: no code obtains a signup handoff).
    let proxies = genui::http::proxy::TrustedProxies::from_env().unwrap_or_else(|r| refuse(r));
    let invitations = auth::invite::Invitations::from_env().unwrap_or_else(|r| refuse(r));
    // A render that fails comes back to `home` as a coded error, which `home`
    // logs through `tracing`; forma-server's own security events go there too.
    // Without a subscriber those lines go nowhere. Keep this.
    tracing_subscriber::fmt::init();

    // Check if frontend has been built
    if Assets::get("manifest.json").is_none() {
        eprintln!("\n  Error: Frontend not built yet.\n");
        eprintln!("  Run these commands first:");
        eprintln!("    cd admin");
        eprintln!("    npm ci");
        eprintln!("    npm run build\n");
        std::process::exit(1);
    }

    let manifest = assets::try_load_manifest::<Assets>()
        .unwrap_or_else(|error| refuse(format!("\n  Error: admin/dist/manifest.json: {error}\n")));
    for route in [
        "/",
        "/about",
        "/invite",
        "/signin",
        "/auth/pending",
        "/auth/callback",
        "/workspace",
        "/record",
        "/w/{app}/{work}",
        "/canvas",
    ] {
        require_ssr(manifest.route(route).map(|entry| entry.mode))
            .unwrap_or_else(|reason| panic!("required SSR route {route}: {reason}"));
    }
    // Every module the build declared, or no server: a module that is
    // missing, corrupt or from another format version stops the server here,
    // before the port binds, naming its route, file and code.
    let pages = load_ir_modules::<Assets>(&manifest).unwrap_or_else(|errors| {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    });
    // Compare the declared page props once, then finalize before opening a listener.
    let home_page = match manifest.route("/").map(|route| route.mode) {
        Some(RouteMode::Ssr) => Some(pages.bind::<HomeProps>("home").unwrap_or_else(bind_errors)),
        Some(RouteMode::Browser) => None,
        None => refuse("missing declared route /"),
    };
    let about_page = match manifest.route("/about").map(|route| route.mode) {
        Some(RouteMode::Ssr) => Some(
            pages
                .bind::<AboutProps>("about")
                .unwrap_or_else(bind_errors),
        ),
        Some(RouteMode::Browser) => None,
        None => refuse("missing declared route /about"),
    };
    let auth_pages = Arc::new(SsrPages {
        auth_origin: auth_config.as_ref().map(|c| c.auth_origin.clone()),
        invite: pages
            .bind::<InviteProps>("invite")
            .unwrap_or_else(bind_errors),
        signin: pages
            .bind::<SigninProps>("signin")
            .unwrap_or_else(bind_errors),
        pending: pages
            .bind::<PendingProps>("pending")
            .unwrap_or_else(bind_errors),
        denied: pages
            .bind::<DeniedProps>("denied")
            .unwrap_or_else(bind_errors),
        workspace: pages
            .bind::<WorkspaceProps>("workspace")
            .unwrap_or_else(bind_errors),
        record: pages
            .bind::<RecordProps>("record")
            .unwrap_or_else(bind_errors),
    });
    // APP-012: the work page.
    let work_page = Arc::new(genui::query::page::SsrWorkPages {
        page: pages
            .bind::<genui::query::page::WorkProps>("work")
            .unwrap_or_else(bind_errors),
    });
    // ADR-011 (APP-092): the pitch canvas.
    let canvas_page = Arc::new(genui::query::canvas::SsrCanvasPages {
        page: pages
            .bind::<genui::query::canvas::CanvasProps>("canvas")
            .unwrap_or_else(bind_errors),
    });
    pages.finalize().unwrap_or_else(|errors| {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    });

    // No service worker: no page registers one, and /sw.js serves
    // forma-server's retiring worker, which deletes the caches an earlier
    // Forma worker left on this origin and unregisters itself. To turn a
    // worker on, see `forma_server::sw`.
    sw::configure(sw::ServiceWorker::off()).expect("configured once, before the first page");

    // D7.13: decided once startup state exists and always before the listener
    // binds (where the APP-006 harness was constructed): a release binary
    // refuses the operator qualification configuration here.
    let qualification = qualification_router();
    let auth = Arc::new(
        Auth::new(auth_config, stores.clone(), auth_pages.clone())
            .unwrap_or_else(|reason| refuse(reason))
            .with_trusted_proxies(proxies)
            .with_invitations(invitations),
    );
    // APP-012 (ADR-007 D7.10): the confined query engine beside the data
    // store, in the image's private cache and scratch under /run/genui.
    let engine = stores.as_ref().map(|_| {
        Arc::new(
            genui::query::Engine::new(genui::query::EngineSettings::image())
                .unwrap_or_else(|_| refuse("query engine directories refused")),
        ) as Arc<dyn genui::query::Executor>
    });
    let work = Arc::new(genui::query::Work::new(auth.clone(), engine, work_page));
    // APP-108 (ADR-011 pitch): folders. Objects through the credentialed
    // `runtime.py object` child under the tenant role, the normalizer child,
    // the engine cache, and the sealed parser the image verified at start.
    let box_state = Arc::new(genui::boxes::routes::Boxes {
        auth: auth.clone(),
        work: work.clone(),
        composer: genui::composer::Composer::from_env(),
        store: genui::boxes::store::ObjectStore::image(),
        normalizer: genui::boxes::register::Normalizer::image(),
        parser: if std::env::var_os(genui::imports::sandbox::MANIFEST_ENV).is_some() {
            genui::imports::sandbox::verify_manifest().ok()
        } else {
            None
        },
    });
    genui::boxes::intent::install(box_state.clone());
    // APP-100 (ADR-011 pitch): Slack, from SLACK_BOT_TOKEN and
    // SLACK_SIGNING_SECRET alone (absent: no route is mounted, the paths are
    // the uniform 404, and nothing posts). The approver notifier follows the
    // event hub for the life of the process.
    let slack = genui::slack::Slack::from_env(auth.clone());
    if let Some(slack) = &slack {
        genui::slack::notify::spawn(slack.clone());
    }
    // The product routes, composed once for the binary and the local dev
    // stand (`genui::product`). The model keys come from the environment
    // alone (absent: the model routes answer unavailable, nothing else
    // changes). The organization's realtime stream is named by
    // GENUI_CANVAS_EVENTS_URL (absent: the island subscribes to nothing).
    let (product_routes, long_routes) = genui::product::routes(genui::product::Product {
        auth: auth.clone(),
        work,
        // APP-013: the command route and the record page share the session,
        // vault and synchronizer key with the ADR-006 routes.
        record_page: auth_pages,
        canvas_pages: canvas_page,
        live_events: std::env::var("GENUI_CANVAS_EVENTS_URL")
            .ok()
            .filter(|url| url.starts_with("/v1/"))
            .unwrap_or_default(),
        demo_organizations: genui::query::canvas::demo_organizations(),
        composer: genui::composer::Composer::from_env(),
        parser: genui::actions::Parser::from_env(),
        slack,
        customizer: genui::widgets::customize::Customizer::from_env(),
        weather: genui::widgets::weather::OpenMeteo::new(),
        boxes: box_state,
    });
    let readiness = genui::readiness::Readiness::start(
        credential_path.into(),
        std::env::var("GENUI_HELPER_PATH")
            .unwrap_or_else(|_| "/app/runtime.py".into())
            .into(),
        std::env::var("GENUI_PYTHON").unwrap_or_else(|_| "python3".into()),
        stores,
    );

    let state = Arc::new(AppState {
        manifest,
        readiness,
        home: home_page,
        about: about_page,
    });

    let app = Router::new()
        .route("/", get(home))
        .route("/health/live", get(|| async { StatusCode::OK }))
        .route(
            "/health/ready",
            get(|State(state): State<Arc<AppState>>| async move {
                if state.readiness.ready() {
                    StatusCode::OK
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        )
        .route("/about", get(about))
        .route("/favicon.ico", get(favicon))
        .route("/sw.js", get(sw::serve_sw::<Assets>))
        .route("/_assets/{*filename}", get(assets::serve_asset::<Assets>))
        .with_state(state);
    // The operator harness (feature builds only) replaces the product routes
    // for its isolated run; the two never share a router.
    // APP-076 G8: the uniform 404 for unknown paths and method mismatches
    // alike (no `Allow`), and the D7.14 handler deadline; the long-deadline
    // paths (model waits, the canvas's library reads) under their own.
    let app = match qualification {
        Some(harness) => genui::http::uniform(app.merge(harness)),
        None => genui::http::uniform_with_many(app.merge(product_routes), long_routes),
    };

    // ADR-006 expiry sweep: idle and expired records are destroyed and their
    // provider refresh tokens revoked.
    let sweeper = auth.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            sweeper.sweep().await;
        }
    });

    // Port zero asks the OS for an available port; useful for isolated tests.
    let port = match std::env::var("FORMA_PORT") {
        Ok(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value
            .parse::<u16>()
            .unwrap_or_else(|_| refuse("FORMA_PORT must be an integer from 0 to 65535")),
        Err(std::env::VarError::NotPresent) => 3000,
        _ => refuse("FORMA_PORT must be an integer from 0 to 65535"),
    };
    let bind_address = match std::env::var("FORMA_BIND_ADDRESS").as_deref() {
        Ok("0.0.0.0") => std::net::Ipv4Addr::UNSPECIFIED,
        Ok("127.0.0.1") | Err(_) => std::net::Ipv4Addr::LOCALHOST,
        _ => refuse("FORMA_BIND_ADDRESS must be 127.0.0.1 or 0.0.0.0"),
    };
    let listener = tokio::net::TcpListener::bind((bind_address, port))
        .await
        .unwrap();
    // Emit readiness only while this process owns the successfully bound socket.
    println!("FORMA_LISTENING http://{}", listener.local_addr().unwrap());
    // D7.14: the peer address reaches the trusted-proxy extraction.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn home(State(state): State<Arc<AppState>>) -> Response {
    let route = "/";
    // Route mode is fixed at startup; an SSR failure never selects a shell.
    let ctx = RenderCtx::new().with_title("Home");
    let rendered = match &state.home {
        Some(home) => home.render(&HomeProps { count: 7 }, &ctx),
        None => render_browser_shell(
            &PageConfig {
                title: "Home",
                route_pattern: route,
                manifest: &state.manifest,
                config_json: None,
                body_class: None,
                personality_css: None,
                body_prefix: None,
            },
            &ctx,
        ),
    };
    respond(route, rendered)
}

async fn about(State(state): State<Arc<AppState>>) -> Response {
    let route = "/about";
    // Route mode is fixed at startup; an SSR failure never selects a shell.
    let ctx = RenderCtx::new().with_title("About");
    let rendered = match &state.about {
        Some(about) => about.render(&AboutProps {}, &ctx),
        None => render_browser_shell(
            &PageConfig {
                title: "About",
                route_pattern: route,
                manifest: &state.manifest,
                config_json: None,
                body_class: None,
                personality_css: None,
                body_prefix: None,
            },
            &ctx,
        ),
    };
    respond(route, rendered)
}

async fn favicon() -> StatusCode {
    StatusCode::NO_CONTENT
}

// A missing or Browser output must not become a successful application shell.
fn require_ssr(mode: Option<RouteMode>) -> Result<(), &'static str> {
    match mode {
        Some(RouteMode::Ssr) => Ok(()),
        Some(RouteMode::Browser) => Err("unexpected Browser output"),
        None => Err("missing route"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::Pages as _;
    use axum::body::to_bytes;
    use axum::http::header;
    use genui::auth::{
        DeniedView, InviteView, PendingView, ResendResult, SigninView, WorkspaceView,
    };
    use genui::commands::{self, Notice, RecordPage as _, RecordView};

    #[test]
    fn required_native_routes_refuse_browser_and_missing_outputs() {
        assert!(require_ssr(Some(RouteMode::Ssr)).is_ok());
        assert_eq!(
            require_ssr(Some(RouteMode::Browser)),
            Err("unexpected Browser output")
        );
        assert_eq!(require_ssr(None), Err("missing route"));
    }

    const AUTH_ORIGIN: &str = "https://auth.getforma.dev";
    fn policy(headers: &axum::http::HeaderMap) -> String {
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .to_owned()
    }
    fn form_action(headers: &axum::http::HeaderMap) -> String {
        policy(headers)
            .split(';')
            .map(str::trim)
            .find(|d| d.starts_with("form-action"))
            .unwrap()
            .to_owned()
    }

    fn pages() -> SsrPages {
        let manifest = assets::try_load_manifest::<Assets>().expect("admin/dist built");
        for route in [
            "/invite",
            "/signin",
            "/auth/pending",
            "/auth/callback",
            "/workspace",
            "/record",
        ] {
            require_ssr(manifest.route(route).map(|entry| entry.mode)).unwrap();
        }
        let pages = load_ir_modules::<Assets>(&manifest).unwrap();
        let ssr = SsrPages {
            auth_origin: Some(AUTH_ORIGIN.into()),
            invite: pages.bind::<InviteProps>("invite").unwrap(),
            signin: pages.bind::<SigninProps>("signin").unwrap(),
            pending: pages.bind::<PendingProps>("pending").unwrap(),
            denied: pages.bind::<DeniedProps>("denied").unwrap(),
            workspace: pages.bind::<WorkspaceProps>("workspace").unwrap(),
            record: pages.bind::<RecordProps>("record").unwrap(),
        };
        // Home and about stay unbound here; finalize judges only bound routes.
        let _ = pages.bind::<HomeProps>("home");
        let _ = pages.bind::<AboutProps>("about");
        let _ = pages.bind::<genui::query::page::WorkProps>("work");
        let _ = pages.bind::<genui::query::canvas::CanvasProps>("canvas");
        pages.finalize().unwrap();
        ssr
    }
    async fn html(response: Response) -> (StatusCode, axum::http::HeaderMap, String) {
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    }

    /// APP-074: the invitation entry renders natively with no script and
    /// `no-store`; one labeled, keyboard-reachable code field posts to
    /// `/invite/validate`; the neutral refusal and the limit are announced
    /// and tied to the field; no code value is ever rendered.
    #[tokio::test]
    async fn invite_page_renders_entry_refusal_and_limit_without_javascript() {
        let pages = pages();
        let (status, headers, body) = html(pages.invite(&InviteView::default())).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
        assert_eq!(
            form_action(&headers),
            format!("form-action 'self' {AUTH_ORIGIN}"),
            "the 303 to the auth origin is a hop of this form's submission"
        );
        assert!(body.contains("action=\"/invite/validate\""));
        assert!(body.contains("method=\"post\""));
        assert!(body.contains("for=\"invite-code\"") && body.contains("id=\"invite-code\""));
        assert!(body.contains("name=\"code\"") && body.contains("autocomplete=\"off\""));
        assert!(!body.contains("<script src") && !body.contains("value=\""));
        assert!(!body.contains("role=\"alert\""));
        assert!(body.contains("aria-describedby=\"invite-help\""));
        let (_, _, refused) = html(pages.invite(&InviteView {
            failed: true,
            ..InviteView::default()
        }))
        .await;
        assert!(refused.contains("role=\"alert\"") && refused.contains("could not be used"));
        assert!(refused.contains("aria-describedby=\"invite-error\""));
        assert!(refused.contains("id=\"invite-error\"") && refused.contains("<form"));
        // APP-074 review m4: each limit names its window; an outage is a page.
        for (window, says) in [
            (auth::InviteLimit::Minute, "in the last minute"),
            (auth::InviteLimit::Day, "today. Try again tomorrow"),
            (auth::InviteLimit::Service, "across the service today"),
        ] {
            let (_, _, limited) = html(pages.invite(&InviteView {
                limited: Some(window),
                ..InviteView::default()
            }))
            .await;
            assert!(limited.contains("Too many attempts") && limited.contains("role=\"alert\""));
            assert!(limited.contains(says), "{window:?}");
            assert!(limited.contains("aria-describedby=\"invite-error\""));
            assert!(
                !limited.contains("could not be used") && !limited.contains("Wait a few minutes")
            );
        }
        let (_, _, unavailable) = html(pages.invite(&InviteView {
            unavailable: true,
            ..InviteView::default()
        }))
        .await;
        assert!(
            unavailable.contains("unavailable right now") && unavailable.contains("role=\"alert\"")
        );
        assert!(unavailable.contains("<form") && !unavailable.contains("Too many attempts"));
    }

    /// The four APP-009 pages render natively with their typed props, no
    /// script, a fresh nonce policy and `no-store`; forms carry exactly the
    /// synchronizer token and post to the ADR-006 routes.
    #[tokio::test]
    async fn auth_pages_render_typed_props_without_javascript() {
        let pages = pages();
        let (status, headers, body) = html(pages.signin(&SigninView {
            action: Some("/signin/mrswmz3infvgw3dnnzxxa4lsom".into()),
            failed: false,
        }))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
        // APP-009 review M1: the sign-in form's 303 goes to the auth origin.
        assert_eq!(
            form_action(&headers),
            format!("form-action 'self' {AUTH_ORIGIN}")
        );
        assert!(policy(&headers).contains("frame-ancestors 'none'"));
        assert!(body.contains("action=\"/signin/mrswmz3infvgw3dnnzxxa4lsom\""));
        assert!(body.contains("method=\"post\""));
        assert!(!body.contains("<script src"));
        assert!(!body.contains("could not continue"));
        let (_, _, body) = html(pages.signin(&SigninView {
            action: None,
            failed: true,
        }))
        .await;
        assert!(body.contains("could not continue") && !body.contains("<form"));
        let (_, pending_headers, body) = html(pages.pending(&PendingView {
            csrf_resend: "R".repeat(43),
            csrf_signout: "S".repeat(43),
            resend_available: true,
            cooldown_seconds: 0,
            hours_remaining: 23,
            result: Some(ResendResult::Requested),
        }))
        .await;
        assert!(body.contains("action=\"/auth/pending/resend\""));
        assert_eq!(
            form_action(&pending_headers),
            "form-action 'self'",
            "same-origin forms keep the strict default"
        );
        assert!(body.contains(&format!("value=\"{}\"", "R".repeat(43))));
        assert!(body.contains(&format!("value=\"{}\"", "S".repeat(43))));
        assert!(body.contains(">23<") && body.contains("was requested"));
        assert!(!body.contains("could not be requested"));
        let (_, _, body) = html(pages.pending(&PendingView {
            csrf_resend: "R".repeat(43),
            csrf_signout: "S".repeat(43),
            resend_available: false,
            cooldown_seconds: 41,
            hours_remaining: 0,
            result: Some(ResendResult::Unavailable),
        }))
        .await;
        assert!(body.contains(">41<") && body.contains("could not be requested"));
        assert!(!body.contains("/auth/pending/resend") && !body.contains("is held"));
        let (_, _, body) = html(pages.denied(&DeniedView {
            csrf_signout: "D".repeat(43),
        }))
        .await;
        assert!(body.contains("action=\"/auth/signout\"") && body.contains(&"D".repeat(43)));
        let (_, _, body) = html(pages.workspace(&WorkspaceView {
            csrf_signout: "W".repeat(43),
            csrf_signout_all: "A".repeat(43),
            items: Vec::new(),
        }))
        .await;
        assert!(body.contains("action=\"/auth/signout-all\"") && body.contains(&"A".repeat(43)));
        // APP-076 G9: an empty listing is the neutral line and no list.
        assert!(body.contains("Nothing to open yet.") && !body.contains("class=\"openable\""));
        assert!(!body.contains("<script src"));
        let work = "/w/00000000-0000-4000-8000-000000000065/00000000-0000-4000-8000-0000000000f2";
        let record = "/work/00000000-0000-4000-8000-0000000000c9/00000000-0000-4000-8000-0000000000cd/records/00000000-0000-4000-8000-0000000000ce/00000000-0000-4000-8000-0000000013b1";
        let (status, _, body) = html(pages.workspace(&WorkspaceView {
            csrf_signout: "W".repeat(43),
            csrf_signout_all: "A".repeat(43),
            items: vec![
                auth::WorkspaceItem {
                    title: "Northstar <operations>".into(),
                    href: work.into(),
                    records: Vec::new(),
                },
                auth::WorkspaceItem {
                    title: "Spend Review".into(),
                    href: record.split("/records/").next().unwrap().replacen("/work/", "/w/", 1),
                    records: vec![auth::RecordLink {
                        label: "New decision record".into(),
                        href: record.into(),
                    }],
                },
            ],
        }))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.contains("Nothing to open yet."));
        assert!(body.contains(&format!("href=\"{work}\"")));
        // ADR-011: each work opens on the canvas too.
        assert!(body.contains("href=\"/canvas\"") && body.contains("Open the canvas"));
        assert!(body.contains("href=\"/canvas#g-00000000-0000-4000-8000-0000000000f2\""));
        assert!(
            body.contains(&format!("href=\"{record}\"")) && body.contains("New decision record")
        );
        // Titles are text, never markup.
        assert!(body.contains("Northstar &lt;operations&gt;") && !body.contains("<operations>"));
        assert_eq!(
            body.matches(
                "Seeded definition, retrieved from the library. Sources seeded by the operator."
            )
            .count(),
            2
        );
        assert!(!body.contains("<script src"));
    }

    /// APP-013 (D7.16): the record page renders its field rows and a form
    /// carrying exactly the per-render key, the token and the premise as
    /// hidden fields, without JavaScript; a reader without record_write sees
    /// no form; a notice flag shows its one message.
    #[tokio::test]
    async fn record_page_renders_rows_and_the_keyed_form_without_javascript() {
        let pages = pages();
        let row = |id: &str, kind: &str, value: &str, options: &str| commands::FieldRow {
            id: id.into(),
            kind: kind.into(),
            value: value.into(),
            input: format!("value.{id}"),
            spec: format!("{{\"type\":\"{kind}\"}}"),
            spec_input: format!("spec.{id}"),
            options: options.into(),
            nullable: false,
            writable: true,
        };
        let view = RecordView {
            app: "00000000-0000-4000-8000-0000000000c9".into(),
            work: "00000000-0000-4000-8000-0000000000cd".into(),
            collection: "00000000-0000-4000-8000-0000000000ce".into(),
            record_id: "00000000-0000-4000-8000-0000000000f0".into(),
            schema: "decision".into(),
            schema_version: "1".into(),
            publication: "00000000-0000-4000-8000-0000000000cc".into(),
            work_version: "1".into(),
            version: "1".into(),
            exists: true,
            fields: vec![
                row(
                    "status",
                    "enum",
                    "needs_context",
                    "needs_context | reviewed",
                ),
                row("note", "text", "Review required", ""),
            ],
            command: "record_decision".into(),
            action: "/v1/work/00000000-0000-4000-8000-0000000000c9/work/commands/record_decision"
                .into(),
            can_write: true,
            csrf: "C".repeat(43),
            idempotency_key: "0123456789abcdef0123456789abcdef".into(),
            notice: Some(Notice::Conflict),
        };
        let (status, headers, body) = html(pages.record(&view)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        assert!(!body.contains("<script src"));
        assert!(body.contains("needs_context | reviewed") && body.contains("Review required"));
        assert!(body.contains("name=\"value.status\"") && body.contains("name=\"spec.note\""));
        assert_eq!(body.matches("0123456789abcdef0123456789abcdef").count(), 1);
        assert!(body.contains(&format!("value=\"{}\"", "C".repeat(43))));
        assert!(
            body.contains("name=\"expected_record_version\" value=\"1\"")
                || body.contains("value=\"1\" name=\"expected_record_version\"")
        );
        assert!(body.contains("The record changed before your command"));
        assert!(!body.contains("was committed"));
        let mut reader = view.clone();
        reader.can_write = false;
        reader.notice = None;
        let (_, _, body) = html(pages.record(&reader)).await;
        assert!(!body.contains("<form") && body.contains("not change it"));
        assert!(!body.contains("The record changed"));
        // APP-013 review m2 and m3: the refused notice; a field the offered
        // command does not write is text, not an input.
        let mut narrow = view.clone();
        narrow.notice = Some(Notice::Refused);
        narrow.fields[1].writable = false;
        let (_, _, body) = html(pages.record(&narrow)).await;
        assert!(
            body.contains("The command was refused") && !body.contains("could not be confirmed")
        );
        assert!(body.contains("name=\"value.status\"") && !body.contains("name=\"value.note\""));
        assert!(!body.contains("name=\"spec.note\""));
    }

    /// APP-012: the native work page renders each view state from typed
    /// props, with no script; only `ready` shows a value, `ready` and
    /// `empty` show their receipt, and a failed, denied, loading or idle view
    /// shows its message and nothing that could be read as a number.
    #[tokio::test]
    async fn work_page_renders_every_view_state_without_a_fabricated_zero() {
        use genui::query::page::{self as work, Outcome, Reason, SsrWorkPages, WorkPages as _};
        use genui::query::{QueryReceipt, WorkerResult};
        use serde_json::json;
        let manifest = assets::try_load_manifest::<Assets>().expect("admin/dist built");
        require_ssr(manifest.route("/w/{app}/{work}").map(|entry| entry.mode)).unwrap();
        let pages = load_ir_modules::<Assets>(&manifest).unwrap();
        let ssr = SsrWorkPages {
            page: pages.bind::<work::WorkProps>("work").unwrap(),
        };
        // The other pages are bound only so finalize judges the whole build.
        let _ = pages.bind::<HomeProps>("home");
        let _ = pages.bind::<AboutProps>("about");
        let _ = pages.bind::<SigninProps>("signin");
        let _ = pages.bind::<PendingProps>("pending");
        let _ = pages.bind::<DeniedProps>("denied");
        let _ = pages.bind::<WorkspaceProps>("workspace");
        let _ = pages.bind::<InviteProps>("invite");
        let _ = pages.bind::<RecordProps>("record");
        let _ = pages.bind::<genui::query::canvas::CanvasProps>("canvas");
        pages.finalize().unwrap();
        let view = |id: &str, kind: &str| genui::storage::WorkView {
            id: id.into(),
            title: id.into(),
            kind: kind.into(),
            integer_columns: Vec::new(),
            column_types: Default::default(),
        };
        let result = |columns: &[&str], rows: Vec<Vec<serde_json::Value>>| WorkerResult {
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            integer_columns: columns
                .iter()
                .filter(|c| matches!(**c, "total_bytes" | "bytes"))
                .map(|c| (*c).to_owned())
                .collect(),
            column_types: Default::default(),
            result_digest: genui::query::worker::result_digest(&rows).unwrap(),
            rows,
            truncated_page: false,
            engine_elapsed_ms: 0,
            wall: Duration::ZERO,
            peak_rss_bytes: 0,
        };
        let receipt = |result: &WorkerResult, digest: &str| QueryReceipt {
            digest: digest.into(),
            document: json!({"digest": digest, "result_digest": result.result_digest, "view_id": "total",
                "source_revisions": [], "parameters": [], "provenance": {}}),
        };
        let total = result(&["total_bytes"], vec![vec![json!("1140000")]]);
        let nothing = result(&["total_bytes"], vec![vec![serde_json::Value::Null]]);
        let buckets = result(
            &["minute", "destination", "bytes"],
            vec![vec![
                json!("2026-10-22T10:00:00Z"),
                json!("203.0.113.9"),
                json!("60000"),
            ]],
        );
        // APP-093 review M4: a money column is shown as currency, its exact
        // minor units kept as the hover title.
        let mut spend = result(
            &["claims", "total_amount"],
            vec![vec![json!("240"), json!("26262004")]],
        );
        spend.column_types = [
            (
                "claims".to_owned(),
                genui::query::worker::ColumnType::Integer,
            ),
            (
                "total_amount".to_owned(),
                genui::query::worker::ColumnType::Money {
                    currency: "USD".into(),
                    minor_unit_exponent: 2,
                },
            ),
        ]
        .into();
        let (ready_receipt, empty_receipt, timeline_receipt, spend_receipt) = (
            receipt(&total, "sha256:receipt-ready"),
            receipt(&nothing, "sha256:receipt-empty"),
            receipt(&buckets, "sha256:receipt-timeline"),
            receipt(&spend, "sha256:receipt-spend"),
        );
        let cards = vec![
            work::card(
                &view("ready", "summary"),
                Outcome::Released(&total, &ready_receipt),
            ),
            work::card(
                &view("spend", "summary"),
                Outcome::Released(&spend, &spend_receipt),
            ),
            work::card(
                &view("timeline", "timeline"),
                Outcome::Released(&buckets, &timeline_receipt),
            ),
            work::card(
                &view("empty", "summary"),
                Outcome::Released(&nothing, &empty_receipt),
            ),
            work::card(
                &view("failed", "summary"),
                Outcome::Failed(Reason::Deadline),
            ),
            work::card(
                &view("unavailable", "summary"),
                Outcome::Failed(Reason::SourceUnavailable),
            ),
            work::card(&view("denied", "summary"), Outcome::Denied),
            work::card(&view("loading", "summary"), Outcome::Loading),
            work::card(&view("idle", "summary"), Outcome::Idle),
        ];
        let outline = genui::storage::WorkOutline {
            title: "Northstar egress".into(),
            description: String::new(),
            views: vec![],
            definition_digest: String::new(),
        };
        let form = work::FormValues {
            window_start: "2026-10-22T10:00".into(),
            window_end: "2026-10-22T10:10".into(),
            destination: String::new(),
        };
        let props = work::props(
            &outline,
            "/w/a/w".into(),
            "Q".repeat(43),
            &form,
            false,
            cards,
        );
        let (status, headers, body) = html(ssr.work(&props)).await;
        // Evidence: the rendered page of every state (synthetic props).
        if let Some(path) = std::env::var_os("GENUI_WORK_PAGE_SNAPSHOT") {
            std::fs::write(path, &body).unwrap();
        }
        assert_eq!(status, StatusCode::OK);
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
        assert!(!body.contains("<script src"));
        assert!(body.contains("action=\"/w/a/w\"") && body.contains("method=\"post\""));
        assert!(body.contains(&format!("value=\"{}\"", "Q".repeat(43))));
        assert!(body.contains("value=\"2026-10-22T10:00\""));
        assert!(!body.contains("was not accepted"));
        let section = |id: &str| -> String {
            let start = body
                .find(&format!("data-view=\"{id}\""))
                .unwrap_or_else(|| panic!("{id}"));
            let rest = &body[start..];
            rest[..rest.find("</section>").unwrap()].to_owned()
        };
        let ready = section("ready");
        assert!(ready.contains("data-state=\"ready\"") && ready.contains(">1,140,000<"));
        assert!(ready.contains("sha256:receipt-ready") && ready.contains("<details"));
        assert!(ready.contains(&total.result_digest));
        let spend = section("spend");
        assert!(spend.contains(">$262,620.04<") && spend.contains("title=\"26262004\""));
        assert!(!spend.contains(">26,262,004<"));
        let timeline = section("timeline");
        assert!(timeline.contains("<table") && timeline.contains(">60,000<"));
        assert!(timeline.contains("title=\"60000\""));
        assert!(timeline.contains(">minute<") && timeline.contains("sha256:receipt-timeline"));
        let empty = section("empty");
        assert!(empty.contains("data-state=\"empty\"") && empty.contains("No rows matched"));
        assert!(empty.contains("sha256:receipt-empty"));
        for (id, state, words) in [
            ("empty", "empty", "No rows matched"),
            ("failed", "failed", "No number is shown"),
            ("unavailable", "failed", "source revision is not available"),
            ("denied", "denied", "Nothing is shown"),
            ("loading", "loading", "Nothing is shown until"),
            ("idle", "idle", "Not run yet"),
        ] {
            let html = section(id);
            assert!(html.contains(&format!("data-state=\"{state}\"")), "{id}");
            assert!(html.contains(words), "{id}");
            assert!(!html.contains("class=\"value\""), "{id} shows a value");
            assert!(!html.contains(">0<") && !html.contains("1,140,000"), "{id}");
            if id != "empty" {
                assert!(!html.contains("<details"), "{id} shows a receipt");
            }
        }
        // The queued-out card keeps the `loading` state key but is labelled
        // busy: nothing is loading on a no-JS page (APP-012 review m7).
        let loading = section("loading");
        assert!(loading.contains("<span class=\"badge\">busy</span>"));
        assert!(!loading.contains("<span class=\"badge\">loading</span>"));
        assert!(section("idle").contains("<span class=\"badge\">idle</span>"));
        // A refused window echoes the form, says nothing ran, and runs nothing.
        let mut refused = props.clone();
        refused.form_error = true;
        refused.views = vec![work::card(&view("total", "summary"), Outcome::Idle)];
        let (_, _, body) = html(ssr.work(&refused)).await;
        assert!(body.contains("was not accepted") && !body.contains("class=\"value\""));
    }

    /// ADR-011 (APP-092, APP-094): the canvas page renders every group, its
    /// app card, its view cards with their values and always-visible receipt
    /// identities, and its record form, in the HTML itself (the island only
    /// adds pan/zoom and runs views again); a failed or denied card shows no
    /// number; the page ships the one canvas island and `no-store`.
    #[tokio::test]
    async fn canvas_page_renders_groups_cards_and_receipts_without_javascript() {
        use genui::query::canvas::{self, CanvasPages as _, SsrCanvasPages};
        use genui::query::page::{self as work, Outcome, Reason};
        use genui::query::{QueryReceipt, WorkerResult};
        use serde_json::json;
        let manifest = assets::try_load_manifest::<Assets>().expect("admin/dist built");
        require_ssr(manifest.route("/canvas").map(|entry| entry.mode)).unwrap();
        let pages = load_ir_modules::<Assets>(&manifest).unwrap();
        let ssr = SsrCanvasPages {
            page: pages.bind::<canvas::CanvasProps>("canvas").unwrap(),
        };
        let _ = pages.bind::<HomeProps>("home");
        let _ = pages.bind::<AboutProps>("about");
        let _ = pages.bind::<SigninProps>("signin");
        let _ = pages.bind::<PendingProps>("pending");
        let _ = pages.bind::<DeniedProps>("denied");
        let _ = pages.bind::<WorkspaceProps>("workspace");
        let _ = pages.bind::<InviteProps>("invite");
        let _ = pages.bind::<RecordProps>("record");
        let _ = pages.bind::<work::WorkProps>("work");
        pages.finalize().unwrap();
        let (app, northstar, spend) = (
            "00000000-0000-4000-8000-000000000065",
            "00000000-0000-4000-8000-0000000000f2",
            "00000000-0000-4000-8000-0000000000cd",
        );
        let view = |id: &str, kind: &str, integers: &[&str]| genui::storage::WorkView {
            id: id.into(),
            title: id.into(),
            kind: kind.into(),
            integer_columns: integers.iter().map(|c| (*c).to_owned()).collect(),
            column_types: Default::default(),
        };
        let result = |columns: &[&str], rows: Vec<Vec<serde_json::Value>>| WorkerResult {
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            integer_columns: columns
                .iter()
                .filter(|c| matches!(**c, "total_bytes" | "bytes"))
                .map(|c| (*c).to_owned())
                .collect(),
            column_types: Default::default(),
            result_digest: genui::query::worker::result_digest(&rows).unwrap(),
            rows,
            truncated_page: false,
            engine_elapsed_ms: 0,
            wall: Duration::ZERO,
            peak_rss_bytes: 0,
        };
        let receipt = |result: &WorkerResult, digest: &str| QueryReceipt {
            digest: digest.into(),
            document: json!({"digest": digest, "result_digest": result.result_digest,
                "mode": "published", "principal_id": "00000000-0000-4000-8000-0000000000aa",
                "membership_epoch": 4, "policy_epoch": 1,
                "source_revisions": [{"input_id": "events", "source_id": "s", "source_revision_id": "r", "source_digest": "sha256:source"}],
                "parameters": [{"field_id": "window", "value": {"type": "text", "value": "2026-10-22T10:00:00Z/2026-10-22T10:10:00Z"}}],
                "provenance": {}}),
        };
        let total = result(&["total_bytes"], vec![vec![json!("1140000")]]);
        let buckets = result(
            &["minute", "destination", "bytes"],
            vec![
                vec![
                    json!("2026-10-22T10:00:00Z"),
                    json!("203.0.113.9"),
                    json!("60000"),
                ],
                vec![
                    json!("2026-10-22T10:10:00Z"),
                    json!("198.51.100.99"),
                    json!("900000"),
                ],
            ],
        );
        let (total_receipt, timeline_receipt) = (
            receipt(&total, "sha256:receipt-total"),
            receipt(&buckets, "sha256:receipt-timeline"),
        );
        let card = |v: &genui::storage::WorkView, outcome| {
            canvas::canvas_card(app, northstar, v, work::card(v, outcome))
        };
        let (total_view, timeline_view, failed_view, denied_view) = (
            view("total", "summary", &["total_bytes"]),
            view("timeline", "timeline", &["bytes"]),
            view("failed", "summary", &["total_bytes"]),
            view("denied", "summary", &["total_bytes"]),
        );
        let field = |id: &str, kind: &str, options: &str, writable: bool| commands::FieldRow {
            id: id.into(),
            kind: kind.into(),
            value: String::new(),
            input: format!("value.{id}"),
            spec: format!("{{\"type\":\"{kind}\"}}"),
            spec_input: format!("spec.{id}"),
            options: options.into(),
            nullable: false,
            writable,
        };
        let record = RecordView {
            app: app.into(),
            work: spend.into(),
            collection: "00000000-0000-4000-8000-0000000000ce".into(),
            record_id: "00000000-0000-4000-8000-0000000013b1".into(),
            schema: "decision".into(),
            schema_version: "1".into(),
            publication: "00000000-0000-4000-8000-0000000000cc".into(),
            work_version: "1".into(),
            version: "0".into(),
            exists: false,
            fields: vec![
                field("status", "enum", "needs_context | reviewed", true),
                field("note", "text", "", true),
            ],
            command: "record_decision".into(),
            action: format!("/v1/work/{app}/{spend}/commands/record_decision"),
            can_write: true,
            csrf: "C".repeat(43),
            idempotency_key: "0123456789abcdef0123456789abcdef".into(),
            notice: None,
        };
        let group = |work: &str, title: &str| canvas::GroupProps {
            id: format!("g-{work}"),
            app: app.into(),
            work: work.into(),
            app_card: format!("a-{work}"),
            title: title.into(),
            label: canvas::SEEDED_LABEL.into(),
            origins: canvas::seeded_origins(),
            href: format!("/w/{app}/{work}"),
            csrf: "Q".repeat(43),
            ..canvas::GroupProps::default()
        };
        let mut first = group(northstar, "Northstar <egress>");
        first.window_start = "2026-10-22T10:00:00".into();
        first.restored = true;
        first.has_cards = true;
        first.cards = vec![
            card(&total_view, Outcome::Released(&total, &total_receipt)),
            card(
                &timeline_view,
                Outcome::Released(&buckets, &timeline_receipt),
            ),
            card(&failed_view, Outcome::Failed(Reason::Deadline)),
            card(&denied_view, Outcome::Denied),
        ];
        let mut second = group(spend, "Spend Review");
        second.has_forms = true;
        second.forms = canvas::form_card(spend, &record).into_iter().collect();
        let groups = vec![first, second];
        let props = canvas::CanvasProps {
            csrf_signout: "S".repeat(43),
            has_groups: true,
            summary: canvas::summary(&groups),
            groups,
            live_events: "/v1/events".into(),
            has_live: true,
            has_organization: true,
            organization: "Northstar <Labs>".into(),
            member_label: "Member ba92f4c5".into(),
            member_initials: "BA".into(),
            demo: true,
            demo_mode: "on".into(),
            ..canvas::CanvasProps::default()
        };
        // APP-104: the library pane, metadata only: categories, numbered
        // apps and cards (a library app by its capability prefix; any other
        // work under "My apps").
        let library = canvas::library(vec![
            canvas::LibrarySource {
                app: app.into(),
                work: spend.into(),
                title: "Spend <Review>".into(),
                grouping: None,
                cards: vec![(format!("a-{spend}"), "App card".into(), "app".into())],
            },
            canvas::LibrarySource {
                app: app.into(),
                work: northstar.into(),
                title: "Northstar egress".into(),
                grouping: Some((
                    2,
                    "outbound-traffic".into(),
                    "security".into(),
                    "traffic".into(),
                )),
                cards: vec![
                    (format!("a-{northstar}"), "App card".into(), "app".into()),
                    (
                        format!("c-{northstar}-total"),
                        "Total".into(),
                        "number".into(),
                    ),
                ],
            },
        ]);
        let props = canvas::CanvasProps {
            has_library: true,
            library_summary: canvas::library_summary(&library),
            library,
            member_key: canvas::member_key("org", "principal"),
            ..props
        };
        let response = ssr.canvas(&props);
        let (status, headers, body) = html(response).await;
        if let Some(path) = std::env::var_os("GENUI_CANVAS_PAGE_SNAPSHOT") {
            std::fs::write(path, &body).unwrap();
        }
        assert_eq!(status, StatusCode::OK);
        assert!(headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store"));
        assert!(headers.contains_key(header::CONTENT_SECURITY_POLICY));
        // The content is in the HTML: everything asserted below sits in the
        // body before the one module script (the island's).
        let start = body.find("<body").unwrap();
        let end = body.find("<script type=\"module\"").unwrap();
        assert_eq!(body.matches("<script type=\"module\"").count(), 1);
        // Text read as a person sees it: the native renderer's hydration
        // markers (`<!--f:…-->`) are removed.
        let mut content = String::new();
        let mut rest = &body[start..end];
        while let Some(open) = rest.find("<!--") {
            content.push_str(&rest[..open]);
            rest = &rest[open..];
            rest = &rest[rest.find("-->").map_or(rest.len(), |close| close + 3)..];
        }
        content.push_str(rest);
        let content = content.as_str();
        assert!(content.contains("2 apps · 5 cards"));
        assert!(content.contains("Northstar &lt;egress&gt;") && !content.contains("<egress>"));
        // The honest provenance: origin chips, the sentence as their title and
        // for screen readers.
        assert_eq!(content.matches(canvas::SEEDED_LABEL).count(), 4);
        assert_eq!(content.matches(">Retrieved from library<").count(), 2);
        // The top bar: the organization (escaped; no picker), the member's
        // avatar, and the live chip the island reveals once subscribed.
        assert!(content.contains(">Northstar &lt;Labs&gt;<") && !content.contains("<Labs>"));
        assert!(content.contains(">BA<") && content.contains("aria-label=\"Member ba92f4c5\""));
        assert!(
            content.contains("id=\"cv-live\"")
                && content.contains("data-live-events=\"/v1/events\"")
        );
        // GENUI_DEMO_MODE: the small-print synthetic-data label and the
        // island's demo-safe layout flag.
        assert!(content.contains("Synthetic data") && content.contains("data-demo=\"on\""));
        assert!(content.contains(&format!("id=\"g-{northstar}\"")));
        assert!(content.contains(&format!("action=\"/w/{app}/{northstar}\"")));
        assert!(content.contains("value=\"2026-10-22T10:00:00\""));
        let section = |id: &str| -> String {
            let start = content
                .find(&format!("data-view=\"{id}\""))
                .unwrap_or_else(|| panic!("{id}"));
            let rest = &content[start..];
            rest[..rest.find("</article>").unwrap()].to_owned()
        };
        let ready = section("total");
        // One insight, compact with its unit; the exact value beside it.
        assert!(ready.contains("data-state=\"ready\"") && ready.contains(">1.14 MB<"));
        assert!(ready.contains(">1,140,000 bytes<") && ready.contains("title=\"1,140,000 bytes\""));
        assert!(ready.contains(&format!(
            "data-query=\"/v1/work/{app}/{northstar}/queries/total\""
        )));
        assert!(ready.contains("data-integers=\"total_bytes\""));
        // The receipt is always on the card: its compact line (seal, digest
        // prefix, source count, time) is the <details> summary, visible
        // closed; opening it shows every field in full, without JavaScript.
        let receipt = &ready[ready.find("class=\"cv-receipt\"").expect("receipt")..];
        let line = &receipt[..receipt.find("</summary>").unwrap()];
        assert!(
            line.contains(">Verified<")
                && line.contains(">receipt-<")
                && line.contains(">1 source<")
        );
        assert!(line.contains("title=\"sha256:receipt-total\"") && line.contains(" UTC<"));
        assert!(receipt.contains("Membership epoch") && receipt.contains(&total.result_digest));
        assert!(
            receipt.contains("Source revision (events)") && receipt.contains("Parameter window")
        );
        assert!(receipt.contains("Receipt document (JSON)"));
        // The timeline is a server-rendered SVG chart, its rows folded beneath.
        let timeline = section("timeline");
        assert!(
            timeline.contains("<svg class=\"cv-chart-svg\"")
                && timeline.contains("viewBox=\"0 0 560 188\"")
        );
        assert!(
            timeline.contains("class=\"cv-bar is-peak\"")
                && timeline
                    .contains("<title>Oct 22, 10:10 · 198.51.100.99 · 900,000 bytes</title>")
        );
        assert!(timeline.contains("Oct 22, 10:00–10:10 UTC. Peak 900 KB at 10:10."));
        assert!(timeline.contains("<details class=\"cv-rows\"") && timeline.contains("<table"));
        assert!(timeline.contains(">60 KB<") && timeline.contains("title=\"60,000 bytes\""));
        assert!(
            timeline.contains(">Oct 22, 10:00<") && timeline.contains("sha256:receipt-timeline")
        );
        for (id, state) in [("failed", "failed"), ("denied", "denied")] {
            let html = section(id);
            assert!(html.contains(&format!("data-state=\"{state}\"")), "{id}");
            assert!(
                !html.contains("value\"") && !html.contains("cv-receipt") && !html.contains("<svg"),
                "{id}"
            );
            assert!(
                !html.contains("1,140,000") && !html.contains("MB") && !html.contains(">0<"),
                "{id}"
            );
        }
        // The record form posts to the audited command route, keyed, without JS.
        assert!(content.contains(&format!(
            "action=\"/v1/work/{app}/{spend}/commands/record_decision\""
        )));
        assert_eq!(
            content.matches("0123456789abcdef0123456789abcdef").count(),
            1
        );
        assert!(content.contains("name=\"value.status\"") && content.contains(">reviewed<"));
        assert!(content.contains("name=\"spec.note\"") && content.contains("New decision record"));
        assert!(content.contains(">Record decision</button>"));
        assert!(content.contains(&format!("value=\"{}\"", "C".repeat(43))));
        assert!(content.contains("action=\"/auth/signout\"") && content.contains(&"S".repeat(43)));
        // No organization picker; the intent bar posts nowhere but the page.
        assert!(!content.to_lowercase().contains("organization picker"));
        assert!(content.contains("What do you need?"));
        // APP-124: the empty canvas's three ways to start; the third is a
        // file picker (and a drop target) that makes a folder of the files.
        assert!(
            content.contains("data-empty-describe") && content.contains("Describe what you need")
        );
        assert!(
            content.contains("Drop files or upload") && content.contains("data-empty-upload-pick")
        );
        assert!(content.contains("type=\"file\"") && content.contains("data-empty-upload"));
        // The library pane: categories in order, numbers, the card ids the
        // island toggles, the command slug, and titles escaped.
        let security = content.find(">Security<").expect("security category");
        let mine = content.find(">My apps<").expect("my apps category");
        assert!(security < mine);
        assert!(content.contains(&format!("id=\"lib-{northstar}\"")));
        assert!(content.contains(&format!("data-card=\"c-{northstar}-total\"")));
        assert!(
            content.contains(">1.1<")
                && content.contains(">2.0<")
                && content.contains(">/traffic<")
        );
        assert!(content.contains("Spend &lt;Review&gt;") && !content.contains("<Review>"));
        assert!(content.contains("2 apps · 3 cards"));
        // An empty canvas offers the business intents and no group.
        let empty = canvas::CanvasProps {
            csrf_signout: "S".repeat(43),
            summary: canvas::summary(&[]),
            ..canvas::CanvasProps::default()
        };
        let (_, _, body) = html(ssr.canvas(&empty)).await;
        assert!(body.contains("Review expenses over $200") && !body.contains("class=\"cv-group\""));
        // No stream configured: no live chip; no organization read: no name.
        assert!(!body.contains("id=\"cv-live\"") && !body.contains("class=\"cv-org\""));
        assert!(!body.contains("Synthetic data") && !body.contains("data-demo=\"on\""));
    }
}
