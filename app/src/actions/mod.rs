//! ADR-011 scenarios (APP-093, APP-095): the action path. "I spent $86 on a
//! client dinner at Nobu, Sarah approves my expenses" becomes a typed,
//! storage-validated command draft the member confirms; the AI never writes.
//!
//! Flow of `POST /v1/act` (`routes`): the utterance is sent, as JSON data
//! inside the user message, to the fast model (the composer's model client:
//! same key from `ANTHROPIC_API_KEY`, same disabled behavior) with the
//! member's catalog of record apps (library apps the member may instantiate
//! now, their record schema and declared commands). The reply is decoded
//! into the closed [`Parsed`] shape; anything else is `could_not_parse`.
//! Then the server resolves everything the model only hinted at:
//! the app (library-first; a member with no saved work of it gets one, the
//! composer's `retrieved` path, "the first time"), the typed values under the
//! published schema (`commands::routes::typed_value`, money in cents, the
//! status forced to `submitted` on a create), the approver (a same-organization
//! member whose display name matches), the target record of a decision (the
//! member's writable records, fuzzy on merchant, note and amount), and a
//! server-minted 128-bit idempotency key. The draft is judged by
//! [`crate::storage::Data::dry_run_command`] (the commit's own rules, rolled
//! back) and returned as a confirmation card. The member's Confirm posts the
//! existing command route with that key: the audited, idempotent, transactional
//! path judges everything again. Nothing the model wrote reaches storage
//! except as a typed value under the published schema; it names no principal,
//! no permission and no SQL.
//!
//! Demo shortcut (APP-098, `crate::demo`; Victor, 2026-10-30: "concept-car
//! demo — honesty applies at release, not on stage"): inside a listed demo
//! organization only, the rehearsed utterances of `docs/pitch/SCENARIOS.md`
//! are parsed instantly and deterministically (`rehearsed`), with no model
//! call; unrehearsed text goes to the real model. Outside a demo organization
//! nothing canned applies. Everything after the parse is identical and real:
//! the typed draft, the storage dry run, the member's Confirm, the audited
//! commit, the share and the live events.
pub mod guard;
pub mod ledger;
pub mod prompt;
pub mod receipt;
pub mod rehearsed;
pub mod resolve;
pub mod routes;
pub mod workflows;

#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
pub(crate) mod tests;

use crate::composer::model::{self, Model, ModelError, ModelRequest};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{sync::Arc, time::Duration};

/// The route's path (long deadline: it waits on the model).
pub const PATH: &str = "/v1/act";
/// The parse call's budget.
pub const PARSE_BUDGET: Duration = Duration::from_secs(12);
const PARSE_MAX_TOKENS: u32 = 400;

/// What the member asked for, as the parser decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// File a new record (an expense claim).
    Create,
    /// Move an existing record along a declared transition (approve/reject).
    Decide,
    /// Show an existing app (the member's saved work of a library app).
    Open,
    /// Not a record action: the canvas falls back to the composer.
    Place,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Decide => "decide",
            Self::Open => "open",
            Self::Place => "place",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "create" => Self::Create,
            "decide" => Self::Decide,
            "open" => Self::Open,
            "place" => Self::Place,
            _ => return None,
        })
    }
}

/// The parser's closed reply shape (model or fallback). Every member is a
/// hint the server resolves; none is trusted as an identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parsed {
    pub kind: Kind,
    /// A catalog app id, or empty.
    pub app_hint: String,
    /// Field id → text value, judged later under the published schema.
    pub values: Map<String, Value>,
    pub approver_name: Option<String>,
    pub target_record_hint: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    action: String,
    #[serde(default)]
    app_hint: Option<String>,
    #[serde(default)]
    values: Option<Map<String, Value>>,
    #[serde(default)]
    approver_name: Option<String>,
    #[serde(default)]
    target_record_hint: Option<String>,
}

/// A bounded, single-line text hint (names, record hints, values).
fn hint(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_owned())
}

/// Decodes the parser's text strictly: the first JSON object in it, as
/// forma-json-v1, into the closed shape; values are kept as text or exact
/// numbers only. `None` for anything else.
pub fn decode(text: &str) -> Option<Parsed> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end < start {
        return None;
    }
    let value = crate::admission::decode(&text.as_bytes()[start..=end]).ok()?;
    let reply: Reply = serde_json::from_value(value).ok()?;
    let kind = Kind::parse(&reply.action)?;
    let mut values = Map::new();
    for (field, value) in reply.values.unwrap_or_default() {
        if !crate::storage::valid_local_id(&field) || values.len() >= 32 {
            continue;
        }
        let text = match value {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => continue,
            _ => continue,
        };
        if let Some(text) = hint(Some(text)) {
            values.insert(field, Value::String(text));
        }
    }
    Some(Parsed {
        kind,
        app_hint: hint(reply.app_hint)
            .filter(|id| crate::storage::valid_identifier(id))
            .unwrap_or_default(),
        values,
        approver_name: hint(reply.approver_name),
        target_record_hint: hint(reply.target_record_hint),
    })
}

/// Why no parse was produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailure {
    /// No model and no fallback applies (503 `composer_unavailable`).
    Unavailable,
    /// The model failed or answered outside the shape (422 `could_not_parse`).
    CouldNotParse(&'static str),
}

/// A parse and how it was obtained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parse {
    pub parsed: Parsed,
    /// True when the rehearsed parser answered (demo organizations only; for
    /// the structured log, never shown).
    pub rehearsed: bool,
    pub duration_ms: u64,
}

/// The utterance parser: the fast model (none: disabled) and its name.
pub struct Parser {
    model: Option<Arc<dyn Model>>,
    model_name: String,
}
impl Parser {
    pub fn new(model: Option<Arc<dyn Model>>, model_name: &str) -> Self {
        Self {
            model,
            model_name: model_name.to_owned(),
        }
    }
    /// The environment's parser: the composer's key and router model.
    pub fn from_env() -> Self {
        let model = model::Anthropic::from_env().map(|client| Arc::new(client) as Arc<dyn Model>);
        let name = std::env::var(model::ROUTER_MODEL_ENV)
            .ok()
            .filter(|value| {
                (1..=64).contains(&value.len())
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
            })
            .unwrap_or_else(|| model::DEFAULT_ROUTER_MODEL.to_owned());
        Self::new(model, &name)
    }
    pub fn enabled(&self) -> bool {
        self.model.is_some()
    }
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Parses `utterance` against `catalog` (the member's record apps,
    /// `prompt::Catalog`). In a demo organization a rehearsed utterance is
    /// answered deterministically and at once; everything else, and every
    /// utterance outside a demo organization, goes to the model. Either
    /// answer passes the read-only guard (`guard`, APP-135): a sentence that
    /// reads or asks never comes back as a create or decide.
    pub async fn parse(
        &self,
        utterance: &str,
        catalog: &[prompt::CatalogApp],
        demo: bool,
        today: &str,
    ) -> Result<Parse, ParseFailure> {
        let started = std::time::Instant::now();
        let elapsed = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        // APP-135: a sentence that reads, with a money threshold, is never a
        // command: `place` at once, no model call.
        if let Some(parsed) = guard::before(utterance) {
            return Ok(Parse {
                parsed,
                rehearsed: false,
                duration_ms: elapsed(),
            });
        }
        if demo {
            if let Some(parsed) = rehearsed::parse(utterance, catalog, today) {
                let parsed = guard::after(utterance, parsed);
                tracing::info!(
                    event = "action.rehearsed",
                    kind = parsed.kind.as_str(),
                    "rehearsed utterance parsed deterministically (demo organization)"
                );
                return Ok(Parse {
                    parsed,
                    rehearsed: true,
                    duration_ms: elapsed(),
                });
            }
        }
        let Some(model) = self.model.as_ref() else {
            return Err(ParseFailure::Unavailable);
        };
        let request = ModelRequest {
            model: self.model_name.clone(),
            system: prompt::PARSE_SYSTEM.to_owned(),
            user: prompt::parse_user(utterance, catalog, today),
            max_tokens: PARSE_MAX_TOKENS,
            budget: PARSE_BUDGET,
        };
        let reply = model.complete(&request).await?;
        decode(&reply.text)
            .map(|parsed| Parse {
                parsed: guard::after(utterance, parsed),
                rehearsed: false,
                duration_ms: elapsed(),
            })
            .ok_or(ParseFailure::CouldNotParse("malformed_parse"))
    }
}

impl From<ModelError> for ParseFailure {
    fn from(error: ModelError) -> Self {
        Self::CouldNotParse(error.code())
    }
}
