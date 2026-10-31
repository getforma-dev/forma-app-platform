//! APP-101 instant pick: the local router that runs before any model call,
//! in every organization (it replaces APP-098's `plan::demo_route`; it is a
//! real router, not a demo shortcut).
//!
//! The member's intent is reduced to normalized tokens (lowercase, numbers
//! and stopwords dropped, a light suffix stemmer, a small synonym table) and
//! scored against each library app the member may open now:
//!
//! - the app's routing metadata from the library sidecar
//!   (`contracts/fixtures/library/catalog.json`, keyed by the definition
//!   digest the seed placed): `routing.intent_examples` (the strongest
//!   signal: the best Dice overlap with one example), `routing.description`,
//!   `capabilities`, and its `negative_examples`, any one of which, fully
//!   present in the intent, **vetoes** the app;
//! - the definition's title, description, input field ids and view titles
//!   (the vocabulary coverage of the intent).
//!
//! An app outside the sidecar (a team-published app) is scored by its title
//! as its one example and its definition's vocabulary. The member's own
//! authoring app (where compositions are drafted) is never a candidate.
//!
//! A confident pick (score at least [`CONFIDENT`], ahead of the runner-up by
//! at least [`MARGIN`]) is retrieved at once with no model call; a near tie
//! or no hit falls back to the router model, today's path. Pure CPU over a
//! handful of short strings: well under a millisecond for the 14-app library
//! (measured in `tests::every_example_picks_its_app_and_no_negative_does`).
//! Nothing here reads the intent as anything but words, and nothing it
//! returns is more than an index into the catalog the server built.
use crate::storage::LibraryApp;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

/// The score a pick must reach.
pub const CONFIDENT: f64 = 0.55;
/// The lead a pick must hold over the runner-up.
pub const MARGIN: f64 = 0.12;
/// `GENUI_COMPOSER_MATCHER=off`: the matcher never fires and the composer
/// behaves as before APP-101 (the ticket's rollback).
pub const MATCHER_ENV: &str = "GENUI_COMPOSER_MATCHER";
/// Weight of the best intent-example overlap; the rest is vocabulary coverage.
const EXAMPLE_WEIGHT: f64 = 0.65;

/// Words that carry no routing signal (they are also what a transform reads,
/// so they never pull an intent toward an app).
const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "all",
    "an",
    "and",
    "any",
    "app",
    "apps",
    "are",
    "as",
    "at",
    "be",
    "below",
    "by",
    "can",
    "do",
    "does",
    "each",
    "every",
    "for",
    "from",
    "get",
    "give",
    "has",
    "have",
    "how",
    "i",
    "in",
    "into",
    "is",
    "it",
    "its",
    "just",
    "last",
    "least",
    "less",
    "let",
    "list",
    "many",
    "me",
    "more",
    "most",
    "much",
    "my",
    "need",
    "new",
    // APP-109: "not" stems like "note"; a negation never names an app.
    "not",
    "of",
    "on",
    "only",
    "or",
    "our",
    "over",
    "past",
    "per",
    "please",
    "see",
    "show",
    "some",
    "than",
    "that",
    "the",
    "their",
    "there",
    "these",
    "this",
    "those",
    "to",
    "top",
    "under",
    "up",
    "us",
    "view",
    "vs",
    "want",
    "was",
    "we",
    "what",
    "whats",
    "which",
    "who",
    "with",
    "you",
    "your",
    // The shape words a transform reads (APP-101): never routing signal.
    "ascending",
    "bar",
    "between",
    "chart",
    "daily",
    "day",
    "days",
    "descending",
    "except",
    "excluding",
    "graph",
    "line",
    "only",
    "since",
    "sort",
    "sorted",
    "table",
    "until",
    "week",
    "weekly",
    "where",
    "without",
    "year",
    "yesterday",
];

/// Stem → canonical stem (applied after stemming, to both sides).
const SYNONYMS: &[(&str, &str)] = &[
    ("egres", "outbound"),
    ("egress", "outbound"),
    ("supplier", "vendor"),
    ("bill", "invoic"),
    ("recruit", "hir"),
    ("recruiting", "hir"),
    ("applicant", "candidat"),
    ("outag", "incident"),
    ("spent", "spend"),
    ("reimburs", "reimbursement"),
    ("reimbursement", "reimbursement"),
    ("tick", "ticket"),
];

/// The outcome of one match, for the decision and the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// The confident pick's index into the library, if any.
    pub pick: Option<usize>,
    /// The best and the runner-up score (for the log and the tests).
    pub best: f64,
    pub runner_up: f64,
    /// `matched`, `near_tie`, `no_hit`, `no_words` or `off`.
    pub reason: &'static str,
    /// The best candidates at or above [`CHOICE_FLOOR`], best first, at
    /// most [`MAX_CHOICES`] (the canvas chooser for a near tie).
    pub ranked: Vec<usize>,
}

/// A candidate the chooser may offer scores at least this.
pub const CHOICE_FLOOR: f64 = 0.35;
/// The chooser offers at most this many apps.
pub const MAX_CHOICES: usize = 3;

/// One app's routing words, prepared once per sidecar entry or per call.
struct Routing {
    examples: Vec<BTreeSet<String>>,
    negatives: Vec<BTreeSet<String>>,
    vocabulary: BTreeSet<String>,
}

/// Normalized tokens of a text: lowercase alphanumeric words, numbers and
/// stopwords dropped, stemmed, synonyms folded.
pub fn tokens(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 2)
        .filter(|word| !word.chars().all(|c| c.is_ascii_digit()))
        .filter(|word| !STOPWORDS.contains(word))
        .map(stem)
        .map(|stem| {
            SYNONYMS
                .iter()
                .find(|(from, _)| *from == stem)
                .map(|(_, to)| (*to).to_owned())
                .unwrap_or(stem)
        })
        .collect()
}

/// A light suffix stemmer: plural, then one derivational suffix, then a
/// trailing `e`; never below three letters. Consistency matters, not
/// linguistics: both sides go through it.
fn stem(word: &str) -> String {
    let mut w = word.to_owned();
    let cut = |w: &mut String, suffix: &str, replacement: &str| -> bool {
        if w.len() > suffix.len() + 2 && w.ends_with(suffix) {
            w.truncate(w.len() - suffix.len());
            w.push_str(replacement);
            true
        } else {
            false
        }
    };
    if !cut(&mut w, "ies", "y") && !w.ends_with("ss") && !w.ends_with("us") {
        cut(&mut w, "s", "");
    }
    let _ = cut(&mut w, "ation", "at")
        || cut(&mut w, "ing", "")
        || cut(&mut w, "ion", "")
        || cut(&mut w, "ed", "")
        || cut(&mut w, "al", "");
    cut(&mut w, "e", "");
    w
}

fn dice(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.intersection(b).count() as f64;
    2.0 * shared / (a.len() + b.len()) as f64
}

/// The sidecar's routing by definition digest (parsed once).
fn sidecar() -> &'static BTreeMap<String, Value> {
    static SIDECAR: OnceLock<BTreeMap<String, Value>> = OnceLock::new();
    SIDECAR.get_or_init(|| {
        crate::storage::library_sidecar()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| {
                let digest = entry.get("definition_digest")?.as_str()?.to_owned();
                Some((digest, entry))
            })
            .collect()
    })
}

fn strings(value: Option<&Value>) -> impl Iterator<Item = &str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

/// The routing words of one library app.
fn routing(app: &LibraryApp) -> Routing {
    let mut vocabulary = tokens(&app.title);
    vocabulary.extend(tokens(&app.description));
    for input in &app.inputs {
        for field in input.fields.as_array().into_iter().flatten() {
            if let Some(id) = field.get("id").and_then(Value::as_str) {
                vocabulary.extend(tokens(id));
            }
            // Enum values ("travel", "udp", "rejected") name this app's data.
            for value in strings(
                field
                    .get("value_type")
                    .and_then(|t| t.get("values"))
                    .or_else(|| field.get("values")),
            ) {
                vocabulary.extend(tokens(value));
            }
        }
    }
    for view in &app.views {
        vocabulary.extend(tokens(&view.title));
    }
    match sidecar().get(&app.definition_digest) {
        Some(entry) => {
            let routing = entry.get("routing");
            let examples: Vec<BTreeSet<String>> =
                strings(routing.and_then(|r| r.get("intent_examples")))
                    .map(tokens)
                    .filter(|set| !set.is_empty())
                    .collect();
            let negatives = strings(routing.and_then(|r| r.get("negative_examples")))
                .map(tokens)
                .filter(|set| !set.is_empty())
                .collect();
            if let Some(description) = routing
                .and_then(|r| r.get("description"))
                .and_then(Value::as_str)
            {
                vocabulary.extend(tokens(description));
            }
            for capability in strings(routing.and_then(|r| r.get("capabilities"))) {
                vocabulary.extend(tokens(capability));
            }
            if let Some(purpose) = entry.get("purpose").and_then(Value::as_str) {
                vocabulary.extend(tokens(purpose));
            }
            for example in &examples {
                vocabulary.extend(example.iter().cloned());
            }
            Routing {
                examples,
                negatives,
                vocabulary,
            }
        }
        None => Routing {
            examples: vec![tokens(&app.title)],
            negatives: Vec::new(),
            vocabulary,
        },
    }
}

/// The one-line purpose the chooser and the command list show: the
/// sidecar's `purpose`, else the definition's description, bounded.
pub fn purpose(app: &LibraryApp) -> String {
    sidecar()
        .get(&app.definition_digest)
        .and_then(|entry| entry.get("purpose"))
        .and_then(Value::as_str)
        .unwrap_or(&app.description)
        .chars()
        .take(160)
        .collect()
}

/// The sidecar's command slug and aliases of a library app (APP-101).
pub fn command(app: &LibraryApp) -> Option<(String, Vec<String>)> {
    let command = sidecar().get(&app.definition_digest)?.get("command")?;
    Some((
        command.get("slug")?.as_str()?.to_owned(),
        strings(command.get("aliases")).map(str::to_owned).collect(),
    ))
}

/// APP-120: the title a library app is shown under (the Library, `GET
/// /v1/commands`) when the sidecar's `command.title` differs from the
/// definition's: stored definitions never change, so the display does.
pub fn display_title(digest: &str) -> Option<String> {
    sidecar()
        .get(digest)?
        .get("command")?
        .get("title")?
        .as_str()
        .map(str::to_owned)
}

/// APP-119: a library app's address copy from the sidecar: the optional
/// `command.hint` and `command.examples` overrides (demo-quality copy) and
/// the routing `intent_examples` the derived examples are made from.
pub fn address_copy(app: &LibraryApp) -> (Option<String>, Vec<String>, Vec<String>) {
    let Some(entry) = sidecar().get(&app.definition_digest) else {
        return (None, Vec::new(), Vec::new());
    };
    let command = entry.get("command");
    let hint = command
        .and_then(|c| c.get("hint"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let examples = strings(command.and_then(|c| c.get("examples")))
        .map(str::to_owned)
        .collect();
    let intents = strings(entry.get("routing").and_then(|r| r.get("intent_examples")))
        .map(str::to_owned)
        .collect();
    (hint, examples, intents)
}

/// APP-104: the library pane's grouping of a library app, by its pinned
/// definition digest: its position in the sidecar, the sidecar's `library_id`, the first routing
/// capability's prefix (`finance.expenses.claim` → `finance`) and its
/// command slug; `None` for a definition outside the sidecar (a composed,
/// adapted or team-published app).
pub fn library_grouping(digest: &str) -> Option<(usize, String, String, String)> {
    static ORDER: OnceLock<Vec<String>> = OnceLock::new();
    let order = ORDER.get_or_init(|| {
        crate::storage::library_sidecar()
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| entry.get("definition_digest")?.as_str().map(str::to_owned))
            .collect()
    });
    let position = order.iter().position(|known| known == digest)?;
    let entry = sidecar().get(digest)?;
    let library_id = entry.get("library_id")?.as_str()?.to_owned();
    let prefix = strings(entry.get("routing").and_then(|r| r.get("capabilities")))
        .next()
        .and_then(|capability| capability.split('.').next())
        .unwrap_or_default()
        .to_owned();
    let slug = entry
        .get("command")
        .and_then(|c| c.get("slug"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some((position, library_id, prefix, slug))
}

/// APP-104: whether `work` is a library app's seeded work (the sidecar's
/// `work_id`): its seeded records are sample data, never a request waiting
/// for a member.
pub fn seeded_work(work: &str) -> bool {
    static SEEDED: OnceLock<BTreeSet<String>> = OnceLock::new();
    SEEDED
        .get_or_init(|| {
            crate::storage::library_sidecar()
                .unwrap_or_default()
                .iter()
                .filter_map(|entry| entry.get("work_id")?.as_str().map(str::to_owned))
                .collect()
        })
        .contains(work)
}

/// Known seeded definition bytes: the current catalog and the two original
/// Phase 1 fixtures. A work id or an organization alone proves no provenance.
pub fn seeded_definition(digest: &str) -> bool {
    static BASELINE: OnceLock<BTreeSet<String>> = OnceLock::new();
    sidecar().contains_key(digest)
        || BASELINE
            .get_or_init(|| {
                [
                    include_str!("../../../contracts/fixtures/impl-2026-10-28.1/northstar.json"),
                    include_str!("../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json"),
                ]
                .iter()
                .filter_map(|text| serde_json::from_str::<Value>(text).ok())
                .filter_map(|bundle| bundle["definition"]["digest"].as_str().map(str::to_owned))
                .collect()
            })
            .contains(digest)
}

/// Every library slug and alias (reserved: a saved work cannot take one).
pub fn library_slugs() -> std::collections::BTreeSet<String> {
    sidecar()
        .values()
        .filter_map(|entry| entry.get("command"))
        .flat_map(|command| {
            command
                .get("slug")
                .and_then(Value::as_str)
                .into_iter()
                .chain(strings(command.get("aliases")))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Scores the intent against the library (module notes). `exclude` is the
/// member's authoring app, never a candidate.
pub fn pick(intent: &str, library: &[LibraryApp], exclude: Option<&str>) -> Match {
    let words = tokens(intent);
    if words.is_empty() {
        return Match {
            pick: None,
            best: 0.0,
            runner_up: 0.0,
            reason: "no_words",
            ranked: Vec::new(),
        };
    }
    let candidates: Vec<(usize, Routing)> = library
        .iter()
        .enumerate()
        .filter(|(_, app)| exclude != Some(app.app.as_str()))
        .map(|(index, app)| (index, routing(app)))
        .collect();
    // Words no candidate knows ("PTO", a merchant, a host name) say nothing
    // about which app; they are left to the transform (APP-101).
    let known: BTreeSet<String> = words
        .iter()
        .filter(|word| candidates.iter().any(|(_, r)| r.vocabulary.contains(*word)))
        .cloned()
        .collect();
    let unknown = words.len() - known.len();
    let all_words = words;
    let words = known;
    if words.is_empty() {
        return Match {
            pick: None,
            best: 0.0,
            runner_up: 0.0,
            reason: "no_hit",
            ranked: Vec::new(),
        };
    }
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (index, routing) in &candidates {
        let index = *index;
        if routing
            .negatives
            .iter()
            .any(|negative| negative.is_subset(&all_words))
        {
            continue;
        }
        let example = routing
            .examples
            .iter()
            .map(|example| dice(&words, example))
            .fold(0.0, f64::max);
        // Unknown words still dilute coverage a little, so a long request
        // mostly about something else does not ride on one shared word.
        let coverage = words.intersection(&routing.vocabulary).count() as f64
            / (words.len() as f64 + unknown as f64 / 2.0);
        scored.push((
            index,
            EXAMPLE_WEIGHT * example + (1.0 - EXAMPLE_WEIGHT) * coverage,
        ));
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let best = scored.first().map_or(0.0, |s| s.1);
    let runner_up = scored.get(1).map_or(0.0, |s| s.1);
    let (pick, reason) = if best < CONFIDENT {
        (None, "no_hit")
    } else if best - runner_up < MARGIN {
        (None, "near_tie")
    } else {
        (scored.first().map(|s| s.0), "matched")
    };
    Match {
        pick,
        best,
        runner_up,
        reason,
        ranked: scored
            .iter()
            .filter(|(_, score)| *score >= CHOICE_FLOOR)
            .take(MAX_CHOICES)
            .map(|(index, _)| *index)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{LibraryView, PublishedInput};
    use serde_json::json;

    /// The library as the composer context lists it, built from the sidecar
    /// (title, purpose as description, input fields, views).
    pub(crate) fn catalog_library() -> Vec<LibraryApp> {
        crate::storage::library_sidecar()
            .unwrap()
            .iter()
            .map(|entry| LibraryApp {
                app: entry["app_id"].as_str().unwrap().into(),
                publication: entry["publication_id"].as_str().unwrap().into(),
                definition_digest: entry["definition_digest"].as_str().unwrap().into(),
                title: entry["title"].as_str().unwrap().into(),
                description: entry["purpose"].as_str().unwrap().into(),
                inputs: entry["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|input| PublishedInput {
                        id: input["id"].as_str().unwrap().into(),
                        kind: input["kind"].as_str().unwrap().into(),
                        contract_digest: "sha256:0".into(),
                        fields: input["fields"].clone(),
                    })
                    .collect(),
                record_schemas: Vec::new(),
                views: entry["views"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|view| LibraryView {
                        id: view["id"].as_str().unwrap().into(),
                        kind: view["kind"].as_str().unwrap().into(),
                        title: view["title"].as_str().unwrap().into(),
                    })
                    .collect(),
                content: json!({}),
            })
            .collect()
    }

    #[test]
    fn stemming_folds_the_forms_the_library_uses() {
        for (a, b) in [
            ("expenses", "expense"),
            ("approval", "approve"),
            ("approvals", "approves"),
            ("investigation", "investigate"),
            ("bytes", "byte"),
            ("waiting", "wait"),
            ("categories", "category"),
        ] {
            assert_eq!(stem(a), stem(b), "{a} {b}");
        }
        assert!(tokens("Review expenses over $200").contains("expens"));
        assert!(!tokens("Review expenses over $200").contains("200"));
        assert!(tokens("the of my over").is_empty());
    }

    #[test]
    fn every_example_picks_its_app_and_no_negative_does() {
        let library = catalog_library();
        assert_eq!(library.len(), 20);
        let sidecar = crate::storage::library_sidecar().unwrap();
        let mut samples = Vec::new();
        for entry in &sidecar {
            let app = entry["app_id"].as_str().unwrap();
            for example in strings(entry["routing"].get("intent_examples")) {
                let started = std::time::Instant::now();
                let found = pick(example, &library, None);
                samples.push(started.elapsed());
                let picked = found.pick.map(|index| library[index].app.as_str());
                assert_eq!(
                    picked,
                    Some(app),
                    "{example:?} should pick {} ({found:?})",
                    entry["title"]
                );
            }
            for negative in strings(entry["routing"].get("negative_examples")) {
                let found = pick(negative, &library, None);
                assert_ne!(
                    found.pick.map(|index| library[index].app.as_str()),
                    Some(app),
                    "negative {negative:?} picked {}",
                    entry["title"]
                );
            }
        }
        samples.sort();
        let p95 = samples[samples.len() * 95 / 100];
        assert!(
            p95 < std::time::Duration::from_millis(50),
            "p95 {p95:?} over {} picks",
            samples.len()
        );
        eprintln!(
            "matcher: {} example picks, p50 {:?}, p95 {:?}, max {:?}",
            samples.len(),
            samples[samples.len() / 2],
            p95,
            samples.last().unwrap()
        );
    }

    #[test]
    fn the_pitch_sentences_route_as_rehearsed() {
        let library = catalog_library();
        let title = |intent: &str| {
            pick(intent, &library, None)
                .pick
                .map(|index| library[index].title.clone())
        };
        assert_eq!(
            title("Review expenses over $200").as_deref(),
            Some("Expense review queue")
        );
        assert_eq!(
            title("Investigate outbound traffic from build-worker-17").as_deref(),
            Some("Outbound traffic investigation")
        );
        assert_eq!(
            title("show outbound traffic from build-worker-17 by destination").as_deref(),
            Some("Outbound traffic investigation")
        );
        // Scenario 4 is a new app: no confident pick, the router model decides.
        assert_eq!(
            title("Build a PTO request app: employees request days off with start date, end date and reason; managers approve or reject; show requests by status and by month"),
            None
        );
        assert_eq!(title("weather in Paris tomorrow"), None);
        assert_eq!(title("the of my"), None);
    }

    #[test]
    fn the_authoring_app_is_never_a_candidate_and_unknown_apps_score_by_title() {
        let mut library = catalog_library();
        let mut own = library[0].clone();
        own.app = "00000000-0000-4000-8000-00000000e0a1".into();
        own.definition_digest = "sha256:not-in-the-sidecar".into();
        own.title = "Expense review queue".into();
        library.push(own);
        // With the composed copy excluded, the library app is picked.
        let found = pick(
            "expense review queue",
            &library,
            Some("00000000-0000-4000-8000-00000000e0a1"),
        );
        assert_eq!(found.pick, Some(0));
        // Included, the two tie: the router model decides.
        let found = pick("expense review queue", &library, None);
        assert_eq!(found.pick, None);
        assert_eq!(found.reason, "near_tie");
        // The chooser offers both.
        assert_eq!(found.ranked[..2], [0, 20]);
    }
}
