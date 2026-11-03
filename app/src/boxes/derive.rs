//! Apps from a box: the capped model context, and the two rule-built
//! definitions (a file's data card; the demo organization's rehearsed
//! metrics app, APP-098 DS-73). Both are ordinary definition content,
//! admitted whole like any generated definition (ADR-011 I5) and bound to
//! the box's registered revisions; nothing here grants or runs anything.
use crate::storage::{BoxFile, FileData, ReadableSource};
use serde_json::{json, Value};

/// A registered data file in a derivation: the file, its revision, and the
/// first rows of its parsed table (display text, at most 20).
#[derive(Clone, Debug)]
pub struct DataFile {
    pub file: BoxFile,
    pub data: FileData,
    pub sample: Vec<Vec<String>>,
}

/// The source the composer pins and binds (the revision's contract).
pub fn readable(data: &FileData) -> ReadableSource {
    ReadableSource {
        source: data.source_id.clone(),
        revision: data.revision_id.clone(),
        contract_digest: data.contract_digest.clone(),
        contract: data.contract.clone(),
    }
}

/// The capped context (ticket APP-108 §5): every file's name and type (at
/// most 20), and per data file (at most 3) its input id, typed fields and at
/// most 20 sample rows. PDFs and images contribute their names only.
pub fn context(box_name: &str, files: &[BoxFile], data: &[DataFile]) -> Value {
    json!({
        "name": box_name,
        "files": files.iter().take(super::MAX_CONTEXT_FILES)
            .map(|f| json!({"name": f.name, "type": f.kind})).collect::<Vec<_>>(),
        "data": data.iter().take(super::MAX_CONTEXT_DATA_FILES).map(|d| json!({
            "input_id": d.data.contract.get("id"),
            "file": d.file.name,
            "row_count": d.data.row_count,
            "fields": d.data.contract.get("fields"),
            "sample_rows": d.sample.iter().take(super::MAX_SAMPLE_ROWS).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "note": "PDFs, images and text files are listed by name only.",
    })
}

fn fields(contract: &Value) -> Vec<(String, String)> {
    contract
        .get("fields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| {
            Some((
                f.get("id")?.as_str()?.to_owned(),
                f.get("value_type")?.get("type")?.as_str()?.to_owned(),
            ))
        })
        .collect()
}
fn reference(input: &str, field: &str) -> Value {
    json!({"input_id": input, "field_id": field})
}
fn numeric(kind: &str) -> bool {
    matches!(kind, "integer" | "decimal" | "money")
}
fn view_id(prefix: &str, suffix: &str) -> String {
    let mut id = format!("{prefix}_{suffix}");
    id.truncate(64);
    id
}
fn plain(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect()
}

/// The views over one source: a KPI summary (rows and the sums of up to four
/// numeric fields), a chart of the first numeric field by the first date or
/// text field, and the rows table.
fn source_views(contract: &Value, file_name: &str, with_chart: bool) -> Vec<Value> {
    let input = contract.get("id").and_then(Value::as_str).unwrap_or("data");
    let fields = fields(contract);
    let mut views = Vec::new();
    let Some((first, _)) = fields.first() else {
        return views;
    };
    let mut measures =
        vec![json!({"id": "rows", "operation": "count", "field": reference(input, first)})];
    for (field, _) in fields.iter().filter(|(_, k)| numeric(k)).take(4) {
        measures.push(
            json!({"id": format!("total_{field}").chars().take(64).collect::<String>(),
            "operation": "sum", "field": reference(input, field)}),
        );
    }
    views.push(
        json!({"id": view_id(input, "kpis"), "kind": "summary", "renderer_version": 1,
        "title": plain(&format!("{file_name}: totals"), 200),
        "query": {"operator_version": 1, "inputs": [input], "null_policy": "unknown",
            "operations": [{"type": "aggregate", "group_by": [], "measures": measures}]}}),
    );
    let group = fields
        .iter()
        .find(|(_, k)| matches!(k.as_str(), "date" | "timestamp"))
        .or_else(|| fields.iter().find(|(_, k)| k == "text"));
    let measure = fields.iter().find(|(_, k)| numeric(k));
    if let (true, Some((group, _)), Some((measure, _))) = (with_chart, group, measure) {
        views.push(json!({"id": view_id(input, "trend"), "kind": "chart", "renderer_version": 1,
            "title": plain(&format!("{measure} by {group}"), 200),
            "query": {"operator_version": 1, "inputs": [input], "null_policy": "unknown",
                "operations": [
                    {"type": "aggregate", "group_by": [reference(input, group)],
                     "measures": [{"id": format!("total_{measure}").chars().take(64).collect::<String>(), "operation": "sum", "field": reference(input, measure)}]},
                ]}}));
    }
    let projected: Vec<Value> = fields
        .iter()
        .take(12)
        .map(|(f, _)| reference(input, f))
        .collect();
    views.push(json!({"id": view_id(input, "rows"), "kind": "table", "renderer_version": 1,
        "title": plain(&format!("{file_name}: rows"), 200),
        "query": {"operator_version": 1, "inputs": [input], "null_policy": "unknown",
            "operations": [{"type": "project", "fields": projected}, {"type": "limit", "rows": 100}]}}));
    views
}

fn content(title: &str, description: &str, contracts: &[Value], views: Vec<Value>) -> Value {
    let layout: Vec<Value> = views
        .iter()
        .enumerate()
        .map(|(row, view)| json!({"view_id": view["id"], "column": 0, "row": row, "width": 12}))
        .collect();
    json!({
        "title": plain(title, 200),
        "description": plain(description, 1000),
        "inputs": contracts,
        "views": views,
        "record_schemas": [],
        "commands": [],
        "links": [],
        "default_layout": layout,
    })
}

/// A file's data card: its totals and its rows (a typed table with receipts).
pub fn data_definition(file_name: &str, row_count: i64, contract: &Value) -> Value {
    let columns = fields(contract).len();
    content(
        file_name,
        &format!(
            "Data from the file {file_name}: {row_count} rows, {columns} columns, types confirmed by the member."
        ),
        std::slice::from_ref(contract),
        source_views(contract, file_name, false),
    )
}

/// The rehearsed metrics app over a box's data files (demo organization
/// only, DS-73): per file its KPIs, a chart and its rows.
pub fn metrics_definition(box_name: &str, purpose: &str, data: &[DataFile]) -> Value {
    let contracts: Vec<Value> = data.iter().map(|d| d.data.contract.clone()).collect();
    let views: Vec<Value> = data
        .iter()
        .flat_map(|d| source_views(&d.data.contract, &d.file.name, true))
        .collect();
    content(
        &format!("{box_name}: {}", plain(purpose, 80)),
        &format!(
            "Built from the folder {box_name} for: {}",
            plain(purpose, 300)
        ),
        &contracts,
        views,
    )
}

/// APP-124 **Make a card**: a minimal app with exactly one card, the most
/// useful view of one data file, chosen by rule (no model): a chart of the
/// first numeric field by the first date (else text) field when the file has
/// both, else its totals, else its rows. Ordinary definition content,
/// admitted whole like every other (ADR-011 I5).
pub fn card_definition(title: &str, data: &DataFile) -> Value {
    let contract = &data.data.contract;
    let views = source_views(contract, &data.file.name, true);
    let id = |suffix: &str| {
        view_id(
            contract.get("id").and_then(Value::as_str).unwrap_or("data"),
            suffix,
        )
    };
    let chosen = ["trend", "kpis", "rows"]
        .iter()
        .find_map(|suffix| views.iter().find(|view| view["id"] == id(suffix)))
        .cloned();
    content(
        title,
        &format!(
            "One card from the file {}: the most useful view of its {} rows.",
            data.file.name, data.data.row_count
        ),
        std::slice::from_ref(contract),
        chosen.into_iter().collect(),
    )
}

/// The name a one-card app is given: the folder's, unless the folder has no
/// name of its own (Uploads, an untitled folder), then the file's stem.
pub fn card_title(box_name: &str, box_kind: &str, file_name: &str) -> String {
    let untitled = box_kind == "uploads" || box_name.to_lowercase().starts_with("untitled");
    if untitled {
        let stem = file_name
            .rsplit_once('.')
            .map_or(file_name, |(stem, _)| stem);
        plain(stem, 80)
    } else {
        plain(box_name, 80)
    }
}

/// A name as an address, exactly as the canvas derives a folder's address
/// (`boxes.ts` `folderSlug`): compatibility-decomposed (NFKD), lowercased,
/// combining marks dropped (`año` → `ano`), runs of other characters one
/// hyphen, no leading hyphen, at most 64 characters, no trailing hyphen.
/// One function for both sides, so a card's `/name` is judged against the
/// folders' addresses the canvas actually answers to.
pub fn address_slug(label: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    let mut slug = String::new();
    for c in label
        .nfkd()
        .flat_map(char::to_lowercase)
        .filter(|c| !is_combining_mark(*c))
    {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(64);
    slug.trim_end_matches('-').to_owned()
}

/// The address a one-card app is offered under: [`address_slug`] of its
/// title, starting with a letter, at most 27 characters (room for `-card`)
/// (`Q3 board pack` → `q3-board-pack`, `2026 sales` → `data-2026-sales`,
/// `Ventas año` → `ventas-ano`).
pub fn card_slug(title: &str) -> String {
    let mut slug = address_slug(title);
    if !slug.starts_with(|c: char| c.is_ascii_lowercase()) {
        slug = format!("data-{slug}");
    }
    slug.truncate(27);
    slug.trim_end_matches('-').to_owned()
}

/// The demo's rehearsed sentence (DS-73): a metrics / KPI / dashboard ask.
pub fn rehearsed(purpose: &str) -> bool {
    let lower = purpose.to_lowercase();
    ["metric", "kpi", "dashboard", "overview"]
        .iter()
        .any(|word| lower.contains(word))
}

/// Data files with distinct input ids, newest last, at most three (the
/// context cap): two files whose names give the same id cannot both be
/// inputs of one definition.
pub fn distinct(mut data: Vec<DataFile>) -> Vec<DataFile> {
    let mut seen = std::collections::BTreeSet::new();
    data.retain(|d| {
        seen.insert(
            d.data
                .contract
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        )
    });
    let skip = data.len().saturating_sub(super::MAX_CONTEXT_DATA_FILES);
    data.into_iter().skip(skip).collect()
}
