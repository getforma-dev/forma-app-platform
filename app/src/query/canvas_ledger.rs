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
/// A record's amount in cents: 0 when it has none, `None` when the stored
/// value does not parse (shown, never silently counted as 0).
fn minor_units(record: &RecordRow, field: &str) -> Option<i128> {
    match value(record, field) {
        None | Some(Value::Null) => Some(0),
        Some(money) => money
            .get("minor_units")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok()),
    }
}

/// A money total: exact, past `i128`, or including an unreadable amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Sum {
    Cents(i128),
    TooLarge,
    Unreadable,
}
impl Default for Sum {
    fn default() -> Self {
        Self::Cents(0)
    }
}
impl Sum {
    fn add(self, amount: Option<i128>) -> Self {
        match (self, amount) {
            (Self::Unreadable, _) | (_, None) => Self::Unreadable,
            (Self::TooLarge, _) => Self::TooLarge,
            (Self::Cents(total), Some(cents)) => {
                total.checked_add(cents).map_or(Self::TooLarge, Self::Cents)
            }
        }
    }
    fn display(self) -> String {
        match self {
            Self::Cents(cents) => dollars(cents),
            Self::TooLarge => "too large".to_owned(),
            Self::Unreadable => "unreadable amount".to_owned(),
        }
    }
    /// Largest first; a total that cannot be shown sorts last.
    fn rank(self) -> i128 {
        match self {
            Self::Cents(cents) => cents,
            Self::TooLarge | Self::Unreadable => i128::MIN,
        }
    }
}
/// Cents as dollars, exactly as a record cell shows money.
pub fn dollars(minor: i128) -> String {
    display(&json!({"type": "money", "minor_units": minor.to_string()}))
}

fn rows(totals: BTreeMap<String, (usize, Sum)>, prefix: &str) -> Vec<LedgerRowProps> {
    let mut rows: Vec<(String, usize, Sum)> =
        totals.into_iter().map(|(k, (n, t))| (k, n, t)).collect();
    rows.sort_by(|a, b| b.2.rank().cmp(&a.2.rank()).then(a.0.cmp(&b.0)));
    rows.into_iter()
        .enumerate()
        .map(|(index, (label, count, total))| LedgerRowProps {
            id: format!("{prefix}{index}"),
            label,
            count: count.to_string(),
            total: total.display(),
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
            let mut people: BTreeMap<String, (usize, Sum)> = BTreeMap::new();
            let mut categories: BTreeMap<String, (usize, Sum)> = BTreeMap::new();
            let mut total = Sum::default();
            for record in &records {
                let cents = minor_units(record, &amount);
                if cents.is_none() {
                    tracing::warn!(
                        event = "ledger.unreadable_amount",
                        work_id = work,
                        record_id = record.record_id,
                        "a stored amount did not parse; the ledger shows it"
                    );
                }
                total = total.add(cents);
                let person = people
                    .entry(crate::directory::label(&record.creator))
                    .or_default();
                person.0 += 1;
                person.1 = person.1.add(cents);
                if !category.is_empty() {
                    let name = value(record, &category)
                        .and_then(|v| v.get("value"))
                        .and_then(Value::as_str)
                        .unwrap_or("—")
                        .to_owned();
                    let entry = categories.entry(name).or_default();
                    entry.0 += 1;
                    entry.1 = entry.1.add(cents);
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
                total: total.display(),
                count: records.len().to_string(),
                has_rows: !records.is_empty(),
                has_no_rows: records.is_empty(),
                people: rows(people, "p"),
                categories: rows(categories, "c"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(amount: Value) -> RecordRow {
        RecordRow {
            record_id: "r".into(),
            collection: "c".into(),
            schema: "expense_claim".into(),
            version: 1,
            commit_sequence: 1,
            committed_at: "2026-10-30T00:00:00Z".into(),
            creator: "p".into(),
            updater: "p".into(),
            values: json!([{"field_id": "amount", "value": amount}]),
        }
    }

    /// Review: the sums used `+=` on `i128` (an overflow panics in a debug
    /// build and wraps in release), and an unparseable amount counted as 0.
    #[test]
    fn sums_are_checked_and_unreadable_amounts_are_shown() {
        let money = |minor: &str| json!({"type": "money", "minor_units": minor});
        assert_eq!(minor_units(&record(money("8600")), "amount"), Some(8600));
        assert_eq!(minor_units(&record(Value::Null), "amount"), Some(0));
        assert_eq!(minor_units(&record(money("86.00")), "amount"), None);
        assert_eq!(minor_units(&record(json!("8600")), "amount"), None);
        let sum = Sum::default().add(Some(8600)).add(Some(50));
        assert_eq!(sum.display(), "$86.50");
        let huge = Sum::default().add(Some(i128::MAX)).add(Some(1));
        assert_eq!(huge, Sum::TooLarge);
        assert_eq!(huge.display(), "too large");
        assert_eq!(huge.add(Some(-5)), Sum::TooLarge);
        let unreadable = sum.add(None).add(Some(1));
        assert_eq!(unreadable.display(), "unreadable amount");
        let ranked = rows(
            [
                ("a".to_owned(), (1, huge)),
                ("b".to_owned(), (1, Sum::Cents(5))),
            ]
            .into(),
            "p",
        );
        assert_eq!(ranked[0].label, "b");
        assert_eq!(ranked[1].total, "too large");
    }
}
