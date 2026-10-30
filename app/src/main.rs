use axum::{
    extract::State,
    http::{header, StatusCode},
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
use genui::commands::{self, Notice, RecordPage, RecordView};
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
    /// APP-074 review m4: the ADR-006 window that refused, named.
    limit_minute: bool,
    limit_day: bool,
    limit_service: bool,
    unavailable: bool,
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
// APP-076 G9: the member's openable works, resolved by the auth route.
#[derive(PageProps)]
struct RecordLinkProps {
    id: String,
    label: String,
    href: String,
}
#[derive(PageProps)]
struct WorkspaceItemProps {
    id: String,
    title: String,
    href: String,
    /// ADR-011: the work's group on the canvas (`/canvas#g-{work}`).
    canvas: String,
    has_records: bool,
    records: Vec<RecordLinkProps>,
}
#[derive(PageProps)]
struct WorkspaceProps {
    csrf_signout: String,
    csrf_signout_all: String,
    has_items: bool,
    items: Vec<WorkspaceItemProps>,
}
// APP-013 record page (D7.16): typed rows and premise fields; the key and the
// token are minted per render by the route, never read from the request.
#[derive(PageProps)]
struct FieldRowProps {
    id: String,
    kind: String,
    value: String,
    input: String,
    spec: String,
    spec_input: String,
    options: String,
    nullable: bool,
    writable: bool,
}
#[derive(PageProps)]
struct RecordProps {
    app: String,
    work: String,
    collection: String,
    record_id: String,
    schema: String,
    schema_version: String,
    publication: String,
    work_version: String,
    version: String,
    exists: bool,
    fields: Vec<FieldRowProps>,
    command: String,
    action: String,
    can_write: bool,
    csrf: String,
    idempotency_key: String,
    committed: bool,
    replayed: bool,
    conflict: bool,
    stale: bool,
    in_flight: bool,
    busy: bool,
    refused: bool,
    unknown: bool,
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
    record: PageHandle<RecordProps>,
}
impl RecordPage for SsrPages {
    fn record(&self, view: &RecordView) -> Response {
        let notice = |expected: Notice| view.notice == Some(expected);
        respond(
            "/record",
            self.record.render(
                &RecordProps {
                    app: view.app.clone(),
                    work: view.work.clone(),
                    collection: view.collection.clone(),
                    record_id: view.record_id.clone(),
                    schema: view.schema.clone(),
                    schema_version: view.schema_version.clone(),
                    publication: view.publication.clone(),
                    work_version: view.work_version.clone(),
                    version: view.version.clone(),
                    exists: view.exists,
                    fields: view
                        .fields
                        .iter()
                        .map(|row| FieldRowProps {
                            id: row.id.clone(),
                            kind: row.kind.clone(),
                            value: row.value.clone(),
                            input: row.input.clone(),
                            spec: row.spec.clone(),
                            spec_input: row.spec_input.clone(),
                            options: row.options.clone(),
                            nullable: row.nullable,
                            writable: row.writable,
                        })
                        .collect(),
                    command: view.command.clone(),
                    action: view.action.clone(),
                    can_write: view.can_write,
                    csrf: view.csrf.clone(),
                    idempotency_key: view.idempotency_key.clone(),
                    committed: notice(Notice::Committed),
                    replayed: notice(Notice::Replayed),
                    conflict: notice(Notice::Conflict),
                    stale: notice(Notice::Stale),
                    in_flight: notice(Notice::InFlight),
                    busy: notice(Notice::Busy),
                    refused: notice(Notice::Refused),
                    unknown: notice(Notice::Unknown),
                },
                &RenderCtx::new().with_title("Record"),
            ),
        )
    }
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
                    limit_minute: view.limited == Some(auth::InviteLimit::Minute),
                    limit_day: view.limited == Some(auth::InviteLimit::Day),
                    limit_service: view.limited == Some(auth::InviteLimit::Service),
                    unavailable: view.unavailable,
                    described_by: if view.failed || view.limited.is_some() || view.unavailable {
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
                    has_items: !view.items.is_empty(),
                    items: view
                        .items
                        .iter()
                        .map(|item| WorkspaceItemProps {
                            id: item.href.clone(),
                            title: item.title.clone(),
                            href: item.href.clone(),
                            canvas: format!(
                                "/canvas#g-{}",
                                item.href.rsplit('/').next().unwrap_or_default()
                            ),
                            has_records: !item.records.is_empty(),
                            records: item
                                .records
                                .iter()
                                .map(|record| RecordLinkProps {
                                    id: record.href.clone(),
                                    label: record.label.clone(),
                                    href: record.href.clone(),
                                })
                                .collect(),
                        })
                        .collect(),
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
    // APP-013: the command route and the record page share the session,
    // vault and synchronizer key with the ADR-006 routes.
    let command_routes = Arc::new(commands::routes::Commands {
        auth: auth.clone(),
        page: auth_pages,
    });
    // APP-012 (ADR-007 D7.10): the confined query engine beside the data
    // store, in the image's private cache and scratch under /run/genui.
    let engine = stores.as_ref().map(|_| {
        Arc::new(
            genui::query::Engine::new(genui::query::EngineSettings::image())
                .unwrap_or_else(|_| refuse("query engine directories refused")),
        ) as Arc<dyn genui::query::Executor>
    });
    let work = Arc::new(genui::query::Work::new(auth.clone(), engine, work_page));
    let work_routes = genui::query::routes::router(work.clone());
    // The organization's realtime stream (`pivot/realtime`) once it exists;
    // until then the island subscribes to nothing.
    let canvas_routes = genui::query::canvas::router(Arc::new(genui::query::canvas::Canvas {
        work,
        pages: canvas_page,
        live_events: std::env::var("GENUI_CANVAS_EVENTS_URL")
            .ok()
            .filter(|url| url.starts_with("/v1/"))
            .unwrap_or_default(),
    }));
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
    // alike (no `Allow`), and the D7.14 handler deadline.
    // APP-093 (ADR-011): the composer route, under its own deadline; the
    // model key comes from the environment alone (absent: the route answers
    // `composer_unavailable` and nothing else changes).
    let compose_routes = genui::composer::routes::router(Arc::new(
        genui::composer::routes::Compose::new(auth.clone(), genui::composer::Composer::from_env()),
    ));
    // ADR-011 scenarios: the action path (`POST /v1/act`, under the long
    // deadline: it waits on the model) and the records listing.
    let act_routes = genui::actions::routes::router(Arc::new(genui::actions::routes::Act::new(
        auth.clone(),
        genui::actions::Parser::from_env(),
    )));
    let records_routes = genui::actions::routes::records_router(auth.clone());
    let app = match qualification {
        Some(harness) => genui::http::uniform(app.merge(harness)),
        None => genui::http::uniform_with_many(
            app.merge(auth::routes::router(auth.clone()))
                .merge(commands::routes::router(command_routes))
                .merge(work_routes)
                // APP-095 (ADR-011 pitch): share with a teammate; the members
                // list; the session-gated event stream re-authorized at delivery.
                .merge(genui::sharing::router(auth.clone()))
                .merge(genui::events::routes::router(auth.clone()))
                .merge(canvas_routes)
                .merge(records_routes),
            vec![
                (genui::composer::routes::PATH, compose_routes),
                (genui::actions::PATH, act_routes),
            ],
        ),
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
        };
        let result = |columns: &[&str], rows: Vec<Vec<serde_json::Value>>| WorkerResult {
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            integer_columns: columns
                .iter()
                .filter(|c| matches!(**c, "total_bytes" | "bytes"))
                .map(|c| (*c).to_owned())
                .collect(),
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
        let (ready_receipt, empty_receipt, timeline_receipt) = (
            receipt(&total, "sha256:receipt-ready"),
            receipt(&nothing, "sha256:receipt-empty"),
            receipt(&buckets, "sha256:receipt-timeline"),
        );
        let cards = vec![
            work::card(
                &view("ready", "summary"),
                Outcome::Released(&total, &ready_receipt),
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
            views: vec![],
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
        let timeline = section("timeline");
        assert!(timeline.contains("<table") && timeline.contains(">60,000<"));
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
        };
        let result = |columns: &[&str], rows: Vec<Vec<serde_json::Value>>| WorkerResult {
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
            integer_columns: columns
                .iter()
                .filter(|c| matches!(**c, "total_bytes" | "bytes"))
                .map(|c| (*c).to_owned())
                .collect(),
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
            vec![vec![
                json!("2026-10-22T10:00:00Z"),
                json!("203.0.113.9"),
                json!("60000"),
            ]],
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
            live_events: String::new(),
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
        let content = &body[start..end];
        assert!(content.contains("2 apps · 5 cards"));
        assert!(content.contains("Northstar &lt;egress&gt;") && !content.contains("<egress>"));
        assert_eq!(content.matches(canvas::SEEDED_LABEL).count(), 2);
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
        assert!(ready.contains("data-state=\"ready\"") && ready.contains(">1,140,000<"));
        assert!(ready.contains(&format!(
            "data-query=\"/v1/work/{app}/{northstar}/queries/total\""
        )));
        assert!(ready.contains("data-integers=\"total_bytes\""));
        // The receipt is visible, not collapsed: its digest and key identities
        // sit outside the inspector's <details>.
        let visible = &ready[..ready.find("<details").unwrap()];
        assert!(
            visible.contains("class=\"cv-receipt\"") && visible.contains("sha256:receipt-total")
        );
        // Key identities are shortened on the card; the inspector keeps them whole.
        assert!(visible.contains(&canvas::abbreviate(&total.result_digest)));
        assert!(visible.contains("Membership epoch") && ready.contains(&total.result_digest));
        assert!(
            visible.contains("Source revision (events)") && visible.contains("Parameter window")
        );
        let timeline = section("timeline");
        assert!(timeline.contains("<table") && timeline.contains(">60,000<"));
        assert!(timeline.contains("sha256:receipt-timeline"));
        for (id, state) in [("failed", "failed"), ("denied", "denied")] {
            let html = section(id);
            assert!(html.contains(&format!("data-state=\"{state}\"")), "{id}");
            assert!(
                !html.contains("class=\"value\"") && !html.contains("cv-receipt"),
                "{id}"
            );
            assert!(!html.contains("1,140,000") && !html.contains(">0<"), "{id}");
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
        assert!(content.contains(&format!("value=\"{}\"", "C".repeat(43))));
        assert!(content.contains("action=\"/auth/signout\"") && content.contains(&"S".repeat(43)));
        // No organization picker; the intent bar posts nowhere but the page.
        assert!(!content.to_lowercase().contains("organization picker"));
        assert!(content.contains("What do you need?"));
        // An empty canvas offers the business intents and no group.
        let empty = canvas::CanvasProps {
            csrf_signout: "S".repeat(43),
            summary: canvas::summary(&[]),
            ..canvas::CanvasProps::default()
        };
        let (_, _, body) = html(ssr.canvas(&empty)).await;
        assert!(body.contains("Review expenses over $200") && !body.contains("class=\"cv-group\""));
    }
}
