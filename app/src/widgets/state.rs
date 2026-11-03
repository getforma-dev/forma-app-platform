//! The closed, typed widget state (APP-103; ADR-011 I5). A widget's look is
//! three keys from fixed sets (`skin`, `accent`, `font`) plus the widget's own
//! arguments; nothing else. Every state that crosses the server, whether the
//! canvas sent it or the model wrote it, is judged here: the exact key set
//! for the widget, every value a member of its closed set (or, for a
//! location, a bounded place name of letters, digits and a few separators).
//! The canvas maps the canonical values to data attributes that its own
//! stylesheet keys on; no value here is ever CSS, markup or script.
use serde_json::{Map, Value};

/// Skins: whole looks implemented in the canvas stylesheet.
pub const SKINS: &[&str] = &["default", "ti84", "matrix", "retro", "glass", "paper"];
/// Accents: a fixed palette (the stylesheet holds the colors).
pub const ACCENTS: &[&str] = &[
    "default", "matrix", "amber", "cyan", "crimson", "violet", "rose", "ice",
];
/// Fonts: fixed stacks (system fonts only; no webfont is fetched).
pub const FONTS: &[&str] = &["default", "mono", "serif", "rounded", "lcd"];
/// Weather units.
pub const UNITS: &[&str] = &["imperial", "metric"];
/// Clock time zones: a fixed list of IANA ids with their display names.
pub const ZONES: &[(&str, &str)] = &[
    ("local", "Local"),
    ("UTC", "UTC"),
    ("America/New_York", "New York"),
    ("America/Chicago", "Chicago"),
    ("America/Denver", "Denver"),
    ("America/Los_Angeles", "Los Angeles"),
    ("America/Anchorage", "Anchorage"),
    ("Pacific/Honolulu", "Honolulu"),
    ("America/Toronto", "Toronto"),
    ("America/Mexico_City", "Mexico City"),
    ("America/Sao_Paulo", "São Paulo"),
    ("America/Bogota", "Bogotá"),
    ("America/Guayaquil", "Guayaquil"),
    ("Europe/London", "London"),
    ("Europe/Dublin", "Dublin"),
    ("Europe/Paris", "Paris"),
    ("Europe/Berlin", "Berlin"),
    ("Europe/Madrid", "Madrid"),
    ("Europe/Rome", "Rome"),
    ("Europe/Amsterdam", "Amsterdam"),
    ("Europe/Stockholm", "Stockholm"),
    ("Europe/Athens", "Athens"),
    ("Europe/Moscow", "Moscow"),
    ("Africa/Cairo", "Cairo"),
    ("Africa/Lagos", "Lagos"),
    ("Africa/Johannesburg", "Johannesburg"),
    ("Asia/Dubai", "Dubai"),
    ("Asia/Kolkata", "Mumbai"),
    ("Asia/Singapore", "Singapore"),
    ("Asia/Hong_Kong", "Hong Kong"),
    ("Asia/Shanghai", "Shanghai"),
    ("Asia/Seoul", "Seoul"),
    ("Asia/Tokyo", "Tokyo"),
    ("Australia/Sydney", "Sydney"),
    ("Pacific/Auckland", "Auckland"),
];

/// The three widgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Widget {
    Weather,
    Calculator,
    Clock,
}
impl Widget {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "weather" => Self::Weather,
            "calculator" => Self::Calculator,
            "clock" => Self::Clock,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Weather => "weather",
            Self::Calculator => "calculator",
            Self::Clock => "clock",
        }
    }
    /// The widget's state keys, in order (common keys first).
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Weather => &["skin", "accent", "font", "location", "units"],
            Self::Calculator => &["skin", "accent", "font"],
            Self::Clock => &["skin", "accent", "font", "zone", "hour12"],
        }
    }
    /// The state a freshly placed widget has.
    pub fn default_state(self) -> Value {
        let mut state = Map::new();
        state.insert("skin".into(), "default".into());
        state.insert("accent".into(), "default".into());
        state.insert("font".into(), "default".into());
        match self {
            Self::Weather => {
                state.insert("location".into(), "Austin".into());
                state.insert("units".into(), "imperial".into());
            }
            Self::Calculator => {}
            Self::Clock => {
                state.insert("zone".into(), "local".into());
                state.insert("hour12".into(), true.into());
            }
        }
        Value::Object(state)
    }
}

/// The canonical member of `set` that `value` names (ASCII case-insensitive,
/// surrounding space ignored), or `None`.
fn closed(set: &'static [&'static str], value: &Value) -> Option<&'static str> {
    let text = value.as_str()?.trim();
    set.iter()
        .copied()
        .find(|item| item.eq_ignore_ascii_case(text))
}

/// The canonical zone id `value` names, by id or display name.
pub fn zone(value: &str) -> Option<&'static str> {
    let text = value.trim();
    ZONES
        .iter()
        .find(|(id, name)| id.eq_ignore_ascii_case(text) || name.eq_ignore_ascii_case(text))
        .map(|(id, _)| *id)
}

/// A place name: 2 to 80 characters of letters, digits, spaces and `,.'-`,
/// trimmed, with single spaces. Anything else (markup, braces, quotes other
/// than the apostrophe, control characters) is refused.
pub fn location(value: &str) -> Option<String> {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = collapsed.chars().count();
    if !(2..=80).contains(&count) {
        return None;
    }
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | ',' | '.' | '\'' | '-');
    if !collapsed.chars().all(allowed) || !collapsed.chars().any(char::is_alphabetic) {
        return None;
    }
    Some(collapsed)
}

/// Judges `value` as `widget`'s state: exactly the widget's keys, every value
/// in its closed set. Returns the canonical state, or `None` (refused).
pub fn validate(widget: Widget, value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let keys = widget.keys();
    if object.len() != keys.len() || !keys.iter().all(|key| object.contains_key(*key)) {
        return None;
    }
    let mut state = Map::new();
    state.insert("skin".into(), closed(SKINS, &object["skin"])?.into());
    state.insert("accent".into(), closed(ACCENTS, &object["accent"])?.into());
    state.insert("font".into(), closed(FONTS, &object["font"])?.into());
    match widget {
        Widget::Weather => {
            state.insert(
                "location".into(),
                location(object["location"].as_str()?)?.into(),
            );
            state.insert("units".into(), closed(UNITS, &object["units"])?.into());
        }
        Widget::Calculator => {}
        Widget::Clock => {
            state.insert("zone".into(), zone(object["zone"].as_str()?)?.into());
            state.insert("hour12".into(), object["hour12"].as_bool()?.into());
        }
    }
    Some(Value::Object(state))
}
