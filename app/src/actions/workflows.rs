//! APP-107: the workflow library on the action path.
//!
//! - The PO match (display only, never blocking): a purchase request is
//!   referred to as `PO-` and the first eight hex digits of its record id
//!   ([`reference`]); an invoice naming a PO is compared, by the server,
//!   with the purchase requests the member may read now (the records listing
//!   authorization, under RLS), on amount and vendor ([`po_flag`]). The flag
//!   travels with the confirmation card ([`draft_flags`]) and the records
//!   listing ([`annotate`]); it grants nothing and changes nothing.
//! - The rehearsed workflow sentences (APP-098 DS-69; demo organizations
//!   only, through `rehearsed::parse`): time off, purchase, invoice, access
//!   and discount requests and their decisions parse without a model call,
//!   into the same closed [`Parsed`] shape. Everything after the parse is the
//!   real path: typed values, the storage dry run (which applies the declared
//!   guards and initial values), the member's Confirm, the audited commit.
use super::{
    prompt::CatalogApp,
    rehearsed::{amount, approver, days_before},
    resolve::{display, DraftValue},
    Kind, Parsed,
};
use crate::storage::{RecordsListing, Stores, TrustedRequest};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The record schema whose records are purchase orders.
pub const PURCHASE_SCHEMA: &str = "purchase_request";
/// At most this many of the member's works are read for the PO match.
const MAX_PO_WORKS: usize = 24;

/// A purchase request's PO reference ("PO-3FA9C21B").
pub fn reference(record_id: &str) -> String {
    let hex: String = record_id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .take(8)
        .collect();
    format!("PO-{}", hex.to_uppercase())
}

/// One purchase request as the PO match reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Purchase {
    pub reference: String,
    pub amount: Option<i128>,
    pub vendor: String,
    pub status: String,
}

fn value_of<'a>(values: &'a Value, field: &str) -> Option<&'a Value> {
    values
        .as_array()?
        .iter()
        .find(|item| item.get("field_id").and_then(Value::as_str) == Some(field))
        .and_then(|item| item.get("value"))
}
fn minor(value: Option<&Value>) -> Option<i128> {
    value?.get("minor_units")?.as_str()?.parse().ok()
}
fn text(value: Option<&Value>) -> Option<&str> {
    value?.get("value")?.as_str()
}

/// The purchase requests of `listing`.
pub fn purchases_of(listing: &RecordsListing) -> Vec<Purchase> {
    listing
        .records
        .iter()
        .filter(|r| r.schema == PURCHASE_SCHEMA)
        .map(|r| Purchase {
            reference: reference(&r.record_id),
            amount: minor(value_of(&r.values, "amount")),
            vendor: text(value_of(&r.values, "vendor")).unwrap_or("").to_owned(),
            status: text(value_of(&r.values, "status")).unwrap_or("").to_owned(),
        })
        .collect()
}

/// Every purchase request the member may read now (bounded; a refused work
/// is skipped, exactly as the records route would refuse it).
pub async fn visible_purchases(stores: &Stores, member: &TrustedRequest) -> Vec<Purchase> {
    let Ok(works) = stores.data.openable_works(member).await else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for work in works
        .iter()
        .filter(|w| {
            w.collections
                .iter()
                .any(|(_, schema)| schema == PURCHASE_SCHEMA)
        })
        .take(MAX_PO_WORKS)
    {
        if let Ok(listing) = stores
            .data
            .list_records(member, &work.app, &work.work)
            .await
        {
            found.extend(purchases_of(&listing));
        }
    }
    found
}

fn dollars(minor: i128) -> String {
    display(
        &json!({"type": "money", "minor_units": minor.to_string(), "currency": "USD", "minor_unit_exponent": 2}),
    )
}

/// The PO match flag of an invoice naming `po` for `amount` from `vendor`.
pub fn po_flag(po: &str, amount: Option<i128>, vendor: &str, purchases: &[Purchase]) -> Value {
    let wanted = po.trim().to_uppercase();
    let Some(purchase) = purchases.iter().find(|p| p.reference == wanted) else {
        return json!({"kind": "po_match", "state": "not_found",
            "label": format!("{wanted} is not one of your purchase requests")});
    };
    let same_vendor = {
        let (a, b) = (
            vendor.trim().to_lowercase(),
            purchase.vendor.trim().to_lowercase(),
        );
        !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a))
    };
    match (amount, purchase.amount) {
        (Some(invoice), Some(ordered)) if invoice != ordered => {
            json!({"kind": "po_match", "state": "mismatch",
            "label": format!("PO mismatch: {wanted} is {}, this invoice is {}", dollars(ordered), dollars(invoice))})
        }
        _ if !same_vendor => json!({"kind": "po_match", "state": "mismatch",
            "label": format!("PO mismatch: {wanted} is from {}", purchase.vendor)}),
        _ => json!({"kind": "po_match", "state": "match",
            "label": format!("Matches {wanted} ({}, {})", purchase.amount.map(dollars).unwrap_or_default(), purchase.status.replace('_', " "))}),
    }
}

/// The flags of a confirmation card's values (an invoice naming a PO).
pub async fn draft_flags(
    stores: &Stores,
    member: &TrustedRequest,
    _schema: &str,
    values: &[DraftValue],
) -> Vec<Value> {
    let get = |field: &str| values.iter().find(|v| v.field == field).map(|v| &v.value);
    let Some(po) = text(get("po_number")).filter(|p| !p.trim().is_empty()) else {
        return Vec::new();
    };
    let purchases = visible_purchases(stores, member).await;
    vec![po_flag(
        po,
        minor(get("amount")),
        text(get("vendor")).unwrap_or(""),
        &purchases,
    )]
}

/// The records listing's display annotations: a purchase request's PO
/// reference, an invoice's PO match. `(record id → reference, record id →
/// flags)`.
pub async fn annotate(
    stores: &Stores,
    member: &TrustedRequest,
    listing: &RecordsListing,
) -> (BTreeMap<String, String>, BTreeMap<String, Vec<Value>>) {
    let mut references = BTreeMap::new();
    for record in listing
        .records
        .iter()
        .filter(|r| r.schema == PURCHASE_SCHEMA)
    {
        references.insert(record.record_id.clone(), reference(&record.record_id));
    }
    let mut flags = BTreeMap::new();
    let naming: Vec<_> = listing
        .records
        .iter()
        .filter_map(|r| {
            let po = text(value_of(&r.values, "po_number"))?.trim();
            (!po.is_empty()).then_some((r, po))
        })
        .collect();
    if !naming.is_empty() {
        let purchases = visible_purchases(stores, member).await;
        for (record, po) in naming {
            let flag = po_flag(
                po,
                minor(value_of(&record.values, "amount")),
                text(value_of(&record.values, "vendor")).unwrap_or(""),
                &purchases,
            );
            flags.insert(record.record_id.clone(), vec![flag]);
        }
    }
    (references, flags)
}

// --- Rehearsed workflow sentences (APP-098 DS-69) -------------------------

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Days since 1970-01-01 of a civil date (Howard Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
fn parse_date(date: &str) -> Option<(i64, i64, i64)> {
    let mut parts = date.split('-');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

/// The words of `text` with dashes and slashes split off ("24–26" → "24",
/// "-", "26").
fn words(text: &str) -> Vec<String> {
    text.replace(['–', '—'], " - ")
        .replace('-', " - ")
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|w| !w.is_empty())
        .map(|w| {
            w.trim_matches(|c: char| c == '.' || c == '!' || c == '?')
                .to_owned()
        })
        .filter(|w| !w.is_empty())
        .collect()
}
fn month_of(word: &str) -> Option<i64> {
    let lower = word.to_lowercase();
    (lower.len() >= 3)
        .then(|| MONTHS.iter().position(|m| lower.starts_with(m)))
        .flatten()
        .map(|i| i as i64 + 1)
}
fn day_of(word: &str) -> Option<i64> {
    let digits: String = word.chars().take_while(char::is_ascii_digit).collect();
    let rest = &word[digits.len()..];
    (!digits.is_empty() && matches!(rest, "" | "st" | "nd" | "rd" | "th"))
        .then(|| digits.parse().ok())
        .flatten()
        .filter(|d| (1..=31).contains(d))
}
fn dated(month: i64, day: i64, today: &str) -> String {
    let (year, this_month, _) = parse_date(today).unwrap_or((2026, 1, 1));
    // A month well behind today's is next year's ("Jan 5" said in October).
    let year = if month + 2 < this_month {
        year + 1
    } else {
        year
    };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Every "Month day" date in the text, with the ranges "Nov 24–26" and
/// "Nov 24 to Dec 2" as two dates.
pub fn dates(text: &str, today: &str) -> Vec<String> {
    let words = words(text);
    let mut found = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if words[i].len() == 10 && parse_date(&words[i]).is_some() {
            found.push(words[i].clone());
        }
        if let (Some(month), Some(day)) = (
            month_of(&words[i]),
            words.get(i + 1).and_then(|w| day_of(w)),
        ) {
            found.push(dated(month, day, today));
            i += 2;
            // "– 26", "to 26", "through Dec 2"
            if let Some(joiner) = words.get(i) {
                if matches!(
                    joiner.to_lowercase().as_str(),
                    "-" | "to" | "through" | "thru" | "until"
                ) {
                    if let Some(day) = words.get(i + 1).and_then(|w| day_of(w)) {
                        found.push(dated(month, day, today));
                        i += 2;
                    } else if let (Some(month), Some(day)) = (
                        words.get(i + 1).and_then(|w| month_of(w)),
                        words.get(i + 2).and_then(|w| day_of(w)),
                    ) {
                        found.push(dated(month, day, today));
                        i += 3;
                    }
                }
            }
            continue;
        }
        i += 1;
    }
    found
}

/// Monday-to-Friday days from `start` to `end` inclusive (calendar days when
/// the range has no weekday).
pub fn working_days(start: &str, end: &str) -> Option<i64> {
    let (a, b) = (parse_date(start)?, parse_date(end)?);
    let (a, b) = (
        days_from_civil(a.0, a.1, a.2),
        days_from_civil(b.0, b.1, b.2),
    );
    if b < a || b - a > 366 {
        return None;
    }
    // 1970-01-01 was a Thursday (weekday 4, Sunday 0).
    let weekdays = (a..=b)
        .filter(|d| !matches!((d + 4).rem_euclid(7), 0 | 6))
        .count() as i64;
    Some(if weekdays == 0 { b - a + 1 } else { weekdays })
}

/// The text after `marker` up to the first stop word or punctuation.
fn phrase_after(text: &str, marker: &str, stops: &[&str]) -> Option<String> {
    let lower = text.to_lowercase();
    let start = lower.find(marker)? + marker.len();
    let rest = &text[start..];
    let mut out = Vec::new();
    for word in rest.split_whitespace() {
        let clean = word.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == '!');
        if clean.is_empty()
            || stops.contains(&clean.to_lowercase().as_str())
            || clean.starts_with('$')
        {
            break;
        }
        out.push(clean.to_owned());
        if word.ends_with(',') || word.ends_with('.') || word.ends_with(';') || out.len() >= 6 {
            break;
        }
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// "Sarah approves", "Sarah and Priya approve", "Sarah, then Priya": the
/// names before an approval verb, else the single-approver forms.
fn approvers(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let clean = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_owned();
    let capital = |w: &str| clean(w).chars().next().is_some_and(char::is_uppercase);
    for (index, word) in words.iter().enumerate() {
        let lower = clean(word).to_lowercase();
        if !matches!(
            lower.as_str(),
            "approve" | "approves" | "sign" | "signs" | "decide" | "decides"
        ) {
            continue;
        }
        let mut names = Vec::new();
        let mut j = index;
        while j > 0 {
            let previous = words[j - 1];
            if previous.ends_with(',') || previous.ends_with(';') {
                break;
            }
            if capital(previous) && !matches!(clean(previous).to_lowercase().as_str(), "i" | "then")
            {
                names.insert(0, clean(previous));
                j -= 1;
            } else if matches!(clean(previous).to_lowercase().as_str(), "and" | "then")
                && !names.is_empty()
            {
                names.insert(0, "and".into());
                j -= 1;
            } else {
                break;
            }
        }
        let joined = names.join(" ");
        if !joined.is_empty() && !joined.starts_with("and") {
            return Some(joined);
        }
    }
    approver(text)
}

fn app_with<'a>(catalog: &'a [CatalogApp], schema: &str) -> Option<&'a CatalogApp> {
    catalog.iter().find(|app| app.schema == schema)
}

/// The workflow app a sentence names by its words (`None`: not a workflow).
fn named_app<'a>(lower: &str, catalog: &'a [CatalogApp]) -> Option<&'a CatalogApp> {
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let word = |words: &[&str]| words.iter().any(|w| tokens.contains(w));
    if has(&["time off", "day off", "days off", "book me off", "sick day"])
        || word(&["pto", "vacation", "leave"])
    {
        return app_with(catalog, "leave_request");
    }
    if has(&["invoice"]) {
        return app_with(catalog, "vendor_invoice");
    }
    if has(&["discount", "deal desk"]) {
        return app_with(catalog, "discount_request");
    }
    if has(&["access"]) {
        return app_with(catalog, "access_request");
    }
    if has(&["purchase", "buy ", "seats", "po-"]) || lower.starts_with("buy") {
        return app_with(catalog, PURCHASE_SCHEMA);
    }
    None
}

fn put(values: &mut Map<String, Value>, field: &str, value: impl Into<String>) {
    values.insert(field.into(), Value::String(value.into()));
}

/// A decision sentence's target status word: approve, reject, grant, deny,
/// dispute, "mark … paid".
fn decision_word(lower: &str) -> Option<&'static str> {
    let first = lower.split_whitespace().next()?;
    Some(match first.trim_matches(|c: char| !c.is_alphanumeric()) {
        "approve" | "ok" | "accept" => "approved",
        "reject" | "decline" => "rejected",
        "grant" => "granted",
        "deny" => "denied",
        "dispute" => "disputed",
        "pay" => "paid",
        "mark" if lower.contains("paid") => "paid",
        "mark" if lower.contains("disputed") => "disputed",
        _ => return None,
    })
}

/// The deterministic parse of a rehearsed workflow sentence, or `None`.
pub fn rehearsed(utterance: &str, catalog: &[CatalogApp], today: &str) -> Option<Parsed> {
    let lower = utterance.to_lowercase();
    // A slash command pins one app: its sentence needs no naming word.
    let app = named_app(&lower, catalog).or_else(|| {
        (catalog.len() == 1
            && [
                "leave_request",
                PURCHASE_SCHEMA,
                "vendor_invoice",
                "access_request",
                "discount_request",
            ]
            .contains(&catalog[0].schema.as_str()))
        .then(|| &catalog[0])
    })?;
    if let Some(status) = decision_word(&lower) {
        let skip = [
            "the", "that", "this", "please", "my", "it", "as", "paid", "disputed",
        ];
        let hint: String = utterance
            .split_whitespace()
            .skip(1)
            .filter(|w| !skip.contains(&w.to_lowercase().trim_matches(',')))
            .collect::<Vec<_>>()
            .join(" ");
        let mut values = Map::new();
        put(&mut values, "status", status);
        return Some(Parsed {
            kind: Kind::Decide,
            app_hint: app.app.clone(),
            values,
            approver_name: None,
            target_record_hint: (!hint.is_empty()).then_some(hint),
        });
    }
    let mut values = Map::new();
    let found = dates(utterance, today);
    match app.schema.as_str() {
        "leave_request" => {
            let start = found.first()?.clone();
            let end = found.get(1).cloned().unwrap_or_else(|| start.clone());
            let kind = if lower.contains("sick") {
                "sick"
            } else if lower.contains("personal") {
                "personal"
            } else {
                "vacation"
            };
            put(&mut values, "leave_type", kind);
            if let Some(days) = working_days(&start, &end) {
                put(&mut values, "days", days.to_string());
            }
            put(&mut values, "start", start);
            put(&mut values, "end", end);
            if let Some(note) = phrase_after(utterance, " for ", &["and", "with", "sarah", "so"]) {
                put(&mut values, "note", note);
            }
        }
        PURCHASE_SCHEMA => {
            put(&mut values, "amount", amount(utterance)?);
            let verb = ["buy ", "purchase ", "order "]
                .iter()
                .find_map(|v| lower.find(v).map(|at| at + v.len()))?;
            let item_end = lower[verb..].find(" from ").map(|at| verb + at)?;
            put(
                &mut values,
                "item",
                utterance[verb..item_end]
                    .trim()
                    .trim_start_matches("a ")
                    .to_owned(),
            );
            put(
                &mut values,
                "vendor",
                phrase_after(utterance, " from ", &["for", "at", "on"])?,
            );
            let center = phrase_after(utterance, "cost center ", &["and", "with"])
                .map(|c| c.to_lowercase())
                .unwrap_or_else(|| "other".into());
            put(&mut values, "cost_center", center);
            if let Some(why) = phrase_after(utterance, " because ", &["sarah", "priya"]) {
                put(&mut values, "justification", why);
            }
        }
        "vendor_invoice" => {
            put(&mut values, "amount", amount(utterance)?);
            put(
                &mut values,
                "vendor",
                phrase_after(utterance, " from ", &["for", "at", "on", "due"])?,
            );
            let joined: Vec<String> = utterance
                .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                .map(|w| {
                    w.trim_matches(|c: char| c == '.' || c == ')' || c == '(')
                        .to_owned()
                })
                .filter(|w| !w.is_empty())
                .collect();
            let number = joined.iter().find(|w| {
                let upper = w.to_uppercase();
                !upper.starts_with("PO-")
                    && w.contains('-')
                    && w.chars().any(|c| c.is_ascii_digit())
                    && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            })?;
            put(&mut values, "invoice_number", number.clone());
            if let Some(po) = joined.iter().find(|w| w.to_uppercase().starts_with("PO-")) {
                put(&mut values, "po_number", po.to_uppercase());
            }
            let due = found
                .first()
                .cloned()
                .unwrap_or_else(|| days_before(today, -30).unwrap_or_else(|| today.to_owned()));
            put(&mut values, "due_date", due);
        }
        "access_request" => {
            let role = ["admin", "write", "read"]
                .into_iter()
                .find(|r| lower.contains(r))
                .unwrap_or("read");
            put(&mut values, "role", role);
            let system = phrase_after(
                utterance,
                "access to ",
                &["until", "for", "through", "till", "because"],
            )?;
            put(
                &mut values,
                "system",
                system.trim_start_matches("the ").to_owned(),
            );
            put(
                &mut values,
                "expires_on",
                found
                    .last()
                    .cloned()
                    .unwrap_or_else(|| days_before(today, -30).unwrap_or_else(|| today.to_owned())),
            );
            let reason = phrase_after(utterance, " for ", &["until", "sarah", "priya"])
                .or_else(|| phrase_after(utterance, " because ", &["sarah", "priya"]))
                .unwrap_or_else(|| "access request".into());
            put(&mut values, "reason", reason);
        }
        "discount_request" => {
            let pct = utterance.split_whitespace().find_map(|w| {
                w.trim_matches(|c: char| !c.is_ascii_digit() && c != '%')
                    .strip_suffix('%')
                    .map(str::to_owned)
            })?;
            put(&mut values, "discount_pct", pct);
            put(&mut values, "deal_amount", amount(utterance)?);
            put(
                &mut values,
                "account",
                phrase_after(utterance, " for ", &["on", "at", "with"])?,
            );
            // The reason: the clause after the deal ("…, competitive pressure, Sarah approves").
            let reason = utterance
                .split(", ")
                .skip(1)
                .map(str::trim)
                .find(|clause| {
                    !clause.is_empty()
                        && !clause.contains('$')
                        && !clause.to_lowercase().contains("approve")
                })
                .map(str::to_owned)
                .unwrap_or_else(|| "deal desk request".into());
            put(&mut values, "reason", reason);
        }
        _ => return None,
    }
    Some(Parsed {
        kind: Kind::Create,
        app_hint: app.app.clone(),
        values,
        approver_name: approvers(utterance),
        target_record_hint: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_ranges_and_working_days() {
        assert_eq!(
            dates("Book me off Nov 24–26 for Thanksgiving", "2026-10-30"),
            ["2026-11-24", "2026-11-26"]
        );
        assert_eq!(
            dates("from Dec 30 to Jan 2", "2026-10-30"),
            ["2026-12-30", "2027-01-02"]
        );
        assert_eq!(dates("until November 30th", "2026-10-30"), ["2026-11-30"]);
        assert_eq!(working_days("2026-11-24", "2026-11-26"), Some(3));
        assert_eq!(
            working_days("2026-11-27", "2026-11-30"),
            Some(2),
            "Fri and Mon"
        );
        assert_eq!(
            working_days("2026-11-28", "2026-11-29"),
            Some(2),
            "a weekend"
        );
    }

    #[test]
    fn po_references_and_the_match_flag() {
        assert_eq!(
            reference("3fa9c21b-0000-4000-8000-000000000001"),
            "PO-3FA9C21B"
        );
        let purchases = vec![Purchase {
            reference: "PO-3FA9C21B".into(),
            amount: Some(720_000),
            vendor: "Figma".into(),
            status: "approved".into(),
        }];
        let flag =
            |po: &str, amount: i128, vendor: &str| po_flag(po, Some(amount), vendor, &purchases);
        assert_eq!(
            flag("po-3fa9c21b", 720_000, "Figma, Inc.")["state"],
            "match"
        );
        let mismatch = flag("PO-3FA9C21B", 745_000, "Figma");
        assert_eq!(mismatch["state"], "mismatch");
        assert_eq!(
            mismatch["label"],
            "PO mismatch: PO-3FA9C21B is $7,200.00, this invoice is $7,450.00"
        );
        assert_eq!(flag("PO-3FA9C21B", 720_000, "Atlas")["state"], "mismatch");
        assert_eq!(flag("PO-00000000", 720_000, "Figma")["state"], "not_found");
    }

    /// The workflow apps as the action catalog lists them, from the library fixtures.
    pub(crate) fn workflow_catalog() -> Vec<CatalogApp> {
        let library: Vec<crate::storage::LibraryApp> = [
            "expense-claims",
            "time-off",
            "purchase-requests",
            "vendor-invoices",
            "access-requests",
            "deal-desk",
        ]
        .iter()
        .map(|name| {
            let path = format!(
                "{}/../contracts/fixtures/library/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let bundle: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let definition = &bundle["definition"];
            let content = definition["content"].clone();
            let schema = &content["record_schemas"][0];
            crate::storage::LibraryApp {
                app: definition["app_id"].as_str().unwrap().into(),
                publication: bundle["publication"]["publication_id"]
                    .as_str()
                    .unwrap()
                    .into(),
                definition_digest: definition["digest"].as_str().unwrap().into(),
                title: content["title"].as_str().unwrap().into(),
                description: content["description"].as_str().unwrap().into(),
                inputs: Vec::new(),
                record_schemas: vec![(
                    schema["id"].as_str().unwrap().into(),
                    schema["fields"].clone(),
                )],
                views: Vec::new(),
                content,
            }
        })
        .collect();
        super::super::prompt::catalog(&library)
    }

    fn value<'a>(parsed: &'a Parsed, field: &str) -> &'a str {
        parsed
            .values
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    #[test]
    fn the_rehearsed_workflow_sentences_parse_into_typed_hints() {
        let catalog = workflow_catalog();
        let today = "2026-10-30";
        let parse = |text: &str| super::super::rehearsed::parse(text, &catalog, today).unwrap();
        let schema = |parsed: &Parsed| {
            catalog
                .iter()
                .find(|a| a.app == parsed.app_hint)
                .unwrap()
                .schema
                .clone()
        };

        let pto = parse("Book me off Nov 24–26 for Thanksgiving, Sarah approves");
        assert_eq!(
            (pto.kind, schema(&pto).as_str()),
            (Kind::Create, "leave_request")
        );
        assert_eq!(
            (
                value(&pto, "start"),
                value(&pto, "end"),
                value(&pto, "days")
            ),
            ("2026-11-24", "2026-11-26", "3")
        );
        assert_eq!(
            (value(&pto, "leave_type"), value(&pto, "note")),
            ("vacation", "Thanksgiving")
        );
        assert_eq!(pto.approver_name.as_deref(), Some("Sarah"));

        let buy = parse("Buy 12 Figma Enterprise seats from Figma for $7,200, cost center design, Sarah and Priya approve");
        assert_eq!(schema(&buy), "purchase_request");
        assert_eq!(
            (
                value(&buy, "item"),
                value(&buy, "vendor"),
                value(&buy, "amount"),
                value(&buy, "cost_center")
            ),
            ("12 Figma Enterprise seats", "Figma", "7200", "design")
        );
        assert_eq!(buy.approver_name.as_deref(), Some("Sarah and Priya"));
        let laptops = parse("Buy two laptops from Atlas Office Supply for $2,400, cost center engineering, Sarah approves");
        assert_eq!(schema(&laptops), "purchase_request");

        let invoice = parse(
            "Log invoice FIG-2207 from Figma for $7,450, PO-3FA9C21B, due Nov 30, Priya approves",
        );
        assert_eq!(schema(&invoice), "vendor_invoice");
        assert_eq!(
            (
                value(&invoice, "invoice_number"),
                value(&invoice, "vendor"),
                value(&invoice, "amount"),
                value(&invoice, "po_number"),
                value(&invoice, "due_date")
            ),
            ("FIG-2207", "Figma", "7450", "PO-3FA9C21B", "2026-11-30")
        );

        let access = parse("I need admin access to the production database until Nov 30 for the billing migration, Sarah approves");
        assert_eq!(schema(&access), "access_request");
        assert_eq!(
            (
                value(&access, "system"),
                value(&access, "role"),
                value(&access, "expires_on"),
                value(&access, "reason")
            ),
            (
                "production database",
                "admin",
                "2026-11-30",
                "the billing migration"
            )
        );

        let deal = parse("Request a 15% discount for Kestrel Logistics on a $120,000 deal, competitive pressure, Sarah approves");
        assert_eq!(schema(&deal), "discount_request");
        assert_eq!(
            (
                value(&deal, "account"),
                value(&deal, "discount_pct"),
                value(&deal, "deal_amount"),
                value(&deal, "reason")
            ),
            ("Kestrel Logistics", "15", "120000", "competitive pressure")
        );
        let auto = parse(
            "Request an 8% discount for Quill Education on a $48,000 renewal, multi-year commitment",
        );
        assert_eq!(
            (value(&auto, "discount_pct"), auto.approver_name.as_deref()),
            ("8", None)
        );

        for (text, status, app) in [
            (
                "approve Diego's Thanksgiving time off",
                "approved",
                "leave_request",
            ),
            (
                "approve the Figma seats purchase",
                "approved",
                "purchase_request",
            ),
            ("mark the Figma invoice paid", "paid", "vendor_invoice"),
            (
                "grant the production database access",
                "granted",
                "access_request",
            ),
            (
                "reject the Kestrel discount",
                "rejected",
                "discount_request",
            ),
        ] {
            let decided = parse(text);
            assert_eq!(
                (
                    decided.kind,
                    value(&decided, "status"),
                    schema(&decided).as_str()
                ),
                (Kind::Decide, status, app),
                "{text}"
            );
        }
        // The expense sentences are untouched.
        assert_eq!(schema(&parse("approve the Nobu dinner")), "expense_claim");
        assert_eq!(
            schema(&parse(
                "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses"
            )),
            "expense_claim"
        );
    }

    #[test]
    fn the_initial_status_comes_from_the_declared_rule_not_the_words() {
        let catalog = workflow_catalog();
        let deal = catalog
            .iter()
            .find(|a| a.schema == "discount_request")
            .unwrap();
        let rules = &deal.create_command().unwrap().rules;
        let pct = |v: &str| {
            crate::storage::rules::state_of(
                &json!([{"field_id": "discount_pct", "value": {"type": "integer", "value": v, "unit": "percent"}}]),
            )
        };
        assert_eq!(
            rules.initial_value("status", &pct("10")).unwrap().0,
            "approved"
        );
        assert_eq!(
            rules.initial_value("status", &pct("15")).unwrap().0,
            "submitted"
        );
        let purchase = catalog
            .iter()
            .find(|a| a.schema == "purchase_request")
            .unwrap();
        let guarded = purchase
            .decide_command()
            .unwrap()
            .rules
            .transitions
            .iter()
            .filter(|t| t.3.is_some())
            .count();
        assert_eq!(guarded, 2);
    }

    #[test]
    fn several_approvers_are_named_in_order() {
        assert_eq!(
            approvers("…, cost center design, Sarah and Priya approve").as_deref(),
            Some("Sarah and Priya")
        );
        assert_eq!(
            approvers("for Thanksgiving, Sarah approves").as_deref(),
            Some("Sarah")
        );
        assert_eq!(approvers("send it to Sarah").as_deref(), Some("Sarah"));
    }

    /// The real Haiku parse of one unrehearsed sentence per workflow app
    /// (ADR-011 §4: the key from the file named by `GENUI_ACT_SMOKE_KEY_FILE`,
    /// read into memory only; never printed). Prints decisions and latencies.
    #[tokio::test]
    #[ignore = "calls the real Anthropic endpoint with the key file named by GENUI_ACT_SMOKE_KEY_FILE"]
    async fn real_haiku_workflow_smoke() {
        use crate::composer::model::{Anthropic, Model, DEFAULT_ENDPOINT, DEFAULT_ROUTER_MODEL};
        use std::sync::Arc;
        let Some(path) = std::env::var_os("GENUI_ACT_SMOKE_KEY_FILE") else {
            eprintln!("skipped: GENUI_ACT_SMOKE_KEY_FILE not set");
            return;
        };
        let text = std::fs::read_to_string(path).expect("the key file");
        let key = text
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("ANTHROPIC_API_KEY=")
                    .map(|k| k.trim().trim_matches('"').to_owned())
            })
            .expect("an ANTHROPIC_API_KEY line");
        let client = Anthropic::new(&key, DEFAULT_ENDPOINT).expect("a usable key");
        drop(text);
        let parser = super::super::Parser::new(
            Some(Arc::new(client) as Arc<dyn Model>),
            DEFAULT_ROUTER_MODEL,
        );
        let catalog = workflow_catalog();
        let schema = |app: &str| {
            catalog
                .iter()
                .find(|a| a.app == app)
                .map(|a| a.schema.clone())
                .unwrap_or_else(|| "-".into())
        };
        let mut right = 0;
        for (utterance, wanted) in [
            ("I'm out sick tomorrow, letting Sarah know", "leave_request"),
            ("We need a new label printer from Atlas Office Supply, about 320 dollars, ops budget; Sarah approves", "purchase_request"),
            ("Atlas Office Supply sent bill A-99 for $120 against PO-3FA9C21B, due end of November", "vendor_invoice"),
            ("Give me read rights on hr-records till December 15 for the benefits audit, Sarah signs off", "access_request"),
            ("Brightwater Foods wants 25% off their $90,000 renewal because of a competitor bid; Sarah decides", "discount_request"),
        ] {
            let started = std::time::Instant::now();
            let parse = parser.parse(utterance, &catalog, false, "2026-10-30").await;
            let ms = started.elapsed().as_millis();
            match parse {
                Ok(parse) => {
                    let got = schema(&parse.parsed.app_hint);
                    right += usize::from(got == wanted && parse.parsed.kind == Kind::Create);
                    eprintln!(
                        "HAIKU_WORKFLOW {ms} ms  {:?}  app={got}  values={}  approver={:?}",
                        parse.parsed.kind,
                        Value::Object(parse.parsed.values.clone()),
                        parse.parsed.approver_name
                    );
                }
                Err(error) => eprintln!("HAIKU_WORKFLOW {ms} ms  refused {error:?}"),
            }
        }
        assert!(right >= 4, "{right} of 5 parsed into the right app");
    }
}
