//! The composer's typed planning (APP-093; ADR-011 §3 I5): model text in,
//! an [`AdmittedDefinition`] or a coded refusal out. Nothing the model wrote
//! is read as anything but JSON data: the router's reply is decoded into a
//! closed struct whose `choice` must name a catalog entry; a generated reply
//! is decoded strictly as forma-json-v1 (`admission::decode`), its source
//! inputs are replaced by the member's exact readable contracts (so the saved
//! work can bind them and no invented input survives), every data view must
//! be one the engine runs, and the whole content is wrapped in the
//! `app_definition` envelope under the trusted organization and authoring
//! app, digested and judged by `admit_definition`. A refusal is a stable
//! code; the retry loop in [`super::Composer`] feeds back the code alone.
use super::Origin;
use crate::{
    admission::{self, digest, AdmissionError, AdmittedDefinition},
    storage::{LibraryApp, ReadableSource},
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// The router's closed reply shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouterReply {
    choice: String,
    confidence: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    title: Option<String>,
}

/// What the router decided, judged against the catalog it was shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Instantiate this library app as it is.
    Retrieve { index: usize },
    /// Start from this library app with the suggested title.
    Adapt { index: usize, title: String },
    /// No library fit: generate.
    Generate,
}

/// Decodes the router's text. Anything outside the closed shape, an unknown
/// id or an unknown confidence is `Generate` (the slow path, never an
/// error the member sees). `confidence` is reported for the log only.
pub fn decide(text: &str, library: &[LibraryApp]) -> (Decision, &'static str) {
    let Some(reply) =
        object(text).and_then(|value| serde_json::from_value::<RouterReply>(value).ok())
    else {
        return (Decision::Generate, "malformed");
    };
    let _ = reply.reason;
    let Some(index) = library.iter().position(|app| app.app == reply.choice) else {
        return (
            Decision::Generate,
            if reply.choice == "none" {
                "none"
            } else {
                "unknown"
            },
        );
    };
    match reply.confidence.as_str() {
        "high" => (Decision::Retrieve { index }, "high"),
        "medium" => {
            let title = reply
                .title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(|title| title.chars().take(80).collect())
                .unwrap_or_else(|| library[index].title.clone());
            (Decision::Adapt { index, title }, "medium")
        }
        _ => (Decision::Generate, "low"),
    }
}

/// A planned definition, admitted and ready to draft.
#[derive(Clone, Debug)]
pub struct Planned {
    pub admitted: AdmittedDefinition,
    pub title: String,
    pub origin: Origin,
}

/// Why one generation attempt was refused: admission's code or one of the
/// composer's own (`not_json`, `unknown_source_input`, `not_runnable`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub code: &'static str,
}
impl From<AdmissionError> for Refusal {
    fn from(error: AdmissionError) -> Self {
        Self { code: error.code() }
    }
}

/// The first JSON object in the text (fences and prose around it are
/// dropped), decoded strictly as forma-json-v1 (no floats, no duplicate
/// keys, bounded depth and size).
fn object(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end < start {
        return None;
    }
    let value = admission::decode(&text.as_bytes()[start..=end]).ok()?;
    value.is_object().then_some(value)
}

/// Judges one generated reply: parses, pins the source inputs to the
/// member's exact contracts, requires runnable views, envelopes under the
/// trusted scope and admits whole.
pub fn plan_generated(
    text: &str,
    sources: &[ReadableSource],
    organization: &str,
    app: &str,
) -> Result<Planned, Refusal> {
    let mut content = object(text).ok_or(Refusal { code: "not_json" })?;
    pin_sources(&mut content, sources)?;
    // Admission first, so a malformed document reports admission's code;
    // then the engine's subset over what admission accepted.
    let admitted = admit(&content, organization, app)?;
    require_runnable(&content)?;
    Ok(Planned {
        title: title(&content),
        admitted,
        origin: Origin::Generated,
    })
}

/// The adapted path: a typed patch (title, description) of a library
/// definition's re-admitted content, re-admitted whole under the trusted
/// scope. Nothing else of the library content changes.
pub fn plan_adapted(
    library: &LibraryApp,
    title: &str,
    description: &str,
    organization: &str,
    app: &str,
) -> Result<Planned, Refusal> {
    let mut content = library.content.clone();
    let map = content
        .as_object_mut()
        .ok_or(Refusal { code: "encoding" })?;
    map.insert("title".into(), json!(title));
    map.insert("description".into(), json!(description));
    let admitted = admit(&content, organization, app)?;
    Ok(Planned {
        title: title.to_owned(),
        admitted,
        origin: Origin::Adapted,
    })
}

/// Every `source` input is replaced by the readable contract of the same
/// id (exact bytes, so its digest equals a bindable revision's); an id
/// outside the member's sources is refused. `records` inputs stay as the
/// model wrote them, for admission to judge.
fn pin_sources(content: &mut Value, sources: &[ReadableSource]) -> Result<(), Refusal> {
    let unknown = Refusal {
        code: "unknown_source_input",
    };
    let Some(inputs) = content.get_mut("inputs").and_then(Value::as_array_mut) else {
        // Admission names the missing or mistyped member.
        return Ok(());
    };
    for input in inputs.iter_mut() {
        let Some(map) = input.as_object_mut() else {
            continue;
        };
        if map.get("kind").and_then(Value::as_str) != Some("source") {
            continue;
        }
        let id = map
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| unknown.clone())?;
        let source = sources
            .iter()
            .find(|source| source.contract.get("id").and_then(Value::as_str) == Some(id))
            .ok_or_else(|| unknown.clone())?;
        *input = source.contract.clone();
    }
    Ok(())
}

/// Every view with a query reads one input through the runnable operators.
fn require_runnable(content: &Value) -> Result<(), Refusal> {
    let runnable = content
        .get("views")
        .and_then(Value::as_array)
        .is_none_or(|views| {
            views
                .iter()
                .all(|view| super::prompt::runnable_view(content, view))
        });
    if runnable {
        Ok(())
    } else {
        Err(Refusal {
            code: "not_runnable",
        })
    }
}

/// The envelope under the trusted scope (organization and authoring app from
/// the request, never from the model), the content's digest, admission.
fn admit(content: &Value, organization: &str, app: &str) -> Result<AdmittedDefinition, Refusal> {
    let canonical = admission::encode(content)?;
    let identity = digest::digest(digest::APP_DEFINITION, &canonical);
    let mut envelope = Map::new();
    envelope.insert("kind".into(), json!("app_definition"));
    envelope.insert("schema_version".into(), json!(1));
    envelope.insert("organization_id".into(), json!(organization));
    envelope.insert("app_id".into(), json!(app));
    envelope.insert("revision_id".into(), json!(app));
    envelope.insert("content".into(), content.clone());
    envelope.insert("digest".into(), json!(identity));
    let bytes = admission::encode(&Value::Object(envelope))?;
    Ok(admission::admit_definition(&bytes)?)
}

fn title(content: &Value) -> String {
    content
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Composed app")
        .to_owned()
}
