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
//!   work page's own authorization (the page reads every listed work at once
//!   through `Data::work_reads`, the same reads and authorization batched). A
//!   store failure on the listing renders the empty canvas, as
//!   `GET /workspace` does, so sign-out stays reachable.
//!   Cards are idle on the server, except the work whose run cookie this
//!   session set through the work page's run form (`routes::restore`): its
//!   views run again under a fresh authorization and release, exactly as the
//!   work page restores them, so a page without JavaScript still shows real
//!   results with their receipts. Nothing else runs on a `GET`.
//!   APP-104 (the library pane): the page renders every openable work as
//!   metadata only (category → app → numbered cards, from each work's outline
//!   and records listing; no view runs) and renders as groups only the works
//!   the board must show without the member's choice: the run cookie's
//!   restored work, and a work with a record waiting for the member's
//!   decision (its records card). Every other card the member brought onto
//!   the canvas (kept in this browser, DS-67) the island places through the
//!   group route below.
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
    storage::{Error, OpenableWork, RecordsListing, TrustedRequest, WorkOutline, WorkReads},
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
    /// APP-104: the library pane (metadata only), "16 apps · 88 cards", and
    /// the key the island keeps this member's placed cards under in this
    /// browser (a digest of organization and principal, DS-67).
    pub has_library: bool,
    pub library: Vec<LibraryCategoryProps>,
    pub library_summary: String,
    pub member_key: String,
}

/// APP-104: one category of the library pane ("Finance", "My apps").
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryCategoryProps {
    pub id: String,
    pub title: String,
    /// "5 apps".
    pub count: String,
    pub apps: Vec<LibraryAppProps>,
}

/// One openable work in the library pane, numbered (`3`); its cards are
/// `3.0` (the app card) and `3.1`, `3.2`, … in canvas order.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryAppProps {
    /// `lib-{work}`.
    pub id: String,
    pub app: String,
    pub work: String,
    pub number: String,
    pub title: String,
    pub has_slug: bool,
    /// "/expense" (the library command), or empty.
    pub slug: String,
    /// Lowercase words the pane's search matches.
    pub search: String,
    pub cards: Vec<LibraryCardProps>,
}

/// One card row: the canvas card id (`c-{work}-{view}`, `f-…`, `r-…`, `l-…`,
/// `a-{work}`), its number, title and kind (number, chart, table, records,
/// form, ledger, app).
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryCardProps {
    pub id: String,
    pub number: String,
    pub title: String,
    pub kind: String,
    pub search: String,
}

/// What the pane is built from for one work: its listing entry, the
/// library sidecar's grouping when it is a library app
/// (`composer::matcher::library_grouping`), and its card rows in canvas order
/// as `(id, title, kind)`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibrarySource {
    pub app: String,
    pub work: String,
    pub title: String,
    pub grouping: Option<(usize, String, String, String)>,
    pub cards: Vec<(String, String, String)>,
}

/// The library categories in pane order: capability prefix → title.
const CATEGORIES: &[(&str, &str)] = &[
    ("finance", "Finance"),
    ("security", "Security"),
    ("ops", "Operations"),
    ("operations", "Operations"),
    ("sales", "Sales"),
    ("support", "Support"),
    ("people", "People"),
];

/// A view kind as the pane names it.
pub fn card_kind(view_kind: &str) -> &'static str {
    match view_kind {
        "summary" | "status" => "number",
        "timeline" | "chart" => "chart",
        _ => "table",
    }
}

/// The library pane (APP-104): library apps grouped by their first routing
/// capability's prefix (in [`CATEGORIES`] order, then any other prefix),
/// each in sidecar order; every other work under "My apps", by title. Apps
/// are numbered 1.. in that order, so a library app keeps its number as
/// compositions are added; cards are numbered `{app}.0` (the app card) and
/// `{app}.1`… in canvas order.
pub fn library(sources: Vec<LibrarySource>) -> Vec<LibraryCategoryProps> {
    let category = |source: &LibrarySource| -> (usize, String, String) {
        match &source.grouping {
            Some((_, _, prefix, _)) => match CATEGORIES.iter().position(|(p, _)| p == prefix) {
                Some(index) => {
                    let title = CATEGORIES[index].1;
                    let first = CATEGORIES
                        .iter()
                        .position(|(_, t)| *t == title)
                        .unwrap_or(index);
                    (first, title.to_lowercase(), title.to_owned())
                }
                None if !prefix.is_empty() => {
                    (CATEGORIES.len(), prefix.clone(), display::label(prefix))
                }
                None => (CATEGORIES.len(), "library".into(), "Library".into()),
            },
            None => (CATEGORIES.len() + 1, "mine".into(), "My apps".into()),
        }
    };
    let mut sorted: Vec<((usize, String, String), LibrarySource)> = sources
        .into_iter()
        .map(|source| (category(&source), source))
        .collect();
    sorted.sort_by(|(a, x), (b, y)| {
        (a.0, &a.1)
            .cmp(&(b.0, &b.1))
            .then_with(|| {
                let position = |s: &LibrarySource| s.grouping.as_ref().map(|g| g.0);
                position(x).cmp(&position(y))
            })
            .then_with(|| x.title.cmp(&y.title))
            .then_with(|| x.work.cmp(&y.work))
    });
    let mut categories: Vec<LibraryCategoryProps> = Vec::new();
    for (number, ((_, id, title), source)) in sorted.into_iter().enumerate() {
        let number = number + 1;
        let slug = source
            .grouping
            .as_ref()
            .map(|g| g.3.clone())
            .filter(|slug| !slug.is_empty())
            .map(|slug| format!("/{slug}"))
            .unwrap_or_default();
        let cards = source
            .cards
            .iter()
            .enumerate()
            .map(|(index, (card, title, kind))| {
                let number = format!("{number}.{index}");
                LibraryCardProps {
                    id: card.clone(),
                    search: format!("{number} {} {kind}", title.to_lowercase()),
                    number,
                    title: title.clone(),
                    kind: kind.clone(),
                }
            })
            .collect();
        let app = LibraryAppProps {
            id: format!("lib-{}", source.work),
            search: format!("{number} {} {slug}", source.title.to_lowercase()),
            app: source.app,
            work: source.work,
            number: number.to_string(),
            title: source.title,
            has_slug: !slug.is_empty(),
            slug,
            cards,
        };
        match categories.last_mut() {
            Some(last) if last.id == id => last.apps.push(app),
            _ => categories.push(LibraryCategoryProps {
                id,
                title,
                count: String::new(),
                apps: vec![app],
            }),
        }
    }
    for category in &mut categories {
        let apps = category.apps.len();
        category.count = format!("{apps} {}", plural(apps, "app"));
    }
    categories
}

/// "16 apps · 88 cards" over the pane.
pub fn library_summary(categories: &[LibraryCategoryProps]) -> String {
    let apps: usize = categories.iter().map(|c| c.apps.len()).sum();
    let cards: usize = categories
        .iter()
        .flat_map(|c| &c.apps)
        .map(|a| a.cards.len())
        .sum();
    format!(
        "{apps} {} · {cards} {}",
        plural(apps, "app"),
        plural(cards, "card")
    )
}

/// The browser key of this member's placed cards (DS-67): the first 16 hex
/// digits of SHA-256 over organization and principal; not an identifier
/// the server accepts anywhere.
pub fn member_key(organization: &str, principal: &str) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(format!("{organization}\n{principal}").as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
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
/// Whether the canvas shows `organization` in demo mode: listed here and by
/// [`crate::demo::is_demo_org`] (so `GENUI_DEMO_MODE` off turns it off).
pub fn is_demo(listed: &std::collections::BTreeSet<String>, organization: &str) -> bool {
    listed.contains(organization) && crate::demo::is_demo_org(organization)
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

/// APP-101: an adapted work's chips ("Adapted to your request", then each
/// judged change, e.g. "Amount > $200"), read from the server-written first
/// line of its admitted definition's description; `None` for any other work.
pub fn adapted_origins(description: &str) -> Option<Vec<OriginProps>> {
    let chips = crate::composer::transform::chips_of(description)?;
    Some(
        std::iter::once(("adapted", "Adapted to your request".to_owned()))
            .chain(chips.into_iter().map(|chip| ("filter", chip)))
            .enumerate()
            .map(|(index, (kind, text))| OriginProps {
                id: format!("o{index}"),
                kind: kind.to_owned(),
                text,
            })
            .collect(),
    )
}

/// APP-108: the chips of a work built from a folder: "Generated" (or "Data
/// from file") and "Built from folder: <name> · N files" (kind `box`, which
/// the island makes a link to the folder).
pub fn folder_origins(lineage: &crate::storage::Lineage) -> Vec<OriginProps> {
    let noun = crate::boxes::NOUN_LOWER;
    let files = lineage.file_count;
    let first = if lineage.kind == "data" {
        ("data", "Data from file".to_owned())
    } else {
        ("generated", "Generated".to_owned())
    };
    let link = if lineage.kind == "data" {
        let name = lineage
            .files
            .first()
            .map(|(file, _)| file.name.clone())
            .unwrap_or_default();
        format!("From {noun}: {} · {name}", lineage.box_name)
    } else {
        format!(
            "Built from {noun}: {} · {files} {}",
            lineage.box_name,
            if files == 1 { "file" } else { "files" }
        )
    };
    [first, ("box", link)]
        .into_iter()
        .enumerate()
        .map(|(index, (kind, text))| OriginProps {
            id: format!("o{index}"),
            kind: kind.to_owned(),
            text,
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
    /// APP-104: why the page rendered this group without the member placing
    /// it: "restored" (the run cookie's work) or "pending" (a record waits
    /// for the member's decision); empty from the group route.
    pub auto: String,
    pub has_cards: bool,
    pub cards: Vec<CanvasCardProps>,
    pub has_forms: bool,
    pub forms: Vec<FormCardProps>,
    /// ADR-011 scenarios: the work's records (one card per collection), read
    /// through `Data::list_records` under the record page's authorization;
    /// the island refreshes them on `record.committed`.
    pub has_records: bool,
    pub records: Vec<RecordsCardProps>,
    /// APP-102: the expense ledger over the same listing (`canvas_ledger`).
    pub has_ledger: bool,
    pub ledger: Vec<super::canvas_ledger::LedgerCardProps>,
}

/// One records card (ADR-011 scenarios): a collection's current records,
/// newest first, with the decide command the member may post on a pending
/// one, and the identities the numbers come from (publication, definition
/// digest, commit sequence) kept visible as the card's provenance.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordsCardProps {
    /// `r-{work}-{collection}`.
    pub id: String,
    pub title: String,
    /// "2 waiting for approval · 5 records".
    pub summary: String,
    /// `GET /v1/work/{app}/{work}/records`.
    pub url: String,
    pub collection: String,
    pub schema: String,
    pub can_write: bool,
    /// `can_write` as the data attribute's text (the SSR subset takes no ternary).
    pub can_write_text: String,
    /// The transition command (`decide`) the member may post, or empty.
    pub decide: String,
    /// The transition field (`status`), or empty.
    pub status_field: String,
    pub count: String,
    pub pending: String,
    /// APP-104: the pending records filed by someone else, outside a library
    /// app's seeded work: a request waiting for this member's decision (the
    /// board shows this card even when the member did not place it).
    pub waiting: String,
    pub has_rows: bool,
    pub has_no_rows: bool,
    pub columns: Vec<CellProps>,
    pub rows: Vec<RecordRowProps>,
    pub publication: String,
    pub definition_digest: String,
    pub work_version: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordRowProps {
    pub id: String,
    pub record_id: String,
    pub version: String,
    pub status: String,
    /// At a declared transition's starting state and the member may decide.
    pub pending: bool,
    pub cells: Vec<CellProps>,
    pub creator: String,
    pub committed_at: String,
    pub commit_sequence: String,
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
    /// Comma-separated exact-integer columns (integer, money minor units,
    /// decimal coefficient), kept for existing consumers.
    pub integer_columns: String,
    /// The view's result column value types as compact JSON, the same
    /// object the query route returns as `column_types` (APP-093 review M4).
    pub column_types: String,
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
        column_types: super::worker::column_types_json(&view.column_types).to_string(),
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
                raw: String::new(),
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

/// A records card from a listing (ADR-011 scenarios), one per collection;
/// none for a work without collections. `principal` is the reader (APP-104:
/// a pending record they filed themselves is not waiting for them).
pub fn records_cards(
    app: &str,
    work: &str,
    listing: &RecordsListing,
    principal: &str,
) -> Vec<RecordsCardProps> {
    let seeded = crate::composer::matcher::seeded_work(work);
    listing
        .collections
        .iter()
        .map(|collection| {
            let decide = collection
                .commands
                .iter()
                .find(|c| !c.transitions.is_empty());
            let status_field = decide
                .and_then(|c| c.transitions.first())
                .map(|(field, _, _)| field.clone())
                .unwrap_or_default();
            let froms: Vec<&str> = decide
                .map(|c| {
                    c.transitions
                        .iter()
                        .map(|(_, from, _)| from.as_str())
                        .collect()
                })
                .unwrap_or_default();
            let shown: Vec<&crate::storage::RecordField> = collection
                .fields
                .iter()
                .filter(|f| f.id != status_field)
                .collect();
            let mut columns: Vec<CellProps> = shown
                .iter()
                .map(|f| CellProps {
                    id: f.id.clone(),
                    text: f.id.replace('_', " "),
                    raw: String::new(),
                })
                .collect();
            if !status_field.is_empty() {
                columns.push(CellProps {
                    id: status_field.clone(),
                    text: status_field.replace('_', " "),
                    raw: String::new(),
                });
            }
            columns.push(CellProps {
                id: "filed_by".into(),
                text: "filed by".into(),
                raw: String::new(),
            });
            let mut pending = 0usize;
            let mut waiting = 0usize;
            let rows: Vec<RecordRowProps> = listing
                .records
                .iter()
                .filter(|r| r.collection == collection.collection)
                .map(|record| {
                    let value = |field: &str| -> Option<&serde_json::Value> {
                        record
                            .values
                            .as_array()?
                            .iter()
                            .find(|item| {
                                item.get("field_id").and_then(serde_json::Value::as_str)
                                    == Some(field)
                            })
                            .and_then(|item| item.get("value"))
                    };
                    let status = value(&status_field)
                        .and_then(|v| v.get("value"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    // APP-107: pending when a declared transition may fire now
                    // (its guard holding on the record).
                    let is_pending = listing.can_write
                        && froms.contains(&status.as_str())
                        && decide.is_some_and(|c| {
                            !crate::actions::resolve::allowed_transitions(
                                &c.transitions,
                                &c.rules,
                                record,
                            )
                            .is_empty()
                        });
                    pending += usize::from(is_pending);
                    waiting += usize::from(is_pending && !seeded && record.creator != principal);
                    let mut cells: Vec<CellProps> = shown
                        .iter()
                        .map(|f| CellProps {
                            id: f.id.clone(),
                            text: value(&f.id)
                                .map(crate::actions::resolve::display)
                                .unwrap_or_else(|| "—".into()),
                            raw: String::new(),
                        })
                        .collect();
                    if !status_field.is_empty() {
                        cells.push(CellProps {
                            id: status_field.clone(),
                            text: status.clone(),
                            raw: String::new(),
                        });
                    }
                    cells.push(CellProps {
                        id: "filed_by".into(),
                        text: crate::directory::label(&record.creator),
                        raw: String::new(),
                    });
                    RecordRowProps {
                        id: format!("rec-{}", record.record_id),
                        record_id: record.record_id.clone(),
                        version: record.version.to_string(),
                        status,
                        pending: is_pending,
                        cells,
                        creator: crate::directory::label(&record.creator),
                        committed_at: record.committed_at.clone(),
                        commit_sequence: record.commit_sequence.to_string(),
                    }
                })
                .collect();
            let count = rows.len();
            let title = schema_title(&collection.schema);
            let summary = match (listing.can_write && decide.is_some(), pending) {
                (true, 0) => format!(
                    "Nothing waiting for your approval · {count} {}",
                    plural(count, "record")
                ),
                (true, n) => format!(
                    "{n} waiting for your approval · {count} {}",
                    plural(count, "record")
                ),
                (false, _) => format!("{count} {}", plural(count, "record")),
            };
            RecordsCardProps {
                id: format!("r-{work}-{}", collection.collection),
                title,
                summary,
                url: format!("/v1/work/{app}/{work}/records"),
                collection: collection.collection.clone(),
                schema: collection.schema.clone(),
                can_write: listing.can_write,
                can_write_text: listing.can_write.to_string(),
                decide: decide.map(|c| c.id.clone()).unwrap_or_default(),
                status_field,
                count: count.to_string(),
                pending: pending.to_string(),
                waiting: waiting.to_string(),
                has_rows: !rows.is_empty(),
                has_no_rows: rows.is_empty(),
                columns,
                rows,
                publication: listing.publication.clone(),
                definition_digest: short_digest(&listing.definition_digest),
                work_version: listing.work_version.to_string(),
            }
        })
        .collect()
}
/// `sha256:<64 hex>` as its first twelve and last six digits.
fn short_digest(digest: &str) -> String {
    match digest.strip_prefix("sha256:") {
        Some(hex) if hex.len() == 64 => format!("sha256:{}…{}", &hex[..12], &hex[58..]),
        _ => digest.to_owned(),
    }
}
fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}
/// "expense_claim" → "Expense claims".
pub fn schema_title(schema: &str) -> String {
    let mut words = schema.replace('_', " ");
    if let Some(first) = words.get(0..1) {
        let upper = first.to_uppercase();
        words.replace_range(0..1, &upper);
    }
    if !words.ends_with('s') {
        words.push('s');
    }
    words
}

/// The page's count line.
pub fn summary(groups: &[GroupProps]) -> String {
    let cards: usize = groups
        .iter()
        .map(|g| g.cards.len() + g.forms.len() + g.records.len())
        .sum();
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
        match self.page.render(
            props,
            // APP-108: the folder viewer frames same-origin file views.
            &RenderCtx::new()
                .with_title("Canvas")
                .with_policy(forma_server::PolicyExtensions::new().frame_src("'self'")),
        ) {
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

/// The canvas page reads every work the member may open (APP-104's library
/// pane is built from those reads), so it runs under the long deadline: on
/// the deployed database a member with the whole library exceeded the 5 s
/// handler deadline (2026-10-30, a bare 503). The listing and the reads are
/// now batched (two transactions of a fixed number of statements, APP-113's
/// deferred `open_listed` work); the long deadline stays as headroom.
pub const PATH: &str = "/canvas";

pub fn router(canvas: Arc<Canvas>) -> Router {
    Router::new()
        .route(PATH, get(canvas_page))
        .route("/v1/canvas/works/{app}/{work}", get(group_json))
        .with_state(canvas)
}

/// A fresh 128-bit idempotency key per render (D7.16).
fn fresh_key() -> Option<String> {
    capability::random_hex::<16>()
}

/// What one work reads before the page or the group route shapes it: the
/// pinned definition's outline (`Data::work_outline`, the work page's own
/// authorization) and, for a work with collections, its records listing
/// (`Data::list_records`, the record page's). Neither runs a view.
struct WorkRead {
    outline: WorkOutline,
    listing: Option<RecordsListing>,
}

/// `Ok(None)` when the work is no longer authorized (it left the listing
/// between the two reads).
async fn read_work(
    canvas: &Canvas,
    member: &Arc<TrustedRequest>,
    listed: &OpenableWork,
) -> Result<Option<WorkRead>, Response> {
    let Some(stores) = &canvas.work.auth().stores else {
        return Err(http::not_found());
    };
    let (app, work) = (listed.app.as_str(), listed.work.as_str());
    let outline: WorkOutline = match stores.data.work_outline(member, app, work).await {
        Ok(outline) => outline,
        Err(Error::Denied) => return Ok(None),
        Err(error) => return Err(routes::storage_refusal(&error)),
    };
    let listing = if listed.collections.is_empty() {
        None
    } else {
        match stores.data.list_records(member, app, work).await {
            Ok(listing) => Some(listing),
            Err(Error::Denied) => None,
            Err(error) => return Err(routes::storage_refusal(&error)),
        }
    };
    Ok(Some(WorkRead { outline, listing }))
}

/// [`read_work`]'s answer from one work of the page's batch
/// (`Data::work_reads`: the same two reads under the same authorization).
fn batched_read(read: WorkReads) -> Result<Option<WorkRead>, Error> {
    let outline = match read.outline {
        Ok(outline) => outline,
        Err(Error::Denied) => return Ok(None),
        Err(error) => return Err(error),
    };
    let listing = match read.records {
        None | Some(Err(Error::Denied)) => None,
        Some(Ok(listing)) => Some(listing),
        Some(Err(error)) => return Err(error),
    };
    Ok(Some(WorkRead { outline, listing }))
}

/// Every listed work's reads in one batch (APP-113's deferred batching: a
/// fixed number of statements however many works the member may open). A
/// member refusal is every work's `Denied`, as each work's own read is.
async fn read_works(
    canvas: &Canvas,
    member: &TrustedRequest,
    listed: &[OpenableWork],
) -> Result<Vec<Result<Option<WorkRead>, Response>>, Response> {
    let Some(stores) = &canvas.work.auth().stores else {
        return Err(http::not_found());
    };
    if listed.is_empty() {
        return Ok(Vec::new());
    }
    let works: Vec<(&str, &str, bool)> = listed
        .iter()
        .map(|w| (w.app.as_str(), w.work.as_str(), !w.collections.is_empty()))
        .collect();
    match stores.data.work_reads(member, &works).await {
        Ok(reads) => Ok(reads
            .into_iter()
            .map(|read| batched_read(read).map_err(|error| routes::storage_refusal(&error)))
            .collect()),
        Err(Error::Denied) => Ok(listed.iter().map(|_| Ok(None)).collect()),
        Err(error) => Err(routes::storage_refusal(&error)),
    }
}

/// The pane's card rows of one work, in canvas order (APP-104), from the
/// same reads the group is built from: the app card, the views, a record
/// form per collection for a writer, the records cards and the ledger.
fn library_source(listed: &OpenableWork, read: &WorkRead) -> LibrarySource {
    let (app, work) = (listed.app.as_str(), listed.work.as_str());
    let mut cards = vec![(format!("a-{work}"), "App card".to_owned(), "app".to_owned())];
    for view in &read.outline.views {
        cards.push((
            format!("c-{work}-{}", view.id),
            display::label(&view.title),
            card_kind(&view.kind).to_owned(),
        ));
    }
    if listed.can_write {
        for (collection, schema) in &listed.collections {
            cards.push((
                format!("f-{work}-{collection}"),
                format!("New {schema} record"),
                "form".to_owned(),
            ));
        }
    }
    if let Some(listing) = &read.listing {
        for card in records_cards(app, work, listing, "") {
            cards.push((card.id, card.title, "records".to_owned()));
        }
        for card in super::canvas_ledger::ledger_cards(app, work, listing) {
            cards.push((card.id, card.title, "ledger".to_owned()));
        }
    }
    // The library's own seeded work stands for the library app; a work the
    // member composed from it (retrieved, adapted) is theirs.
    let library = crate::composer::matcher::seeded_work(work);
    LibrarySource {
        app: app.to_owned(),
        work: work.to_owned(),
        // APP-120: a library app shows its catalog display title, if any.
        title: library
            .then(|| crate::composer::matcher::display_title(&read.outline.definition_digest))
            .flatten()
            .unwrap_or_else(|| read.outline.title.clone()),
        grouping: if library {
            crate::composer::matcher::library_grouping(&read.outline.definition_digest)
        } else {
            None
        },
        cards,
    }
}

/// One group for `listed`, under the member's authority (the group route).
async fn group(
    canvas: &Canvas,
    cookie: &str,
    member: &Arc<TrustedRequest>,
    listed: &OpenableWork,
) -> Result<Option<GroupProps>, Response> {
    let Some(read) = read_work(canvas, member, listed).await? else {
        return Ok(None);
    };
    assemble(canvas, cookie, member, listed, read, Restore::Absent)
        .await
        .map(Some)
}

/// The group from its reads; `restore` is this request's run cookie
/// verdict (its views run again only for `Restore::Parameters`).
async fn assemble(
    canvas: &Canvas,
    cookie: &str,
    member: &Arc<TrustedRequest>,
    listed: &OpenableWork,
    read: WorkRead,
    restore: Restore,
) -> Result<GroupProps, Response> {
    let auth = canvas.work.auth();
    let Some(stores) = &auth.stores else {
        return Err(http::not_found());
    };
    let (app, work) = (listed.app.as_str(), listed.work.as_str());
    let WorkRead { outline, listing } = read;
    // APP-121 (and the APP-120 gap): the app card's header and every
    // placement message show the app's display name: the organization's
    // label, else the catalog's display title for the library's own work,
    // else the definition's title.
    let stored = if crate::composer::matcher::seeded_work(work) {
        crate::composer::matcher::display_title(&outline.definition_digest)
            .unwrap_or_else(|| outline.title.clone())
    } else {
        outline.title.clone()
    };
    let title = crate::composer::labels::title_of(stores, member.as_ref(), app, stored).await;
    let mut echo = FormValues::default();
    let mut restored = false;
    let cards = match restore {
        Restore::Parameters(parameters) => {
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
    let (records, ledger) = match &listing {
        Some(listing) => (
            records_cards(app, work, listing, member.principal()),
            super::canvas_ledger::ledger_cards(app, work, listing),
        ),
        None => (Vec::new(), Vec::new()),
    };
    let adapted = adapted_origins(&outline.description);
    // APP-108: a work built from a folder (or a file's data card) names it.
    let folder = match stores.workspace.box_lineage(member, app, work).await {
        Ok(lineage) => lineage,
        Err(error) => {
            tracing::info!(code = %error, "canvas.lineage_unavailable");
            None
        }
    };
    let (label, origins) = match (&folder, adapted) {
        (Some(lineage), _) => (
            if lineage.kind == "data" {
                format!("Data from a file in your {} (types you confirmed; receipts name the file's revision).", crate::boxes::NOUN_LOWER)
            } else {
                format!(
                    "Generated from your {}'s capped context (typed definition, admitted).",
                    crate::boxes::NOUN_LOWER
                )
            },
            folder_origins(lineage),
        ),
        (None, Some(adapted)) => (
            "Adapted to your request from a library app (typed transform, admitted).".to_owned(),
            adapted,
        ),
        (None, None) => (SEEDED_LABEL.to_owned(), seeded_origins()),
    };
    Ok(GroupProps {
        id: format!("g-{work}"),
        app: app.to_owned(),
        work: work.to_owned(),
        app_card: format!("a-{work}"),
        title,
        label,
        origins,
        href: format!("/w/{app}/{work}"),
        csrf: auth.csrf.token(csrf::Form::Query, cookie),
        window_start: echo.window_start,
        window_end: echo.window_end,
        destination: echo.destination,
        restored,
        auto: String::new(),
        has_cards: !cards.is_empty(),
        cards,
        has_forms: !forms.is_empty(),
        forms,
        has_records: !records.is_empty(),
        records,
        has_ledger: !ledger.is_empty(),
        ledger,
    })
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
    let started = std::time::Instant::now();
    let mut groups = Vec::new();
    let mut sources = Vec::new();
    let listed_works = listing(&canvas, &member).await;
    let listed_ms = started.elapsed().as_millis();
    let reads = match read_works(&canvas, &member, &listed_works).await {
        Ok(reads) => reads,
        Err(refusal) => {
            tracing::warn!(
                event = "canvas.read_refused",
                organization_id = member.organization(),
                works = listed_works.len(),
                duration_ms = started.elapsed().as_millis() as u64,
                "canvas work reads refused"
            );
            return refusal;
        }
    };
    let read_ms = started.elapsed().as_millis();
    for (listed, read) in listed_works.iter().zip(reads) {
        let read = match read {
            Ok(Some(read)) => read,
            Ok(None) => continue,
            Err(refusal) => {
                tracing::warn!(
                    event = "canvas.read_refused",
                    organization_id = member.organization(),
                    works = listed_works.len(),
                    duration_ms = started.elapsed().as_millis() as u64,
                    "canvas work read refused"
                );
                return refusal;
            }
        };
        sources.push(library_source(listed, &read));
        let verdict = routes::restore(auth, &parts.headers, &listed.app, &listed.work, &cookie);
        let restored = matches!(verdict, Restore::Parameters(_));
        let pending = read.listing.as_ref().is_some_and(|listing| {
            records_cards(&listed.app, &listed.work, listing, member.principal())
                .iter()
                .any(|r| r.waiting != "0")
        });
        if !(restored || pending) {
            continue;
        }
        match assemble(&canvas, &cookie, &member, listed, read, verdict).await {
            Ok(mut group) => {
                group.auto = if restored { "restored" } else { "pending" }.to_owned();
                groups.push(group);
            }
            Err(refusal) => return refusal,
        }
    }
    tracing::info!(
        event = "canvas.page_built",
        organization_id = member.organization(),
        works = listed_works.len(),
        groups = groups.len(),
        listing_ms = listed_ms as u64,
        read_ms = read_ms as u64,
        duration_ms = started.elapsed().as_millis() as u64,
        "canvas page built"
    );
    let library = library(sources);
    let organization = organization_name(&canvas, &member).await;
    let principal = member.principal();
    let demo = is_demo(&canvas.demo_organizations, member.organization());
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
        has_library: !library.is_empty(),
        library_summary: library_summary(&library),
        library,
        member_key: member_key(member.organization(), principal),
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
    let group = match group(&canvas, &cookie, &member, &listed).await {
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
