//! Demo shortcut (APP-098): the deterministic parser of the rehearsed pitch
//! utterances (`docs/pitch/SCENARIOS.md`), used only for a listed demo
//! organization (`actions::Parser::parse`): a rehearsed utterance answers at
//! once without a model call; anything it does not recognize (`None`) goes
//! to the real model. It produces the same closed [`Parsed`] shape the model
//! does, from plain pattern matching: an amount ("$86", "86 dollars"), a
//! merchant ("at Nobu"), a category from a few keywords, a date word ("last
//! night", "yesterday", "today"), an approver ("Sarah approves my expenses",
//! "send it to Sarah", "for Sarah to approve"), the decide forms ("approve
//! the Nobu dinner", "reject ...") and the open forms ("what's waiting for my
//! approval"). Everything after the parse is the real path.
//!
//! Proper fix: none needed for correctness; the parser exists so that a
//! rehearsed demo is never hostage to model latency. Remove, or keep behind
//! `GENUI_DEMO_MODE`, after November 2 (APP-097).
use super::{prompt::CatalogApp, Kind, Parsed};
use serde_json::{Map, Value};

const DECIDE_WORDS: &[(&str, &str)] = &[
    ("approve", "approved"),
    ("approved", "approved"),
    ("accept", "approved"),
    ("ok ", "approved"),
    ("sign off", "approved"),
    ("reject", "rejected"),
    ("rejected", "rejected"),
    ("decline", "rejected"),
    ("deny", "rejected"),
];
const CATEGORIES: &[(&str, &str)] = &[
    ("dinner", "meals"),
    ("lunch", "meals"),
    ("breakfast", "meals"),
    ("coffee", "meals"),
    ("meal", "meals"),
    ("restaurant", "meals"),
    ("drinks", "meals"),
    ("flight", "travel"),
    ("hotel", "travel"),
    ("uber", "travel"),
    ("lyft", "travel"),
    ("taxi", "travel"),
    ("train", "travel"),
    ("airfare", "travel"),
    ("mileage", "travel"),
    ("travel", "travel"),
    ("subscription", "software"),
    ("license", "software"),
    ("saas", "software"),
    ("software", "software"),
    ("github", "software"),
    ("figma", "software"),
    ("notion", "software"),
    ("monitor", "office"),
    ("keyboard", "office"),
    ("desk", "office"),
    ("chair", "office"),
    ("supplies", "office"),
    ("office", "office"),
];

pub(super) fn lower_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '$' && c != '.')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// The byte index in `text` of the first ASCII-case-insensitive match of
/// `needle`. Indices of `text.to_lowercase()` do not address `text` (some
/// characters change byte length when lowercased, "İ"), so callers that slice
/// the original text search it with this.
pub fn find_ignore_ascii_case(text: &str, needle: &str) -> Option<usize> {
    text.char_indices().map(|(at, _)| at).find(|&at| {
        text.get(at..at + needle.len())
            .is_some_and(|window| window.eq_ignore_ascii_case(needle))
    })
}

/// A dollar amount in the text as a decimal string ("86", "86.50").
pub fn amount(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Every slice below starts at `i` or after an ASCII byte, so only `i`
        // must sit on a character boundary (a receipt may say "Café").
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let dollar = bytes[i] == b'$';
        let start = if dollar { i + 1 } else { i };
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b',') {
            end += 1;
        }
        let mut whole: String = text[start..end].chars().filter(|c| *c != ',').collect();
        let mut cents = String::new();
        if end < bytes.len() && bytes[end] == b'.' {
            let mut j = end + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() && j - end <= 2 {
                j += 1;
            }
            if j > end + 1 {
                cents = text[end + 1..j].to_owned();
                end = j;
            }
        }
        if !whole.is_empty() && whole.len() <= 9 {
            let rest = text[end..].trim_start().to_lowercase();
            let currency_word =
                rest.starts_with("dollar") || rest.starts_with("usd") || rest.starts_with("bucks");
            // A bare number counts only with a dollar sign or a currency word.
            if dollar || currency_word {
                if whole.len() > 1 && whole.starts_with('0') {
                    whole = whole.trim_start_matches('0').to_owned();
                    if whole.is_empty() {
                        whole = "0".into();
                    }
                }
                return Some(if cents.is_empty() {
                    whole
                } else {
                    format!("{whole}.{cents}")
                });
            }
        }
        i = end.max(i + 1);
    }
    None
}

/// The words after "at" up to a stop (comma, period, "last", "yesterday",
/// "on", "for", "and", a dollar amount).
pub(super) fn merchant(text: &str) -> Option<String> {
    let stops = [
        "last",
        "yesterday",
        "today",
        "on",
        "for",
        "and",
        "with",
        "this",
        "which",
        "that",
        "so",
        "then",
    ];
    let words: Vec<&str> = text.split_whitespace().collect();
    let position = words.iter().position(|w| w.eq_ignore_ascii_case("at"))?;
    let mut name = Vec::new();
    for word in &words[position + 1..] {
        let clean = word.trim_matches(|c: char| c == ',' || c == '.' || c == ';' || c == '!');
        if clean.is_empty()
            || clean.starts_with('$')
            || stops.contains(&clean.to_lowercase().as_str())
        {
            break;
        }
        name.push(clean.to_owned());
        if word.ends_with(',') || word.ends_with('.') || word.ends_with(';') || name.len() >= 4 {
            break;
        }
    }
    (!name.is_empty()).then(|| name.join(" "))
}

pub(super) fn category(words: &[String]) -> &'static str {
    for (keyword, category) in CATEGORIES {
        if words
            .iter()
            .any(|w| w == keyword || w.trim_end_matches('s') == *keyword)
        {
            return category;
        }
    }
    "other"
}

/// `YYYY-MM-DD` minus `days` (proleptic Gregorian).
pub fn days_before(today: &str, days: i64) -> Option<String> {
    let mut parts = today.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let z = era * 146_097 + doe - 719_468 - days;
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

pub(super) fn spent_on(lower: &str, today: &str) -> String {
    if lower.contains("last night") || lower.contains("yesterday") || lower.contains("last evening")
    {
        days_before(today, 1).unwrap_or_else(|| today.to_owned())
    } else if lower.contains("day before yesterday") {
        days_before(today, 2).unwrap_or_else(|| today.to_owned())
    } else {
        // A literal YYYY-MM-DD in the text wins.
        lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_ascii_digit() && c != '-'))
            .find(|w| w.len() == 10 && w.as_bytes()[4] == b'-' && w.as_bytes()[7] == b'-')
            .map(str::to_owned)
            .unwrap_or_else(|| today.to_owned())
    }
}

/// "<Name> approves", "<Name> signs off", "send (it) to <Name>", "for <Name>
/// to approve", "ask <Name>", "approver (is) <Name>", "cc <Name>".
pub(super) fn approver(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let clean = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '\'')
            .to_owned()
    };
    let is_name = |w: &str| {
        let c = clean(w);
        c.chars().next().is_some_and(char::is_uppercase) && c.len() >= 2
    };
    for (index, word) in words.iter().enumerate() {
        let lower = clean(word).to_lowercase();
        if matches!(
            lower.as_str(),
            "approves" | "approve" | "signs" | "reviews" | "handles"
        ) && index >= 1
            && is_name(words[index - 1])
            && !matches!(
                clean(words[index - 1]).to_lowercase().as_str(),
                "i" | "please"
            )
        {
            // "Sarah Chen approves": take up to two capitalized words before.
            let mut name = vec![clean(words[index - 1])];
            if index >= 2
                && is_name(words[index - 2])
                && !matches!(
                    clean(words[index - 2]).to_lowercase().as_str(),
                    "i" | "nobu"
                )
            {
                name.insert(0, clean(words[index - 2]));
            }
            return Some(name.join(" "));
        }
        if matches!(
            lower.as_str(),
            "to" | "ask" | "approver" | "cc" | "notify" | "manager"
        ) && index + 1 < words.len()
        {
            let mut j = index + 1;
            if clean(words[j]).eq_ignore_ascii_case("is")
                || clean(words[j]).eq_ignore_ascii_case("it")
            {
                j += 1;
            }
            if j < words.len() && clean(words[j]).eq_ignore_ascii_case("to") {
                j += 1;
            }
            if j < words.len() && is_name(words[j]) {
                let mut name = vec![clean(words[j])];
                if j + 1 < words.len() && is_name(words[j + 1]) && !words[j].ends_with(',') {
                    name.push(clean(words[j + 1]));
                }
                return Some(name.join(" "));
            }
        }
    }
    None
}

/// The deterministic parse, or `None` when no rehearsed pattern applies.
pub fn parse(utterance: &str, catalog: &[CatalogApp], today: &str) -> Option<Parsed> {
    // APP-107 (DS-69): the workflow apps' rehearsed sentences first; they
    // fire only on words naming a workflow (time off, purchase, invoice,
    // access, discount) or inside a slash command pinned to one.
    if let Some(parsed) = super::workflows::rehearsed(utterance, catalog, today) {
        return Some(parsed);
    }
    let lower = utterance.to_lowercase();
    let words = lower_words(utterance);
    let expense_app = catalog.iter().find(|app| {
        app.field("amount").is_some()
            && app.field("status").is_some()
            && app.create_command().is_some()
    });
    // Decide: "approve the Nobu dinner", "reject Victor's hotel".
    if let Some((_, status)) = DECIDE_WORDS.iter().find(|(word, _)| {
        words.first().is_some_and(|w| w == word.trim()) || lower.starts_with(word)
    }) {
        let app = expense_app.or(catalog.first())?;
        let hint: String = utterance
            .split_whitespace()
            .skip(1)
            .filter(|w| {
                !matches!(
                    w.to_lowercase().trim_matches(','),
                    "the" | "that" | "this" | "please" | "expense" | "claim"
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut values = Map::new();
        values.insert("status".into(), Value::String((*status).to_owned()));
        return Some(Parsed {
            kind: Kind::Decide,
            app_hint: app.app.clone(),
            values,
            approver_name: None,
            target_record_hint: (!hint.is_empty()).then_some(hint),
        });
    }
    // Open: "what's waiting for my approval", "show my expenses", "open expenses".
    if (lower.contains("waiting for")
        || lower.contains("pending")
        || lower.starts_with("show")
        || lower.starts_with("open"))
        && (lower.contains("approv") || lower.contains("expense") || lower.contains("claim"))
    {
        let app = expense_app?;
        return Some(Parsed {
            kind: Kind::Open,
            app_hint: app.app.clone(),
            values: Map::new(),
            approver_name: None,
            target_record_hint: None,
        });
    }
    // Create: an amount with a spending verb. APP-135: the noun "expense(s)"
    // is no verb ("Review expenses over $200" filed a $200 claim); "expense"
    // counts only as the sentence's first word ("expense $40 for lunch").
    let spent = lower.contains("spent")
        || lower.contains("paid")
        || lower.contains("bought")
        || lower.contains("reimburse")
        || lower.contains("expensed")
        || words.first().is_some_and(|w| w == "expense");
    if let (Some(amount), true, Some(app)) = (amount(utterance), spent, expense_app) {
        let mut values = Map::new();
        values.insert("amount".into(), Value::String(amount));
        values.insert(
            "merchant".into(),
            Value::String(merchant(utterance).unwrap_or_else(|| "Unknown merchant".into())),
        );
        values.insert(
            "category".into(),
            Value::String(category(&words).to_owned()),
        );
        values.insert("spent_on".into(), Value::String(spent_on(&lower, today)));
        // The note: the words between "on" and "at" ("a client dinner").
        let note = utterance
            .split_whitespace()
            .skip_while(|w| !w.eq_ignore_ascii_case("on"))
            .skip(1)
            .take_while(|w| {
                !w.eq_ignore_ascii_case("at") && !w.starts_with('$') && !w.ends_with(',')
            })
            .collect::<Vec<_>>()
            .join(" ");
        if !note.is_empty() {
            values.insert("note".into(), Value::String(note));
        }
        values.insert("status".into(), Value::String("submitted".into()));
        return Some(Parsed {
            kind: Kind::Create,
            app_hint: app.app.clone(),
            values,
            approver_name: approver(utterance),
            target_record_hint: None,
        });
    }
    // Not rehearsed: the model decides.
    None
}
