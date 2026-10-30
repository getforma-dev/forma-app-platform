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
