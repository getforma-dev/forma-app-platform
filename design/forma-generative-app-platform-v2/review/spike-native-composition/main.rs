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
struct Cell { id: String, text: String }
#[derive(PageProps)]
struct Row { id: String, cells: Vec<Cell> }
#[derive(PageProps)]
struct Bar { id: String, x: String, height: String, label: String }
#[derive(PageProps)]
struct Choice { id: String, label: String }
#[derive(PageProps)]
struct Field { id: String, label: String, is_text: bool, is_enum: bool, choices: Vec<Choice> }
#[derive(PageProps)]
struct Item { id: String, label: String, value: String }
#[derive(PageProps)]
enum Kind {
    #[props(rename = "summary")] Summary,
    #[props(rename = "table")] Table,
    #[props(rename = "timeline")] Timeline,
    #[props(rename = "form")] Form,
}
#[derive(PageProps)]
struct Card {
    id: String,
    title: String,
    kind: Kind,
    is_summary: bool,
    is_table: bool,
    is_timeline: bool,
    is_form: bool,
    style: String,
    items: Vec<Item>,
    rows: Vec<Row>,
    bars: Vec<Bar>,
    fields: Vec<Field>,
}
#[derive(PageProps)]
struct HomeProps { cards: Vec<Card> }

/// A resolved app definition: what the platform's query engine would hand the renderer.
fn fixture() -> HomeProps {
    let card = |id: &str, title: &str, kind: Kind, style: &str| Card {
        id: id.into(), title: title.into(),
        is_summary: matches!(kind, Kind::Summary), is_table: matches!(kind, Kind::Table),
        is_timeline: matches!(kind, Kind::Timeline), is_form: matches!(kind, Kind::Form),
        kind, style: style.into(), items: vec![], rows: vec![], bars: vec![], fields: vec![],
    };
    let mut summary = card("c1", "Case summary", Kind::Summary, "left:0;top:0");
    summary.items = vec![
        Item { id: "total".into(), label: "Total bytes".into(), value: "1,140,000".into() },
        Item { id: "unmatched".into(), label: "Unmatched".into(), value: "900,000".into() },
    ];
    let mut table = card("c2", "Evidence table", Kind::Table, "left:320px;top:0");
    table.rows = (1..=3).map(|n| Row { id: format!("r{n}"), cells: vec![
        Cell { id: format!("r{n}t"), text: format!("10:0{n}:00") },
        Cell { id: format!("r{n}d"), text: "198.51.100.7".into() },
        Cell { id: format!("r{n}b"), text: format!("{}", n * 300_000) },
    ] }).collect();
    let mut timeline = card("c3", "Activity timeline", Kind::Timeline, "left:0;top:240px");
    timeline.bars = (0..6).map(|n| Bar { id: format!("b{n}"), x: format!("{}", n * 60), height: format!("{}", 10 + n * 15), label: format!("minute {n}") }).collect();
    let mut form = card("c4", "Decision", Kind::Form, "left:320px;top:240px");
    form.fields = vec![
        Field { id: "note".into(), label: "Note".into(), is_text: true, is_enum: false, choices: vec![] },
        Field { id: "status".into(), label: "Status".into(), is_text: false, is_enum: true, choices: vec![
            Choice { id: "needs_context".into(), label: "Needs context".into() },
            Choice { id: "reviewed".into(), label: "Reviewed".into() },
        ] },
    ];
    HomeProps { cards: vec![summary, table, timeline, form] }
}
#[derive(PageProps)]
struct AboutProps {}

struct AppState {
    manifest: forma_server::AssetManifest,
    home: Option<PageHandle<HomeProps>>,
    about: Option<PageHandle<AboutProps>>,
}

#[tokio::main]
async fn main() {
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

    let state = Arc::new(AppState {
        manifest,
        home: home_page,
        about: about_page,
    });

    let app = Router::new()
        .route("/", get(home))
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
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
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
        Some(home) => home.render(&fixture(), &ctx),
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
