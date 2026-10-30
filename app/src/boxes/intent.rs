//! Folder requests in plain words, on every surface (the canvas action path,
//! Slack, MCP): "build a metrics app from this folder", "build a metrics app
//! from the Q3 metrics folder", "from folder 1", "brief me on the Q3 box",
//! `/folder <name|number> formation <instruction>`.
//!
//! Classification is closed and deterministic (no model call): a sentence is
//! a folder request only when it names the folder (the canvas's active chip
//! or @-mention id, or "from/on the <name> folder", "folder <n>") and asks to
//! build something or for a brief. The folder is resolved among the member's
//! own folders by number or name: one match acts, several are a chooser,
//! none is not a folder request. Nothing is guessed.
use super::routes::{numbered, Boxes};
use crate::storage::TrustedRequest;
use std::sync::{Arc, OnceLock};

static INSTALLED: OnceLock<Arc<Boxes>> = OnceLock::new();

/// The process's folder state, for the surfaces that do not carry it (the
/// action route, Slack, MCP). Set once by `main` (and the campaigns).
pub fn install(state: Arc<Boxes>) {
    let _ = INSTALLED.set(state);
}
pub fn installed() -> Option<Arc<Boxes>> {
    INSTALLED.get().cloned()
}

/// What the member asks of a folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ask {
    /// Build an app; the purpose is the sentence.
    Build(String),
    /// The folder's brief.
    Brief,
}

/// A folder named in the words.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FolderRef {
    Number(usize),
    Name(String),
}

const BUILD: &[&str] = &[
    "build",
    "make",
    "create",
    "generate",
    "app",
    "dashboard",
    "tracker",
    "chart",
    "report",
    "show",
];
const BRIEF: &[&str] = &[
    "brief",
    "explain",
    "summarize",
    "summarise",
    "summary",
    "overview",
    "what's in",
    "whats in",
    "what is in",
];

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Build or brief, or neither.
pub fn ask(text: &str) -> Option<Ask> {
    let lower = text.to_lowercase();
    let ws = words(text);
    let has = |list: &[&str]| {
        list.iter().any(|w| {
            if w.contains(' ') {
                lower.contains(w)
            } else {
                ws.iter().any(|x| x == w)
            }
        })
    };
    if has(BUILD) {
        return Some(Ask::Build(text.trim().chars().take(500).collect()));
    }
    has(BRIEF).then_some(Ask::Brief)
}

/// The folder a sentence names: "folder 2", "box 2", "from the Q3 metrics
/// folder", "on the Q3 box", "from @Q3 metrics"; `None` otherwise.
pub fn folder_ref(text: &str) -> Option<FolderRef> {
    let lower = text.to_lowercase();
    let nouns = ["folder", "box", "storage"];
    // "folder 2", "box #2"
    for noun in nouns {
        let mut rest = lower.as_str();
        while let Some(at) = rest.find(noun) {
            let after = rest[at + noun.len()..].trim_start().trim_start_matches('#');
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() && digits.len() <= 3 {
                return digits.parse().ok().map(FolderRef::Number);
            }
            rest = &rest[at + noun.len()..];
        }
    }
    // "@Q3 metrics" (to the end of the sentence or a comma).
    if let Some(at) = text.find('@') {
        let name: String = text[at + 1..]
            .split([',', '.', '\n'])
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(60)
            .collect();
        if !name.is_empty() {
            return Some(FolderRef::Name(name));
        }
    }
    // "from|on|in|of the <name> folder"
    for noun in nouns {
        for lead in [
            "from the ",
            "on the ",
            "in the ",
            "of the ",
            "from my ",
            "on my ",
            "from ",
            "on ",
        ] {
            if let Some(start) = lower.find(lead) {
                let tail = &lower[start + lead.len()..];
                if let Some(end) = tail.find(&format!(" {noun}")) {
                    let name = text[start + lead.len()..start + lead.len() + end].trim();
                    if !name.is_empty()
                        && name.chars().count() <= 60
                        && !matches!(name.to_lowercase().as_str(), "this" | "that" | "my" | "the")
                    {
                        return Some(FolderRef::Name(name.to_owned()));
                    }
                }
            }
        }
    }
    None
}

/// The purpose without the folder reference ("build a metrics app").
pub fn purpose(text: &str) -> String {
    let mut out = text.to_owned();
    if let Some(at) = out.find('@') {
        out.truncate(at);
    }
    let lower = out.to_lowercase();
    for lead in [
        " from the ",
        " from my ",
        " from this ",
        " from folder",
        " from box",
        " from ",
    ] {
        if let Some(at) = lower.find(lead) {
            if lower[at..].contains("folder")
                || lower[at..].contains("box")
                || lead == " from " && text.contains('@')
            {
                out.truncate(at);
                break;
            }
        }
    }
    let out = out.trim().trim_end_matches([',', '.']).trim();
    if out.chars().count() < 3 {
        text.trim().to_owned()
    } else {
        out.to_owned()
    }
}

/// `/folder <name|number> formation <instruction>` (or `/box`, `/storage`):
/// the folder reference (None: "this" folder) and the instruction.
pub fn formation(text: &str) -> Option<(Option<FolderRef>, String)> {
    let trimmed = text.trim().trim_start_matches('/');
    let (command, rest) = trimmed.split_once(char::is_whitespace)?;
    if !super::COMMAND_ALIASES.contains(&command.to_lowercase().as_str()) {
        return None;
    }
    let lower = rest.to_lowercase();
    let at = lower.find(super::AI_KEYWORD)?;
    let reference = rest[..at].trim();
    let instruction = rest[at + super::AI_KEYWORD.len()..].trim().to_owned();
    if instruction.is_empty() {
        return None;
    }
    let reference = if reference.is_empty() {
        None
    } else if let Ok(n) = reference.trim_start_matches('#').parse::<usize>() {
        Some(FolderRef::Number(n))
    } else {
        Some(FolderRef::Name(reference.trim_matches('"').to_owned()))
    };
    Some((reference, instruction))
}

/// How a reference resolved among the member's folders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolved {
    One(String, String),
    Many(Vec<(usize, String, String)>),
    None,
}

/// Resolves a reference by number, exact name, then a unique name prefix or
/// containment; several equally good matches are the chooser.
pub async fn resolve(state: &Boxes, member: &TrustedRequest, reference: &FolderRef) -> Resolved {
    let Ok(rows) = numbered(state, member).await else {
        return Resolved::None;
    };
    match reference {
        FolderRef::Number(n) => rows
            .iter()
            .find(|(number, _)| number == n)
            .map_or(Resolved::None, |(_, r)| {
                Resolved::One(r.box_id.clone(), r.name.clone())
            }),
        FolderRef::Name(name) => {
            let wanted = name.trim().to_lowercase();
            let exact: Vec<_> = rows
                .iter()
                .filter(|(_, r)| r.name.to_lowercase() == wanted)
                .collect();
            let loose: Vec<_> = if exact.is_empty() {
                rows.iter()
                    .filter(|(_, r)| {
                        let have = r.name.to_lowercase();
                        have.contains(&wanted) || wanted.contains(&have)
                    })
                    .collect()
            } else {
                exact
            };
            match loose.as_slice() {
                [] => Resolved::None,
                [(_, r)] => Resolved::One(r.box_id.clone(), r.name.clone()),
                many => Resolved::Many(
                    many.iter()
                        .map(|(n, r)| (*n, r.box_id.clone(), r.name.clone()))
                        .collect(),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_are_closed() {
        assert_eq!(
            ask("build a metrics app and explain it"),
            Some(Ask::Build("build a metrics app and explain it".into()))
        );
        assert_eq!(ask("brief me"), Some(Ask::Brief));
        assert_eq!(ask("what's in it?"), Some(Ask::Brief));
        assert_eq!(ask("approve the Nobu dinner"), None);
    }

    #[test]
    fn folder_references_by_number_name_and_mention() {
        assert_eq!(
            folder_ref("build a metrics app from folder 1"),
            Some(FolderRef::Number(1))
        );
        assert_eq!(folder_ref("from box #2 please"), Some(FolderRef::Number(2)));
        assert_eq!(
            folder_ref("build a metrics app from the Q3 metrics folder"),
            Some(FolderRef::Name("Q3 metrics".into()))
        );
        assert_eq!(
            folder_ref("brief me on the Q3 box"),
            Some(FolderRef::Name("Q3".into()))
        );
        assert_eq!(
            folder_ref("build an app from @Q3 metrics"),
            Some(FolderRef::Name("Q3 metrics".into()))
        );
        assert_eq!(folder_ref("build a metrics app from this folder"), None);
        assert_eq!(folder_ref("log my $86 dinner"), None);
    }

    #[test]
    fn purposes_drop_the_reference() {
        assert_eq!(
            purpose("build a metrics app from the Q3 metrics folder"),
            "build a metrics app"
        );
        assert_eq!(
            purpose("build a metrics app from @Q3 metrics"),
            "build a metrics app"
        );
        assert_eq!(purpose("build a metrics app"), "build a metrics app");
    }

    #[test]
    fn formation_splits_reference_and_instruction() {
        assert_eq!(
            formation("/folder formation brief me"),
            Some((None, "brief me".into()))
        );
        assert_eq!(
            formation("/folder 2 formation build me a metrics app"),
            Some((Some(FolderRef::Number(2)), "build me a metrics app".into()))
        );
        assert_eq!(
            formation("folder Q3 metrics formation explain it"),
            Some((
                Some(FolderRef::Name("Q3 metrics".into())),
                "explain it".into()
            ))
        );
        assert_eq!(
            formation("/box Q3 formation build"),
            Some((Some(FolderRef::Name("Q3".into())), "build".into()))
        );
        assert_eq!(formation("/expense formation x"), None);
        assert_eq!(formation("/folder formation"), None);
    }
}

/// The answer to a folder request on the action route, Slack or MCP.
#[derive(Clone, Debug)]
pub enum Answer {
    /// `{"via":"folder","kind":"build_from_box"|"brief"|"choose_folder", …}`.
    Done(serde_json::Value),
    /// The box id was given but the words ask nothing of a folder.
    NotAFolderRequest,
    /// Refused (its response for HTTP surfaces).
    Refused(super::routes::BuildRefusal),
}

/// Acts on a folder request: `box_id` (the canvas chip or mention) or a
/// folder named in the words; `None` when the words neither carry nor name a
/// folder (the caller's own path continues).
pub async fn act(member: &TrustedRequest, utterance: &str, box_id: Option<&str>) -> Option<Answer> {
    use serde_json::json;
    let state = installed()?;
    let (reference, instruction) = match formation(utterance) {
        Some((reference, instruction)) => (reference, instruction),
        None => (folder_ref(utterance), utterance.to_owned()),
    };
    let target = match (box_id, &reference) {
        (Some(id), _) if crate::storage::valid_identifier(id) => {
            Resolved::One(id.to_owned(), String::new())
        }
        (Some(_), _) => return Some(Answer::Refused(super::routes::BuildRefusal::NotFound)),
        (None, Some(reference)) => {
            ask(&instruction)?;
            resolve(&state, member, reference).await
        }
        (None, None) => return None,
    };
    let Some(wanted) = ask(&instruction) else {
        return Some(Answer::NotAFolderRequest);
    };
    let (box_id, _) = match target {
        Resolved::One(id, name) => (id, name),
        Resolved::Many(choices) => {
            return Some(Answer::Done(
                json!({"via": "folder", "kind": "choose_folder", "noun": super::NOUN,
                "choices": choices.iter().map(|(n, id, name)| json!({"box_id": id, "number": n, "name": name})).collect::<Vec<_>>()}),
            ))
        }
        Resolved::None => return None,
    };
    Some(match wanted {
        Ask::Build(sentence) => {
            let purpose = purpose(&sentence);
            match super::routes::build_from_box(&state, member, &box_id, &purpose, None).await {
                Ok(built) => {
                    let mut value = super::routes::built_json(&built);
                    value["via"] = json!("folder");
                    value["kind"] = json!("build_from_box");
                    value["action"] = json!("open");
                    Answer::Done(value)
                }
                Err(refused) => Answer::Refused(refused),
            }
        }
        Ask::Brief => match super::brief::generate(&state, member, &box_id).await {
            Ok(brief) => Answer::Done(
                json!({"via": "folder", "kind": "brief", "box_id": box_id, "brief": brief}),
            ),
            Err(refused) => Answer::Refused(refused),
        },
    })
}
