use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use forma_ir::PageProps;
use forma_server::{assets, load_ir_modules, sw, PageHandle, RenderCtx, RouteMode};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Instant};

#[derive(Embed)]
#[folder = "admin/dist/"]
struct Assets;
#[derive(PageProps, Serialize)]
struct Cell {
    id: String,
    text: String,
}
#[derive(PageProps, Serialize)]
struct Row {
    id: String,
    cells: Vec<Cell>,
}
#[derive(PageProps, Serialize)]
struct Bar {
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    label: String,
}
#[derive(PageProps, Serialize)]
struct Choice {
    id: String,
    label: String,
}
#[allow(non_snake_case)]
#[derive(PageProps, Serialize)]
struct Field {
    id: String,
    label: String,
    isText: bool,
    isEnum: bool,
    choices: Vec<Choice>,
}
#[derive(PageProps, Serialize)]
struct Item {
    id: String,
    label: String,
    value: String,
}
#[allow(non_snake_case)]
#[derive(PageProps, Serialize)]
struct Card {
    id: String,
    title: String,
    kind: String,
    style: String,
    tabIndex: i32,
    isSummary: bool,
    isTable: bool,
    isDetail: bool,
    isTimeline: bool,
    isLinked: bool,
    isChart: bool,
    isFilter: bool,
    isForm: bool,
    isNote: bool,
    isStatus: bool,
    items: Vec<Item>,
    rows: Vec<Row>,
    bars: Vec<Bar>,
    fields: Vec<Field>,
    text: String,
    status: String,
    filterId: String,
}
#[derive(PageProps, Serialize)]
struct CanvasProps {
    cards: Vec<Card>,
    filter: String,
    epoch: u32,
    linked: String,
    outcome: String,
}
const KINDS: [&str; 9] = [
    "summary", "table", "detail", "timeline", "chart", "filter", "form", "note", "status",
];
const HOSTILE: &str = "Café 東京 <script>globalThis.pwned=true</script> & \"onmouseover=alert(1)\"";
fn fixture(name: &str) -> CanvasProps {
    let mut cards = Vec::new();
    if name != "empty" {
        for (index, kind) in KINDS.iter().enumerate() {
            let rows = if name == "rows500" { 500 } else { 3 };
            let mut card = Card {
                id: format!("view-{kind}"),
                title: format!("{kind} view"),
                kind: kind.to_string(),
                style: "min-height: 200px;".into(),
                tabIndex: -1,
                isSummary: *kind == "summary",
                isTable: *kind == "table",
                isDetail: *kind == "detail",
                isTimeline: *kind == "timeline",
                isLinked: ["summary", "table", "detail", "timeline", "chart", "status"]
                    .contains(kind),
                isChart: *kind == "chart",
                isFilter: *kind == "filter",
                isForm: *kind == "form",
                isNote: *kind == "note",
                isStatus: *kind == "status",
                items: vec![],
                rows: vec![],
                bars: vec![],
                fields: vec![],
                text: HOSTILE.into(),
                status: "Ready for review".into(),
                filterId: "linked-destination".into(),
            };
            if card.isSummary || card.isDetail {
                card.items = vec![
                    Item {
                        id: "bytes".into(),
                        label: "Total bytes".into(),
                        value: "1,140,000".into(),
                    },
                    Item {
                        id: "asset".into(),
                        label: "Asset".into(),
                        value: HOSTILE.into(),
                    },
                ];
            }
            if card.isTable {
                card.rows = (0..rows)
                    .map(|n| Row {
                        id: format!("row-{n}"),
                        cells: vec![
                            Cell {
                                id: "asset".into(),
                                text: format!("asset-{n}"),
                            },
                            Cell {
                                id: "text".into(),
                                text: HOSTILE.into(),
                            },
                        ],
                    })
                    .collect();
            }
            if card.isTimeline || card.isChart {
                card.bars = (0..6)
                    .map(|n| Bar {
                        id: format!("bar-{n}"),
                        x: (n * 35) as f64 + 0.5,
                        y: 0.0,
                        width: 12.5,
                        height: (10 + n * 9) as f64,
                        label: format!("bucket {n}: server value"),
                    })
                    .collect();
            }
            if card.isForm {
                card.fields = vec![
                    Field {
                        id: "observation".into(),
                        label: "Observation".into(),
                        isText: true,
                        isEnum: false,
                        choices: vec![],
                    },
                    Field {
                        id: "decision".into(),
                        label: "Decision".into(),
                        isText: false,
                        isEnum: true,
                        choices: vec![
                            Choice {
                                id: "review".into(),
                                label: "Needs review".into(),
                            },
                            Choice {
                                id: "accepted".into(),
                                label: "Accepted".into(),
                            },
                        ],
                    },
                ];
            }
            if name == "long" {
                card.title = format!("{} {index} {HOSTILE}", "Long label 東京 ".repeat(80));
            }
            if name == "nested-empty" {
                card.items.clear();
                card.rows.clear();
                card.bars.clear();
                card.fields.clear();
            }
            cards.push(card);
        }
    }
    CanvasProps {
        cards,
        filter: "all".into(),
        epoch: 0,
        linked: "All: 1,140,000 bytes".into(),
        outcome: "Not submitted".into(),
    }
}
fn admit(props: &CanvasProps) -> Result<(), &'static str> {
    if props.cards.len() > 20 {
        return Err("card bound");
    }
    let mut keys = std::collections::HashSet::new();
    for card in &props.cards {
        if !keys.insert(&card.id) || card.rows.len() > 500 {
            return Err("key or row bound");
        }
        for bar in &card.bars {
            if [bar.x, bar.y, bar.width, bar.height]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0 || *v > 600.0)
            {
                return Err("geometry bound");
            }
        }
    }
    Ok(())
}
fn require_ssr(mode: Option<RouteMode>) -> Result<(), &'static str> {
    match mode {
        Some(RouteMode::Ssr) => Ok(()),
        _ => Err("required native route absent or Browser"),
    }
}
struct AppState {
    home: PageHandle<CanvasProps>,
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let manifest = assets::try_load_manifest::<Assets>().expect("paired manifest");
    require_ssr(manifest.route("/").map(|r| r.mode)).expect("native mode");
    let pages = load_ir_modules::<Assets>(&manifest).expect("native modules");
    let home = pages.bind::<CanvasProps>("home").expect("typed binding");
    pages.finalize().expect("finalize before listener");
    sw::configure(sw::ServiceWorker::off()).expect("worker policy");
    if std::env::args().any(|a| a == "--bench") {
        bench(&home);
        return;
    }
    let router = Router::new()
        .route("/", get(page))
        .route("/linked", get(linked))
        .route("/commands", post(command))
        .route("/sw.js", get(sw::serve_sw::<Assets>))
        .route("/_assets/{*filename}", get(assets::serve_asset::<Assets>))
        .with_state(Arc::new(AppState { home }));
    let port: u16 = std::env::var("FORMA_PORT")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .expect("port");
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    println!("FORMA_LISTENING http://{}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}
async fn page(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let name = query.get("state").map(String::as_str).unwrap_or("normal");
    if !["normal", "empty", "nested-empty", "long", "rows500"].contains(&name) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let props = fixture(name);
    admit(&props).expect("fixture admitted");
    match state
        .home
        .render(&props, &RenderCtx::new().with_title("Canvas qualification"))
    {
        Ok(response) => response.into_response(),
        Err(error) => {
            tracing::error!(code = error.code(), "native render refused");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CACHE_CONTROL, "no-store")],
                "Render refused",
            )
                .into_response()
        }
    }
}
#[derive(Deserialize)]
struct Linked {
    filter: String,
    epoch: u32,
}
async fn linked(Query(input): Query<Linked>) -> Response {
    let label = match input.filter.as_str() {
        "all" => "All: 1,140,000 bytes",
        "unmatched" => "Unmatched: 900,000 bytes",
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    // Intentional bounded delay supplies a real stale-response browser control.
    if input.epoch == 1 {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"epoch":input.epoch,"label":label})),
    )
        .into_response()
}
async fn command(Form(values): Form<HashMap<String, String>>) -> Response {
    if values.len() != 3
        || values.get("Idempotency-Key").map(String::as_str) != Some("synthetic-render-key")
        || !values
            .get("observation")
            .is_some_and(|v| !v.is_empty() && v.len() <= 500)
        || !values
            .get("decision")
            .is_some_and(|v| ["review", "accepted"].contains(&v.as_str()))
    {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"outcome":"Synthetic observation accepted"})),
    )
        .into_response()
}
fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // RUSAGE_SELF is process lifetime high-water RSS, not bytes per render.
    unsafe {
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()), 0);
        let value = usage.assume_init().ru_maxrss as u64;
        if cfg!(target_os = "macos") {
            value
        } else {
            value * 1024
        }
    }
}
fn bench(home: &PageHandle<CanvasProps>) {
    let mut results = Vec::new();
    for name in ["normal", "empty", "nested-empty", "long", "rows500"] {
        let props = fixture(name);
        admit(&props).unwrap();
        let typed_bytes = serde_json::to_vec(&props).unwrap().len();
        let start = Instant::now();
        let first = home.render(&props, &RenderCtx::new()).unwrap();
        let first_us = start.elapsed().as_micros();
        let html_bytes = first.counts().bytes;
        drop(first);
        let mut samples = Vec::new();
        let begin = Instant::now();
        for _ in 0..200 {
            let start = Instant::now();
            let response = home.render(&props, &RenderCtx::new()).unwrap();
            std::hint::black_box(response.counts());
            drop(response);
            samples.push(start.elapsed().as_nanos() as u64);
        }
        let elapsed = begin.elapsed().as_secs_f64();
        samples.sort_unstable();
        results.push(serde_json::json!({"state":name,"typed_json_utf8_bytes":typed_bytes,"native_walk_html_bytes":html_bytes,"first_render_us":first_us,"iterations":200,"elapsed_seconds":elapsed,"renders_per_second":200.0/elapsed,"median_render_ns":samples[100],"p99_local_sample_ns":samples[197],"process_lifetime_peak_rss_bytes":peak_rss_bytes()}));
    }
    let mut over = fixture("normal");
    over.cards[7].text = "界".repeat(400_000);
    let refusal = home
        .render(&over, &RenderCtx::new())
        .err()
        .expect("over-cap state must refuse");
    println!(
        "BENCH_JSON {}",
        serde_json::json!({"scope":"local sequential release renderer, no network/database/browser or client SLO claim", "states":results,"over_cap":{"typed_json_utf8_bytes":serde_json::to_vec(&over).unwrap().len(),"code":refusal.code(),"refused":true}})
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(PageProps)]
    struct WrongProps {
        count: u32,
    }
    #[test]
    fn actual_native_schema_and_unbound_route_refuse_before_listening() {
        let manifest = assets::try_load_manifest::<Assets>().unwrap();
        let pages = load_ir_modules::<Assets>(&manifest).unwrap();
        assert!(pages.bind::<WrongProps>("home").is_err());
        assert!(pages.finalize().is_err());
    }
    #[test]
    fn native_mode_is_mandatory() {
        assert!(require_ssr(Some(RouteMode::Ssr)).is_ok());
        assert!(require_ssr(None).is_err());
        assert!(require_ssr(Some(RouteMode::Browser)).is_err());
    }
    #[test]
    fn geometry_rows_keys_are_bounded() {
        let mut p = fixture("normal");
        admit(&p).unwrap();
        p.cards[3].bars[0].x = f64::NAN;
        assert!(admit(&p).is_err());
        p.cards[3].bars[0].x = 601.0;
        assert!(admit(&p).is_err());
        let mut p = fixture("rows500");
        admit(&p).unwrap();
        p.cards[1].rows.push(Row {
            id: "extra".into(),
            cells: vec![],
        });
        assert!(admit(&p).is_err());
        let mut p = fixture("normal");
        p.cards[1].id = p.cards[0].id.clone();
        assert!(admit(&p).is_err());
    }
}
