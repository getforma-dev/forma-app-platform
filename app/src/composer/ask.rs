//! APP-122 Ask: a read-only conversation about the records in scope
//! (docs/pitch/CANVAS-RULES.md, "Ask and Shape").
//!
//! Where the member types decides the job, never the model: Ask is the Ask
//! tab (or `?` in the bar), and it can only answer. Structurally:
//!
//! - the route reads the scoped works in one `READ ONLY` transaction
//!   (`storage::Data::read_only_listings`), so no Ask path can write;
//! - the answer is one closed shape, `{text, references}` ([`judge`]): plain
//!   text and the opaque references (`r1`, `r2` …) of the records it used.
//!   Any other key is refused; a view spec (`presentation`, `filters`,
//!   `colors` …) is refused by name (`view_spec`), so a question can never
//!   become a change to a card (ADR-011 I5: model output is data);
//! - the receipts are the server's: each reference maps to its record's
//!   provenance (record id, version, commit sequence, the work's publication
//!   and definition digest; ADR-011 I3), never to model text.
//!
//! Asked to change something, Ask answers with the address to type
//! (`/shape expenses.ledger …`); it never switches jobs.
//!
//! In a demo organization (APP-098, ADR-011: a concept-car demo), the
//! rehearsed questions ([`rehearsed`]: the red/waiting records, a count, a
//! total, a status) are answered from the same records without a model call;
//! anything else goes to the model, and without a model the route answers
//! `composer_unavailable`, as the widget customizer does.
//!
//! Listings are capped (`storage::RECORDS_LIMIT` newest records per work).
//! When a work in scope has more, the read is *partial*: every answer says
//! so ("among the newest 200 …"), a count reads "at least", and a total is
//! never presented as the total. Every computed number (a count, a total, a
//! list) references every record it counted, so its receipts cover all the
//! rows it used (ADR-011 I3). Money is added per currency, never across.
use crate::actions::resolve;
use crate::storage::{RecordRow, RecordsListing, RECORDS_LIMIT};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// A question: 1 to 500 characters, no control characters.
pub const MAX_QUESTION: usize = 500;
/// The answer's text: at most this many characters.
pub const MAX_TEXT: usize = 1500;
/// At most this many references (and receipts) in a model's answer: as many
/// as the records it is shown, so a number it states can cite every record
/// it used. A rehearsed (computed) answer references every record it counted.
pub const MAX_REFERENCES: usize = MAX_MODEL_RECORDS;
/// At most this many records go to the model.
pub const MAX_MODEL_RECORDS: usize = 200;
/// How many records a rehearsed answer names in its text.
const NAMED: usize = 8;

/// Keys of a view, theme or style spec: an Ask answer carrying one is refused
/// as `view_spec` (the separation is structural, APP-122 §4).
pub const VIEW_KEYS: &[&str] = &[
    "presentation",
    "palette",
    "columns",
    "filters",
    "highlight",
    "group_by",
    "bucket",
    "measure",
    "sort",
    "limit",
    "title",
    "receipts",
    "mode",
    "accent",
    "density",
    "skin",
    "font",
    "chart",
    "color",
    "colors",
    "view",
    "spec",
    "theme",
    "style",
];

/// Status values that read as "red": a record waiting on a decision, or one
/// refused, disputed or flagged.
pub const ATTENTION: &[&str] = &[
    "submitted",
    "pending",
    "requested",
    "waiting",
    "open",
    "overdue",
    "unpaid",
    "rejected",
    "denied",
    "declined",
    "disputed",
    "needs_context",
    "flagged",
    "mismatch",
    "failed",
    "blocked",
    "escalated",
];

/// A question's text, trimmed, or `None`.
pub fn question(value: &str) -> Option<&str> {
    let trimmed = value.trim().trim_start_matches('?').trim();
    let count = trimmed.chars().count();
    ((1..=MAX_QUESTION).contains(&count) && !trimmed.chars().any(char::is_control))
        .then_some(trimmed)
}

/// One record in scope, with what the answer and its receipt need.
#[derive(Clone, Debug)]
pub struct Row {
    /// `r1`, `r2` …: what the model cites.
    pub reference: String,
    /// The app's address as the canvas showed it (`expenses`), or empty.
    pub address: String,
    pub app: String,
    pub work: String,
    pub publication: String,
    pub definition_digest: String,
    pub work_version: i64,
    pub record: RecordRow,
    /// `(field_id, type, display, raw value)`, in schema order.
    pub fields: Vec<(String, String, String, Value)>,
}

impl Row {
    fn field(&self, id: &str) -> Option<&(String, String, String, Value)> {
        self.fields.iter().find(|(field, ..)| field == id)
    }
    /// The status-like field: `status` or `state` if it is an enum, else the first enum.
    fn status(&self) -> Option<&(String, String, String, Value)> {
        self.fields
            .iter()
            .find(|(id, kind, ..)| kind == "enum" && (id == "status" || id == "state"))
            .or_else(|| self.fields.iter().find(|(_, kind, ..)| kind == "enum"))
    }
    fn status_value(&self) -> String {
        self.status()
            .and_then(|(_, _, _, value)| value.get("value").and_then(Value::as_str))
            .unwrap_or("")
            .to_owned()
    }
    /// The first money field: its currency, minor-unit exponent and minor units.
    fn money(&self) -> Option<(String, u32, i128)> {
        let (_, _, _, value) = self.fields.iter().find(|(_, kind, ..)| kind == "money")?;
        let minor = value.get("minor_units")?.as_str()?.parse().ok()?;
        let currency = value
            .get("currency")
            .and_then(Value::as_str)
            .filter(|c| c.len() == 3 && c.bytes().all(|b| b.is_ascii_uppercase()))
            .unwrap_or("USD")
            .to_owned();
        let exponent = value
            .get("minor_unit_exponent")
            .and_then(Value::as_u64)
            .and_then(|e| u32::try_from(e).ok())
            .filter(|e| *e <= 6)
            .unwrap_or(2);
        Some((currency, exponent, minor))
    }
    /// A short name for the record: its merchant, vendor, title … or its id.
    pub fn label(&self) -> String {
        const NAMES: &[&str] = &[
            "merchant",
            "vendor",
            "title",
            "name",
            "employee",
            "subject",
            "description",
            "requester",
            "reason",
        ];
        let name = NAMES
            .iter()
            .find_map(|id| self.field(id).filter(|(_, kind, ..)| kind == "text"))
            .or_else(|| self.fields.iter().find(|(_, kind, ..)| kind == "text"))
            .map(|(_, _, display, _)| display.clone())
            .unwrap_or_else(|| short_id(&self.record.record_id));
        let mut parts = vec![truncate(&name, 60)];
        if let Some((_, _, display, _)) = self.fields.iter().find(|(_, kind, ..)| kind == "money") {
            parts.push(display.clone());
        }
        let status = self.status_value();
        if !status.is_empty() {
            parts.push(status.replace('_', " "));
        }
        parts.join(", ")
    }
}

/// Minor units as money text at `exponent`: `$513.00`, `€12.50`, `1,200 JPY`.
pub fn money_text(minor: i128, currency: &str, exponent: u32) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let digits = minor.unsigned_abs().to_string();
    let exponent = exponent as usize;
    let padded = format!("{digits:0>width$}", width = exponent + 1);
    let (whole, fraction) = padded.split_at(padded.len() - exponent);
    let whole = crate::query::page::display(whole);
    let number = if exponent == 0 {
        whole
    } else {
        format!("{whole}.{fraction}")
    };
    match currency {
        "USD" => format!("{sign}${number}"),
        "EUR" => format!("{sign}€{number}"),
        "GBP" => format!("{sign}£{number}"),
        "JPY" => format!("{sign}¥{number}"),
        other => format!("{sign}{number} {other}"),
    }
}

/// A stored decimal (`{coefficient, scale}`) as text at its scale: `123.45`.
fn decimal_text(value: &Value) -> Option<String> {
    let coefficient = value.get("coefficient")?.as_str()?;
    let scale = usize::try_from(value.get("scale")?.as_u64()?).ok()?;
    let (sign, digits) = match coefficient.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", coefficient),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || scale > 18 {
        return None;
    }
    if scale == 0 {
        return Some(format!("{sign}{digits}"));
    }
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    Some(format!("{sign}{whole}.{fraction}"))
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(limit.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

/// The records of every listing in scope, newest first per work, referenced
/// `r1`, `r2` … in that order. `addresses` aligns with `listings`.
pub fn rows(listings: &[RecordsListing], addresses: &[String]) -> Vec<Row> {
    let mut out = Vec::new();
    for (listing, address) in listings.iter().zip(addresses) {
        for record in &listing.records {
            let types = listing
                .collections
                .iter()
                .find(|collection| collection.collection == record.collection);
            let fields = record
                .values
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    let id = item.get("field_id")?.as_str()?.to_owned();
                    let value = item.get("value")?.clone();
                    let kind = types
                        .and_then(|c| c.fields.iter().find(|f| f.id == id))
                        .and_then(|f| f.value_type.get("type"))
                        .and_then(Value::as_str)
                        .or_else(|| value.get("type").and_then(Value::as_str))
                        .unwrap_or("")
                        .to_owned();
                    // A decimal reads at its scale (123.45), never as its bare coefficient.
                    let shown = if kind == "decimal" {
                        decimal_text(&value).unwrap_or_else(|| resolve::display(&value))
                    } else {
                        resolve::display(&value)
                    };
                    let display = truncate(&shown, 120);
                    Some((id, kind, display, value))
                })
                .collect();
            out.push(Row {
                reference: format!("r{}", out.len() + 1),
                address: address.clone(),
                app: listing.app.clone(),
                work: listing.work.clone(),
                publication: listing.publication.clone(),
                definition_digest: listing.definition_digest.clone(),
                work_version: listing.work_version,
                record: record.clone(),
                fields,
            });
        }
    }
    out
}

/// What Ask answers: text, and the references of the records it used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Answer {
    pub text: String,
    pub references: Vec<String>,
}

/// Judges the model's reply against Ask's closed shape (module notes). The
/// first JSON object in the text, decoded strictly (forma-json-v1), must be
/// `{"text": string, "references": [string]}`: a view-spec key is
/// `view_spec`, any other key `unknown_key`, no object `not_json`, bad text
/// `invalid_text`, a reference outside `known` `unknown_reference`.
pub fn judge(reply: &str, known: &BTreeSet<String>) -> Result<Answer, &'static str> {
    let start = reply.find('{').ok_or("not_json")?;
    let end = reply.rfind('}').ok_or("not_json")?;
    if end < start {
        return Err("not_json");
    }
    let value: Value =
        crate::admission::decode(&reply.as_bytes()[start..=end]).map_err(|_| "not_json")?;
    let object = value.as_object().ok_or("not_json")?;
    judge_object(object, known)
}

/// [`judge`] on a decoded object.
pub fn judge_object(
    object: &Map<String, Value>,
    known: &BTreeSet<String>,
) -> Result<Answer, &'static str> {
    if object.keys().any(|key| VIEW_KEYS.contains(&key.as_str())) {
        return Err("view_spec");
    }
    if object
        .keys()
        .any(|key| key != "text" && key != "references")
    {
        return Err("unknown_key");
    }
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or("invalid_text")?
        .trim();
    let count = text.chars().count();
    if !(1..=MAX_TEXT).contains(&count) || text.chars().any(|c| c.is_control() && c != '\n') {
        return Err("invalid_text");
    }
    let mut references = Vec::new();
    match object.get("references") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) if items.len() <= MAX_REFERENCES => {
            for item in items {
                let reference = item.as_str().ok_or("unknown_reference")?;
                if !known.contains(reference) {
                    return Err("unknown_reference");
                }
                if !references.iter().any(|r| r == reference) {
                    references.push(reference.to_owned());
                }
            }
        }
        Some(_) => return Err("unknown_reference"),
    }
    Ok(Answer {
        text: text.to_owned(),
        references,
    })
}

/// The receipt of a referenced record (ADR-011 I3): its identity and the
/// work's provenance, from storage only.
pub fn receipt(row: &Row) -> Value {
    json!({
        "kind": "record",
        "reference": row.reference,
        "app_id": row.app,
        "work_id": row.work,
        "collection_id": row.record.collection,
        "record_id": row.record.record_id,
        "version": row.record.version,
        "commit_sequence": row.record.commit_sequence,
        "committed_at": row.record.committed_at,
        "publication_id": row.publication,
        "definition_digest": row.definition_digest,
        "work_version": row.work_version,
    })
}

/// A link to a record the answer used: what the canvas shows and finds.
pub fn reference(row: &Row) -> Value {
    json!({
        "ref": row.reference,
        "app_id": row.app,
        "work_id": row.work,
        "record_id": row.record.record_id,
        "address": row.address,
        "label": row.label(),
    })
}

/// The route's answer document: exactly `{text, receipts, references}`.
pub fn document(answer: &Answer, rows: &[Row]) -> Value {
    let used: Vec<&Row> = answer
        .references
        .iter()
        .filter_map(|reference| rows.iter().find(|row| &row.reference == reference))
        .collect();
    json!({
        "text": answer.text,
        "receipts": used.iter().map(|row| receipt(row)).collect::<Vec<_>>(),
        "references": used.iter().map(|row| reference(row)).collect::<Vec<_>>(),
    })
}

// --- The model ---

/// The system prompt: the closed answer shape and the one rule of the job.
pub const SYSTEM: &str = "You answer a member's question about the records of their apps. You can only read: you never change records, cards, colors, charts or anything else.\n\
You receive JSON: the question, the scope (the card or canvas it was asked on, by its address) and the records in scope, each with a reference (r1, r2 …), its app's address and its fields as displayed.\n\
Answer with ONLY one JSON object, exactly: {\"text\": \"<your answer in plain sentences, at most 1200 characters, no markdown, no HTML>\", \"references\": [\"r1\", …]}.\n\
\"references\" lists every record your answer relies on (at most 200), using only the references you were given: a count or a total lists every record it counted. Never invent a record, an amount or a reference; if the records do not answer the question, say so.\n\
If \"partial\" is true, you were shown only some of the records (the newest of each app): say so, write \"at least\" for a count, and never present a sum as the full total. Add money only within one currency.\n\
\"Red\" records are the ones needing attention: waiting on a decision (submitted, pending, requested, open, overdue, unpaid) or refused, disputed or flagged.\n\
If the member asks you to change how something looks or what it shows (colors, a chart, a filter, a highlight), do not do it and do not describe a view: answer in text with the command to type, for example \"Ask only reads. To change how it looks, type /shape expenses.ledger highlight overdue in red\". To change records they use the app: /<app> <words>.\n\
Never return any other key: no view, chart, filter, color, theme or style.";

/// The user message: the question, scope and records as JSON data.
/// `partial`: a work in scope had more records than its listing holds.
pub fn user(question: &str, scope: &str, rows: &[Row], partial: bool) -> String {
    let records: Vec<Value> = rows
        .iter()
        .take(MAX_MODEL_RECORDS)
        .map(|row| {
            let fields: Map<String, Value> = row
                .fields
                .iter()
                .map(|(id, _, display, _)| (id.clone(), Value::String(display.clone())))
                .collect();
            json!({
                "ref": row.reference,
                "app": row.address,
                "fields": fields,
                "filed_by": crate::directory::label(&row.record.creator),
                "committed_at": row.record.committed_at,
            })
        })
        .collect();
    json!({
        "question": question,
        "scope": scope,
        "records": records,
        "more_records_not_shown": partial || rows.len() > MAX_MODEL_RECORDS,
        "partial": partial || rows.len() > MAX_MODEL_RECORDS,
    })
    .to_string()
}

// --- Rehearsed questions (demo organizations only; the caller checks) ---

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// Verbs that ask for a change, at the start of the question.
const CHANGES: &[&str] = &[
    "make",
    "change",
    "highlight",
    "color",
    "colour",
    "turn",
    "paint",
    "restyle",
    "shape",
    "sort",
    "hide",
    "rename",
    "delete",
    "remove",
    "approve",
    "reject",
    "file",
    "add",
    "update",
    "set",
    "chart",
    "plot",
];

/// The command a change request should have been (Ask never switches jobs).
pub fn redirect(question: &str, scope: &str, rows: &[Row]) -> Answer {
    let target = if scope.is_empty() {
        rows.first()
            .map(|row| row.address.clone())
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| "<card>".to_owned())
    } else {
        scope.to_owned()
    };
    // `card:expenses.ledger` → the app `app:expenses`; `expenses.ledger` → `expenses`.
    let app = match target.split_once(':') {
        Some((_, name)) => format!("app:{}", name.split('.').next().unwrap_or(name)),
        None => target.split('.').next().unwrap_or(&target).to_owned(),
    };
    let words = truncate(question.trim().trim_end_matches(['.', '?', '!']), 120);
    Answer {
        text: format!(
            "Ask only reads, so nothing changed. To change how it looks or what it shows, type /shape {target} {words}. To change records, tell the app: /{app} …"
        ),
        references: Vec::new(),
    }
}

/// A rehearsed answer to `question` over `rows`, or `None` (then the model).
/// `partial`: a work in scope had more records than were read (module notes).
pub fn rehearsed(question: &str, scope: &str, rows: &[Row], partial: bool) -> Option<Answer> {
    let words = words(question);
    if words.first().is_some_and(|w| CHANGES.contains(&w.as_str())) {
        return Some(redirect(question, scope, rows));
    }
    // Why, explain and summarize need reasoning: the model's.
    if words.first().is_some_and(|w| {
        ["why", "explain", "summarize", "summarise", "describe"].contains(&w.as_str())
    }) {
        return None;
    }
    let has = |w: &str| words.iter().any(|word| word == w);
    let where_ = if scope.is_empty() {
        "on your canvas".to_owned()
    } else {
        format!("in {scope}")
    };
    // Which records the question is about.
    let attention = [
        "red",
        "attention",
        "overdue",
        "late",
        "flagged",
        "problem",
        "problems",
    ]
    .iter()
    .any(|w| has(w));
    let statuses: BTreeSet<String> = rows.iter().map(Row::status_value).collect();
    let named_status = statuses
        .iter()
        .find(|status| {
            !status.is_empty()
                && (has(status) || question.to_lowercase().contains(&status.replace('_', " ")))
        })
        .cloned();
    let waiting = ["waiting", "pending"].iter().any(|w| has(w));
    let (chosen, what): (Vec<&Row>, String) = if attention {
        (
            rows.iter()
                .filter(|row| ATTENTION.contains(&row.status_value().as_str()))
                .collect(),
            "need attention (shown red: waiting on a decision, refused, disputed or flagged)"
                .to_owned(),
        )
    } else if let Some(status) = named_status.as_ref() {
        (
            rows.iter()
                .filter(|row| &row.status_value() == status)
                .collect(),
            format!("are {}", status.replace('_', " ")),
        )
    } else if waiting {
        (
            rows.iter()
                .filter(|row| {
                    ["submitted", "pending", "requested", "waiting", "open"]
                        .contains(&row.status_value().as_str())
                })
                .collect(),
            "are waiting on a decision".to_owned(),
        )
    } else {
        (rows.iter().collect(), String::new())
    };
    let filtered = attention || named_status.is_some() || waiting;
    let total = ["total", "sum", "spend", "spent", "much", "cost", "amount"]
        .iter()
        .any(|w| has(w));
    let count = (has("how") && has("many")) || has("count") || has("number");
    let noun = |n: usize| if n == 1 { "record" } else { "records" };
    // Every computed answer cites every record it counted.
    let all = |chosen: &[&Row]| -> Vec<String> {
        chosen.iter().map(|row| row.reference.clone()).collect()
    };
    let among = if partial {
        format!(" among the newest {RECORDS_LIMIT} of each app")
    } else {
        String::new()
    };
    if total {
        // Per currency: amounts in different currencies are never added.
        let mut sums: BTreeMap<(String, u32), i128> = BTreeMap::new();
        for (currency, exponent, minor) in chosen.iter().filter_map(|row| row.money()) {
            *sums.entry((currency, exponent)).or_default() += minor;
        }
        if sums.is_empty() {
            sums.insert(("USD".to_owned(), 2), 0);
        }
        let amounts: Vec<String> = sums
            .iter()
            .map(|((currency, exponent), minor)| money_text(*minor, currency, *exponent))
            .collect();
        let amount = amounts.join(" and ");
        let mixed = if amounts.len() > 1 {
            " (each currency added separately; different currencies are never added together)"
        } else {
            ""
        };
        let scope_words = if filtered {
            format!("the {} {} that {what}", chosen.len(), noun(chosen.len()))
        } else {
            format!("{} {}", chosen.len(), noun(chosen.len()))
        };
        let text = if partial {
            format!(
                "Not the full total: only the newest {RECORDS_LIMIT} records of each app were read, and older ones exist. Those add up to {amount}{mixed} across {scope_words}{among} {where_}."
            )
        } else {
            format!("The total is {amount}{mixed} across {scope_words} {where_}.")
        };
        return Some(Answer {
            text,
            references: all(&chosen),
        });
    }
    if count && !filtered {
        let text = if partial {
            format!(
                "There are at least {} {} {where_}: only the newest {RECORDS_LIMIT} of each app were read, and older ones exist.",
                chosen.len(),
                noun(chosen.len())
            )
        } else {
            format!(
                "There are {} {} {where_}.",
                chosen.len(),
                noun(chosen.len())
            )
        };
        return Some(Answer {
            text,
            references: all(&chosen),
        });
    }
    if !filtered {
        return None;
    }
    if chosen.is_empty() {
        return Some(Answer {
            text: format!(
                "No record{among} {where_} {}.",
                what.replacen("are ", "is ", 1)
                    .replacen("need ", "needs ", 1)
            ),
            references: Vec::new(),
        });
    }
    let named: Vec<String> = chosen.iter().take(NAMED).map(|row| row.label()).collect();
    let more = chosen.len().saturating_sub(NAMED);
    let tail = if more > 0 {
        format!("; and {more} more")
    } else {
        String::new()
    };
    let older = if partial {
        " Older records exist and were not read."
    } else {
        ""
    };
    Some(Answer {
        text: format!(
            "{} {}{among} {where_} {what}: {}{tail}.{older}",
            chosen.len(),
            noun(chosen.len()),
            named.join("; ")
        ),
        references: all(&chosen),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: usize, merchant: &str, cents: &str, status: &str) -> Row {
        Row {
            reference: format!("r{n}"),
            address: "expenses".into(),
            app: "00000000-0000-4000-8000-000000001681".into(),
            work: "00000000-0000-4000-8000-000000001684".into(),
            publication: "00000000-0000-4000-8000-0000000016a1".into(),
            definition_digest: format!("sha256:{}", "a".repeat(64)),
            work_version: 3,
            record: RecordRow {
                record_id: format!("00000000-0000-4000-8000-00000000c00{n}"),
                collection: "00000000-0000-4000-8000-000000001683".into(),
                schema: "expense_claim".into(),
                version: 1,
                commit_sequence: n as i64,
                committed_at: "2026-10-30T12:00:00Z".into(),
                creator: "00000000-0000-4000-8000-00000000006f".into(),
                updater: "00000000-0000-4000-8000-00000000006f".into(),
                values: Value::Null,
            },
            fields: vec![
                (
                    "amount".into(),
                    "money".into(),
                    resolve::display(&json!({"type": "money", "minor_units": cents})),
                    json!({"type": "money", "minor_units": cents}),
                ),
                (
                    "merchant".into(),
                    "text".into(),
                    merchant.into(),
                    json!({"type": "text", "value": merchant}),
                ),
                (
                    "status".into(),
                    "enum".into(),
                    status.into(),
                    json!({"type": "enum", "value": status}),
                ),
            ],
        }
    }
    fn sample() -> Vec<Row> {
        vec![
            row(1, "Nobu", "8600", "submitted"),
            row(2, "Delta", "41200", "approved"),
            row(3, "Figma", "1500", "rejected"),
        ]
    }
    fn known(rows: &[Row]) -> BTreeSet<String> {
        rows.iter().map(|row| row.reference.clone()).collect()
    }

    #[test]
    fn the_answer_shape_is_closed_and_a_view_spec_is_refused() {
        let rows = sample();
        let known = known(&rows);
        let answer = judge(
            r#"Sure: {"text": "Two claims need attention.", "references": ["r1", "r3", "r1"]}"#,
            &known,
        )
        .unwrap();
        assert_eq!(answer.references, ["r1", "r3"]);
        // A view spec, alone or beside text, is refused by name.
        for reply in [
            r#"{"presentation": "table", "filters": [{"field": "status", "op": "eq", "value": "rejected"}]}"#,
            r#"{"text": "Here you go", "highlight": [{"field": "status", "op": "eq", "value": "submitted", "color": "crimson"}]}"#,
            r#"{"text": "Made it red", "palette": "crimson"}"#,
        ] {
            assert_eq!(judge(reply, &known), Err("view_spec"), "{reply}");
        }
        assert_eq!(
            judge(r#"{"text": "x", "sql": "DELETE"}"#, &known),
            Err("unknown_key")
        );
        assert_eq!(judge("Just prose, no object.", &known), Err("not_json"));
        assert_eq!(
            judge(r#"{"text": "x", "references": ["r9"]}"#, &known),
            Err("unknown_reference")
        );
        assert_eq!(judge(r#"{"text": ""}"#, &known), Err("invalid_text"));
        assert_eq!(judge(r#"{"text": 1.5}"#, &known), Err("not_json"));
    }

    #[test]
    fn receipts_come_from_storage_not_from_the_model() {
        let rows = sample();
        let answer = Answer {
            text: "One".into(),
            references: vec!["r3".into()],
        };
        let document = document(&answer, &rows);
        let keys: Vec<&str> = document
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["receipts", "references", "text"]);
        assert_eq!(
            document["receipts"][0]["record_id"],
            "00000000-0000-4000-8000-00000000c003"
        );
        assert_eq!(document["receipts"][0]["commit_sequence"], 3);
        assert_eq!(
            document["receipts"][0]["definition_digest"],
            rows[2].definition_digest
        );
        assert_eq!(
            document["references"][0]["label"],
            "Figma, $15.00, rejected"
        );
    }

    #[test]
    fn rehearsed_questions_read_the_records() {
        let rows = sample();
        let red = rehearsed("where are all the red invoices?", "", &rows, false).unwrap();
        assert_eq!(red.references, ["r1", "r3"]);
        assert!(
            red.text
                .starts_with("2 records on your canvas need attention"),
            "{}",
            red.text
        );
        assert!(red.text.contains("Nobu, $86.00, submitted"), "{}", red.text);
        let total = rehearsed("what's the total spend?", "expenses.ledger", &rows, false).unwrap();
        assert_eq!(
            total.text,
            "The total is $513.00 across 3 records in expenses.ledger."
        );
        let approved = rehearsed("which ones are approved", "", &rows, false).unwrap();
        assert_eq!(approved.references, ["r2"]);
        let count = rehearsed("how many claims are there", "expenses", &rows, false).unwrap();
        assert_eq!(count.text, "There are 3 records in expenses.");
        assert_eq!(
            rehearsed("why did spend jump in March", "", &rows, false),
            None
        );
        assert_eq!(
            rehearsed("summarize the quarter for me", "", &rows, false),
            None
        );
    }

    #[test]
    fn a_change_request_answers_with_the_address_to_type() {
        let rows = sample();
        let answer =
            rehearsed("make the overdue ones red", "expenses.ledger", &rows, false).unwrap();
        assert!(answer.references.is_empty());
        assert!(
            answer
                .text
                .contains("/shape expenses.ledger make the overdue ones red"),
            "{}",
            answer.text
        );
        assert!(answer
            .text
            .starts_with("Ask only reads, so nothing changed."));
    }

    #[test]
    fn questions_are_bounded_text() {
        assert_eq!(
            question("? where are the red ones "),
            Some("where are the red ones")
        );
        assert_eq!(question("?"), None);
        assert_eq!(question(&"x".repeat(MAX_QUESTION + 1)), None);
        assert_eq!(question("bad\u{7}bell"), None);
    }

    #[test]
    fn a_partial_read_never_presents_a_partial_sum_as_the_total() {
        let rows = sample();
        let total = rehearsed("what's the total spend?", "expenses", &rows, true).unwrap();
        assert!(
            total
                .text
                .starts_with("Not the full total: only the newest 200"),
            "{}",
            total.text
        );
        assert!(!total.text.starts_with("The total is"), "{}", total.text);
        assert!(total.text.contains("$513.00"), "{}", total.text);
        let count = rehearsed("how many claims are there", "expenses", &rows, true).unwrap();
        assert!(
            count.text.starts_with("There are at least 3 records"),
            "{}",
            count.text
        );
        let red = rehearsed("where are the red ones", "expenses", &rows, true).unwrap();
        assert!(
            red.text
                .contains("2 records among the newest 200 of each app in expenses"),
            "{}",
            red.text
        );
        assert!(
            red.text.ends_with("Older records exist and were not read."),
            "{}",
            red.text
        );
        // The model is told too.
        let sent: Value = serde_json::from_str(&user("total?", "expenses", &rows, true)).unwrap();
        assert_eq!(sent["partial"], true);
        assert_eq!(sent["more_records_not_shown"], true);
        let whole: Value = serde_json::from_str(&user("total?", "expenses", &rows, false)).unwrap();
        assert_eq!(whole["partial"], false);
    }

    #[test]
    fn a_computed_number_cites_every_record_it_counted() {
        let rows: Vec<Row> = (1..=120)
            .map(|n| {
                row(
                    n,
                    "Vendor",
                    "100",
                    if n % 2 == 0 { "approved" } else { "submitted" },
                )
            })
            .collect();
        let total = rehearsed("what's the total spend?", "", &rows, false).unwrap();
        assert_eq!(
            total.references.len(),
            120,
            "every summed record is a receipt"
        );
        assert!(total.text.contains("$120.00"), "{}", total.text);
        let count = rehearsed("how many are there", "", &rows, false).unwrap();
        assert_eq!(count.references.len(), 120);
        let red = rehearsed("where are the red ones", "", &rows, false).unwrap();
        assert_eq!(red.references.len(), 60);
        let document = document(&total, &rows);
        assert_eq!(document["receipts"].as_array().unwrap().len(), 120);
    }

    #[test]
    fn money_is_added_per_currency_and_decimals_read_at_their_scale() {
        let mut rows = sample();
        rows[1].fields[0].3 = json!({"type": "money", "minor_units": "1250", "currency": "EUR", "minor_unit_exponent": 2});
        let total = rehearsed("what's the total spend?", "", &rows, false).unwrap();
        assert!(total.text.contains("€12.50 and $101.00"), "{}", total.text);
        assert!(
            total.text.contains("never added together"),
            "{}",
            total.text
        );
        assert_eq!(money_text(123_456, "JPY", 0), "¥123,456");
        assert_eq!(money_text(-5, "USD", 2), "-$0.05");
        assert_eq!(money_text(1, "CHF", 2), "0.01 CHF");
        assert_eq!(
            decimal_text(
                &json!({"type": "decimal", "coefficient": "12345", "scale": 2, "unit": "hours"})
            )
            .as_deref(),
            Some("123.45")
        );
        assert_eq!(
            decimal_text(&json!({"coefficient": "-5", "scale": 3})).as_deref(),
            Some("-0.005")
        );
    }

    #[test]
    fn a_namespaced_scope_redirects_to_its_namespaced_forms() {
        let rows = sample();
        let answer = rehearsed("make it red", "card:expenses.ledger", &rows, false).unwrap();
        assert!(
            answer
                .text
                .contains("/shape card:expenses.ledger make it red"),
            "{}",
            answer.text
        );
        assert!(answer.text.contains("/app:expenses …"), "{}", answer.text);
    }
}
