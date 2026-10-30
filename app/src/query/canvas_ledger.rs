//! APP-102: the expense ledger card, server-rendered first like the records
//! card (`canvas::records_cards`), then refreshed by the island
//! (`admin/src/canvas/ledger.ts`) on every `record.committed` of its work.
//! One card per collection whose schema has a money field and a date field.
//! The totals here are the server's first paint over the whole listing (all
//! dates, every status); the island recomputes them for a date range and a
//! status filter over the same listing. Aggregation is client-side over the
//! listed live records, at most `RECORDS_LIMIT` and labeled so (DS-58); the
//! engine view over records inputs is PH02 Track C (DS-47).
use crate::{
    actions::resolve::display,
    storage::{RecordRow, RecordsListing},
};
use forma_ir::PageProps;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The scope label every ledger carries (DS-58).
pub const SCOPE: &str = "live records, last 200";

#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct LedgerCardProps {
    /// `l-{work}-{collection}`.
    pub id: String,
    pub title: String,
    /// `GET /v1/work/{app}/{work}/records`.
    pub url: String,
    pub collection: String,
    pub amount_field: String,
    pub date_field: String,
    pub category_field: String,
    pub status_field: String,
    pub scope: String,
    pub total: String,
    pub count: String,
    pub has_rows: bool,
    pub has_no_rows: bool,
    pub people: Vec<LedgerRowProps>,
    pub categories: Vec<LedgerRowProps>,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct LedgerRowProps {
    pub id: String,
    pub label: String,
    pub count: String,
    pub total: String,
}

fn value<'a>(record: &'a RecordRow, field: &str) -> Option<&'a Value> {
    record
        .values
        .as_array()?
        .iter()
        .find(|item| item.get("field_id").and_then(Value::as_str) == Some(field))
        .and_then(|item| item.get("value"))
}
fn minor_units(record: &RecordRow, field: &str) -> i128 {
    value(record, field)
        .and_then(|v| v.get("minor_units"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
/// Cents as dollars, exactly as a record cell shows money.
pub fn dollars(minor: i128) -> String {
    display(&json!({"type": "money", "minor_units": minor.to_string()}))
}

fn rows(totals: BTreeMap<String, (usize, i128)>, prefix: &str) -> Vec<LedgerRowProps> {
    let mut rows: Vec<(String, usize, i128)> =
        totals.into_iter().map(|(k, (n, t))| (k, n, t)).collect();
    rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    rows.into_iter()
        .enumerate()
        .map(|(index, (label, count, total))| LedgerRowProps {
            id: format!("{prefix}{index}"),
            label,
            count: count.to_string(),
            total: dollars(total),
        })
        .collect()
}

/// The ledger cards of a listing (none for a work without money and date
/// fields).
pub fn ledger_cards(app: &str, work: &str, listing: &RecordsListing) -> Vec<LedgerCardProps> {
    listing
        .collections
        .iter()
        .filter_map(|collection| {
            let kind = |id: &str| {
                collection
                    .fields
                    .iter()
                    .find(|f| f.value_type.get("type").and_then(Value::as_str) == Some(id))
                    .map(|f| f.id.clone())
            };
            let amount = kind("money")?;
            let date = kind("date")?;
            let status = collection
                .commands
                .iter()
                .find_map(|c| c.transitions.first().map(|(f, _, _)| f.clone()))
                .unwrap_or_default();
            let category = collection
                .fields
                .iter()
                .find(|f| {
                    f.id != status
                        && f.value_type.get("type").and_then(Value::as_str) == Some("enum")
                })
                .map(|f| f.id.clone())
                .unwrap_or_default();
            let records: Vec<&RecordRow> = listing
                .records
                .iter()
                .filter(|r| r.collection == collection.collection)
                .collect();
            let mut people: BTreeMap<String, (usize, i128)> = BTreeMap::new();
            let mut categories: BTreeMap<String, (usize, i128)> = BTreeMap::new();
            let mut total = 0i128;
            for record in &records {
                let cents = minor_units(record, &amount);
                total += cents;
                let person = people
                    .entry(crate::directory::label(&record.creator))
                    .or_default();
                person.0 += 1;
                person.1 += cents;
                if !category.is_empty() {
                    let name = value(record, &category)
                        .and_then(|v| v.get("value"))
                        .and_then(Value::as_str)
                        .unwrap_or("—")
                        .to_owned();
                    let entry = categories.entry(name).or_default();
                    entry.0 += 1;
                    entry.1 += cents;
                }
            }
            let title = if collection.schema.starts_with("expense") {
                "Expense ledger".to_owned()
            } else {
                format!("{} ledger", super::canvas::schema_title(&collection.schema))
            };
            Some(LedgerCardProps {
                id: format!("l-{work}-{}", collection.collection),
                title,
                url: format!("/v1/work/{app}/{work}/records"),
                collection: collection.collection.clone(),
                amount_field: amount,
                date_field: date,
                category_field: category,
                status_field: status,
                scope: SCOPE.to_owned(),
                total: dollars(total),
                count: records.len().to_string(),
                has_rows: !records.is_empty(),
                has_no_rows: records.is_empty(),
                people: rows(people, "p"),
                categories: rows(categories, "c"),
            })
        })
        .collect()
}
