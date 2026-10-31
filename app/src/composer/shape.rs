//! APP-122 Shape: changing how a card, an app or a widget looks or what it
//! shows (docs/pitch/CANVAS-RULES.md, "Ask and Shape").
//!
//! Shape never answers in words and never touches records. Each target has
//! its own closed spec, judged here, which the canvas previews and applies
//! only when the member clicks (session-only in this ticket):
//!
//! - a **card** ([`CardView`]): its view over the card's own data: chart type
//!   (`presentation`), colors (`palette`, `highlight`), `columns`, `filters`,
//!   computed fields (`group_by` with a date `bucket`, `measure`), `sort`,
//!   `limit` and `title`. Fields are the card's own (the canvas sends their
//!   ids and types; every name in the spec must be one of them). A view may
//!   filter or hide records but can never drop receipts: `"receipts": false`
//!   is refused (`drops_receipts`) and the canvas shows the receipts of every
//!   number it computes;
//! - an **app** ([`AppTheme`]): a theme across its cards (`mode`, `accent`,
//!   `density`); each card keeps its own view;
//! - a **widget**: the widget's closed style state, through the existing
//!   customizer (`widgets::customize`, APP-103's Restyle, now reached through
//!   `/shape`).
//!
//! The model's reply must be one JSON object of the target's shape: prose
//! (no object, or an object carrying `text`/`answer`) is refused as `prose`,
//! an unknown key as `unknown_key` (ADR-011 I5: model output is data, never
//! code, markup or SQL). In a demo organization the rehearsed sentences
//! ([`rehearsed_card`], [`rehearsed_theme`]) map to fixed specs without a
//! model call (APP-098); anything else goes to the model.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The words: 1 to 300 characters, no control characters.
pub const MAX_WORDS: usize = 300;
/// The fixed palette of card colors, highlights and app accents (the
/// canvas stylesheet holds the colors; nothing here is CSS).
pub const PALETTE: &[&str] = &[
    "default", "green", "crimson", "amber", "cyan", "violet", "rose", "ice", "slate",
];
pub const PRESENTATIONS: &[&str] = &["table", "list", "bar", "line", "number"];
pub const OPS: &[&str] = &["eq", "ne", "lt", "lte", "gt", "gte", "in"];
pub const MEASURES: &[&str] = &["count", "sum", "avg", "min", "max"];
pub const BUCKETS: &[&str] = &["day", "week", "month", "year"];
pub const MODES: &[&str] = &["default", "light", "dark"];
pub const DENSITIES: &[&str] = &["default", "compact", "comfortable", "spacious"];
/// Field types a card may declare.
pub const FIELD_TYPES: &[&str] = &[
    "text",
    "enum",
    "money",
    "integer",
    "decimal",
    "date",
    "timestamp",
    "boolean",
    "reference",
    "unknown",
];
const MAX_FIELDS: usize = 40;
const MAX_VALUES: usize = 32;
const MAX_CONDITIONS: usize = 4;
const MAX_COLUMNS: usize = 12;
const MAX_LIMIT: u32 = 200;
const MAX_TITLE: usize = 80;
/// Keys of a prose answer: a Shape reply carrying one is refused as `prose`.
const PROSE_KEYS: &[&str] = &[
    "text",
    "answer",
    "message",
    "reply",
    "explanation",
    "response",
    "references",
];

/// The member's words, trimmed, or `None`.
pub fn words(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    ((1..=MAX_WORDS).contains(&count) && !trimmed.chars().any(char::is_control)).then_some(trimmed)
}

/// A field of the card being shaped, as the canvas declared it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub id: String,
    pub kind: String,
    /// An enum's declared values (empty: unknown).
    pub values: Vec<String>,
}

fn field_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn enum_value(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b' ' | b'.'))
}

/// The card's fields from the request: `[{id, type, values?}]`, at most 40.
pub fn fields(value: &Value) -> Option<Vec<Field>> {
    let items = value.as_array()?;
    if items.is_empty() || items.len() > MAX_FIELDS {
        return None;
    }
    let mut out: Vec<Field> = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object()?;
        if object
            .keys()
            .any(|k| !["id", "type", "values"].contains(&k.as_str()))
        {
            return None;
        }
        let id = object.get("id")?.as_str()?;
        let kind = object.get("type")?.as_str()?;
        if !field_id(id) || !FIELD_TYPES.contains(&kind) || out.iter().any(|f| f.id == id) {
            return None;
        }
        let values = match object.get("values") {
            None => Vec::new(),
            Some(list) => {
                let list = list.as_array()?;
                if list.len() > MAX_VALUES {
                    return None;
                }
                list.iter()
                    .map(|v| v.as_str().filter(|v| enum_value(v)).map(str::to_owned))
                    .collect::<Option<Vec<_>>>()?
            }
        };
        out.push(Field {
            id: id.to_owned(),
            kind: kind.to_owned(),
            values,
        });
    }
    Some(out)
}

/// One condition: `{field, op, value}`; `value` is a string, an integer or,
/// for `in`, a list of strings.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub field: String,
    pub op: String,
    pub value: Value,
}

/// A highlight: rows matching `when` take `color`.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Highlight {
    pub field: String,
    pub op: String,
    pub value: Value,
    pub color: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Measure {
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sort {
    pub field: String,
    pub direction: String,
}

/// A card's view spec (module notes). Every member is optional; at least one
/// must be set.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CardView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub highlight: Vec<Highlight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measure: Option<Measure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Only `true` is accepted (and dropped: the canvas always shows receipts).
    #[serde(default, skip_serializing)]
    pub receipts: Option<bool>,
}

impl CardView {
    fn is_empty(&self) -> bool {
        self.presentation.is_none()
            && self.palette.is_none()
            && self.columns.is_empty()
            && self.filters.is_empty()
            && self.highlight.is_empty()
            && self.group_by.is_none()
            && self.measure.is_none()
            && self.sort.is_none()
            && self.limit.is_none()
            && self.title.is_none()
    }
}

/// An app's theme (module notes).
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AppTheme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<String>,
}

/// The first JSON object of a reply, decoded strictly; prose is refused.
fn object_of(reply: &str) -> Result<Map<String, Value>, &'static str> {
    let start = reply.find('{').ok_or("prose")?;
    let end = reply.rfind('}').ok_or("prose")?;
    if end < start {
        return Err("prose");
    }
    let value: Value =
        crate::admission::decode(&reply.as_bytes()[start..=end]).map_err(|_| "prose")?;
    match value {
        Value::Object(object) => Ok(object),
        _ => Err("prose"),
    }
}

fn keys_of(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), &'static str> {
    if object.keys().any(|key| PROSE_KEYS.contains(&key.as_str())) {
        return Err("prose");
    }
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("unknown_key");
    }
    Ok(())
}

const CARD_KEYS: &[&str] = &[
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
];

/// Judges a reply as a card view over `fields` (module notes).
pub fn judge_card(reply: &str, fields: &[Field]) -> Result<CardView, &'static str> {
    judge_card_object(object_of(reply)?, fields)
}

/// [`judge_card`] on a decoded object.
pub fn judge_card_object(
    object: Map<String, Value>,
    fields: &[Field],
) -> Result<CardView, &'static str> {
    if object
        .get("receipts")
        .is_some_and(|value| value != &Value::Bool(true))
    {
        return Err("drops_receipts");
    }
    keys_of(&object, CARD_KEYS)?;
    let view: CardView =
        serde_json::from_value(Value::Object(object)).map_err(|_| "invalid_view")?;
    check_card(&view, fields)?;
    Ok(view)
}

fn closed(set: &[&str], value: &str) -> bool {
    set.contains(&value)
}

fn find<'a>(fields: &'a [Field], id: &str) -> Result<&'a Field, &'static str> {
    fields.iter().find(|f| f.id == id).ok_or("unknown_field")
}

fn numeric(kind: &str) -> bool {
    matches!(kind, "money" | "integer" | "decimal")
}

fn decimal_text(value: &str) -> bool {
    let body = value.strip_prefix('-').unwrap_or(value);
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    (1..=15).contains(&whole.len())
        && whole.bytes().all(|b| b.is_ascii_digit())
        && fraction.len() <= 6
        && fraction.bytes().all(|b| b.is_ascii_digit())
}

fn date_text(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

fn condition(field: &Field, op: &str, value: &Value) -> Result<(), &'static str> {
    if !closed(OPS, op) {
        return Err("invalid_operator");
    }
    let ordered = matches!(op, "lt" | "lte" | "gt" | "gte");
    let one = |text: &str| -> Result<(), &'static str> {
        let ok = match field.kind.as_str() {
            "enum" => {
                enum_value(text)
                    && (field.values.is_empty() || field.values.iter().any(|v| v == text))
            }
            "money" | "integer" | "decimal" => decimal_text(text),
            "date" => date_text(text),
            "timestamp" => date_text(text.get(..10).unwrap_or("")) && text.len() <= 25,
            "boolean" => text == "true" || text == "false",
            _ => {
                (1..=80).contains(&text.chars().count())
                    && !text
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '<' | '>' | '{' | '}' | ';'))
            }
        };
        ok.then_some(()).ok_or("invalid_value")
    };
    if ordered && !(numeric(&field.kind) || matches!(field.kind.as_str(), "date" | "timestamp")) {
        return Err("invalid_operator");
    }
    match (op, value) {
        ("in", Value::Array(items)) if (1..=12).contains(&items.len()) => {
            for item in items {
                one(item.as_str().ok_or("invalid_value")?)?;
            }
            Ok(())
        }
        ("in", _) => Err("invalid_value"),
        (_, Value::String(text)) => one(text),
        (_, Value::Number(number)) if number.is_i64() => one(&number.to_string()),
        (_, Value::Bool(flag)) if field.kind == "boolean" => one(&flag.to_string()),
        _ => Err("invalid_value"),
    }
}

fn check_card(view: &CardView, fields: &[Field]) -> Result<(), &'static str> {
    if view.is_empty() {
        return Err("empty_view");
    }
    if view
        .presentation
        .as_deref()
        .is_some_and(|p| !closed(PRESENTATIONS, p))
    {
        return Err("invalid_presentation");
    }
    if view.palette.as_deref().is_some_and(|p| !closed(PALETTE, p)) {
        return Err("invalid_color");
    }
    if view.columns.len() > MAX_COLUMNS {
        return Err("too_many");
    }
    for column in &view.columns {
        find(fields, column)?;
    }
    if view.filters.len() > MAX_CONDITIONS || view.highlight.len() > MAX_CONDITIONS {
        return Err("too_many");
    }
    for filter in &view.filters {
        condition(find(fields, &filter.field)?, &filter.op, &filter.value)?;
    }
    for highlight in &view.highlight {
        condition(
            find(fields, &highlight.field)?,
            &highlight.op,
            &highlight.value,
        )?;
        if !closed(PALETTE, &highlight.color) || highlight.color == "default" {
            return Err("invalid_color");
        }
    }
    let group = view
        .group_by
        .as_deref()
        .map(|id| find(fields, id))
        .transpose()?;
    if let Some(bucket) = view.bucket.as_deref() {
        let dated = group.is_some_and(|f| matches!(f.kind.as_str(), "date" | "timestamp"));
        if !closed(BUCKETS, bucket) || !dated {
            return Err("invalid_bucket");
        }
    }
    if let Some(measure) = &view.measure {
        if !closed(MEASURES, &measure.op) {
            return Err("invalid_measure");
        }
        match (measure.op.as_str(), measure.field.as_deref()) {
            ("count", None) => {}
            (_, Some(id)) => {
                let field = find(fields, id)?;
                if measure.op != "count" && !numeric(&field.kind) {
                    return Err("invalid_measure");
                }
            }
            _ => return Err("invalid_measure"),
        }
    }
    if let Some(sort) = &view.sort {
        if sort.field != "value" {
            find(fields, &sort.field)?;
        }
        if sort.direction != "asc" && sort.direction != "desc" {
            return Err("invalid_sort");
        }
    }
    if view
        .limit
        .is_some_and(|limit| !(1..=MAX_LIMIT).contains(&limit))
    {
        return Err("invalid_limit");
    }
    if let Some(title) = view.title.as_deref() {
        let count = title.chars().count();
        if !(1..=MAX_TITLE).contains(&count)
            || title
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>' | '{' | '}'))
        {
            return Err("invalid_title");
        }
    }
    Ok(())
}

/// Judges a reply as an app theme (module notes).
pub fn judge_theme(reply: &str) -> Result<AppTheme, &'static str> {
    judge_theme_object(object_of(reply)?)
}

/// [`judge_theme`] on a decoded object.
pub fn judge_theme_object(object: Map<String, Value>) -> Result<AppTheme, &'static str> {
    keys_of(&object, &["mode", "accent", "density"])?;
    let theme: AppTheme =
        serde_json::from_value(Value::Object(object)).map_err(|_| "invalid_theme")?;
    if theme.mode.is_none() && theme.accent.is_none() && theme.density.is_none() {
        return Err("empty_view");
    }
    let ok = theme.mode.as_deref().is_none_or(|m| closed(MODES, m))
        && theme.accent.as_deref().is_none_or(|a| closed(PALETTE, a))
        && theme
            .density
            .as_deref()
            .is_none_or(|d| closed(DENSITIES, d));
    ok.then_some(theme).ok_or("invalid_theme")
}

// --- The model ---

/// The card system prompt: the closed view shape over the card's fields.
pub fn card_system(fields: &[Field]) -> String {
    let fields: Vec<Value> = fields
        .iter()
        .map(|f| {
            if f.values.is_empty() {
                serde_json::json!({"id": f.id, "type": f.kind})
            } else {
                serde_json::json!({"id": f.id, "type": f.kind, "values": f.values})
            }
        })
        .collect();
    format!(
        "You change how one card on a dashboard looks or what it shows. You never answer in words and never change records.\n\
You receive JSON: the card's address and the member's request. The card's fields: {fields}.\n\
Answer with ONLY one JSON object, the view, using only these optional keys:\n\
- \"presentation\": one of [{presentations}]\n\
- \"palette\": one of [{palette}] (the card's color)\n\
- \"columns\": [field ids] to show, in order (at most 12)\n\
- \"filters\": at most 4 of {{\"field\", \"op\", \"value\"}}: op one of [{ops}]; eq/ne/in only on text, enum and boolean; a value is a string (money as a decimal string like \"200\", dates as YYYY-MM-DD, enum values exactly as declared), an integer, or for \"in\" a list of strings\n\
- \"highlight\": at most 4 of {{\"field\", \"op\", \"value\", \"color\"}}: rows matching take the color (one of the palette, not default)\n\
- \"group_by\": a field id; \"bucket\": one of [{buckets}] when grouping by a date\n\
- \"measure\": {{\"op\": one of [{measures}], \"field\": a numeric field id (optional for count)}}\n\
- \"sort\": {{\"field\": a field id or \"value\", \"direction\": \"asc\" or \"desc\"}}; \"limit\": 1 to 200; \"title\": a short plain title\n\
\"Red\" records are the ones needing attention: waiting on a decision (submitted, pending, requested, open, overdue, unpaid) or refused, disputed or flagged. \"Overdue\" means waiting on a decision.\n\
A view may filter or hide records but never removes receipts. Never write prose, CSS, HTML, SQL, code or any other key. If the request cannot be expressed, return {{}}.",
        fields = Value::Array(fields),
        presentations = PRESENTATIONS.join(", "),
        palette = PALETTE.join(", "),
        ops = OPS.join(", "),
        buckets = BUCKETS.join(", "),
        measures = MEASURES.join(", "),
    )
}

/// The app theme system prompt.
pub fn theme_system() -> String {
    format!(
        "You theme every card of one app on a dashboard. You never answer in words and never change records or what a card shows.\n\
Answer with ONLY one JSON object using only these optional keys: \"mode\": one of [{modes}], \"accent\": one of [{palette}], \"density\": one of [{densities}].\n\
Never write prose, CSS, HTML, color codes or any other key. If the request cannot be expressed, return {{}}.",
        modes = MODES.join(", "),
        palette = PALETTE.join(", "),
        densities = DENSITIES.join(", "),
    )
}

/// The user message: the address and the request, as JSON data.
pub fn user(address: &str, words: &str) -> String {
    serde_json::json!({"address": address, "request": words}).to_string()
}

// --- Rehearsed sentences (demo organizations only; the caller checks) ---

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '$' | '.' | ',' | '_') {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A color word as a palette member.
pub fn color(word: &str) -> Option<&'static str> {
    Some(match word {
        "green" | "matrix" | "emerald" => "green",
        "red" | "crimson" | "scarlet" => "crimson",
        "amber" | "orange" | "yellow" | "gold" => "amber",
        "cyan" | "teal" | "aqua" => "cyan",
        "violet" | "purple" => "violet",
        "rose" | "pink" => "rose",
        "ice" | "blue" => "ice",
        "slate" | "gray" | "grey" => "slate",
        "default" => "default",
        _ => return None,
    })
}

const WAITING: &[&str] = &[
    "submitted",
    "pending",
    "requested",
    "waiting",
    "open",
    "overdue",
    "unpaid",
];

fn status_field(fields: &[Field]) -> Option<&Field> {
    fields
        .iter()
        .find(|f| f.kind == "enum" && (f.id == "status" || f.id == "state"))
        .or_else(|| fields.iter().find(|f| f.kind == "enum"))
}

fn first_of<'a>(fields: &'a [Field], kinds: &[&str]) -> Option<&'a Field> {
    fields.iter().find(|f| kinds.contains(&f.kind.as_str()))
}

fn among(field: &Field, set: &[&str]) -> Vec<String> {
    if field.values.is_empty() {
        set.iter().map(|v| (*v).to_owned()).collect()
    } else {
        field
            .values
            .iter()
            .filter(|v| set.contains(&v.as_str()))
            .cloned()
            .collect()
    }
}

fn amount(word: &str) -> Option<String> {
    let cleaned = word.trim_start_matches('$').replace(',', "");
    decimal_text(&cleaned).then_some(cleaned)
}

/// The records a phrase names ("red invoices", "overdue", "approved",
/// "over $200"), as a condition over `fields`.
pub fn subject(phrase: &str, fields: &[Field]) -> Option<Condition> {
    let words: Vec<&str> = phrase.split(' ').collect();
    let has = |w: &str| words.contains(&w);
    if let Some(status) = status_field(fields) {
        let attention = ["red", "attention", "problem", "problems", "flagged"]
            .iter()
            .any(|w| has(w));
        let waiting = ["overdue", "late", "waiting", "pending", "open", "unpaid"]
            .iter()
            .any(|w| has(w));
        if attention || waiting {
            let set = if attention {
                crate::composer::ask::ATTENTION
            } else {
                WAITING
            };
            let values = among(status, set);
            if values.is_empty() {
                return None;
            }
            return Some(Condition {
                field: status.id.clone(),
                op: "in".into(),
                value: Value::Array(values.into_iter().map(Value::String).collect()),
            });
        }
    }
    for field in fields.iter().filter(|f| f.kind == "enum") {
        if let Some(value) = field
            .values
            .iter()
            .find(|v| has(v) || has(&v.replace('_', " ")) || phrase.contains(&v.replace('_', " ")))
        {
            return Some(Condition {
                field: field.id.clone(),
                op: "eq".into(),
                value: Value::String(value.clone()),
            });
        }
    }
    let money = first_of(fields, &["money", "decimal", "integer"])?;
    for (at, word) in words.iter().enumerate() {
        let op = match *word {
            "over" | "above" | "more" | "greater" | "exceeding" => "gt",
            "under" | "below" | "less" | "fewer" => "lt",
            _ => continue,
        };
        let value = words[at + 1..]
            .iter()
            .filter(|w| **w != "than")
            .find_map(|w| amount(w))?;
        return Some(Condition {
            field: money.id.clone(),
            op: op.into(),
            value: Value::String(value),
        });
    }
    None
}

/// The field a word names ("category", "status", "merchant", "month" …).
fn field_named<'a>(word: &str, fields: &'a [Field]) -> Option<&'a Field> {
    let singular = word.trim_end_matches('s');
    fields
        .iter()
        .find(|f| f.id == word || f.id == singular || f.id.replace('_', " ") == word)
        .or_else(|| {
            fields
                .iter()
                .find(|f| f.id.split('_').any(|part| part == word || part == singular))
        })
        .or_else(|| match word {
            "person" | "people" | "who" | "employee" => fields
                .iter()
                .find(|f| f.id == "creator" || f.id == "employee"),
            "vendor" | "merchant" => fields
                .iter()
                .find(|f| f.id == "merchant" || f.id == "vendor"),
            _ => None,
        })
}

/// DS: the rehearsed card sentences (module notes): clauses separated by
/// commas or "and". `None`: nothing rehearsed matched (then the model).
pub fn rehearsed_card(words: &str, fields: &[Field]) -> Option<CardView> {
    let text = normalize(words);
    let mut view = CardView::default();
    let money = first_of(fields, &["money"]).or_else(|| first_of(fields, &["decimal", "integer"]));
    for clause in text
        .split(',')
        .flat_map(|part| part.split(" and "))
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        let tokens: Vec<&str> = clause.split(' ').collect();
        let last_color = tokens.last().and_then(|w| color(w));
        // "highlight overdue in red", "overdue in red", "highlight rejected".
        if tokens.first() == Some(&"highlight")
            || (last_color.is_some() && tokens.contains(&"in") && tokens.len() > 2)
        {
            let body = clause.trim_start_matches("highlight").trim();
            let (phrase, chosen) = match body.rsplit_once(" in ") {
                Some((phrase, tail)) if color(tail).is_some() => (phrase, color(tail)),
                _ => (body, None),
            };
            let condition = subject(phrase, fields)?;
            view.highlight.push(Highlight {
                field: condition.field,
                op: condition.op,
                value: condition.value,
                color: chosen.unwrap_or("amber").to_owned(),
            });
            continue;
        }
        // "only red invoices", "just the approved ones", "show only over $200".
        if let Some(rest) = [
            "show only ",
            "only show ",
            "only ",
            "just ",
            "filter to ",
            "filter ",
        ]
        .iter()
        .find_map(|prefix| clause.strip_prefix(prefix))
        {
            view.filters.push(subject(rest, fields)?);
            continue;
        }
        // "total by month", "count by status", "by category".
        if let Some(at) = tokens.iter().position(|w| *w == "by" || *w == "per") {
            let target = tokens.get(at + 1)?;
            let (group, bucket) = if BUCKETS.contains(target)
                || target.trim_end_matches('s') != *target
                    && BUCKETS.contains(&target.trim_end_matches('s'))
            {
                let date = first_of(fields, &["date", "timestamp"])?;
                (date, Some(target.trim_end_matches('s').to_owned()))
            } else {
                (field_named(target, fields)?, None)
            };
            let before = &tokens[..at];
            let op = if before
                .iter()
                .any(|w| ["count", "number", "many"].contains(w))
            {
                "count"
            } else if before
                .iter()
                .any(|w| ["average", "avg", "mean"].contains(w))
            {
                "avg"
            } else if money.is_some() {
                "sum"
            } else {
                "count"
            };
            view.group_by = Some(group.id.clone());
            view.bucket = bucket;
            view.measure = Some(Measure {
                op: op.into(),
                field: if op == "count" {
                    None
                } else {
                    money.map(|f| f.id.clone())
                },
            });
            if view.presentation.is_none() {
                view.presentation = Some(
                    if tokens.contains(&"line") || tokens.contains(&"trend") {
                        "line"
                    } else {
                        "bar"
                    }
                    .into(),
                );
            }
            continue;
        }
        // "as a bar chart", "line chart", "as a table", "just the number".
        if let Some(kind) = tokens.iter().find_map(|w| match *w {
            "bar" | "bars" | "chart" => Some("bar"),
            "line" | "trend" => Some("line"),
            "table" => Some("table"),
            "list" => Some("list"),
            "number" | "kpi" => Some("number"),
            _ => None,
        }) {
            view.presentation = Some(kind.into());
            if kind == "number" && view.measure.is_none() {
                view.measure = Some(Measure {
                    op: if money.is_some() { "sum" } else { "count" }.into(),
                    field: money.map(|f| f.id.clone()),
                });
            }
            continue;
        }
        // "sort by amount", "newest first", "largest first".
        if let Some(rest) = clause.strip_prefix("sort by ") {
            let mut parts = rest.split(' ');
            let field = field_named(parts.next()?, fields)?;
            let direction =
                if rest.contains("asc") || rest.contains("smallest") || rest.contains("oldest") {
                    "asc"
                } else {
                    "desc"
                };
            view.sort = Some(Sort {
                field: field.id.clone(),
                direction: direction.into(),
            });
            continue;
        }
        if let Some(n) = ["top ", "first "]
            .iter()
            .find_map(|p| clause.strip_prefix(p))
            .and_then(|rest| rest.split(' ').next())
            .and_then(|n| n.parse::<u32>().ok())
        {
            view.limit = Some(n);
            continue;
        }
        if let Some(title) = ["call it ", "title it ", "title "]
            .iter()
            .find_map(|p| clause.strip_prefix(p))
        {
            view.title = Some(title.chars().take(MAX_TITLE).collect());
            continue;
        }
        // "make it green", "green".
        if let Some(chosen) = tokens.iter().rev().find_map(|w| color(w)) {
            view.palette = Some(chosen.into());
            continue;
        }
        if ["reset", "default", "original"]
            .iter()
            .any(|w| tokens.contains(w))
        {
            view.palette = Some("default".into());
            continue;
        }
        return None;
    }
    check_card(&view, fields).ok()?;
    Some(view)
}

/// The rehearsed app themes: a mode, an accent and a density in any order
/// ("dark green, compact"; "light"; "reset").
pub fn rehearsed_theme(words: &str) -> Option<AppTheme> {
    let text = normalize(words).replace(',', " ");
    let mut theme = AppTheme::default();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        match word {
            "dark" | "night" | "dim" => theme.mode = Some("dark".into()),
            "light" | "bright" | "day" => theme.mode = Some("light".into()),
            "compact" | "dense" | "tight" | "small" => theme.density = Some("compact".into()),
            "spacious" | "roomy" | "airy" | "large" => theme.density = Some("spacious".into()),
            "comfortable" | "normal" => theme.density = Some("comfortable".into()),
            "reset" | "default" | "original" => {
                theme = AppTheme {
                    mode: Some("default".into()),
                    accent: Some("default".into()),
                    density: Some("default".into()),
                };
            }
            "make" | "it" | "the" | "a" | "an" | "theme" | "with" | "in" | "and" | "mode"
            | "colors" | "colours" | "accent" | "look" | "style" => {}
            other => theme.accent = Some(color(other)?.into()),
        }
    }
    let judged = judge_theme_object(match serde_json::to_value(&theme).ok()? {
        Value::Object(object) => object,
        _ => return None,
    });
    judged.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ledger() -> Vec<Field> {
        fields(&json!([
            {"id": "amount", "type": "money"},
            {"id": "merchant", "type": "text"},
            {"id": "category", "type": "enum", "values": ["meals", "travel", "software", "office", "other"]},
            {"id": "spent_on", "type": "date"},
            {"id": "status", "type": "enum", "values": ["submitted", "approved", "rejected"]},
        ]))
        .unwrap()
    }

    #[test]
    fn shape_refuses_prose_and_unknown_keys() {
        let fields = ledger();
        for reply in [
            "Sure! The red invoices are Nobu and Figma.",
            r#"{"text": "The red ones are Nobu and Figma", "references": ["r1"]}"#,
            r#"{"answer": "two"}"#,
            "[1, 2]",
        ] {
            assert_eq!(judge_card(reply, &fields), Err("prose"), "{reply}");
        }
        assert_eq!(
            judge_card(r#"{"presentation": "bar", "css": "body{}"}"#, &fields),
            Err("unknown_key")
        );
        assert_eq!(judge_theme("Dark green looks great."), Err("prose"));
        assert_eq!(
            judge_theme(r#"{"accent": "green", "text": "done"}"#),
            Err("prose")
        );
        assert_eq!(
            judge_theme(r##"{"accent": "#00ff00"}"##),
            Err("invalid_theme")
        );
    }

    #[test]
    fn a_view_that_would_drop_receipts_is_refused() {
        let fields = ledger();
        assert_eq!(
            judge_card(r#"{"presentation": "number", "receipts": false}"#, &fields),
            Err("drops_receipts")
        );
        assert_eq!(
            judge_card(r#"{"receipts": "hidden"}"#, &fields),
            Err("drops_receipts")
        );
        let kept = judge_card(r#"{"presentation": "number", "receipts": true}"#, &fields).unwrap();
        assert_eq!(
            serde_json::to_value(&kept).unwrap(),
            json!({"presentation": "number"})
        );
    }

    #[test]
    fn a_view_names_only_the_card_s_fields_and_values() {
        let fields = ledger();
        let cases = [
            (r#"{"columns": ["amount", "password"]}"#, "unknown_field"),
            (
                r#"{"filters": [{"field": "status", "op": "eq", "value": "lost"}]}"#,
                "invalid_value",
            ),
            (
                r#"{"filters": [{"field": "status", "op": "gt", "value": "approved"}]}"#,
                "invalid_operator",
            ),
            (
                r#"{"filters": [{"field": "amount", "op": "gt", "value": "1e9"}]}"#,
                "invalid_value",
            ),
            (
                r#"{"highlight": [{"field": "status", "op": "eq", "value": "rejected", "color": "default"}]}"#,
                "invalid_color",
            ),
            (
                r#"{"group_by": "merchant", "bucket": "month"}"#,
                "invalid_bucket",
            ),
            (
                r#"{"measure": {"op": "sum", "field": "merchant"}}"#,
                "invalid_measure",
            ),
            (r#"{"limit": 0}"#, "invalid_limit"),
            (r#"{"title": "<b>x</b>"}"#, "invalid_title"),
            (r#"{"presentation": "pie"}"#, "invalid_presentation"),
            ("{}", "empty_view"),
        ];
        for (reply, code) in cases {
            assert_eq!(judge_card(reply, &fields), Err(code), "{reply}");
        }
        let view = judge_card(
            r#"{"group_by": "spent_on", "bucket": "month", "measure": {"op": "sum", "field": "amount"}, "presentation": "bar"}"#,
            &fields,
        )
        .unwrap();
        assert_eq!(view.bucket.as_deref(), Some("month"));
    }

    #[test]
    fn rehearsed_card_sentences() {
        let fields = ledger();
        let highlight = rehearsed_card("highlight overdue in red", &fields).unwrap();
        assert_eq!(
            serde_json::to_value(&highlight).unwrap(),
            json!({"highlight": [{"field": "status", "op": "in", "value": ["submitted"], "color": "crimson"}]})
        );
        let red = rehearsed_card("only red invoices", &fields).unwrap();
        assert_eq!(
            serde_json::to_value(&red).unwrap(),
            json!({"filters": [{"field": "status", "op": "in", "value": ["submitted", "rejected"]}]})
        );
        let monthly = rehearsed_card("total by month", &fields).unwrap();
        assert_eq!(
            serde_json::to_value(&monthly).unwrap(),
            json!({"presentation": "bar", "group_by": "spent_on", "bucket": "month", "measure": {"op": "sum", "field": "amount"}})
        );
        let combined =
            rehearsed_card("only approved, as a line chart and make it green", &fields).unwrap();
        assert_eq!(combined.filters[0].value, json!("approved"));
        assert_eq!(combined.presentation.as_deref(), Some("line"));
        assert_eq!(combined.palette.as_deref(), Some("green"));
        let over = rehearsed_card("only over $200", &fields).unwrap();
        assert_eq!(
            over.filters[0],
            Condition {
                field: "amount".into(),
                op: "gt".into(),
                value: json!("200")
            }
        );
        assert_eq!(
            rehearsed_card("tell me a story about the ledger", &fields),
            None
        );
        // A rehearsed spec is judged like a model's.
        for view in [highlight, red, monthly] {
            let object = match serde_json::to_value(&view).unwrap() {
                Value::Object(o) => o,
                _ => unreachable!(),
            };
            assert_eq!(judge_card_object(object, &fields).unwrap(), view);
        }
    }

    #[test]
    fn rehearsed_themes() {
        assert_eq!(
            rehearsed_theme("dark green, compact").unwrap(),
            AppTheme {
                mode: Some("dark".into()),
                accent: Some("green".into()),
                density: Some("compact".into())
            }
        );
        assert_eq!(
            rehearsed_theme("reset").unwrap().accent.as_deref(),
            Some("default")
        );
        assert_eq!(rehearsed_theme("make it look like a spaceship"), None);
    }

    #[test]
    fn fields_are_closed() {
        assert!(fields(&json!([{"id": "Amount", "type": "money"}])).is_none());
        assert!(fields(&json!([{"id": "amount", "type": "css"}])).is_none());
        assert!(fields(&json!([{"id": "amount", "type": "money", "style": 1}])).is_none());
        assert!(fields(&json!([{"id": "s", "type": "enum", "values": ["<b>"]}])).is_none());
        assert!(fields(&json!([])).is_none());
    }
}
