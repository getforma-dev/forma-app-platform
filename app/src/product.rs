//! The product's route composition (ADR-011 pitch), shared by the binary
//! (`main.rs`) and the local dev stand (`storage::dev_stand`), so the stand
//! serves exactly the routers the deployment serves. The caller builds the
//! states (from the environment in the binary, from the fixture in the
//! stand); this module only composes them.
use crate::{
    actions,
    auth::Auth,
    boxes,
    commands::{self, RecordPage},
    composer::{self, Composer},
    events, mcp,
    query::{canvas, Work},
    sharing, slack, widgets,
};
use axum::Router;
use std::{collections::BTreeSet, sync::Arc};

/// Everything the product routes are built from.
pub struct Product {
    pub auth: Arc<Auth>,
    pub work: Arc<Work>,
    /// APP-013: the record page behind the command route.
    pub record_page: Arc<dyn RecordPage>,
    pub canvas_pages: Arc<dyn canvas::CanvasPages>,
    /// The realtime URL the canvas hands its island (empty: none).
    pub live_events: String,
    /// `GENUI_DEMO_ORGANIZATIONS` (APP-098).
    pub demo_organizations: BTreeSet<String>,
    /// The composer (`POST /v1/compose`) and the MCP server's model.
    pub composer: Composer,
    /// The action path's utterance parser (`POST /v1/act`).
    pub parser: actions::Parser,
    /// APP-100: absent, no Slack route is mounted (the uniform 404).
    pub slack: Option<Arc<slack::Slack>>,
    pub customizer: widgets::customize::Customizer,
    pub weather: widgets::weather::OpenMeteo,
    /// APP-108: folders (the caller installs the folder intent).
    pub boxes: Arc<boxes::routes::Boxes>,
}

/// The product routes: the short-deadline router, and the long-deadline
/// paths (each waits on a model or reads the whole library), for
/// `http::uniform_with_many`.
pub fn routes(product: Product) -> (Router, Vec<(&'static str, Router)>) {
    let Product {
        auth,
        work,
        record_page,
        canvas_pages,
        live_events,
        demo_organizations,
        composer,
        parser,
        slack,
        customizer,
        weather,
        boxes,
    } = product;
    let command_routes = Arc::new(commands::routes::Commands {
        auth: auth.clone(),
        page: record_page,
    });
    let work_routes = crate::query::routes::router(work.clone());
    // APP-099 (ADR-011 pitch): Forma as an MCP server (`POST /v1/mcp`, long
    // deadline: `forma_compose_app` waits on the model), bearer agent tokens
    // only, over the same query engine, composer and storage operations.
    let mcp_routes = mcp::router(Arc::new(mcp::Mcp::new(work.clone(), composer.clone())));
    let canvas_routes = canvas::router(Arc::new(canvas::Canvas {
        work,
        pages: canvas_pages,
        live_events,
        demo_organizations,
    }));
    // APP-122: Ask and Shape, two routes with two closed output shapes,
    // under the long deadline (each may wait on the fast model).
    let talk_routes = composer::talk::router(Arc::new(composer::talk::Talk::new(
        auth.clone(),
        composer.clone(),
    )));
    // APP-093 (ADR-011): the composer route, under its own deadline.
    let compose_routes = composer::routes::router(Arc::new(composer::routes::Compose::new(
        auth.clone(),
        composer,
    )));
    // ADR-011 scenarios: the action path (`POST /v1/act`, under the long
    // deadline: it waits on the model) and the records listing.
    let act_routes =
        actions::routes::router(Arc::new(actions::routes::Act::new(auth.clone(), parser)));
    let records_routes = actions::routes::records_router(auth.clone());
    let slack_routes = slack::router(slack);
    // APP-103: the canvas widgets' weather and customize routes, under the
    // long deadline (Open-Meteo and the fast model are outside services).
    let widget_routes = widgets::routes::router(Arc::new(widgets::routes::Widgets::new(
        auth.clone(),
        customizer,
        weather,
    )));
    let router = Router::new()
        .merge(crate::auth::routes::router(auth.clone()))
        .merge(commands::routes::router(command_routes))
        .merge(work_routes)
        // APP-095 (ADR-011 pitch): share with a teammate; the members
        // list; the session-gated event stream re-authorized at delivery.
        .merge(sharing::router(auth.clone()))
        .merge(events::routes::router(auth.clone()))
        .merge(canvas_routes.clone())
        .merge(records_routes)
        // APP-099: the canvas's "Connect an agent" token routes.
        .merge(mcp::tokens::router(auth.clone()))
        .merge(mcp::governance::router(auth.clone()))
        .merge(slack_routes)
        // APP-101: the slash-command list and "save as /name".
        .merge(composer::commands::router(auth.clone()))
        // APP-121: organization labels and personal shortcuts.
        .merge(composer::labels::router(auth))
        .merge(boxes::routes::router(boxes));
    let long = vec![
        // The canvas page reads every openable work (library pane).
        (canvas::PATH, canvas_routes),
        (composer::routes::PATH, compose_routes),
        (actions::PATH, act_routes),
        (mcp::PATH, mcp_routes),
        (widgets::WEATHER_PATH, widget_routes.clone()),
        (widgets::CUSTOMIZE_PATH, widget_routes),
        (composer::talk::ASK_PATH, talk_routes.clone()),
        (composer::talk::SHAPE_PATH, talk_routes),
    ];
    (router, long)
}
