//! Customize a widget in words (APP-103): the member's sentence and the
//! widget's current state go to the fast model (the composer's client and
//! router model, as `actions::Parser` uses them), which answers the whole
//! state again, in the same shape. This is the pattern of our own
//! `forma-vector-studio/src/server/customizer.rs` (the model returns state
//! JSON of the same shape), narrowed to a closed, typed state: the reply is
//! decoded strictly and judged by [`state::validate`]; a key outside the
//! widget's shape or a value outside its closed set (an unknown skin, CSS,
//! markup, a script) refuses the whole reply and the widget keeps its state
//! (ADR-011 I5: model output is data, never code or markup).
//!
//! Demo shortcut (APP-098 DS-66): inside a demo organization the rehearsed
//! sentences (`rehearsed`) map to fixed state changes with no model call.
//! Outside a demo organization every sentence goes to the model.
use super::state::{self, Widget, ACCENTS, FONTS, SKINS, UNITS, ZONES};
use crate::composer::model::{self, Model, ModelRequest};
use serde_json::{json, Map, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// The model call's budget (the route answers well inside it; a restyle is
/// a short reply).
pub const BUDGET: Duration = Duration::from_secs(6);
const MAX_TOKENS: u32 = 200;

/// How a new state was obtained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Rehearsed,
    Model,
}
impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rehearsed => "rehearsed",
            Self::Model => "model",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub state: Value,
    pub source: Source,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// No model and nothing rehearsed applies (503 `composer_unavailable`).
    Unavailable,
    /// The model failed, or answered outside the closed state (422
    /// `could_not_customize`): the widget keeps its state.
    Refused(&'static str),
}

/// A member's sentence: 1 to 300 characters, no control characters.
pub fn text(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    ((1..=300).contains(&count) && !trimmed.chars().any(char::is_control)).then_some(trimmed)
}

/// Lowercase words of `text`, punctuation dropped, "ti-84"/"ti 84" as "ti84".
fn normalize(text: &str) -> String {
    let lowered: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    lowered
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("ti 84", "ti84")
}

/// DS-66: the rehearsed sentences of the pitch, as fixed changes to the
/// state (demo organizations only; the caller checks).
pub fn rehearsed(widget: Widget, sentence: &str) -> Option<Map<String, Value>> {
    let words = normalize(sentence);
    let set = |pairs: &[(&str, Value)]| {
        let mut patch = Map::new();
        for (key, value) in pairs {
            patch.insert((*key).to_owned(), value.clone());
        }
        Some(patch)
    };
    let look = |skin: &str, accent: &str, font: &str| {
        set(&[
            ("skin", skin.into()),
            ("accent", accent.into()),
            ("font", font.into()),
        ])
    };
    let strip = |w: &str| {
        w.trim_start_matches("make it ")
            .trim_start_matches("make this ")
            .trim_start_matches("turn it into ")
            .trim_start_matches("a ")
            .trim_start_matches("an ")
            .trim_start_matches("look like ")
            .trim_start_matches("the ")
            .to_owned()
    };
    let core = strip(&words);
    match core.as_str() {
        "ti84 in matrix style"
        | "ti84 in the matrix style"
        | "ti84 matrix"
        | "matrix ti84"
        | "ti84 in matrix" => return look("ti84", "matrix", "lcd"),
        "ti84" | "ti84 calculator" | "graphing calculator" => {
            return look("ti84", "default", "lcd")
        }
        "matrix" | "matrix style" | "in matrix style" | "in the matrix style" => {
            return look("matrix", "matrix", "mono")
        }
        "retro" | "retro style" | "80s" => return look("retro", "default", "lcd"),
        "glass" | "glassy" | "frosted glass" | "glass style" => {
            return look("glass", "default", "rounded")
        }
        "paper" | "paper style" | "minimal" | "like paper" => {
            return look("paper", "default", "serif")
        }
        "default" | "reset" | "back to default" | "normal" => {
            return look("default", "default", "default")
        }
        "amber" | "green" | "cyan" | "violet" | "purple" | "rose" | "pink" | "red" | "ice"
        | "blue" => {
            let accent = match core.as_str() {
                "green" => "matrix",
                "purple" => "violet",
                "pink" => "rose",
                "red" => "crimson",
                "blue" => "ice",
                other => other,
            };
            return set(&[("accent", accent.into())]);
        }
        _ => {}
    }
    match widget {
        Widget::Weather => match core.as_str() {
            "show celsius" | "celsius" | "metric" | "use celsius" | "in celsius" => {
                set(&[("units", "metric".into())])
            }
            "show fahrenheit" | "fahrenheit" | "imperial" | "use fahrenheit" | "in fahrenheit" => {
                set(&[("units", "imperial".into())])
            }
            other => {
                let place = other.trim_start_matches("show ").trim_start_matches("in ");
                [
                    "austin",
                    "tokyo",
                    "new york",
                    "san francisco",
                    "london",
                    "quito",
                    "seattle",
                ]
                .contains(&place)
                .then(|| {
                    let mut patch = Map::new();
                    patch.insert("location".into(), title_case(place).into());
                    patch
                })
            }
        },
        Widget::Clock => match core.as_str() {
            "24 hour" | "24 hours" | "24h" | "24 hour time" | "use 24 hour time"
            | "military time" => set(&[("hour12", false.into())]),
            "12 hour" | "12h" | "12 hour time" | "use 12 hour time" => {
                set(&[("hour12", true.into())])
            }
            other => {
                let place = other.trim_start_matches("show ").trim_start_matches("in ");
                state::zone(place).map(|zone| {
                    let mut patch = Map::new();
                    patch.insert("zone".into(), zone.into());
                    patch
                })
            }
        },
        Widget::Calculator => None,
    }
}

fn title_case(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `state` with `patch` applied, judged again.
pub fn apply(widget: Widget, current: &Value, patch: &Map<String, Value>) -> Option<Value> {
    let mut next = current.as_object()?.clone();
    for (key, value) in patch {
        if !next.contains_key(key) {
            continue;
        }
        next.insert(key.clone(), value.clone());
    }
    state::validate(widget, &Value::Object(next))
}

/// The system prompt: the closed sets, the shape, the refusal rule.
pub fn system(widget: Widget) -> String {
    let list = |set: &[&str]| set.join(", ");
    let zones: Vec<&str> = ZONES.iter().map(|(id, _)| *id).collect();
    let arguments = match widget {
        Widget::Weather => format!(
            "\"location\": a place name (city, optional region; letters, digits, spaces and , . ' - only; 2 to 80 characters), \"units\": one of [{}]",
            list(UNITS)
        ),
        Widget::Calculator => String::from("no other keys"),
        Widget::Clock => format!(
            "\"zone\": one of [{}], \"hour12\": true or false",
            list(&zones)
        ),
    };
    format!(
        "You restyle a {widget} widget on a dashboard. You receive JSON with the widget's current state and the member's request. \
Answer with ONLY one JSON object: the complete new state, with exactly the same keys as the current state, nothing else.\n\
Keys and allowed values (closed sets; use the exact lowercase spelling):\n\
- \"skin\": one of [{skins}] (ti84 = a TI-84 graphing calculator body; matrix = black with green code rain; retro = 1980s beige plastic with LED digits; glass = frosted translucent; paper = light, ink on paper)\n\
- \"accent\": one of [{accents}] (matrix = phosphor green; crimson = red; ice = pale blue)\n\
- \"font\": one of [{fonts}] (lcd = calculator/LED display digits)\n\
- {arguments}\n\
Change only what the request asks for; keep every other value. A style name maps to the closest skin, accent and font together (for example \"TI-84 in Matrix style\" is skin ti84, accent matrix, font lcd). \
Never write CSS, HTML, color codes, scripts or any value outside the sets. If the request cannot be expressed with these values, return the current state unchanged.",
        widget = widget.as_str(),
        skins = list(SKINS),
        accents = list(ACCENTS),
        fonts = list(FONTS),
    )
}

/// The user message: the state and the request, as JSON data.
pub fn user(widget: Widget, current: &Value, sentence: &str) -> String {
    json!({"widget": widget.as_str(), "current_state": current, "request": sentence}).to_string()
}

/// Decodes the model's text: the first JSON object in it, as forma-json-v1,
/// judged as the widget's state. `None` refuses the whole reply.
pub fn decode(widget: Widget, text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end < start {
        return None;
    }
    let value = crate::admission::decode(&text.as_bytes()[start..=end]).ok()?;
    state::validate(widget, &value)
}

/// The customizer: the fast model (none: disabled) and its name.
pub struct Customizer {
    model: Option<Arc<dyn Model>>,
    model_name: String,
}
impl Customizer {
    pub fn new(model: Option<Arc<dyn Model>>, model_name: &str) -> Self {
        Self {
            model,
            model_name: model_name.to_owned(),
        }
    }
    /// The composer's key and router model, as `actions::Parser::from_env`.
    pub fn from_env() -> Self {
        let name = std::env::var(model::ROUTER_MODEL_ENV)
            .ok()
            .filter(|value| {
                (1..=64).contains(&value.len())
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
            })
            .unwrap_or_else(|| model::DEFAULT_ROUTER_MODEL.to_owned());
        let model = model::Anthropic::from_env().map(|client| Arc::new(client) as Arc<dyn Model>);
        Self::new(model, &name)
    }
    pub fn enabled(&self) -> bool {
        self.model.is_some()
    }

    /// The new state for `sentence`. `current` must already be valid.
    pub async fn customize(
        &self,
        widget: Widget,
        current: &Value,
        sentence: &str,
        demo: bool,
    ) -> Result<Outcome, Failure> {
        let started = Instant::now();
        let elapsed = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if demo {
            if let Some(next) = rehearsed(widget, sentence).and_then(|p| apply(widget, current, &p))
            {
                return Ok(Outcome {
                    state: next,
                    source: Source::Rehearsed,
                    duration_ms: elapsed(),
                });
            }
        }
        let Some(model) = self.model.as_ref() else {
            return Err(Failure::Unavailable);
        };
        let request = ModelRequest {
            model: self.model_name.clone(),
            system: system(widget),
            user: user(widget, current, sentence),
            max_tokens: MAX_TOKENS,
            budget: BUDGET,
        };
        let reply = model
            .complete(&request)
            .await
            .map_err(|error| Failure::Refused(error.code()))?;
        decode(widget, &reply.text)
            .map(|state| Outcome {
                state,
                source: Source::Model,
                duration_ms: elapsed(),
            })
            .ok_or(Failure::Refused("outside_closed_state"))
    }
}
