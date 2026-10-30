//! APP-102: opening the expense ledger from the action path. `/expense` with
//! no text, "show all expenses from Oct 1 to Oct 30", or any `open` whose
//! parse carries a date range (`values.from`, `values.to`) answers `open`
//! with `view: "ledger"` and the range; the canvas brings the work's ledger
//! card into view with that range. Deterministic and not a demo shortcut:
//! a bare command and a date range written in words.
use super::{prompt::CatalogApp, receipt, Kind, Parsed};
use serde_json::{Map, Value};

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `/expense` or `/expenses` alone: open the expense app's ledger.
pub fn bare_command(utterance: &str, catalog: &[CatalogApp]) -> Option<Parsed> {
    let text = utterance.trim().to_lowercase();
    if text != "/expense" && text != "/expenses" && text != "/ledger" {
        return None;
    }
    let app = receipt::expense_app(catalog)?;
    let mut values = Map::new();
    values.insert("view".into(), Value::String("ledger".into()));
    Some(Parsed {
        kind: Kind::Open,
        app_hint: app.app.clone(),
        values,
        approver_name: None,
        target_record_hint: None,
    })
}

/// Whether the member's words ask for the ledger rather than the records.
pub fn wants_ledger(utterance: &str, values: &Map<String, Value>) -> bool {
    let lower = utterance.to_lowercase();
    values.get("view").and_then(Value::as_str) == Some("ledger")
        || range_of(values).is_some()
        || lower.contains("ledger")
        || lower.contains("all expenses")
        || lower.contains("total")
        || lower.contains("spend by")
}

fn date_shaped(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit())
        && (1..=12).contains(&value[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&value[8..10].parse::<u8>().unwrap_or(0))
}

/// The range a parse carries (`values.from`, `values.to`, both dates, in
/// order), if any.
pub fn range_of(values: &Map<String, Value>) -> Option<(String, String)> {
    let from = values.get("from")?.as_str()?.trim();
    let to = values.get("to")?.as_str()?.trim();
    (date_shaped(from) && date_shaped(to) && from <= to).then(|| (from.to_owned(), to.to_owned()))
}

/// One date in words: "Oct 1", "October 1st", "Oct 1, 2026", "10/1",
/// "2026-10-01". The year defaults to `today`'s.
fn date_at(words: &[String], at: usize, year: i64) -> Option<(String, usize)> {
    let word = words.get(at)?;
    if date_shaped(word) {
        return Some((word.clone(), 1));
    }
    if let Some((m, d)) = word.split_once('/') {
        let (d, y) = match d.split_once('/') {
            Some((d, y)) => (d, y.parse::<i64>().ok()),
            None => (d, None),
        };
        let (m, d) = (m.parse::<u32>().ok()?, d.parse::<u32>().ok()?);
        let y = y
            .map(|y| if y < 100 { 2000 + y } else { y })
            .unwrap_or(year);
        let text = format!("{y:04}-{m:02}-{d:02}");
        return date_shaped(&text).then_some((text, 1));
    }
    let month = MONTHS
        .iter()
        .position(|m| word.len() >= 3 && word.starts_with(m))?
        + 1;
    let day: u32 = words
        .get(at + 1)?
        .trim_end_matches(|c: char| c.is_ascii_alphabetic())
        .parse()
        .ok()?;
    let (y, used) = match words.get(at + 2).and_then(|w| w.parse::<i64>().ok()) {
        Some(y) if (1990..=2100).contains(&y) => (y, 3),
        _ => (year, 2),
    };
    let text = format!("{y:04}-{month:02}-{day:02}");
    date_shaped(&text).then_some((text, used))
}

/// "from <date> to|through|until <date>" or "between <date> and <date>".
pub fn range(utterance: &str, today: &str) -> Option<(String, String)> {
    let year: i64 = today.get(..4)?.parse().ok()?;
    let words: Vec<String> = utterance
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .map(|w| {
            w.trim_matches(|c: char| c == '.' || c == '?' || c == '!')
                .to_owned()
        })
        .collect();
    for (index, word) in words.iter().enumerate() {
        if word != "from" && word != "between" {
            continue;
        }
        let Some((from, used)) = date_at(&words, index + 1, year) else {
            continue;
        };
        let joint = index + 1 + used;
        if !matches!(
            words.get(joint).map(String::as_str),
            Some("to" | "through" | "until" | "and" | "-")
        ) {
            continue;
        }
        let Some((to, _)) = date_at(&words, joint + 1, year) else {
            continue;
        };
        if from <= to {
            return Some((from, to));
        }
    }
    None
}

/// Adds the range of `utterance` to an `open` parse's values (the model may
/// have set it already; the words win when they state one).
pub fn with_range(mut parsed: Parsed, utterance: &str, today: &str) -> Parsed {
    if parsed.kind == Kind::Open {
        if let Some((from, to)) = range(utterance, today) {
            parsed.values.insert("from".into(), Value::String(from));
            parsed.values.insert("to".into(), Value::String(to));
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_in_words_parse_and_bad_ones_do_not() {
        let today = "2026-10-30";
        assert_eq!(
            range("show all expenses from Oct 1 to Oct 30", today),
            Some(("2026-10-01".into(), "2026-10-30".into()))
        );
        assert_eq!(
            range("expenses between October 3rd, 2025 and Nov 2, 2025", today),
            Some(("2025-10-03".into(), "2025-11-02".into()))
        );
        assert_eq!(
            range("from 10/1 to 10/15", today),
            Some(("2026-10-01".into(), "2026-10-15".into()))
        );
        assert_eq!(
            range("from 2026-09-01 through 2026-09-30", today),
            Some(("2026-09-01".into(), "2026-09-30".into()))
        );
        assert_eq!(range("from Oct 30 to Oct 1", today), None);
        assert_eq!(range("from Foo 1 to Oct 3", today), None);
        assert_eq!(range("show my expenses", today), None);
    }
}
