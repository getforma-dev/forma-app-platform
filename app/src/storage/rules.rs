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
use serde_json::{Map, Value};
use std::cmp::Ordering;

/// One declared guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard {
    pub field: String,
    pub comparison: String,
    /// The typed literal (contract `value`).
    pub value: Value,
}

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

impl Guard {
    fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            field: object.get("field_id")?.as_str()?.to_owned(),
            comparison: object.get("comparison")?.as_str()?.to_owned(),
            value: object.get("value")?.clone(),
        })
    }

    /// Whether the guard holds on `state` (field id → typed value).
    pub fn holds(&self, state: &Map<String, Value>) -> bool {
        let (Some(have), Some(want)) = (state.get(&self.field).and_then(key), key(&self.value))
        else {
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
        format!(
            "{} {symbol} {}",
            self.field.replace('_', " "),
            crate::actions::resolve::display(&self.value)
        )
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
}
