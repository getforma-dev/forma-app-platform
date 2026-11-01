//! APP-158/159 trusted projection protocol, immutable snapshot receipts and
//! native page props. Values exist only for a complete, released execution.
use super::{
    page,
    worker::{ColumnType, Failure, QueryError},
};
use crate::{
    admission,
    storage::{record_component_digest, RecordComponentsRun},
};
use forma_ir::PageProps;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{future::Future, pin::Pin};

pub type RecordExecution<'a> =
    Pin<Box<dyn Future<Output = Result<RecordComponentsResult, QueryError>> + Send + 'a>>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordComponentsResult {
    pub document: Value,
    pub result_digest: String,
}
pub fn default_parameters() -> Value {
    json!({"filter_epoch":0,"filters":[]})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    filter_epoch: i64,
    filters: Vec<FilterValue>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum FilterValue {
    Text(TextFilter),
    Dates(DateFilter),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextFilter {
    id: String,
    value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateFilter {
    id: String,
    start: String,
    end: String,
}
pub fn parameters_shaped(value: &Value) -> bool {
    let Ok(p) = serde_json::from_value::<Parameters>(value.clone()) else {
        return false;
    };
    if !(0..=9_007_199_254_740_991).contains(&p.filter_epoch) || p.filters.len() > 8 {
        return false;
    }
    let mut ids = std::collections::BTreeSet::new();
    p.filters.iter().all(|f| match f {
        FilterValue::Text(f) => {
            crate::storage::valid_local_id(&f.id)
                && ids.insert(f.id.clone())
                && (1..=200).contains(&f.value.chars().count())
        }
        FilterValue::Dates(f) => {
            crate::storage::valid_local_id(&f.id)
                && ids.insert(f.id.clone())
                && date(&f.start)
                && date(&f.end)
                && f.start < f.end
        }
    })
}
fn date(value: &str) -> bool {
    value.len() == 10 && admission::contract_timestamp(&format!("{value}T00:00:00Z"))
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireResult {
    outcome: String,
    filter_epoch: i64,
    components: Vec<Projection>,
    elapsed_ms: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    id: String,
    kind: String,
    population: u64,
    truncated_page: bool,
    partial: bool,
    total_scope: String,
    state: String,
    rows: Vec<ProjectionRow>,
    columns: Vec<ProjectionColumn>,
    value: Option<String>,
    groups: Vec<ProjectionGroup>,
    budget: Option<ProjectionBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_type: Option<Value>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionColumn {
    id: String,
    value_type: Value,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionRow {
    id: String,
    version: i64,
    sequence: i64,
    cells: Vec<Value>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionGroup {
    label: String,
    missing: bool,
    value: String,
    count: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionBudget {
    origin: String,
    minor_units: String,
    remaining_minor_units: String,
    ratio_basis_points: Option<String>,
}
fn integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && digits.len() <= 38
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && value != "-0"
}
fn failure() -> QueryError {
    QueryError::Failed(Failure::Engine)
}
pub(crate) fn parse_result(
    run: &RecordComponentsRun,
    parameters: &Value,
    stdout: &[u8],
) -> Result<RecordComponentsResult, QueryError> {
    if stdout.len() > 1_048_576
        || stdout.last() != Some(&b'\n')
        || stdout[..stdout.len() - 1].contains(&b'\n')
    {
        return Err(failure());
    }
    let document = admission::decode(&stdout[..stdout.len() - 1]).map_err(|_| failure())?;
    if document["outcome"] == "refused" || document["outcome"] == "failed" {
        if !document.as_object().is_some_and(|o| {
            o.len() == 3 && o.contains_key("reason") && o.contains_key("elapsed_ms")
        }) || document["elapsed_ms"].as_u64().is_none()
            || !document["reason"].as_str().is_some_and(|s| {
                (1..=64).contains(&s.len())
                    && s.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
            })
        {
            return Err(failure());
        }
        return Err(if document["reason"] == "deadline" {
            QueryError::Failed(Failure::Deadline)
        } else {
            QueryError::Refused("record_projection_refused".into())
        });
    }
    let parsed: WireResult = serde_json::from_value(document.clone()).map_err(|_| failure())?;
    let declarations = run.payload()["components"].as_array().ok_or_else(failure)?;
    if parsed.outcome != "record_components"
        || Some(parsed.filter_epoch) != parameters["filter_epoch"].as_i64()
        || parsed.components.len() != declarations.len()
        || parsed.elapsed_ms > 1800
    {
        return Err(failure());
    }
    for (v, d) in parsed.components.iter().zip(declarations) {
        if v.id != d["id"]
            || v.kind != d["kind"]
            || v.population > 2000
            || v.partial
            || v.total_scope != "full_authorized_filtered_collection"
            || v.state != if v.population == 0 { "empty" } else { "ready" }
            || v.rows.len() > 200
            || v.groups.len() > 2000
            || v.value.as_ref().is_some_and(|s| !integer(s))
        {
            return Err(failure());
        }
        if v.rows.iter().any(|r| {
            !crate::storage::valid_identifier(&r.id)
                || r.version < 1
                || r.sequence < 1
                || r.cells.len() != v.columns.len()
        }) || v
            .groups
            .iter()
            .any(|g| g.label.chars().count() > 2000 || !integer(&g.value) || g.count > 2000)
        {
            return Err(failure());
        }
        if let Some(b) = &v.budget {
            if b.origin != "preset"
                || !integer(&b.minor_units)
                || b.minor_units.starts_with('-')
                || !integer(&b.remaining_minor_units)
                || b.ratio_basis_points
                    .as_ref()
                    .is_some_and(|s| !signed_decimal_integer(s))
            {
                return Err(failure());
            }
        }
    }
    let mut result_body = document.clone();
    result_body
        .as_object_mut()
        .ok_or_else(failure)?
        .remove("elapsed_ms");
    let result_digest = record_component_digest("forma:record-components-result:v1", &result_body)
        .map_err(|_| failure())?;
    Ok(RecordComponentsResult {
        document,
        result_digest,
    })
}
fn signed_decimal_integer(value: &str) -> bool {
    let v = value.strip_prefix('-').unwrap_or(value);
    !v.is_empty() && v.len() <= 43 && v.bytes().all(|c| c.is_ascii_digit())
}

/// Receipts persist independently of a page refresh and identify every current
/// record revision in the snapshot. They disclose no unselected field values.
/// Hashes prove identity, not authority; every replay must authorize afresh.
pub fn receipt(
    run: &RecordComponentsRun,
    result: &RecordComponentsResult,
    parameters: &Value,
) -> Result<Value, QueryError> {
    let declarations = run.payload()["components"].as_array().ok_or_else(failure)?;
    let coverage:Vec<Value>=result.document["components"].as_array().ok_or_else(failure)?.iter().map(|component| {
        let declaration=declarations.iter().find(|view|view["id"]==component["id"]).ok_or_else(failure)?;
        Ok(json!({"view_id":component["id"],"kind":component["kind"],"population":component["population"],"truncated_page":component["truncated_page"],"budget":declaration["records"].get("budget").cloned().unwrap_or(Value::Null)}))
    }).collect::<Result<_,QueryError>>()?;
    let mut r = json!({"kind":"record_components_receipt","schema_version":1,"organization_id":run.organization(),"app_id":run.app(),"work_id":run.work(),"publication_id":run.publication(),"definition_digest":run.definition_digest(),"work_version":run.work_version(),"policy_epoch":run.policy_epoch(),"actor":run.attribution(),"operator_version":1,"parameters":parameters,"snapshot":run.snapshot(),"snapshot_digest":run.snapshot_digest(),"result_digest":result.result_digest,"partial":false,"total_scope":"full_authorized_filtered_collection"});
    r["components"] = json!(coverage);
    let digest =
        record_component_digest("forma:record-components-receipt:v1", &r).map_err(|_| failure())?;
    r["digest"] = json!(digest);
    admission::admit_record_components_receipt(&r).map_err(|_| failure())?;
    Ok(r)
}

#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordComponentProps {
    pub card_id: String,
    pub id: String,
    pub title: String,
    pub kind: String,
    pub state: String,
    pub message: String,
    pub schema: String,
    pub ready: bool,
    pub empty: bool,
    pub failed: bool,
    pub has_summary: bool,
    pub summary: String,
    pub summary_raw: String,
    pub has_table: bool,
    pub columns: Vec<RecordColumnProps>,
    pub rows: Vec<RecordRowProps>,
    pub has_groups: bool,
    pub groups: Vec<RecordGroupProps>,
    pub population: String,
    pub truncated: bool,
    pub has_budget: bool,
    pub budget: String,
    pub remaining: String,
    pub ratio: String,
    pub filters: Vec<RecordFilterProps>,
    pub has_filters: bool,
    pub filter_epoch: i64,
    pub has_receipt: bool,
    pub receipt_digest: String,
    pub receipt_json: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordColumnProps {
    pub id: String,
    pub label: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordCellProps {
    pub id: String,
    pub text: String,
    pub raw: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordRowProps {
    pub id: String,
    pub version: String,
    pub cells: Vec<RecordCellProps>,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordGroupProps {
    pub id: String,
    pub label: String,
    pub value: String,
    pub raw: String,
    pub width: String,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordOptionProps {
    pub value: String,
    pub label: String,
    pub selected: bool,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordFilterProps {
    pub id: String,
    pub label: String,
    pub is_text: bool,
    pub is_enum: bool,
    pub is_date: bool,
    pub value: String,
    pub start: String,
    pub end: String,
    pub options: Vec<RecordOptionProps>,
}
fn column_type(value: &Value) -> ColumnType {
    match value["type"].as_str() {
        Some("money") => ColumnType::Money {
            currency: value["currency"].as_str().unwrap_or("USD").into(),
            minor_unit_exponent: value["minor_unit_exponent"].as_u64().unwrap_or(2) as u32,
        },
        _ => ColumnType::Integer,
    }
}
fn shown(value: &Value) -> (String, String) {
    let kind = value["type"].as_str().unwrap_or("null");
    if kind == "null" {
        return ("—".into(), String::new());
    }
    if kind == "reference" {
        let id = value["record_id"].as_str().unwrap_or_default().to_owned();
        return (
            format!("{}: {}", value["input_id"].as_str().unwrap_or("Record"), id),
            id,
        );
    }
    let raw = match kind {
        "money" => value["minor_units"].as_str().unwrap_or_default().to_owned(),
        "decimal" => value["coefficient"].as_str().unwrap_or_default().to_owned(),
        _ => value["value"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value["value"].to_string()),
    };
    let display = match kind {
        "money" => page::format_value(&raw, &column_type(value)),
        "integer" => page::format_value(&raw, &ColumnType::Integer),
        "decimal" => page::format_value(
            &raw,
            &ColumnType::Decimal {
                scale: value["scale"].as_u64().unwrap_or(0) as u32,
            },
        ),
        _ => raw.clone(),
    };
    (display, raw)
}
fn filter_owner<'a>(components: &'a Value, filter: &Value) -> Option<&'a str> {
    let targets = filter["targets"].as_array()?;
    let components = components.as_array()?;
    components
        .iter()
        .find(|c| c["kind"] == "records_list" && targets.contains(&c["id"]))
        .or_else(|| components.iter().find(|c| targets.contains(&c["id"])))?["id"]
        .as_str()
}
fn base(run: &RecordComponentsRun, parameters: &Value) -> Vec<RecordComponentProps> {
    run.payload()["components"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let filters = run.payload()["filters"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| filter_owner(&run.payload()["components"], f) == v["id"].as_str())
                .map(|f| {
                    let id = f["id"].as_str().unwrap_or_default();
                    let input = parameters["filters"]
                        .as_array()
                        .and_then(|fs| fs.iter().find(|p| p["id"] == id));
                    let field = run.payload()["datasets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|d| d["input_id"] == f["input_id"])
                        .and_then(|d| d["fields"].as_array())
                        .and_then(|fs| fs.iter().find(|field| field["id"] == f["field"]));
                    let current = input.and_then(|p| p["value"].as_str()).unwrap_or_default();
                    RecordFilterProps {
                        id: id.into(),
                        label: super::canvas_display::label(f["field"].as_str().unwrap_or(id)),
                        is_text: f["kind"] == "text_contains",
                        is_enum: f["kind"] == "enum",
                        is_date: f["kind"] == "date_range",
                        value: current.into(),
                        start: input
                            .and_then(|p| p["start"].as_str())
                            .unwrap_or_default()
                            .into(),
                        end: input
                            .and_then(|p| p["end"].as_str())
                            .unwrap_or_default()
                            .into(),
                        options: field
                            .and_then(|f| f["value_type"]["values"].as_array())
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str())
                            .map(|v| RecordOptionProps {
                                value: v.into(),
                                label: v.into(),
                                selected: v == current,
                            })
                            .collect(),
                    }
                })
                .collect::<Vec<_>>();
            RecordComponentProps {
                id: v["id"].as_str().unwrap_or_default().into(),
                title: v["title"].as_str().unwrap_or_default().into(),
                kind: v["kind"].as_str().unwrap_or_default().into(),
                schema: v["records"]["schema_id"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                has_filters: !filters.is_empty(),
                filters,
                filter_epoch: parameters["filter_epoch"].as_i64().unwrap_or(0),
                ..Default::default()
            }
        })
        .collect()
}
pub fn unavailable(run: &RecordComponentsRun, message: &str) -> Vec<RecordComponentProps> {
    unavailable_with_parameters(run, &default_parameters(), message)
}
pub fn unavailable_with_parameters(
    run: &RecordComponentsRun,
    parameters: &Value,
    message: &str,
) -> Vec<RecordComponentProps> {
    base(run, parameters)
        .into_iter()
        .map(|mut p| {
            p.state = "failed".into();
            p.failed = true;
            p.message = message.into();
            p
        })
        .collect()
}
pub fn props(
    run: &RecordComponentsRun,
    result: &RecordComponentsResult,
    parameters: &Value,
) -> Result<Vec<RecordComponentProps>, QueryError> {
    let receipt = receipt(run, result, parameters)?;
    let wire: WireResult =
        serde_json::from_value(result.document.clone()).map_err(|_| failure())?;
    let mut props = base(run, parameters);
    for (p, r) in props.iter_mut().zip(wire.components) {
        p.state = r.state;
        p.ready = true;
        p.empty = r.population == 0;
        p.population = r.population.to_string();
        p.truncated = r.truncated_page;
        p.has_receipt = true;
        p.receipt_digest = receipt["digest"].as_str().unwrap_or_default().into();
        p.receipt_json = serde_json::to_string(&receipt).map_err(|_| failure())?;
        p.has_table = p.kind == "records_list";
        p.columns = r
            .columns
            .iter()
            .map(|c| RecordColumnProps {
                id: c.id.clone(),
                label: super::canvas_display::label(&c.id),
            })
            .collect();
        p.rows = r
            .rows
            .into_iter()
            .map(|row| RecordRowProps {
                id: row.id,
                version: row.version.to_string(),
                cells: row
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let (text, raw) = shown(v);
                        RecordCellProps {
                            id: p.columns[i].id.clone(),
                            text,
                            raw,
                        }
                    })
                    .collect(),
            })
            .collect();
        let value_type = r
            .value_type
            .as_ref()
            .map(column_type)
            .unwrap_or(ColumnType::Integer);
        if let Some(value) = r.value {
            p.has_summary = true;
            p.summary = page::format_value(&value, &value_type);
            p.summary_raw = value;
        }
        let max = r
            .groups
            .iter()
            .filter_map(|g| g.value.parse::<i128>().ok())
            .map(i128::unsigned_abs)
            .max()
            .unwrap_or(0);
        p.has_groups = !r.groups.is_empty();
        p.groups = r
            .groups
            .into_iter()
            .enumerate()
            .map(|(i, g)| {
                let n = g
                    .value
                    .parse::<i128>()
                    .ok()
                    .map(i128::unsigned_abs)
                    .unwrap_or(0);
                // Pure display geometry, separate from exact value. Dividing first
                // avoids multiplication overflow for the largest admitted money.
                let width = if max == 0 {
                    0
                } else {
                    ((n as f64 / max as f64) * 100.0).round().clamp(0.0, 100.0) as u32
                };
                RecordGroupProps {
                    id: i.to_string(),
                    label: if g.missing {
                        format!("{} (not entered)", g.label)
                    } else {
                        g.label
                    },
                    value: page::format_value(&g.value, &value_type),
                    raw: g.value,
                    width: format!("{width}%"),
                }
            })
            .collect();
        if let Some(b) = r.budget {
            p.has_budget = true;
            p.budget = format!(
                "Preset budget: {}",
                page::format_value(&b.minor_units, &value_type)
            );
            p.remaining = format!(
                "{} remaining",
                page::format_value(&b.remaining_minor_units, &value_type)
            );
            p.ratio = b
                .ratio_basis_points
                .map(|bp| format!("{}% used", fixed_decimal(&bp, 2)))
                .unwrap_or_else(|| "Percentage unavailable: preset budget is zero".into());
        }
        p.message = if p.empty {
            "No matching records".into()
        } else if p.truncated {
            format!(
                "Showing a bounded page of {} matching records; totals cover all matches.",
                p.population
            )
        } else {
            format!("{} matching records", p.population)
        };
    }
    Ok(props)
}
fn fixed_decimal(value: &str, places: usize) -> String {
    let (negative, digits) = value
        .strip_prefix('-')
        .map(|v| ("-", v))
        .unwrap_or(("", value));
    let padded = format!("{:0>width$}", digits, width = places + 1);
    format!(
        "{negative}{}.{}",
        &padded[..padded.len() - places],
        &padded[padded.len() - places..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_filter_has_one_list_owner() {
        let components = json!([{"id":"sum","kind":"record_summary"},{"id":"rows","kind":"records_list"},{"id":"second","kind":"records_list"}]);
        assert_eq!(
            filter_owner(&components, &json!({"targets":["sum","rows","second"]})),
            Some("rows")
        );
        assert_eq!(
            filter_owner(&components, &json!({"targets":["sum"]})),
            Some("sum")
        );
        assert!(!parameters_shaped(
            &json!({"filter_epoch":0,"filters":[{"id":"period","start":"2026-02-30","end":"2026-03-05"}]})
        ));
    }
    #[test]
    fn body_is_closed_and_bounded() {
        assert!(parameters_shaped(&default_parameters()));
        assert!(!parameters_shaped(
            &json!({"filter_epoch":0,"filters":[],"app":"foreign"})
        ));
        assert!(!parameters_shaped(
            &json!({"filter_epoch":0,"filters":[{"id":"name","value":"x"},{"id":"name","value":"y"}]})
        ));
        assert!(!parameters_shaped(&json!({"filter_epoch":-1,"filters":[]})));
    }
    #[test]
    fn exact_money_and_ratio_display() {
        assert_eq!(shown(&json!({"type":"money","minor_units":"4500","currency":"USD","minor_unit_exponent":2})).0,"$45.00");
        assert_eq!(fixed_decimal("4500", 2), "45.00");
        assert_eq!(fixed_decimal("-1", 2), "-0.01");
    }
}
