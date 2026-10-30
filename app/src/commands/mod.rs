//! APP-013: the declared record command over HTTP (ADR-007 D7.9, D7.14,
//! D7.15, D7.16; design §9) and the server-rendered record page whose form
//! carries a per-render idempotency key.
//!
//! `POST /v1/work/{app}/{work}/commands/{command}` takes the contract's
//! `command_request` as JSON (≤ 256 KiB) with the key in the `Idempotency-Key`
//! header, or the record page's form (the key in its hidden field, the
//! synchronizer token tied to the workspace session). Answers: 200 with the
//! `command_outcome` document for `Committed` and `Replayed`; 409 `conflict`
//! with the authorized `recovery_view`; 412 `stale` with the work's current
//! revision; 409 `in_flight` and 409 `busy` with `Retry-After: 2`; 400 for a
//! missing or malformed key or body; 413 over the body limit; 404 for every
//! absent, foreign or unauthorized target; 503 `unknown_outcome` when the
//! COMMIT's answer was lost (the client re-reads by key: `GET
//! /v1/work/{app}/{work}/commands/{command}/outcomes/{key}`), 503 `unavailable`
//! for a store outage. The organization and principal come from the session,
//! never from the body or the path. Rendering is the binary's (`RecordPage`).
pub mod payload;
pub mod routes;

#[cfg(test)]
mod tests;

use crate::storage::{RecordCommand, RecordDetail};
use axum::response::Response;

/// A form notice the record page shows after a redirect (closed set).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    Committed,
    Replayed,
    Conflict,
    Stale,
    InFlight,
    Busy,
    Refused,
    /// The COMMIT's answer was lost: the record page shows the current state.
    Unknown,
}
impl Notice {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "committed" => Self::Committed,
            "replayed" => Self::Replayed,
            "conflict" => Self::Conflict,
            "stale" => Self::Stale,
            "in_flight" => Self::InFlight,
            "busy" => Self::Busy,
            "refused" => Self::Refused,
            "unknown" => Self::Unknown,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Replayed => "replayed",
            Self::Conflict => "conflict",
            Self::Stale => "stale",
            Self::InFlight => "in_flight",
            Self::Busy => "busy",
            Self::Refused => "refused",
            Self::Unknown => "unknown",
        }
    }
}

/// One field row of the record page: the current value as text, and the
/// form input for it when the shown command writes it. `spec` is the
/// published field's `value_type` as compact JSON, echoed by the form so that
/// the route can type the submitted text; storage re-judges it against the
/// published schema, so a tampered spec only fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldRow {
    pub id: String,
    pub kind: String,
    pub value: String,
    pub input: String,
    pub spec: String,
    /// The echo's input name, `spec.<field>`.
    pub spec_input: String,
    pub options: String,
    pub nullable: bool,
    pub writable: bool,
}

/// What the record page shows; the binary renders it with typed props.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordView {
    pub app: String,
    pub work: String,
    pub collection: String,
    pub record_id: String,
    pub schema: String,
    pub schema_version: String,
    pub publication: String,
    pub work_version: String,
    pub version: String,
    pub exists: bool,
    pub fields: Vec<FieldRow>,
    /// The command the form submits (the schema's first declared command
    /// admitting the record's state), or empty.
    pub command: String,
    pub action: String,
    /// `record_write` on the work now and a command to offer: the form shows.
    pub can_write: bool,
    pub csrf: String,
    /// D7.16: a fresh 128-bit key per render, in the form's hidden field.
    pub idempotency_key: String,
    pub notice: Option<Notice>,
}

pub trait RecordPage: Send + Sync {
    fn record(&self, view: &RecordView) -> Response;
}

/// The text a form input carries for a stored value, and the text back.
fn value_text(value: &serde_json::Value) -> String {
    let get = |key: &str| {
        value
            .get(key)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("null") => String::new(),
        Some("decimal") => get("coefficient"),
        Some("money") => get("minor_units"),
        Some("reference") => get("record_id"),
        _ => get("value"),
    }
}

/// The page's rows from a storage detail: the form shows the first declared
/// command storage would admit on the record's current state (a `create` on
/// an absent record; on an existing one an `update`, or a `create` one of
/// whose declared transitions starts at the field's current value, as
/// `storage::commands::compose` requires), and no form when none would.
pub fn view(
    app: &str,
    work: &str,
    detail: &RecordDetail,
    csrf: String,
    idempotency_key: String,
    notice: Option<Notice>,
) -> RecordView {
    let current = |field: &str| {
        detail
            .fields
            .iter()
            .find(|f| f.id == field)
            .and_then(|f| f.value.get("value"))
            .and_then(serde_json::Value::as_str)
    };
    let admits = |c: &&RecordCommand| match (detail.version, c.operation.as_str()) {
        (0, operation) => operation == "create",
        (_, "update") => true,
        (_, "create") => c
            .transitions
            .iter()
            .any(|(field, from, _)| current(field) == Some(from.as_str())),
        _ => false,
    };
    // ADR-011 scenarios: on an absent record a pure `create` (no declared
    // transitions, the one that files a new record) is offered before a
    // transition-only `create` such as `decide`, whatever their ids sort as.
    let command = detail
        .commands
        .iter()
        .filter(admits)
        .min_by_key(|c| (detail.version == 0 && !c.transitions.is_empty()) as u8)
        .cloned();
    let fields = detail
        .fields
        .iter()
        .map(|field| {
            let kind = field
                .value_type
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("text")
                .to_owned();
            let options = field
                .value_type
                .get("values")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .unwrap_or_default();
            FieldRow {
                id: field.id.clone(),
                kind,
                value: value_text(&field.value),
                input: format!("value.{}", field.id),
                spec: serde_json::to_string(&field.value_type).unwrap_or_default(),
                spec_input: format!("spec.{}", field.id),
                options,
                nullable: field.nullable,
                writable: command
                    .as_ref()
                    .is_some_and(|c| c.writable_fields.contains(&field.id)),
            }
        })
        .collect();
    RecordView {
        app: app.to_owned(),
        work: work.to_owned(),
        collection: detail.collection.clone(),
        record_id: detail.record_id.clone(),
        schema: detail.schema.clone(),
        schema_version: detail.schema_version.to_string(),
        publication: detail.publication.clone(),
        work_version: detail.work_version.to_string(),
        version: detail.version.to_string(),
        exists: detail.version > 0,
        fields,
        action: command
            .as_ref()
            .map(|c| format!("/v1/work/{app}/{work}/commands/{}", c.id))
            .unwrap_or_default(),
        can_write: detail.can_write && command.is_some(),
        command: command.map(|c| c.id).unwrap_or_default(),
        csrf,
        idempotency_key,
        notice,
    }
}
