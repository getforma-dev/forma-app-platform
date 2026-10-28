use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use forma_ir::PageProps;
use forma_server::{
    assets, load_ir_modules, render_browser_shell, sw, PageConfig, PageHandle, RenderCtx, RouteMode,
};
use rust_embed::Embed;
use std::sync::Arc;

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

#[tokio::main]
async fn main() {
    let credential_path =
        std::env::var("GENUI_RUNTIME_CONFIG_FILE").expect("GENUI_RUNTIME_CONFIG_FILE is required");
    let config = genui::config::Config::load(std::path::Path::new(&credential_path))
        .unwrap_or_else(|reason| {
            eprintln!("configuration refused: {reason}");
            std::process::exit(1);
        });
    let _services = genui::roles::Services::new(credential_path.clone().into());
    drop(config);
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

    let manifest = assets::try_load_manifest::<Assets>().unwrap_or_else(|error| {
        eprintln!("\n  Error: admin/dist/manifest.json: {error}\n");
        std::process::exit(1);
    });
    for route in ["/", "/about"] {
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
        Some(RouteMode::Ssr) => Some(pages.bind::<HomeProps>("home").unwrap_or_else(|errors| {
            for error in errors {
                eprintln!("{error}");
            }
            std::process::exit(1);
        })),
        Some(RouteMode::Browser) => None,
        None => {
            eprintln!("missing declared route /");
            std::process::exit(1);
        }
    };
    let about_page = match manifest.route("/about").map(|route| route.mode) {
        Some(RouteMode::Ssr) => Some(pages.bind::<AboutProps>("about").unwrap_or_else(|errors| {
            for error in errors {
                eprintln!("{error}");
            }
            std::process::exit(1);
        })),
        Some(RouteMode::Browser) => None,
        None => {
            eprintln!("missing declared route /about");
            std::process::exit(1);
        }
    };
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

    let readiness = genui::readiness::Readiness::start(
        credential_path.into(),
        std::env::var("GENUI_HELPER_PATH")
            .unwrap_or_else(|_| "/app/runtime.py".into())
            .into(),
        std::env::var("GENUI_PYTHON").unwrap_or_else(|_| "python3".into()),
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

    // Port zero asks the OS for an available port; useful for isolated tests.
    let port = match std::env::var("FORMA_PORT") {
        Ok(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value.parse::<u16>().unwrap_or_else(|_| {
                eprintln!("FORMA_PORT must be an integer from 0 to 65535");
                std::process::exit(1);
            })
        }
        Err(std::env::VarError::NotPresent) => 3000,
        _ => {
            eprintln!("FORMA_PORT must be an integer from 0 to 65535");
            std::process::exit(1);
        }
    };
    let bind_address = match std::env::var("FORMA_BIND_ADDRESS").as_deref() {
        Ok("0.0.0.0") => std::net::Ipv4Addr::UNSPECIFIED,
        Ok("127.0.0.1") | Err(_) => std::net::Ipv4Addr::LOCALHOST,
        _ => {
            eprintln!("FORMA_BIND_ADDRESS must be 127.0.0.1 or 0.0.0.0");
            std::process::exit(1);
        }
    };
    let listener = tokio::net::TcpListener::bind((bind_address, port))
        .await
        .unwrap();
    // Emit readiness only while this process owns the successfully bound socket.
    println!("FORMA_LISTENING http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
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
    match rendered {
        // A `PageResponse` sends the HTML with the policy for its nonce,
        // `Cache-Control: no-store` (the nonce is this response's alone) and
        // the document security headers.
        Ok(page) => page.into_response(),
        // Forma logs nothing: count failures by code and route here, and
        // answer with your own error page. The error stays in the log, never
        // in the response, and no empty shell takes the page's place.
        Err(error) => {
            tracing::error!(code = error.code(), route = error.route(), "{error}");
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
    match rendered {
        // A `PageResponse` sends the HTML with the policy for its nonce,
        // `Cache-Control: no-store` (the nonce is this response's alone) and
        // the document security headers.
        Ok(page) => page.into_response(),
        // Forma logs nothing: count failures by code and route here, and
        // answer with your own error page. The error stays in the log, never
        // in the response, and no empty shell takes the page's place.
        Err(error) => {
            tracing::error!(code = error.code(), route = error.route(), "{error}");
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
    #[test]
    fn required_native_routes_refuse_browser_and_missing_outputs() {
        assert!(require_ssr(Some(RouteMode::Ssr)).is_ok());
        assert_eq!(
            require_ssr(Some(RouteMode::Browser)),
            Err("unexpected Browser output")
        );
        assert_eq!(require_ssr(None), Err("missing route"));
    }
}
