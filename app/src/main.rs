use axum::{
    extract::State,
    http::{header, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use forma_ir::PageProps;
use forma_server::{
    assets, load_ir_modules, render_browser_shell, sw, PageConfig, PageHandle, PageResponse,
    PolicyExtensions, RenderCtx, RenderError, RouteMode,
};
use genui::auth::{
    self, Auth, AuthConfig, DeniedView, InviteView, PendingView, ResendResult, SigninView,
    WorkspaceView,
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
// APP-009 (ADR-006) pages: typed props only; no page reads a request value.
// APP-074: the invitation entry never shows a submitted code back.
#[derive(PageProps)]
struct InviteProps {
    failed: bool,
    limited: bool,
    described_by: String,
}
#[derive(PageProps)]
struct SigninProps {
    action: String,
    has_action: bool,
    failed: bool,
}
#[derive(PageProps)]
struct PendingProps {
    csrf_resend: String,
    csrf_signout: String,
    resend_available: bool,
    cooldown_seconds: u32,
    held: bool,
    hours_remaining: u32,
    requested: bool,
    unavailable: bool,
}
#[derive(PageProps)]
struct DeniedProps {
    csrf_signout: String,
}
#[derive(PageProps)]
struct WorkspaceProps {
    csrf_signout: String,
    csrf_signout_all: String,
}

struct AppState {
    manifest: forma_server::AssetManifest,
    readiness: Arc<genui::readiness::Readiness>,
    home: Option<PageHandle<HomeProps>>,
    about: Option<PageHandle<AboutProps>>,
}

/// The APP-009 pages, rendered as complete `PageResponse`s (fresh nonce,
/// CSP and `no-store` together). The auth module decides status and cookies.
struct SsrPages {
    /// The configured fAuth origin: the two anonymous forms' `POST` answers
    /// `303` there, and Chromium and WebKit judge `form-action` on every hop
    /// of a form submission's redirect chain (APP-009 review M1).
    auth_origin: Option<String>,
    invite: PageHandle<InviteProps>,
    signin: PageHandle<SigninProps>,
    pending: PageHandle<PendingProps>,
    denied: PageHandle<DeniedProps>,
    workspace: PageHandle<WorkspaceProps>,
}
impl SsrPages {
    /// A page whose form hands off to the auth origin: `form-action 'self'`
    /// plus that exact origin (forma-server `PolicyExtensions::form_action`).
    /// Without a configured origin the form fails neutrally and stays `'self'`.
    fn handoff(&self, title: &str) -> RenderCtx {
        let ctx = RenderCtx::new().with_title(title);
        match &self.auth_origin {
            Some(origin) => ctx.with_policy(PolicyExtensions::new().form_action(origin.clone())),
            None => ctx,
        }
    }
}
impl auth::Pages for SsrPages {
    fn invite(&self, view: &InviteView) -> Response {
        respond(
            "/invite",
            self.invite.render(
                &InviteProps {
                    failed: view.failed,
                    limited: view.limited,
                    described_by: if view.failed || view.limited {
                        "invite-error"
                    } else {
                        "invite-help"
                    }
                    .into(),
                },
                &self.handoff("Enter your invitation code"),
            ),
        )
    }
    fn signin(&self, view: &SigninView) -> Response {
        respond(
            "/signin",
            self.signin.render(
                &SigninProps {
                    action: view.action.clone().unwrap_or_default(),
                    has_action: view.action.is_some(),
                    failed: view.failed,
                },
                &self.handoff("Sign in"),
            ),
        )
    }
    fn pending(&self, view: &PendingView) -> Response {
        respond(
            "/auth/pending",
            self.pending.render(
                &PendingProps {
                    csrf_resend: view.csrf_resend.clone(),
                    csrf_signout: view.csrf_signout.clone(),
                    resend_available: view.resend_available,
                    cooldown_seconds: view.cooldown_seconds,
                    held: view.hours_remaining > 0,
                    hours_remaining: view.hours_remaining,
                    requested: view.result == Some(ResendResult::Requested),
                    unavailable: view.result == Some(ResendResult::Unavailable),
                },
                &RenderCtx::new().with_title("Confirm your email address"),
            ),
        )
    }
    fn denied(&self, view: &DeniedView) -> Response {
        respond(
            "/auth/callback",
            self.denied.render(
                &DeniedProps {
                    csrf_signout: view.csrf_signout.clone(),
                },
                &RenderCtx::new().with_title("No access"),
            ),
        )
    }
    fn workspace(&self, view: &WorkspaceView) -> Response {
        respond(
            "/workspace",
            self.workspace.render(
                &WorkspaceProps {
                    csrf_signout: view.csrf_signout.clone(),
                    csrf_signout_all: view.csrf_signout_all.clone(),
                },
                &RenderCtx::new().with_title("Workspace"),
            ),
        )
    }
}

/// A `PageResponse` sends the HTML with the policy for its nonce,
/// `Cache-Control: no-store` and the document security headers. Forma logs
/// nothing: a failure is counted here by code and route and answered with our
/// own error, never an empty shell.
fn respond(route: &str, rendered: Result<PageResponse, RenderError>) -> Response {
    match rendered {
        Ok(page) => page.into_response(),
        Err(error) => {
            tracing::error!(code = error.code(), route, "{error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                ],
                "Internal Server Error",
            )
                .into_response()
        }
    }
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
        Auth::new(auth_config, stores.clone(), auth_pages)
            .unwrap_or_else(|reason| refuse(reason))
            .with_trusted_proxies(proxies)
            .with_invitations(invitations),
    );
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
    let app = match qualification {
        Some(harness) => app.merge(harness),
        None => app.merge(auth::routes::router(auth.clone())),
    }
    .fallback(genui::http::fallback)
    .method_not_allowed_fallback(genui::http::fallback)
    .layer(middleware::from_fn(genui::http::deadline));

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
        };
        // Home and about stay unbound here; finalize judges only bound routes.
        let _ = pages.bind::<HomeProps>("home");
        let _ = pages.bind::<AboutProps>("about");
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
        let (status, headers, body) = html(pages.invite(&InviteView {
            failed: false,
            limited: false,
        }))
        .await;
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
            limited: false,
        }))
        .await;
        assert!(refused.contains("role=\"alert\"") && refused.contains("could not be used"));
        assert!(refused.contains("aria-describedby=\"invite-error\""));
        assert!(refused.contains("id=\"invite-error\"") && refused.contains("<form"));
        let (_, _, limited) = html(pages.invite(&InviteView {
            failed: false,
            limited: true,
        }))
        .await;
        assert!(limited.contains("Too many attempts") && limited.contains("role=\"alert\""));
        assert!(!limited.contains("could not be used"));
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
        }))
        .await;
        assert!(body.contains("action=\"/auth/signout-all\"") && body.contains(&"A".repeat(43)));
    }
}
