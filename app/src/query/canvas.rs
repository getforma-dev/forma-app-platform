//! The pitch canvas (ADR-011 §1; APP-092, APP-094): the signed-in
//! workspace as one canvas holding every saved work the member may open now,
//! each a group of cards (its runnable views and its record forms).
//!
//! - `GET /canvas`: the canvas page (Forma 3.2 native SSR; the canvas is one
//!   island activated after load). A live workspace session is required, as
//!   for `GET /w/{app}/{work}` (`routes::member`): anonymous, pre-access,
//!   expired, revoked and foreign callers get the uniform 404, and so does a
//!   query string. The listing is `Data::openable_works` (the workspace's
//!   clause: `discover` on the app and `record_read` on the work, re-read
//!   after the member fence); each group's views are `Data::work_outline`, the
//!   work page's own authorization. A store failure on the listing renders
//!   the empty canvas, as `GET /workspace` does, so sign-out stays reachable.
//!   Cards are idle on the server, except the work whose run cookie this
//!   session set through the work page's run form (`routes::restore`): its
//!   views run again under a fresh authorization and release, exactly as the
//!   work page restores them, so a page without JavaScript still shows real
//!   results with their receipts. Nothing else runs on a `GET`.
//! - `GET /v1/canvas/works/{app}/{work}`: one group as JSON, for the island
//!   to place a work it did not render (the composer's newly published work,
//!   a shared work, a live event). Only a work in the member's current
//!   listing answers; anything else is the uniform 404. Its cards are idle:
//!   the island runs them through the existing JSON query route.
//!
//! The island runs every number through `POST /v1/work/{app}/{work}/queries/
//! {view}` (authorization, engine, receipt, release) and every record form
//! through `POST /v1/work/{app}/{work}/commands/{command}` (idempotent,
//! transactional, audited). This module adds no data path.
use super::{
    canvas_display::{self as display, ChartProps},
    page::{self, CardProps, CellProps, FormValues, PairProps},
    routes::{self, Restore, Work},
};
use crate::{
    auth::{capability, csrf},
    commands,
    http::{self, UniformPath},
    storage::{Error, OpenableWork, TrustedRequest, WorkOutline},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use forma_ir::PageProps;
use forma_server::{PageHandle, RenderCtx};
use serde::Serialize;
use std::sync::Arc;

/// The honest provenance label of every Phase 1 work (design brief §2.6,
/// keep): the definitions and sources are seeded, nothing is generated.
pub const SEEDED_LABEL: &str =
    "Seeded definition, retrieved from the library. Sources seeded by the operator.";

/// The page's typed props (camelCase in `admin/src/canvas/app.tsx`).
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasProps {
    pub csrf_signout: String,
    pub has_groups: bool,
    pub groups: Vec<GroupProps>,
    /// "2 apps · 5 cards": the top bar's count.
    pub summary: String,
    /// The organization's server-sent events URL once the realtime route
    /// exists (`pivot/realtime`); empty: the island subscribes to nothing.
    pub live_events: String,
    /// The live chip is offered (it shows once the island subscribes).
    pub has_live: bool,
    /// The member's organization (its display name), in the top bar; no
    /// organization picker.
    pub has_organization: bool,
    pub organization: String,
    /// The member's neutral label (`storage::sharing::member_label`: 0002
    /// principals carry no display name) and its two-letter avatar.
    pub member_label: String,
    pub member_initials: String,
    /// `GENUI_DEMO_ORGANIZATIONS` (APP-098 demo shortcuts; the session's
    /// organization is one of them): the small-print
    /// synthetic-data label, and `demo_mode` = "on" for the island's
    /// demo-safe layout (each load starts from the default arrangement).
    pub demo: bool,
    pub demo_mode: String,
}

/// The demo organizations (APP-098): `GENUI_DEMO_ORGANIZATIONS`, the same
/// value and parse as the composer's (`composer::demo_organizations`); a
/// member of any other organization sees the canvas exactly as without demo
/// mode. Empty by default.
pub fn demo_organizations() -> std::collections::BTreeSet<String> {
    crate::composer::demo_organizations(
        &std::env::var(crate::composer::DEMO_ORGANIZATIONS_ENV).unwrap_or_default(),
    )
}

/// An origin chip on an app card ("Retrieved from library", "Seeded
/// definition", "Generated"): `kind` styles it.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct OriginProps {
    pub id: String,
    pub kind: String,
    pub text: String,
}

/// The origin chips of a seeded Phase 1 work (`SEEDED_LABEL`, as chips).
pub fn seeded_origins() -> Vec<OriginProps> {
    [
        ("library", "Retrieved from library"),
        ("seeded", "Seeded definition"),
    ]
    .iter()
    .enumerate()
    .map(|(index, (kind, text))| OriginProps {
        id: format!("o{index}"),
        kind: (*kind).to_owned(),
        text: (*text).to_owned(),
    })
    .collect()
}

/// One saved work on the canvas.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct GroupProps {
    /// `g-{work}`: the group's element id (the `#` target of the workspace's
    /// "Open on canvas" link).
    pub id: String,
    pub app: String,
    pub work: String,
    /// `a-{work}`: the app card's canvas item id.
    pub app_card: String,
    pub title: String,
    pub label: String,
    pub origins: Vec<OriginProps>,
    /// `/w/{app}/{work}`: the work page and its run form (without JavaScript
    /// the group's run form posts there).
    pub href: String,
    pub csrf: String,
    pub window_start: String,
    pub window_end: String,
    pub destination: String,
    /// The views ran on this request from the session's run cookie.
    pub restored: bool,
    pub has_cards: bool,
    pub cards: Vec<CanvasCardProps>,
    pub has_forms: bool,
    pub forms: Vec<FormCardProps>,
}

/// One view card: the work page's card state model (`page::card`) plus the
/// island's query route and the view's integer columns, shown for reading
/// (`canvas_display`): one insight number or a chart, the rows, and the
/// receipt as a compact "Verified" line that expands inline to every field.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasCardProps {
    /// `c-{work}-{view}`: a safe canvas item id.
    pub id: String,
    pub view: String,
    pub title: String,
    pub kind: String,
    pub state: String,
    pub badge: String,
    pub has_message: bool,
    pub message: String,
    /// `POST` target of the island: the existing JSON query route.
    pub query: String,
    /// Comma-separated exact-integer columns (digit-grouped, nothing else).
    pub integer_columns: String,
    /// A one-row summary: its first value, compact with its unit, and the
    /// exact value; the other values as small stats.
    pub has_insight: bool,
    pub insight_label: String,
    pub insight: String,
    pub insight_exact: String,
    pub has_stats: bool,
    pub stats: Vec<StatProps>,
    /// A table result a chart reads honestly (`canvas_display::chart`).
    pub has_chart: bool,
    pub chart: ChartProps,
    /// The rows, open when there is no chart, folded under it otherwise.
    pub has_table: bool,
    pub has_open_table: bool,
    pub has_folded_table: bool,
    pub rows_label: String,
    pub columns: Vec<CellProps>,
    pub rows: Vec<CanvasRowProps>,
    pub has_receipt: bool,
    /// The receipt's full digest (the compact line's title and first row).
    pub receipt_digest: String,
    /// The digest's first eight hex digits, shown on the card.
    pub receipt_short: String,
    /// "2 sources".
    pub receipt_sources: String,
    /// When this page received the released result ("15:04 UTC"); not a
    /// receipt field.
    pub receipt_time: String,
    /// Every receipt row, in full (the inline inspector).
    pub receipt: Vec<PairProps>,
    pub receipt_json: String,
}

/// A secondary summary value.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct StatProps {
    pub id: String,
    pub label: String,
    pub value: String,
    pub exact: String,
}
/// A table cell for reading (`text`) with its exact value (`exact`, the
/// cell's title); `class` right-aligns numbers.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasCellProps {
    pub id: String,
    pub text: String,
    pub exact: String,
    pub class: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasRowProps {
    pub id: String,
    pub cells: Vec<CanvasCellProps>,
}

/// One record form (APP-013, D7.16): a fresh record of a collection, the
/// premise as hidden fields, a per-render key and the synchronizer token.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct FormCardProps {
    /// `f-{work}-{collection}`.
    pub id: String,
    pub title: String,
    pub command: String,
    pub action: String,
    pub csrf: String,
    pub idempotency_key: String,
    pub collection: String,
    pub publication: String,
    pub schema: String,
    pub schema_version: String,
    pub record: String,
    pub record_version: String,
    pub work_version: String,
    pub fields: Vec<FormFieldProps>,
    /// The submit button: the command for reading ("Record decision").
    pub submit: String,
}

#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct FormFieldProps {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub input: String,
    pub value: String,
    pub spec: String,
    pub spec_input: String,
    pub writable: bool,
    pub read_only: bool,
    /// Writable as free text (no declared choices).
    pub has_text: bool,
    pub has_choices: bool,
    pub choices: Vec<ChoiceProps>,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct ChoiceProps {
    pub id: String,
    pub value: String,
}

/// The receipt digest's first eight hex digits.
pub fn digest_prefix(digest: &str) -> String {
    let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
    hex.chars().take(8).collect()
}

/// "1 source", "2 sources": the receipt's source revisions.
pub fn sources_label(rows: &[PairProps]) -> String {
    let count = rows
        .iter()
        .filter(|row| row.id.starts_with("source-"))
        .count();
    format!("{count} {}", if count == 1 { "source" } else { "sources" })
}

/// Now, as the compact receipt line shows it ("15:04 UTC").
fn now_label() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    display::released_at(&crate::events::timestamp(seconds))
}

/// A canvas card from the work page's card for `view` of `work`.
pub fn canvas_card(
    app: &str,
    work: &str,
    view: &crate::storage::WorkView,
    card: CardProps,
) -> CanvasCardProps {
    let integer = |column: &str| view.integer_columns.iter().any(|name| name == column);
    let mut canvas = CanvasCardProps {
        id: format!("c-{work}-{}", view.id),
        view: view.id.clone(),
        title: display::label(&card.title),
        kind: card.kind,
        state: card.state,
        badge: card.badge,
        has_message: !card.message.is_empty(),
        message: card.message,
        query: format!("/v1/work/{app}/{work}/queries/{}", view.id),
        integer_columns: view.integer_columns.join(","),
        has_receipt: card.has_receipt,
        receipt_short: digest_prefix(&card.receipt_digest),
        receipt_digest: card.receipt_digest,
        receipt_sources: sources_label(&card.receipt),
        receipt_time: if card.has_receipt {
            now_label()
        } else {
            String::new()
        },
        receipt: card.receipt,
        receipt_json: card.receipt_json,
        ..CanvasCardProps::default()
    };
    if card.has_summary {
        let mut pairs = card.summary.iter().map(|pair| {
            let (value, exact) = display::cell(&pair.label, integer(&pair.label), &pair.value);
            (pair, value, exact)
        });
        if let Some((pair, value, exact)) = pairs.next() {
            canvas.has_insight = true;
            canvas.insight_label = display::label(&pair.label);
            canvas.insight = value;
            canvas.insight_exact = exact;
        }
        canvas.stats = pairs
            .map(|(pair, value, exact)| StatProps {
                id: pair.id.clone(),
                label: display::label(&pair.label),
                value,
                exact,
            })
            .collect();
        canvas.has_stats = !canvas.stats.is_empty();
    }
    if card.has_table {
        let names: Vec<String> = card.columns.iter().map(|c| c.text.clone()).collect();
        let texts: Vec<Vec<String>> = card
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|c| c.text.clone()).collect())
            .collect();
        if let Some(chart) = display::chart(&names, &texts, &view.integer_columns) {
            canvas.has_chart = true;
            canvas.chart = chart;
        }
        canvas.has_table = true;
        canvas.has_open_table = !canvas.has_chart;
        canvas.has_folded_table = canvas.has_chart;
        canvas.rows_label = format!(
            "{} {}",
            card.rows.len(),
            if card.rows.len() == 1 { "row" } else { "rows" }
        );
        canvas.columns = card
            .columns
            .iter()
            .map(|column| CellProps {
                id: column.id.clone(),
                text: display::label(&column.text),
            })
            .collect();
        canvas.rows = card
            .rows
            .iter()
            .map(|row| CanvasRowProps {
                id: row.id.clone(),
                cells: row
                    .cells
                    .iter()
                    .zip(&names)
                    .map(|(cell, name)| {
                        let numeric = integer(name);
                        let (text, exact) = display::cell(name, numeric, &cell.text);
                        CanvasCellProps {
                            id: cell.id.clone(),
                            text,
                            exact,
                            class: if numeric { "value num" } else { "value" }.to_owned(),
                        }
                    })
                    .collect(),
            })
            .collect();
    }
    canvas
}

/// A record form from the record page's model; `None` when the member may
/// not write it now or no declared command applies.
pub fn form_card(work: &str, view: &commands::RecordView) -> Option<FormCardProps> {
    if !view.can_write {
        return None;
    }
    Some(FormCardProps {
        id: format!("f-{work}-{}", view.collection),
        title: format!("New {} record", view.schema),
        command: view.command.clone(),
        action: view.action.clone(),
        csrf: view.csrf.clone(),
        idempotency_key: view.idempotency_key.clone(),
        collection: view.collection.clone(),
        publication: view.publication.clone(),
        schema: view.schema.clone(),
        schema_version: view.schema_version.clone(),
        record: view.record_id.clone(),
        record_version: view.version.clone(),
        work_version: view.work_version.clone(),
        fields: view
            .fields
            .iter()
            .map(|field| {
                let choices: Vec<ChoiceProps> = if field.options.is_empty() {
                    Vec::new()
                } else {
                    field
                        .options
                        .split(" | ")
                        .enumerate()
                        .map(|(index, value)| ChoiceProps {
                            id: format!("o{index}"),
                            value: value.to_owned(),
                        })
                        .collect()
                };
                FormFieldProps {
                    id: field.id.clone(),
                    kind: field.kind.clone(),
                    label: field.id.replace('_', " "),
                    input: field.input.clone(),
                    value: field.value.clone(),
                    spec: field.spec.clone(),
                    spec_input: field.spec_input.clone(),
                    writable: field.writable,
                    read_only: !field.writable,
                    has_text: field.writable && choices.is_empty(),
                    has_choices: field.writable && !choices.is_empty(),
                    choices,
                }
            })
            .collect(),
        submit: display::label(&view.command),
    })
}

/// The page's count line.
pub fn summary(groups: &[GroupProps]) -> String {
    let cards: usize = groups.iter().map(|g| g.cards.len() + g.forms.len()).sum();
    let apps = groups.len();
    format!(
        "{apps} {} · {cards} {}",
        if apps == 1 { "app" } else { "apps" },
        if cards == 1 { "card" } else { "cards" }
    )
}

/// Renders the canvas page; the route decides state and status.
pub trait CanvasPages: Send + Sync {
    fn canvas(&self, props: &CanvasProps) -> Response;
}

/// The Forma 3.2 native page (`canvas` in `admin/build.ts`).
pub struct SsrCanvasPages {
    pub page: PageHandle<CanvasProps>,
}
impl CanvasPages for SsrCanvasPages {
    fn canvas(&self, props: &CanvasProps) -> Response {
        match self
            .page
            .render(props, &RenderCtx::new().with_title("Canvas"))
        {
            Ok(page) => page.into_response(),
            Err(error) => {
                tracing::error!(code = error.code(), route = "/canvas", "{error}");
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
}

/// The routes' state.
pub struct Canvas {
    pub work: Arc<Work>,
    pub pages: Arc<dyn CanvasPages>,
    /// The realtime URL the page hands the island (empty: none yet).
    pub live_events: String,
    /// `GENUI_DEMO_ORGANIZATIONS` (APP-098): the organizations whose members
    /// see the demo shortcuts; every other organization is unchanged.
    pub demo_organizations: std::collections::BTreeSet<String>,
}

pub fn router(canvas: Arc<Canvas>) -> Router {
    Router::new()
        .route("/canvas", get(canvas_page))
        .route("/v1/canvas/works/{app}/{work}", get(group_json))
        .with_state(canvas)
}

/// A fresh 128-bit idempotency key per render (D7.16).
fn fresh_key() -> Option<String> {
    let bytes = capability::random_bytes::<16>()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// One group for `listed`, under the member's authority. `Ok(None)` when the
/// work is no longer authorized (it left the listing between the two reads);
/// `restore`, when given, is this request's run cookie verdict.
async fn group(
    canvas: &Canvas,
    cookie: &str,
    member: &Arc<TrustedRequest>,
    listed: &OpenableWork,
    restore: bool,
    headers: &axum::http::HeaderMap,
) -> Result<Option<GroupProps>, Response> {
    let auth = canvas.work.auth();
    let Some(stores) = &auth.stores else {
        return Err(http::not_found());
    };
    let (app, work) = (listed.app.as_str(), listed.work.as_str());
    let outline: WorkOutline = match stores.data.work_outline(member, app, work).await {
        Ok(outline) => outline,
        Err(Error::Denied) => return Ok(None),
        Err(error) => return Err(routes::storage_refusal(&error)),
    };
    let mut echo = FormValues::default();
    let mut restored = false;
    let cards = match restore.then(|| routes::restore(auth, headers, app, work, cookie)) {
        Some(Restore::Parameters(parameters)) => {
            let views = routes::run_views(
                &canvas.work,
                member.clone(),
                &outline,
                app,
                work,
                &parameters,
            )
            .await?;
            echo = routes::echo_of(&parameters);
            restored = true;
            outline
                .views
                .iter()
                .zip(views)
                .map(|(view, card)| canvas_card(app, work, view, card))
                .collect()
        }
        _ => outline
            .views
            .iter()
            .map(|view| canvas_card(app, work, view, page::card(view, page::Outcome::Idle)))
            .collect::<Vec<_>>(),
    };
    let mut forms = Vec::new();
    if listed.can_write {
        for (collection, _) in &listed.collections {
            let (Some(record), Some(key)) = (capability::random_uuid(), fresh_key()) else {
                return Err(http::unavailable());
            };
            match stores
                .data
                .read_record_detail(member, app, work, collection, &record)
                .await
            {
                Ok(detail) => {
                    let view = commands::view(
                        app,
                        work,
                        &detail,
                        auth.csrf.token(csrf::Form::Command, cookie),
                        key,
                        None,
                    );
                    forms.extend(form_card(work, &view));
                }
                Err(Error::Denied) => {}
                Err(error) => return Err(routes::storage_refusal(&error)),
            }
        }
    }
    Ok(Some(GroupProps {
        id: format!("g-{work}"),
        app: app.to_owned(),
        work: work.to_owned(),
        app_card: format!("a-{work}"),
        title: outline.title,
        label: SEEDED_LABEL.to_owned(),
        origins: seeded_origins(),
        href: format!("/w/{app}/{work}"),
        csrf: auth.csrf.token(csrf::Form::Query, cookie),
        window_start: echo.window_start,
        window_end: echo.window_end,
        destination: echo.destination,
        restored,
        has_cards: !cards.is_empty(),
        cards,
        has_forms: !forms.is_empty(),
        forms,
    }))
}

/// The member's openable works; a denial is the empty listing, as on
/// `GET /workspace`.
async fn listing(canvas: &Canvas, member: &TrustedRequest) -> Vec<OpenableWork> {
    let Some(stores) = &canvas.work.auth().stores else {
        return Vec::new();
    };
    match stores.data.openable_works(member).await {
        Ok(works) => works,
        Err(Error::Denied) => Vec::new(),
        Err(error) => {
            tracing::info!(code = %error, "canvas.listing_unavailable");
            Vec::new()
        }
    }
}

/// The member's organization display name; empty when it cannot be read
/// now (the top bar then shows the brand alone).
async fn organization_name(canvas: &Canvas, member: &TrustedRequest) -> String {
    let Some(stores) = &canvas.work.auth().stores else {
        return String::new();
    };
    match stores.workspace.organization_name(member).await {
        Ok(name) => name,
        Err(error) => {
            tracing::info!(code = %error, "canvas.organization_unavailable");
            String::new()
        }
    }
}

/// The avatar's two letters: the member label's first two hex digits.
pub fn initials(principal: &str) -> String {
    principal
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// `GET /canvas`.
async fn canvas_page(State(canvas): State<Arc<Canvas>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let auth = canvas.work.auth();
    let (cookie, member) = match routes::member(auth, &parts.headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let member = Arc::new(member);
    let mut groups = Vec::new();
    for listed in listing(&canvas, &member).await {
        match group(&canvas, &cookie, &member, &listed, true, &parts.headers).await {
            Ok(Some(group)) => groups.push(group),
            Ok(None) => {}
            Err(refusal) => return refusal,
        }
    }
    let organization = organization_name(&canvas, &member).await;
    let principal = member.principal();
    let demo = canvas.demo_organizations.contains(member.organization());
    let props = CanvasProps {
        csrf_signout: auth.csrf.token(csrf::Form::SignOut, &cookie),
        has_groups: !groups.is_empty(),
        summary: summary(&groups),
        groups,
        live_events: canvas.live_events.clone(),
        has_live: !canvas.live_events.is_empty(),
        has_organization: !organization.is_empty(),
        organization,
        member_label: crate::storage::member_label(principal),
        member_initials: initials(principal),
        demo,
        demo_mode: if demo { "on" } else { "" }.to_owned(),
    };
    canvas.pages.canvas(&props)
}

/// `GET /v1/canvas/works/{app}/{work}`: one group, idle, as JSON.
async fn group_json(
    State(canvas): State<Arc<Canvas>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, _) = request.into_parts();
    let auth = canvas.work.auth();
    let (cookie, member) = match routes::member(auth, &parts.headers).await {
        Ok(live) => live,
        Err(refusal) => return refusal,
    };
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let member = Arc::new(member);
    let Some(listed) = listing(&canvas, &member)
        .await
        .into_iter()
        .find(|listed| listed.app == app && listed.work == work)
    else {
        return http::not_found();
    };
    let group = match group(&canvas, &cookie, &member, &listed, false, &parts.headers).await {
        Ok(Some(group)) => group,
        Ok(None) => return http::not_found(),
        Err(refusal) => return refusal,
    };
    let Ok(body) = serde_json::to_vec(&group) else {
        return http::unavailable();
    };
    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

#[cfg(test)]
#[path = "canvas_tests.rs"]
mod tests;
