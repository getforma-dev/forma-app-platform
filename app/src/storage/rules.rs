//! APP-107: declared workflow rules (contract amendment, additive and
//! optional; `contracts/schema.py` `guard`, `command_declaration.initial`).
//!
//! A transition may carry a guard (`when`): `field comparison literal` over a
//! field of the same record schema, judged on the record as the command
//! composes it. A filing `create` may declare the `initial` value of an enum
//! field: the first entry whose guard holds (an entry without a guard always
//! holds). Storage judges both in `commands::compose`, so the dry run behind
//! a confirmation card and the audited commit apply the same rule; the
//! action path, Slack, MCP and the canvas only read them to offer what
//! storage would accept. A model never chooses a status: an initial value is
//! computed from the typed record, and a transition whose guard fails is
//! refused like an undeclared one.
//!
//! Comparison: money by minor units, integer by value, decimal by coefficient
//! (admission pins the unit and scale to the field's), date and timestamp by
//! their fixed-width text, text/enum/boolean by equality. A null or absent
//! value satisfies no guard.
//!
//! ADR-020 C3 (edition impl-2026-11-14.2): a command's `requires` entries are
//! the same guards plus an optional `via` (a reference field of this record:
//! the guard is judged on the referenced record) and a literal that may be
//! the server-time literal `{"type": "now", "offset_seconds"?}`. `now` is the
//! commit transaction's instant (`commands::commit`), never a value a caller
//! supplies; [`Guard::holds_at`] substitutes it. A transition's or an initial
//! rule's guard admits neither (`$defs.guard` is unchanged, and the parse
//! here refuses them too, so stored content can never give them a meaning).
use serde_json::{Map, Value};
use std::cmp::Ordering;

/// One declared guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    pub field: String,
    pub comparison: String,
    /// The typed literal (contract `value`), or a `requires` entry's `now`.
    pub value: Value,
    /// ADR-020 C3: a `requires` entry's reference field of this record; the
    /// guard is judged on the record it names (`commands::judge_requires`).
    pub via: Option<String>,
}

/// ADR-020 C3: the bound of a `now` literal's `offset_seconds` (one week).
pub const MAX_NOW_OFFSET_SECONDS: i64 = 604_800;

/// The rules a command declares. `Default` declares none: the plain
/// transition list governs alone.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rules {
    /// Every declared transition `(field, from, to)` with its guard.
    pub transitions: Vec<(String, String, String, Option<Guard>)>,
    /// `initial`: `(field, value, guard)` in declaration order.
    pub initial: Vec<(String, String, Option<Guard>)>,
}

#[derive(PartialEq, PartialOrd)]
enum Key {
    Number(i128),
    Text(String),
    Flag(bool),
}

fn key(value: &Value) -> Option<Key> {
    let text = |name: &str| value.get(name).and_then(Value::as_str);
    match value.get("type").and_then(Value::as_str)? {
        "money" => text("minor_units")?.parse().ok().map(Key::Number),
        "integer" => text("value")?.parse().ok().map(Key::Number),
        "decimal" => text("coefficient")?.parse().ok().map(Key::Number),
        "boolean" => value.get("value").and_then(Value::as_bool).map(Key::Flag),
        "date" | "timestamp" | "text" | "enum" => text("value").map(|t| Key::Text(t.to_owned())),
        _ => None,
    }
}

/// The `offset_seconds` of a `now` literal (0 when absent), or `None` when
/// `value` is not exactly a `now` literal of the contract (`$defs.now`: the
/// member is −604800..−1 or 1..604800; absence is the only spelling of 0).
pub fn now_offset(value: &Value) -> Option<i64> {
    let object = value.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some("now") {
        return None;
    }
    match object.get("offset_seconds") {
        None if object.len() == 1 => Some(0),
        Some(offset) if object.len() == 2 => offset
            .as_i64()
            .filter(|s| *s != 0 && s.abs() <= MAX_NOW_OFFSET_SECONDS),
        _ => None,
    }
}

/// Seconds since the Unix epoch of a contract timestamp
/// (`YYYY-MM-DDTHH:MM:SSZ`, proleptic Gregorian, UTC).
pub fn unix_seconds(timestamp: &str) -> Option<i64> {
    if !crate::admission::contract_timestamp(timestamp) {
        return None;
    }
    let part = |from: usize, to: usize| timestamp.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (part(0, 4)?, part(5, 7)?, part(8, 10)?);
    let (hour, minute, second) = (part(11, 13)?, part(14, 16)?, part(17, 19)?);
    // Days from the civil date (H. Hinnant's algorithm).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// The contract timestamp of a Unix second (the inverse of [`unix_seconds`]).
pub fn timestamp_of(seconds: i64) -> Option<String> {
    let (days, rem) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let text = format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    );
    crate::admission::contract_timestamp(&text).then_some(text)
}

impl Guard {
    /// A transition's or an initial rule's guard (`$defs.guard`): no `via`,
    /// and never the `now` literal.
    fn parse(value: &Value) -> Option<Self> {
        let guard = Self::parse_requirement(value)?;
        (guard.via.is_none() && guard.value.get("type").and_then(Value::as_str) != Some("now"))
            .then_some(guard)
    }

    /// ADR-020 C3: one `requires` entry (`$defs.requirement`): a guard with
    /// an optional `via`, whose literal may be `now`.
    pub fn parse_requirement(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let via = match object.get("via") {
            None => None,
            Some(via) => Some(via.as_str()?.to_owned()),
        };
        let literal = object.get("value")?;
        if literal.get("type").and_then(Value::as_str) == Some("now") {
            now_offset(literal)?;
        }
        Some(Self {
            field: object.get("field_id")?.as_str()?.to_owned(),
            comparison: object.get("comparison")?.as_str()?.to_owned(),
            value: literal.clone(),
            via,
        })
    }

    /// Whether the guard holds on `state` (field id → typed value). A `now`
    /// literal satisfies nothing here: only [`Guard::holds_at`] knows the
    /// instant.
    pub fn holds(&self, state: &Map<String, Value>) -> bool {
        self.judge(state, &self.value)
    }

    /// ADR-020 C3: whether the guard holds on `state` at `now` (the commit
    /// transaction's instant, a contract timestamp): a `now` literal is the
    /// timestamp `now + offset_seconds`; any other literal is itself.
    pub fn holds_at(&self, state: &Map<String, Value>, now: &str) -> bool {
        match now_offset(&self.value) {
            Some(offset) => {
                let Some(at) = unix_seconds(now).and_then(|s| timestamp_of(s + offset)) else {
                    return false;
                };
                self.judge(
                    state,
                    &serde_json::json!({"type": "timestamp", "value": at}),
                )
            }
            None => self.judge(state, &self.value),
        }
    }

    fn judge(&self, state: &Map<String, Value>, literal: &Value) -> bool {
        let (Some(have), Some(want)) = (state.get(&self.field).and_then(key), key(literal)) else {
            return false;
        };
        let Some(order) = have.partial_cmp(&want) else {
            return false;
        };
        match self.comparison.as_str() {
            "eq" => order == Ordering::Equal,
            "ne" => order != Ordering::Equal,
            "lt" => order == Ordering::Less,
            "lte" => order != Ordering::Greater,
            "gt" => order == Ordering::Greater,
            "gte" => order != Ordering::Less,
            _ => false,
        }
    }

    /// The guard as people read it ("amount > $5,000.00").
    pub fn describe(&self) -> String {
        let symbol = match self.comparison.as_str() {
            "eq" => "=",
            "ne" => "≠",
            "lt" => "<",
            "lte" => "≤",
            "gt" => ">",
            _ => "≥",
        };
        let literal = match now_offset(&self.value) {
            Some(0) => "now".to_owned(),
            Some(offset) if offset < 0 => format!("now − {} s", -offset),
            Some(offset) => format!("now + {offset} s"),
            None => crate::actions::resolve::display(&self.value),
        };
        format!("{} {symbol} {literal}", self.field.replace('_', " "))
    }
}

/// ADR-020 C3: a command's `requires` entries in declaration order (none
/// when absent); `None` when an entry is malformed (stored content is
/// admitted content, so that is unavailability).
pub fn requirements(declared: &Value) -> Option<Vec<Guard>> {
    match declared.get("requires") {
        None => Some(Vec::new()),
        Some(list) => list
            .as_array()?
            .iter()
            .map(Guard::parse_requirement)
            .collect(),
    }
}

impl Rules {
    /// The rules of a declared command (stored content or the catalog):
    /// `transitions[].when` and `initial`. `None` when a member is malformed
    /// (stored content is admitted content, so that is unavailability).
    pub fn from_declaration(declared: &Value) -> Option<Self> {
        let mut rules = Self::default();
        for item in declared.get("transitions")?.as_array()? {
            let part = |name: &str| item.get(name)?.as_str().map(str::to_owned);
            let when = match item.get("when") {
                None => None,
                Some(value) => Some(Guard::parse(value)?),
            };
            rules
                .transitions
                .push((part("field_id")?, part("from")?, part("to")?, when));
        }
        if let Some(initial) = declared.get("initial") {
            for item in initial.as_array()? {
                let part = |name: &str| item.get(name)?.as_str().map(str::to_owned);
                let when = match item.get("when") {
                    None => None,
                    Some(value) => Some(Guard::parse(value)?),
                };
                rules
                    .initial
                    .push((part("field_id")?, part("value")?, when));
            }
        }
        Some(rules)
    }

    /// Whether `field: from → to` may fire on the composed `state`: when the
    /// rules name that transition, one of its declarations must be unguarded
    /// or hold; when they do not, the plain declaration governs (true).
    pub fn allows(&self, field: &str, from: &str, to: &str, state: &Map<String, Value>) -> bool {
        let mut named = self
            .transitions
            .iter()
            .filter(|(f, a, b, _)| f == field && a == from && b == to)
            .peekable();
        if named.peek().is_none() {
            return true;
        }
        named.any(|(_, _, _, when)| when.as_ref().is_none_or(|guard| guard.holds(state)))
    }

    /// The fields this command declares an initial value of, in order.
    pub fn initial_fields(&self) -> Vec<&str> {
        let mut fields: Vec<&str> = Vec::new();
        for (field, _, _) in &self.initial {
            if !fields.contains(&field.as_str()) {
                fields.push(field);
            }
        }
        fields
    }

    /// The declared starting value of `field` for `state`, with the guard
    /// that chose it: `None` when no entry holds (storage refuses the create).
    pub fn initial_value(
        &self,
        field: &str,
        state: &Map<String, Value>,
    ) -> Option<(&str, Option<&Guard>)> {
        self.initial
            .iter()
            .filter(|(f, _, _)| f == field)
            .find(|(_, _, when)| when.as_ref().is_none_or(|guard| guard.holds(state)))
            .map(|(_, value, when)| (value.as_str(), when.as_ref()))
    }
}

/// A record's `{field_id, value}` array as a map.
pub fn state_of(values: &Value) -> Map<String, Value> {
    values
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("field_id")?.as_str()?.to_owned(),
                item.get("value")?.clone(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn money(minor: &str) -> Value {
        json!({"type": "money", "minor_units": minor, "currency": "USD", "minor_unit_exponent": 2})
    }
    fn purchase() -> Rules {
        Rules::from_declaration(&json!({"transitions": [
            {"field_id": "status", "from": "submitted", "to": "approved", "when": {"field_id": "amount", "comparison": "lte", "value": money("500000")}},
            {"field_id": "status", "from": "submitted", "to": "manager_approved", "when": {"field_id": "amount", "comparison": "gt", "value": money("500000")}},
            {"field_id": "status", "from": "manager_approved", "to": "approved"},
            {"field_id": "status", "from": "submitted", "to": "rejected"},
        ]}))
        .unwrap()
    }

    #[test]
    fn a_guarded_transition_fires_only_when_its_guard_holds() {
        let rules = purchase();
        let small = state_of(&json!([{"field_id": "amount", "value": money("500000")}]));
        let large = state_of(&json!([{"field_id": "amount", "value": money("500001")}]));
        assert!(rules.allows("status", "submitted", "approved", &small));
        assert!(!rules.allows("status", "submitted", "manager_approved", &small));
        assert!(!rules.allows("status", "submitted", "approved", &large));
        assert!(rules.allows("status", "submitted", "manager_approved", &large));
        assert!(rules.allows("status", "manager_approved", "approved", &large));
        assert!(rules.allows("status", "submitted", "rejected", &large));
        // A null amount satisfies no guard.
        let null = state_of(&json!([{"field_id": "amount", "value": {"type": "null"}}]));
        assert!(!rules.allows("status", "submitted", "approved", &null));
        assert!(!rules.allows("status", "submitted", "manager_approved", &null));
        // No rules: the plain declaration governs.
        assert!(Rules::default().allows("status", "a", "b", &small));
    }

    #[test]
    fn the_first_initial_entry_that_holds_chooses_the_value() {
        let rules = Rules::from_declaration(&json!({"transitions": [], "initial": [
            {"field_id": "status", "value": "approved", "when": {"field_id": "discount_pct", "comparison": "lte", "value": {"type": "integer", "value": "10", "unit": "percent"}}},
            {"field_id": "status", "value": "submitted"},
        ]}))
        .unwrap();
        let pct = |v: &str| {
            state_of(
                &json!([{"field_id": "discount_pct", "value": {"type": "integer", "value": v, "unit": "percent"}}]),
            )
        };
        assert_eq!(rules.initial_fields(), ["status"]);
        let (value, guard) = rules.initial_value("status", &pct("10")).unwrap();
        assert_eq!(value, "approved");
        assert_eq!(guard.unwrap().describe(), "discount pct ≤ 10%");
        assert_eq!(
            rules.initial_value("status", &pct("11")).unwrap().0,
            "submitted"
        );
        assert!(rules.initial_value("note", &pct("11")).is_none());
        // Malformed declarations are refused.
        assert!(Rules::from_declaration(&json!({"transitions": [{"field_id": "status", "from": "a", "to": "b", "when": {"field_id": 1}}]})).is_none());
    }

    fn at(value: &str) -> Value {
        json!({"type": "timestamp", "value": value})
    }
    fn opened(value: &str) -> Map<String, Value> {
        state_of(&json!([{"field_id": "opened_at", "value": at(value)}]))
    }

    /// ADR-020 C3: `via` and `now` belong to a `requires` entry only; a
    /// transition's or an initial rule's guard carrying either is malformed
    /// stored content (admission refuses both at the schema step).
    #[test]
    fn via_and_now_parse_only_as_a_requirement() {
        let window = json!({"via": "item", "field_id": "opened_at", "comparison": "gte", "value": {"type": "now", "offset_seconds": -30}});
        let guard = Guard::parse_requirement(&window).unwrap();
        assert_eq!(guard.via.as_deref(), Some("item"));
        assert_eq!(guard.describe(), "opened at ≥ now − 30 s");
        let open = json!({"via": "item", "field_id": "state", "comparison": "eq", "value": {"type": "enum", "value": "open"}});
        assert_eq!(
            Guard::parse_requirement(&open).unwrap().describe(),
            "state = open"
        );
        let declared = json!({"requires": [window, open]});
        assert_eq!(requirements(&declared).unwrap().len(), 2);
        assert_eq!(requirements(&json!({})).unwrap(), Vec::<Guard>::new());
        // A malformed entry or a non-array is unavailability.
        assert!(requirements(&json!({"requires": [{"field_id": "x"}]})).is_none());
        assert!(requirements(&json!({"requires": {}})).is_none());
        // `now` is exactly the contract literal: an explicit 0, an offset
        // past a week, a fraction or an extra member is refused.
        for literal in [
            json!({"type": "now", "offset_seconds": 0}),
            json!({"type": "now", "offset_seconds": 604_801}),
            json!({"type": "now", "offset_seconds": -604_801}),
            json!({"type": "now", "offset_seconds": "30"}),
            json!({"type": "now", "value": "2026-11-14T00:00:00Z"}),
        ] {
            let entry = json!({"field_id": "opened_at", "comparison": "gte", "value": literal});
            assert!(Guard::parse_requirement(&entry).is_none(), "{literal}");
        }
        for offset in [-604_800, -1, 1, 604_800] {
            let entry = json!({"field_id": "opened_at", "comparison": "gte", "value": {"type": "now", "offset_seconds": offset}});
            assert!(Guard::parse_requirement(&entry).is_some(), "{offset}");
        }
        // Neither member gives a transition or an initial rule a meaning.
        let with_via = json!({"transitions": [{"field_id": "state", "from": "a", "to": "b", "when": {"via": "item", "field_id": "state", "comparison": "eq", "value": {"type": "enum", "value": "a"}}}]});
        assert!(Rules::from_declaration(&with_via).is_none());
        let with_now = json!({"transitions": [], "initial": [{"field_id": "state", "value": "a", "when": {"field_id": "opened_at", "comparison": "gte", "value": {"type": "now"}}}]});
        assert!(Rules::from_declaration(&with_now).is_none());
    }

    /// ADR-020 C3: `now` is the instant `holds_at` is given, shifted by the
    /// offset, compared as a timestamp; `holds` (no instant) never satisfies
    /// it; a null or absent field satisfies no guard.
    #[test]
    fn a_now_literal_is_judged_at_the_given_instant() {
        let window = Guard::parse_requirement(&json!({"field_id": "opened_at", "comparison": "gte", "value": {"type": "now", "offset_seconds": -30}})).unwrap();
        let now = "2026-11-29T15:00:30Z";
        assert!(window.holds_at(&opened("2026-11-29T15:00:30Z"), now));
        assert!(
            window.holds_at(&opened("2026-11-29T15:00:00Z"), now),
            "the bound is inclusive (gte)"
        );
        assert!(!window.holds_at(&opened("2026-11-29T14:59:59Z"), now));
        assert!(
            !window.holds(&opened("2026-11-29T15:00:30Z")),
            "no instant, no judgment"
        );
        assert!(!window.holds_at(
            &state_of(&json!([{"field_id": "opened_at", "value": {"type": "null"}}])),
            now
        ));
        assert!(!window.holds_at(&Map::new(), now));
        // A deadline: `closes_at > now`, across a day and a year boundary.
        let deadline = Guard::parse_requirement(
            &json!({"field_id": "closes_at", "comparison": "gt", "value": {"type": "now"}}),
        )
        .unwrap();
        let closes =
            |value: &str| state_of(&json!([{"field_id": "closes_at", "value": at(value)}]));
        assert!(deadline.holds_at(&closes("2027-01-01T00:00:00Z"), "2026-12-31T23:59:59Z"));
        assert!(!deadline.holds_at(&closes("2027-01-01T00:00:00Z"), "2027-01-01T00:00:00Z"));
        let later = Guard::parse_requirement(&json!({"field_id": "closes_at", "comparison": "lte", "value": {"type": "now", "offset_seconds": 86_400}})).unwrap();
        assert!(later.holds_at(&closes("2024-03-01T12:00:00Z"), "2024-02-29T12:00:00Z"));
        assert!(!later.holds_at(&closes("2024-03-01T12:00:01Z"), "2024-02-29T12:00:00Z"));
        // An instant that is not a contract timestamp judges nothing.
        assert!(!deadline.holds_at(&closes("2027-01-01T00:00:00Z"), "2026-12-31 23:59:59"));
        // An ordinary literal is unchanged by the instant.
        let state = Guard::parse_requirement(&json!({"field_id": "state", "comparison": "eq", "value": {"type": "enum", "value": "open"}})).unwrap();
        let open =
            state_of(&json!([{"field_id": "state", "value": {"type": "enum", "value": "open"}}]));
        assert!(state.holds_at(&open, now) && state.holds(&open));
    }

    #[test]
    fn contract_timestamps_round_trip_through_unix_seconds() {
        for seconds in [
            0_i64,
            951_782_400,
            1_709_208_000,
            1_795_000_000,
            4_102_444_799,
        ] {
            let text = timestamp_of(seconds).unwrap();
            assert_eq!(text, crate::events::timestamp(seconds as u64));
            assert_eq!(unix_seconds(&text), Some(seconds));
        }
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_seconds("2000-02-29T00:00:00Z"), Some(951_782_400));
        assert_eq!(unix_seconds("2026-02-29T00:00:00Z"), None);
        assert_eq!(unix_seconds("2026-11-29T15:00:00+00:00"), None);
    }
}
