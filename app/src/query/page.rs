//! APP-012 work page model (`GET`/`POST /w/{app}/{work}`; plan Task 9 Tier B
//! surface). Typed presentation props for the Forma 3.2 native page
//! `app/admin/src/work/app.tsx` and the one place a view's outcome becomes
//! what the page shows.
//!
//! Every view card is in exactly one state: `idle` (not run yet), `loading`
//! (nothing computed: the engine's admission for this organization was full
//! or the request expired in its queue; the card is labelled `busy`, since
//! with no island nothing is in flight when the page renders, APP-012 review
//! m7), `ready`, `empty` (a released result with no row or only unknown
//! cells), `failed` or `denied`. Only `ready` and `empty` carry a receipt, and
//! only `ready` carries a value; a failed or denied card is built from no
//! result at all, so it can never show a number, a fabricated zero included
//! (design §6.3, C06). Values are the engine's exact strings, formatted by
//! their column's value type (APP-093 review M4): a money column's minor
//! units as currency at its exponent, a decimal column's coefficient at its
//! scale, an integer digit-grouped, text, dates and timestamps verbatim
//! (review m11). The exact string stays in each cell's `raw` (the page's
//! hover title) and the receipt's result digest covers it; the only
//! transformation is placing digits, never arithmetic.
use super::{
    receipt::QueryReceipt,
    worker::{ColumnType, WorkerResult},
};
use crate::storage::{WorkOutline, WorkView};
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use forma_ir::PageProps;
use forma_server::{PageHandle, RenderCtx};
use serde_json::Value;

/// The page's typed props (camelCase in `work/app.tsx`).
#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkProps {
    pub title: String,
    pub can_share: bool,
    pub can_edit_presentation: bool,
    pub presentation: crate::composer::presentation::PresentationProps,
    pub app: String,
    pub work: String,
    pub forms: Vec<super::canvas::FormCardProps>,
    pub record_cards: Vec<super::canvas::RecordsCardProps>,
    pub records_truncated: bool,
    /// `/w/{app}/{work}`: the run form's own route.
    pub action: String,
    /// The synchronizer token of the run form, tied to the session cookie.
    pub csrf: String,
    pub window_start: String,
    pub window_end: String,
    pub destination: String,
    /// The submitted parameters were refused (422); nothing ran.
    pub form_error: bool,
    pub views: Vec<CardProps>,
    pub parts: Vec<PartProps>,
    /// The work has runnable source views, so the page shows the run form.
    pub has_query: bool,
    pub can_window: bool,
    pub can_destination: bool,
    /// The work has no runnable source views. Records are the page's controls.
    pub records_only: bool,
    pub has_records: bool,
    pub records: Vec<RecordLinkProps>,
    /// `/canvas#g-{work}` for this work.
    pub canvas: String,
    /// APP-098 DS-84: the work reads the library's synthetic sample source,
    /// so its page says the numbers are sample data, as its canvas chip does.
    pub sample_data: bool,
    /// APP-098 DS-40: the page is shown in a listed demo organization, so it
    /// says its people and records are synthetic, as the canvas does.
    pub demo_data: bool,
}

/// One admitted part. The discriminants and closed layout classes are made
/// here, not interpreted as markup or CSS from a definition.
#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct PartProps {
    pub id: String,
    pub anchor: String,
    pub title: String,
    pub kind: String,
    pub class: String,
    pub text: String,
    pub collection: String,
    pub is_note: bool,
    pub is_form: bool,
    pub has_selector: bool,
    pub selection_name: String,
    pub selected: String,
    pub choices: Vec<SelectionProps>,
    pub message: String,
    pub forms: Vec<super::canvas::FormCardProps>,
    pub views: Vec<CardProps>,
    pub has_record_components: bool,
    pub record_components: Vec<super::record_components::RecordComponentProps>,
}

#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionProps {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordLinkProps {
    pub id: String,
    pub label: String,
    pub href: String,
}

#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CardProps {
    pub id: String,
    pub title: String,
    pub kind: String,
    /// One of `idle`, `loading`, `ready`, `empty`, `failed`, `denied`.
    pub state: String,
    /// The badge the page shows: the state's name, except that `loading`
    /// reads `busy` (nothing is in flight on a no-JS page; review m7).
    pub badge: String,
    pub message: String,
    pub idle: bool,
    pub loading: bool,
    pub ready: bool,
    pub empty: bool,
    pub failed: bool,
    pub denied: bool,
    pub has_summary: bool,
    pub summary: Vec<PairProps>,
    pub has_table: bool,
    pub columns: Vec<CellProps>,
    pub rows: Vec<RowProps>,
    pub has_chart: bool,
    pub chart: super::canvas_display::ChartProps,
    pub has_receipt: bool,
    pub receipt_digest: String,
    pub receipt: Vec<PairProps>,
    /// The receipt document exactly as the JSON route returns it.
    pub receipt_json: String,
}

#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct PairProps {
    pub id: String,
    pub label: String,
    pub value: String,
    /// A result value's exact engine string (the hover title); empty for a
    /// receipt row.
    pub raw: String,
}
#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct CellProps {
    pub id: String,
    pub text: String,
    /// A result cell's exact engine string (the hover title); empty for a
    /// column header.
    pub raw: String,
}
#[derive(PageProps, serde::Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RowProps {
    pub id: String,
    pub cells: Vec<CellProps>,
}

/// Why a view has no result. None of these carries a number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    /// Past its budget: interrupted at the deadline, late past it, or killed
    /// at the memory line (a request that expired in the admission queue is
    /// the busy card instead, since nothing ran).
    Deadline,
    /// The engine failed or answered outside the protocol.
    Engine,
    /// The bound revision could not be fetched and verified.
    SourceUnavailable,
    /// The typed query is outside the qualified operator subset.
    NotQualified,
    /// More than one page without pagination.
    NarrowTheQuery,
    /// The workspace store is unavailable.
    Store,
}
impl Reason {
    fn message(self) -> &'static str {
        match self {
            Self::Deadline => {
                "This result could not be computed: the query is past its budget (it did not finish within its deadline or its memory line). Narrow the window. No number is shown."
            }
            Self::Engine => "This result could not be computed. No number is shown.",
            Self::SourceUnavailable => {
                "This result could not be computed: its source revision is not available. No number is shown."
            }
            Self::NotQualified => {
                "This view uses an operation that is not qualified yet. No number is shown."
            }
            Self::NarrowTheQuery => {
                "This result is larger than one page. Narrow the window. No number is shown."
            }
            Self::Store => {
                "This result could not be computed: the workspace store is unavailable. No number is shown."
            }
        }
    }
}

/// A view's outcome on this request.
pub enum Outcome<'a> {
    Idle,
    /// Nothing computed: the organization's engine admission was full, or the
    /// request expired in its queue (the `busy` card).
    Loading,
    /// A released result with its receipt (`ready` or `empty` by its rows).
    Released(&'a WorkerResult, &'a QueryReceipt),
    Failed(Reason),
    Denied,
    /// APP-198: the list moved between the snapshot's capture and its
    /// release, twice over (the route captured once more); nothing is shown
    /// until a re-read, and access is not in question (`loading`, badge
    /// `stale`).
    Stale,
}

/// Exact decimal strings gain digit grouping; anything else is unchanged.
pub fn display(value: &str) -> String {
    let (sign, digits) = match value.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", value),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return value.to_owned();
    }
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    format!("{sign}{grouped}")
}

/// An exact integer string split into its sign and ASCII digits; `None`
/// for anything else (shown verbatim).
fn exact_integer(value: &str) -> Option<(bool, &str)> {
    let (negative, digits) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some((negative, digits))
}

/// `digits` with the decimal point `scale` places from the right, the
/// integer part digit-grouped (`12345`, 2 → `123.45`; `5`, 2 → `0.05`).
fn scaled(digits: &str, scale: u32) -> String {
    let scale = scale as usize;
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    let whole = display(whole.trim_start_matches('0'));
    let whole = if whole.is_empty() {
        "0".to_owned()
    } else {
        whole
    };
    if fraction.is_empty() {
        whole
    } else {
        format!("{whole}.{fraction}")
    }
}

/// The symbol a currency is shown with: `$`, `€`, `£`, or the ISO code and
/// a space.
fn currency_prefix(currency: &str) -> String {
    match currency {
        "USD" => "$".to_owned(),
        "EUR" => "€".to_owned(),
        "GBP" => "£".to_owned(),
        other => format!("{other} "),
    }
}

/// An exact engine value as its column type displays it (APP-093 review
/// M4): money minor units as currency (`26262004`, USD, 2 → `$262,620.04`),
/// a decimal coefficient at its scale, an integer digit-grouped; text
/// (digits included), dates, timestamps and booleans verbatim. A value that
/// is not an exact integer string is shown verbatim, never reinterpreted.
pub fn format_value(value: &str, kind: &ColumnType) -> String {
    let numeric = match kind {
        ColumnType::Integer | ColumnType::Money { .. } | ColumnType::Decimal { .. } => {
            exact_integer(value)
        }
        _ => None,
    };
    let Some((negative, digits)) = numeric else {
        return value.to_owned();
    };
    let sign = if negative { "-" } else { "" };
    match kind {
        ColumnType::Money {
            currency,
            minor_unit_exponent,
        } => format!(
            "{sign}{}{}",
            currency_prefix(currency),
            scaled(digits, *minor_unit_exponent)
        ),
        ColumnType::Decimal { scale } => format!("{sign}{}", scaled(digits, *scale)),
        _ => format!("{sign}{}", display(digits)),
    }
}

/// A cell's text: formatted by its column's value type when the result
/// carries one; otherwise an integer column's exact value gains digit
/// grouping and a text or timestamp column is shown verbatim, digits
/// included (review m11).
fn cell_text(value: &Value, kind: Option<&ColumnType>, integer: bool) -> String {
    match value {
        Value::String(text) => match kind {
            Some(kind) => format_value(text, kind),
            None if integer => display(text),
            None => text.clone(),
        },
        Value::Bool(flag) => flag.to_string(),
        // An unknown cell is shown as unknown, never as zero.
        _ => "unknown".to_owned(),
    }
}

/// A cell's exact engine string, for its hover title.
fn raw_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

fn pair(id: &str, label: &str, value: String) -> PairProps {
    PairProps {
        id: id.to_owned(),
        label: label.to_owned(),
        value,
        raw: String::new(),
    }
}

fn text(document: &Value, key: &str) -> String {
    match &document[key] {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The receipt inspector's rows, in a fixed order.
pub fn receipt_rows(document: &Value) -> Vec<PairProps> {
    let mut rows = vec![
        pair("digest", "Receipt digest", text(document, "digest")),
        pair("result", "Result digest", text(document, "result_digest")),
        pair("mode", "Mode", text(document, "mode")),
        pair("app", "App", text(document, "app_id")),
        pair("work", "Saved work", text(document, "work_id")),
        pair(
            "publication",
            "Publication",
            text(document, "publication_id"),
        ),
        pair(
            "definition",
            "Definition digest",
            text(document, "definition_digest"),
        ),
        pair("view", "View", text(document, "view_id")),
        pair(
            "operator",
            "Operator version",
            text(document, "operator_version"),
        ),
    ];
    for (index, source) in document["source_revisions"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        rows.push(pair(
            &format!("source-{index}"),
            &format!("Source revision ({})", text(source, "input_id")),
            format!(
                "source {} revision {} digest {}",
                text(source, "source_id"),
                text(source, "source_revision_id"),
                text(source, "source_digest")
            ),
        ));
    }
    for (index, parameter) in document["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        rows.push(pair(
            &format!("parameter-{index}"),
            &format!("Parameter {}", text(parameter, "field_id")),
            text(&parameter["value"], "value"),
        ));
    }
    rows.extend([
        pair(
            "snapshot",
            "Record snapshot sequence",
            text(document, "record_snapshot_sequence"),
        ),
        pair(
            "principal",
            if document["kind"] == "shared_query_receipt" {
                "Guest account"
            } else {
                "Principal"
            },
            text(
                document,
                if document["kind"] == "shared_query_receipt" {
                    "account_id"
                } else {
                    "principal_id"
                },
            ),
        ),
        pair(
            "membership",
            if document["kind"] == "shared_query_receipt" {
                "Share version"
            } else {
                "Membership epoch"
            },
            text(
                document,
                if document["kind"] == "shared_query_receipt" {
                    "share_version"
                } else {
                    "membership_epoch"
                },
            ),
        ),
        pair("policy", "Policy epoch", text(document, "policy_epoch")),
        pair("filter", "Filter epoch", text(document, "filter_epoch")),
        pair(
            "query",
            "Query digest",
            text(&document["provenance"], "query_digest"),
        ),
        pair(
            "predicate",
            "Effective query digest",
            text(&document["provenance"], "predicate_digest"),
        ),
        pair("scope", "Total scope", text(document, "total_scope")),
        pair("partial", "Partial", text(document, "partial")),
        pair(
            "truncated",
            "Truncated page",
            text(document, "truncated_page"),
        ),
    ]);
    rows
}

/// A result with no row, or whose every cell is unknown (an aggregate over
/// no input), is empty: it is shown as "no rows matched", never as zero.
pub fn is_empty(result: &WorkerResult) -> bool {
    result.rows.iter().all(|row| row.iter().all(Value::is_null))
}

/// The card of `view` for `outcome`.
pub fn card(view: &WorkView, outcome: Outcome<'_>) -> CardProps {
    let mut card = CardProps {
        id: view.id.clone(),
        title: view.title.clone(),
        kind: view.kind.clone(),
        ..CardProps::default()
    };
    let mut badge = None;
    let state = match outcome {
        Outcome::Idle => {
            card.idle = true;
            card.message = "Not run yet. Choose a window and run the views.".into();
            "idle"
        }
        Outcome::Loading => {
            card.loading = true;
            badge = Some("busy");
            card.message =
                "Busy: the query engine has no room for this run yet. Run again in a few seconds. Nothing is shown until the result is computed."
                    .into();
            "loading"
        }
        Outcome::Failed(reason) => {
            card.failed = true;
            card.message = reason.message().into();
            "failed"
        }
        Outcome::Denied => {
            card.denied = true;
            card.message = "You do not have access to this result. Nothing is shown.".into();
            "denied"
        }
        Outcome::Stale => {
            card.loading = true;
            badge = Some("stale");
            card.message =
                "Updating: the list changed while this view was computed. It is read again; nothing is shown until then."
                    .into();
            "loading"
        }
        Outcome::Released(result, receipt) => {
            card.has_receipt = true;
            card.receipt_digest = receipt.digest.clone();
            card.receipt = receipt_rows(&receipt.document);
            card.receipt_json = serde_json::to_string_pretty(&receipt.document).unwrap_or_default();
            if is_empty(result) {
                card.empty = true;
                card.message = "No rows matched this window and filter.".into();
                "empty"
            } else {
                card.ready = true;
                let integer = |column: usize| {
                    result
                        .columns
                        .get(column)
                        .is_some_and(|name| result.integer_columns.contains(name))
                };
                let kind = |column: usize| {
                    result
                        .columns
                        .get(column)
                        .and_then(|name| result.column_types.get(name))
                };
                if matches!(view.kind.as_str(), "summary" | "status" | "detail")
                    && result.rows.len() == 1
                {
                    card.has_summary = true;
                    card.summary = result
                        .columns
                        .iter()
                        .zip(&result.rows[0])
                        .enumerate()
                        .map(|(index, (column, value))| PairProps {
                            raw: raw_text(value),
                            ..pair(
                                &format!("m{index}"),
                                column,
                                cell_text(value, kind(index), integer(index)),
                            )
                        })
                        .collect();
                } else {
                    card.has_table = true;
                    card.columns = result
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(index, column)| CellProps {
                            id: format!("c{index}"),
                            text: column.clone(),
                            raw: String::new(),
                        })
                        .collect();
                    card.rows = result
                        .rows
                        .iter()
                        .enumerate()
                        .map(|(index, row)| RowProps {
                            id: format!("r{index}"),
                            cells: row
                                .iter()
                                .enumerate()
                                .map(|(column, value)| CellProps {
                                    id: format!("r{index}c{column}"),
                                    text: cell_text(value, kind(column), integer(column)),
                                    raw: raw_text(value),
                                })
                                .collect(),
                        })
                        .collect();
                }
                if result.truncated_page {
                    card.message = "Showing the first page of rows.".into();
                }
                if matches!(view.kind.as_str(), "chart" | "timeline") {
                    // The existing chart renderer qualifies non-negative
                    // integer series only. Money/decimal remain exact tables;
                    // a minor-unit coefficient is never labeled a currency.
                    let integer_columns = result
                        .column_types
                        .iter()
                        .filter_map(|(id, kind)| {
                            matches!(kind, ColumnType::Integer).then_some(id.clone())
                        })
                        .collect::<Vec<_>>();
                    let rows = result
                        .rows
                        .iter()
                        .map(|row| row.iter().map(raw_text).collect())
                        .collect::<Vec<Vec<String>>>();
                    if let Some(chart) =
                        super::canvas_display::chart(&result.columns, &rows, &integer_columns)
                    {
                        card.has_chart = true;
                        card.chart = chart;
                    }
                }
                "ready"
            }
        }
    };
    card.state = state.into();
    card.badge = badge.unwrap_or(state).into();
    card
}

/// The form values a page shows back.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FormValues {
    pub window_start: String,
    pub window_end: String,
    pub destination: String,
}

/// New-record links for a member who can write. Each href is a fresh record
/// id, the same shape as the workspace listing.
pub fn record_links(outline: &WorkOutline, app: &str, work: &str) -> Vec<RecordLinkProps> {
    if !outline.can_write {
        return Vec::new();
    }
    outline
        .collections
        .iter()
        .filter_map(|(collection, schema)| {
            let id = crate::auth::capability::random_uuid()?;
            Some(RecordLinkProps {
                id: id.clone(),
                label: format!("New {schema} record"),
                href: format!("/work/{app}/{work}/records/{collection}/{id}"),
            })
        })
        .collect()
}

/// The canvas group for this work.
pub fn canvas_href(work: &str) -> String {
    format!("/canvas#g-{work}")
}

/// Record links and the canvas address for one work page.
#[derive(Default)]
pub struct WorkChrome {
    pub record_run: Option<crate::storage::RecordComponentsRun>,
    /// Kept server-side for the final authorization/pin check before release.
    pub publication: String,
    pub work_version: i64,
    pub can_write: bool,
    pub can_share: bool,
    pub can_edit_presentation: bool,
    pub app: String,
    pub work: String,
    pub forms: Vec<super::canvas::FormCardProps>,
    pub form_choices: std::collections::BTreeMap<String, Vec<SelectionProps>>,
    pub record_cards: Vec<super::canvas::RecordsCardProps>,
    pub records_truncated: bool,
    pub records: Vec<RecordLinkProps>,
    pub record_components: Vec<super::record_components::RecordComponentProps>,
    pub canvas: String,
    /// APP-098 DS-40: the viewer's organization is a listed demo organization
    /// (`demo::is_demo_org`), whose people and records are synthetic.
    pub demo: bool,
}

/// Trusted admitted order/layout shared by member and read-only guest documents.
pub(crate) fn document_parts(
    outline: &WorkOutline,
    views: &[CardProps],
    chrome: &WorkChrome,
) -> Vec<PartProps> {
    let mut row = 0_u32;
    let mut declared_row = None;
    let mut occupied = 0_u16;
    outline.parts.iter().map(|part| {
        // Compact unused grid rows, and flow colliding slots to a new row so
        // admitted overlapping positions never hide another part's controls.
        let column = part.column.min(11);
        let width = part.width.clamp(1, 12 - column);
        let mask = ((1_u16 << width) - 1) << column;
        if declared_row != Some(part.row) || occupied & mask != 0 {
            row += 1;
            occupied = 0;
        }
        declared_row = Some(part.row);
        occupied |= mask;
        let forms: Vec<_> = chrome.forms.iter().filter(|form| form.id == part.id).cloned().collect();
        let choices = chrome.form_choices.get(&part.id).cloned().unwrap_or_default();
        let view = views.iter().find(|view| view.id == part.id).cloned().or_else(|| {
            (!matches!(part.kind.as_str(), "form" | "note") && !super::record_routes::is_record_kind(&part.kind)).then(|| card(&WorkView {
                id: part.id.clone(), title: part.title.clone(), kind: part.kind.clone(),
                integer_columns: Vec::new(), column_types: Default::default(),
            }, Outcome::Failed(Reason::NotQualified)))
        });
        PartProps {
            id: part.id.clone(), anchor: format!("part-{}", part.id), title: part.title.clone(), kind: part.kind.clone(),
            class: format!("document-part document-column-{column} document-width-{width} document-row-{row}"),
            text: part.text.clone(),
            collection: outline.collections.iter().find(|(_, schema)| schema == &part.schema).map(|(id, _)| id.clone()).unwrap_or_default(),
            is_note: part.kind == "note", is_form: part.kind == "form",
            has_selector: !choices.is_empty(), selection_name: format!("selection.{}", part.id),
            selected: choices.iter().find(|item| item.selected).map(|item| item.id.clone()).unwrap_or_default(),
            message: if part.kind == "form" && forms.is_empty() {
                if !outline.can_write { "This form is read only for your access." }
                else if part.creates { "This form is not available for this app revision." }
                else { "Choose a record that this command can change. No applicable record is available yet." }
            } else { "" }.into(),
            choices, forms, views: view.into_iter().collect(),
            has_record_components: super::record_routes::is_record_kind(&part.kind),
            record_components: chrome.record_components.iter().filter(|component| component.id == part.id).cloned().collect(),
        }
    }).collect()
}

/// Old definitions retain their automatic collection list. An explicitly
/// placed records list owns that collection's presentation in composed apps.
pub(crate) fn automatic_records(
    outline: &WorkOutline,
    records: Vec<super::canvas::RecordsCardProps>,
) -> Vec<super::canvas::RecordsCardProps> {
    records
        .into_iter()
        .filter(|record| {
            !outline
                .parts
                .iter()
                .any(|part| part.kind == "records_list" && part.schema == record.schema)
        })
        .collect()
}

/// The whole page.
pub fn props(
    outline: &WorkOutline,
    action: String,
    csrf: String,
    form: &FormValues,
    form_error: bool,
    views: Vec<CardProps>,
    chrome: WorkChrome,
) -> WorkProps {
    let has_query = !views.is_empty();
    let parts = document_parts(outline, &views, &chrome);
    WorkProps {
        title: outline.title.clone(),
        presentation: crate::composer::presentation::project(
            outline.presentation.as_ref(),
            &outline
                .parts
                .iter()
                .filter(|part| part.kind != "form" || outline.can_write)
                .map(|part| part.id.clone())
                .collect(),
        ),
        can_edit_presentation: chrome.can_edit_presentation,
        app: chrome.app,
        work: chrome.work,
        forms: chrome.forms,
        record_cards: chrome.record_cards,
        records_truncated: chrome.records_truncated,
        action,
        csrf,
        window_start: form.window_start.clone(),
        window_end: form.window_end.clone(),
        destination: form.destination.clone(),
        form_error,
        views,
        parts,
        has_query,
        can_window: outline.can_window,
        can_destination: outline.can_destination,
        // No runnable source views, and the work actually has collections.
        // An empty definition must not claim that it keeps records.
        records_only: !has_query && !outline.collections.is_empty(),
        has_records: !chrome.records.is_empty(),
        records: chrome.records,
        canvas: chrome.canvas,
        can_share: chrome.can_share,
        // In a demo organization the "Synthetic data" note covers the records
        // too, so the sample note (whose records are the member's own) yields.
        sample_data: outline.sample_data && !chrome.demo,
        demo_data: chrome.demo,
    }
}

/// Renders the work page; the routes decide state and status.
pub trait WorkPages: Send + Sync {
    fn work(&self, props: &WorkProps) -> Response;
}

/// The Forma 3.2 native page (`work` in `admin/build.ts`), bound and
/// finalized by the binary before it listens.
pub struct SsrWorkPages {
    pub page: PageHandle<WorkProps>,
}
impl WorkPages for SsrWorkPages {
    /// A complete `PageResponse` (fresh nonce, CSP, `no-store`); a render
    /// failure is counted and answered with a plain 500, never a shell.
    fn work(&self, props: &WorkProps) -> Response {
        match self
            .page
            .render(props, &RenderCtx::new().with_title(&props.title))
        {
            Ok(page) => page.into_response(),
            Err(error) => {
                tracing::error!(code = error.code(), route = "/w/{app}/{work}", "{error}");
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
