//! APP-135: the read-only guard of the action parser. A sentence that reads
//! (its verb shows, lists, finds, reviews, filters …) or asks a question
//! never becomes a create or decide draft, whatever the rehearsed parser or
//! the model answered: Do is only the app's own commands said as a command
//! (`docs/pitch/CANVAS-RULES.md`, the four jobs). A money amount alone does
//! not imply filing either: "expenses over $200" names a threshold, not a
//! claim. Both answer `place` (the composer picks the app and, where the app
//! supports it, reshapes it with the filter: APP-101's "Review expenses over
//! $200" → Expense review, Amount > $200); an `open` stays `open` unless the
//! words carry a money threshold it cannot show.
//!
//! Deterministic and not a demo shortcut: it applies in every organization,
//! before the model (a read sentence with a threshold answers `place` with
//! no model call) and after it (a create or decide the words do not support
//! is turned into `place`). The model's output stays data either way.
use super::{rehearsed, Kind, Parsed};
use serde_json::Map;

/// Leading words that only make a sentence polite or first-person, never
/// change its verb ("please show …", "can you list …", "I want to see …").
const PREFIXES: &[&[&str]] = &[
    &["please"],
    &["pls"],
    &["kindly"],
    &["can", "you"],
    &["could", "you"],
    &["would", "you"],
    &["will", "you"],
    &["can", "u"],
    &["can", "i"],
    &["could", "i"],
    &["may", "i"],
    &["can", "we"],
    &["let", "me"],
    &["let's"],
    &["lets"],
    &["help", "me"],
    &["i", "want", "to"],
    &["i", "need", "to"],
    &["i'd", "like", "to"],
    &["i", "would", "like", "to"],
    &["just"],
    &["quickly"],
    &["now"],
];

/// Verbs that read: they show, list, find or narrow records, never change them.
const READ_VERBS: &[&str] = &[
    "show",
    "list",
    "find",
    "review",
    "see",
    "view",
    "display",
    "filter",
    "search",
    "look",
    "check",
    "browse",
    "compare",
    "summarize",
    "summarise",
    "count",
    "sort",
    "pull",
    "tell",
    "explain",
    "audit",
    "inspect",
    "investigate",
    "analyze",
    "analyse",
    "break",
    "chart",
    "graph",
    "plot",
];

/// A sentence's first word that makes it a question.
const QUESTION_WORDS: &[&str] = &[
    "what", "what's", "whats", "which", "who", "who's", "whose", "whom", "when", "where", "why",
    "how", "how's", "is", "are", "was", "were", "am", "do", "does", "did", "any",
];

/// Words before an amount that make it a threshold ("over $200").
const COMPARATORS: &[&[&str]] = &[
    &["over"],
    &["above"],
    &["under"],
    &["below"],
    &["exceeding"],
    &["beyond"],
    &["more", "than"],
    &["less", "than"],
    &["greater", "than"],
    &["bigger", "than"],
    &["larger", "than"],
    &["higher", "than"],
    &["lower", "than"],
    &["fewer", "than"],
    &["at", "least"],
    &["at", "most"],
    &["upwards", "of"],
    &["up", "to"],
    &[">"],
    &[">="],
    &["<"],
    &["<="],
    &["≥"],
    &["≤"],
];

/// Verbs that file: with one of them a threshold-looking amount is still a
/// filing ("I spent over $200 at Nobu").
const FILING_VERBS: &[&str] = &[
    "spent",
    "spend",
    "paid",
    "pay",
    "bought",
    "buy",
    "purchased",
    "file",
    "filed",
    "submit",
    "submitted",
    "log",
    "logged",
    "record",
    "expensed",
    "charged",
    "reimburse",
    "reimbursed",
    "book",
    "order",
    "ordered",
];

/// The sentence's words: lowercase, split on whitespace, outer punctuation
/// trimmed except the characters an amount or a comparator needs.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| {
                !c.is_alphanumeric() && !matches!(c, '$' | '\'' | '<' | '>' | '=' | '≥' | '≤')
            })
            .to_lowercase()
            .replace('’', "'")
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn starts_with(words: &[String], phrase: &[&str]) -> bool {
    words.len() >= phrase.len() && words.iter().zip(phrase).all(|(w, p)| w == p)
}

/// The words after the polite or first-person prefixes, and whether one was
/// taken ("can you approve the Nobu dinner?" is a request, not a question).
fn verb_words(words: &[String]) -> (&[String], bool) {
    let mut rest = words;
    let mut prefixed = false;
    'strip: loop {
        for prefix in PREFIXES {
            if starts_with(rest, prefix) && rest.len() > prefix.len() {
                rest = &rest[prefix.len()..];
                prefixed = true;
                continue 'strip;
            }
        }
        return (rest, prefixed);
    }
}

/// True when the sentence reads or asks: its verb (after any polite prefix)
/// is a read verb, or it is a question (a question word first, or a final
/// "?" on a sentence that is not a polite request).
pub fn reads(utterance: &str) -> bool {
    let all = words(utterance);
    let (rest, prefixed) = verb_words(&all);
    let Some(verb) = rest.first() else {
        return false;
    };
    if READ_VERBS.contains(&verb.as_str()) {
        return true;
    }
    if all
        .first()
        .is_some_and(|first| QUESTION_WORDS.contains(&first.as_str()))
    {
        return true;
    }
    !prefixed && utterance.trim_end().ends_with('?')
}

/// A money amount written as a threshold ("over $200", "more than 50
/// dollars", "> $1,000").
pub fn threshold(utterance: &str) -> bool {
    let words = words(utterance);
    words.iter().enumerate().any(|(at, word)| {
        // "<$200", ">=$200" written without a space.
        let glued = word.trim_start_matches(['<', '>', '=', '≥', '≤']);
        if glued.len() < word.len() && money_at(&[glued.to_owned()], 0) {
            return true;
        }
        COMPARATORS
            .iter()
            .any(|phrase| starts_with(&words[at..], phrase) && money_at(&words, at + phrase.len()))
    })
}

/// Whether the words at `at` are an amount ("$200", "200 dollars").
fn money_at(words: &[String], at: usize) -> bool {
    let Some(word) = words.get(at) else {
        return false;
    };
    let digits = word.strip_prefix('$').unwrap_or(word);
    if !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let text = words[at..words.len().min(at + 2)].join(" ");
    rehearsed::amount(&text).is_some()
}

/// Whether the sentence names a filing verb.
pub fn files(utterance: &str) -> bool {
    words(utterance)
        .iter()
        .any(|w| FILING_VERBS.contains(&w.as_str()))
}

/// `place`, with nothing the words did not ask for.
fn place() -> Parsed {
    Parsed {
        kind: Kind::Place,
        app_hint: String::new(),
        values: Map::new(),
        approver_name: None,
        target_record_hint: None,
    }
}

/// Before any parse: a read sentence with a money threshold answers `place`
/// at once (whatever a parse would say, the guard would answer `place`), so
/// no model call is spent on it.
pub fn before(utterance: &str) -> Option<Parsed> {
    (reads(utterance) && threshold(utterance)).then(place)
}

/// After the parse (rehearsed or model): a create or decide from a sentence
/// that reads or asks, or a create whose only money is a threshold with no
/// filing verb, becomes `place`; an `open` whose words carry a threshold it
/// cannot show becomes `place` too (the composer filters).
pub fn after(utterance: &str, parsed: Parsed) -> Parsed {
    let reads = reads(utterance);
    let threshold = threshold(utterance);
    let demoted = match parsed.kind {
        Kind::Create | Kind::Decide if reads => true,
        Kind::Create if threshold && !files(utterance) => true,
        Kind::Open if reads && threshold => true,
        _ => false,
    };
    if demoted {
        tracing::info!(
            event = "action.read_guard",
            parsed = parsed.kind.as_str(),
            reads,
            threshold,
            "a sentence that reads answered place, not a command draft"
        );
        return place();
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_verbs_and_questions_read() {
        for text in [
            "Review expenses over $200",
            "show my expenses over $200",
            "list claims above $1,000",
            "find the Nobu expense",
            "please show expenses over 200 dollars",
            "can you list the $86 claims?",
            "I want to see expenses over $200",
            "what did I spend at Nobu?",
            "which claims are over $200",
            "did Sarah approve the Nobu dinner?",
            "how much did we spend on travel",
            "expenses over $200?",
            "Filter expenses to over $500",
        ] {
            assert!(reads(text), "{text}");
        }
        for text in [
            "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses",
            "approve the Nobu dinner",
            "Reject Victor's hotel",
            "can you approve the Nobu dinner?",
            "please file $42 at Sweetgreen",
            "$86 Nobu",
            "Paid 42.50 dollars for a Figma subscription yesterday; send it to Sarah Chen",
            "Book me off Nov 24–26 for Thanksgiving, Sarah approves",
            "mark the Figma invoice paid",
            "expenses over $200",
        ] {
            assert!(!reads(text), "{text}");
        }
    }

    #[test]
    fn thresholds_are_amounts_after_a_comparator() {
        assert!(threshold("Review expenses over $200"));
        assert!(threshold("claims more than 50 dollars"));
        assert!(threshold("expenses >$1,000"));
        assert!(threshold("expenses > $1,000"));
        assert!(threshold("at least $20"));
        assert!(!threshold("I spent $86 at Nobu"));
        assert!(!threshold("over the weekend I paid $40"));
        assert!(!threshold("expenses over 200"));
        assert!(!threshold("$86 Nobu"));
    }

    fn parsed(kind: Kind) -> Parsed {
        let mut values = Map::new();
        values.insert("amount".into(), "200".into());
        Parsed {
            kind,
            app_hint: "00000000-0000-4000-8000-000000001681".into(),
            values,
            approver_name: Some("Sarah".into()),
            target_record_hint: None,
        }
    }

    #[test]
    fn a_read_sentence_never_keeps_a_create_or_decide() {
        for text in [
            "Review expenses over $200",
            "show me the $86 Nobu claim",
            "did Sarah approve the Nobu dinner?",
        ] {
            for kind in [Kind::Create, Kind::Decide] {
                let out = after(text, parsed(kind));
                assert_eq!(out.kind, Kind::Place, "{text}");
                assert!(out.values.is_empty() && out.approver_name.is_none());
            }
        }
        // A threshold with no filing verb is no filing.
        assert_eq!(
            after("expenses over $200", parsed(Kind::Create)).kind,
            Kind::Place
        );
        // An open stays open, unless it carries a threshold it cannot show.
        assert_eq!(
            after("what's waiting for my approval?", parsed(Kind::Open)).kind,
            Kind::Open
        );
        assert_eq!(
            after("show my expenses over $200", parsed(Kind::Open)).kind,
            Kind::Place
        );
        // Filings and decisions said as commands are untouched.
        for (text, kind) in [
            ("I spent $86 at Nobu, Sarah approves", Kind::Create),
            ("I spent over $200 at Nobu last night", Kind::Create),
            ("$86 Nobu", Kind::Create),
            ("approve the Nobu dinner", Kind::Decide),
            ("can you approve the Nobu dinner?", Kind::Decide),
        ] {
            assert_eq!(after(text, parsed(kind)).kind, kind, "{text}");
        }
        assert!(before("Review expenses over $200").is_some());
        assert!(before("show my expenses").is_none());
        assert!(before("I spent $86 at Nobu").is_none());
    }
}
