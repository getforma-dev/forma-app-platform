//! APP-013 declared record commands (design §6 D5, §8, §9; C07; ADR-002
//! §Retry, completion and concurrency; ADR-004; ADR-007 D7.7, D7.8, D7.9,
//! D7.14; plan Task 10).
//!
//! One command is one READ COMMITTED transaction of the data role, in the
//! D7.9 order: the member precheck (`record_write` on the work, judged on
//! current state) → the idempotency claim (`INSERT … ON CONFLICT DO NOTHING`,
//! no savepoint; a taken key reads the committed outcome, so an identical
//! payload is `Replayed` and a different one `Conflict`, both delivered only
//! after the authority fence re-checks current visibility) → the
//! `genui_lock` member fence → the work fence with the caller's publication
//! and version premise (`Stale` names the work's current revision) → the
//! organization's commit sequence `FOR UPDATE` (D7.7, one row per
//! organization, created by the operator channel) → the record row `FOR
//! UPDATE` and the `expected_record_version` comparison (0 means absent; a
//! mismatch is `Conflict` with the authorized recovery view) → the data chain's
//! head `FOR UPDATE` → the revision, the current-version pointer, the record's
//! source dependencies, exactly one `command.committed` audit event, the
//! advanced sequence and the outcome (with a foreign key to the claim), all
//! committed together. A lock wait past `lock_timeout` on the claim, the
//! sequence or the record row is `InFlight`; one on an authorization fence is
//! `Busy` (D7.9's 409 `busy` row). Anything that is not `Committed` rolls the
//! transaction back, which releases the claim. A COMMIT whose answer is lost is
//! `Error::UnknownOutcome`: the caller re-reads the outcome by key under
//! current authority (`read_outcome`), never resubmits under a new key.
//!
//! Nothing a caller supplies is trusted as authority or as admitted content:
//! the organization and principal come from the trusted request, the command
//! is checked against the declaration of the publication the work pins, every
//! value against the published record schema's field types (the contract's
//! `compatible` rule) and, on an existing record, against the declared
//! transitions; the record's full state is composed server-side and stored as
//! canonical forma-json-v1 bytes.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::{append, fresh, local_id, Chain, CurrentRevision, Event, MAX_CANONICAL_BYTES},
    pool::Data,
    require, valid_id, Error, Result, TrustedRequest,
};
use crate::{admission, commands::payload};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline; every command transaction is bounded by it.
pub const COMMAND_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// Values in one command (contract `maxItems`).
pub const MAX_COMMAND_VALUES: usize = 32;
/// A text value, in Unicode scalar values (contract `TEXT`).
const MAX_TEXT_CHARACTERS: usize = 2000;
/// ADR-002: synthetic outcome retention.
pub const RETRY_HORIZON_SECONDS: i64 = 86_400;
/// The audit kind of the data chain (D7.8, closed).
pub const COMMAND_COMMITTED: &str = "command.committed";

/// A declared command as the caller states it (contract `command_request`,
/// edition 1); `values` is the `{field_id, value}` array. The organization and
/// principal are never part of it: they come from the trusted request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandRequest {
    pub app: String,
    pub work: String,
    pub publication: String,
    pub command: String,
    pub schema: String,
    pub schema_version_expected: i64,
    pub record: String,
    pub expected_record_version: i64,
    pub expected_work_version: i64,
    pub values: Value,
    pub idempotency_key: String,
}

/// ADR-002: at least 128 bits from a cryptographically secure generator,
/// carried as exactly 32 lowercase hex digits (contract pattern). The shape
/// check cannot establish entropy; the renderer and the island own that.
pub fn idempotency_key_shaped(key: &str) -> bool {
    key.len() == 32
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

const SAFE_MAX: i64 = 9_007_199_254_740_991;

impl CommandRequest {
    /// The contract's shape, before any storage work: UUID identifiers, local
    /// ids, bounded versions, the key shape and 1..=32 `{field_id, value}`
    /// objects with distinct field ids. Value typing is judged against the
    /// published schema inside the transaction.
    pub fn well_formed(&self) -> bool {
        let mut seen = BTreeSet::new();
        valid_id(&self.app)
            && valid_id(&self.work)
            && valid_id(&self.publication)
            && valid_id(&self.record)
            && local_id(&self.command)
            && local_id(&self.schema)
            && (1..=SAFE_MAX).contains(&self.schema_version_expected)
            && (0..=SAFE_MAX).contains(&self.expected_record_version)
            && (1..=SAFE_MAX).contains(&self.expected_work_version)
            && idempotency_key_shaped(&self.idempotency_key)
            && self.values.as_array().is_some_and(|values| {
                (1..=MAX_COMMAND_VALUES).contains(&values.len())
                    && values.iter().all(|item| {
                        item.as_object().is_some_and(|object| {
                            object.len() == 2
                                && object
                                    .get("field_id")
                                    .and_then(Value::as_str)
                                    .is_some_and(|id| local_id(id) && seen.insert(id.to_owned()))
                                && object.get("value").is_some_and(Value::is_object)
                        })
                    })
            })
    }
    /// The complete `command_request` document, the key included; the payload
    /// digest (`forma:command-payload:v1`) covers it without the key.
    pub fn document(&self) -> Value {
        json!({
            "kind": "command_request",
            "schema_version": 1,
            "app_id": self.app,
            "work_id": self.work,
            "publication_id": self.publication,
            "command_id": self.command,
            "schema_id": self.schema,
            "schema_version_expected": self.schema_version_expected,
            "record_id": self.record,
            "expected_record_version": self.expected_record_version,
            "expected_work_version": self.expected_work_version,
            "values": self.values,
            "idempotency_key": self.idempotency_key,
        })
    }
}

/// The committed outcome (contract `command_outcome`, edition 1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutcome {
    pub organization: String,
    pub app: String,
    pub work: String,
    pub principal: String,
    pub command: String,
    pub idempotency_key: String,
    pub payload_digest: String,
    pub record_id: String,
    pub record_version: i64,
    pub work_version: i64,
    pub commit_sequence: i64,
    pub audit_event_id: String,
    pub committed_at: String,
    pub retry_expires_at: String,
}
impl CommandOutcome {
    pub fn document(&self) -> Value {
        json!({
            "kind": "command_outcome",
            "schema_version": 1,
            "organization_id": self.organization,
            "app_id": self.app,
            "work_id": self.work,
            "principal_id": self.principal,
            "command_id": self.command,
            "idempotency_key": self.idempotency_key,
            "payload_digest": self.payload_digest,
            "state": "committed",
            "record_id": self.record_id,
            "record_version": self.record_version,
            "work_version": self.work_version,
            "commit_sequence": self.commit_sequence,
            "audit_event_id": self.audit_event_id,
            "committed_at": self.committed_at,
            "retry_expires_at": self.retry_expires_at,
        })
    }
}

/// Why a command conflicted (409 with the recovery view, D7.9).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictReason {
    /// `expected_record_version` is not the record's current version.
    ExpectedVersion,
    /// The key is taken by a committed command with another payload.
    PayloadMismatch,
    /// The key is taken by a committed command whose 24-hour retry horizon
    /// has passed (ADR-002: explicit reconciliation, never a resubmission).
    RetryExpired,
}
impl ConflictReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::ExpectedVersion => "expected_version",
            Self::PayloadMismatch => "payload_mismatch",
            Self::RetryExpired => "retry_expired",
        }
    }
}

/// The authorized recovery view (C07; contract `recovery_view`, edition
/// impl-2026-10-29.3): the record as the caller may currently see it, read in
/// the fenced transaction that refused the command, and the stored outcome
/// when the key is the conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryView {
    pub organization: String,
    pub app: String,
    pub work: String,
    pub publication: String,
    pub work_version: i64,
    pub record_id: String,
    pub command: String,
    pub idempotency_key: String,
    pub reason: ConflictReason,
    pub expected_record_version: i64,
    /// 0 when the record does not exist.
    pub current_record_version: i64,
    /// The record's current `{field_id, value}` array; `None` when absent.
    pub current_values: Option<Value>,
    pub stored_outcome: Option<CommandOutcome>,
}
impl RecoveryView {
    pub fn document(&self) -> Value {
        json!({
            "kind": "recovery_view",
            "schema_version": 1,
            "organization_id": self.organization,
            "app_id": self.app,
            "work_id": self.work,
            "publication_id": self.publication,
            "work_version": self.work_version,
            "record_id": self.record_id,
            "command_id": self.command,
            "idempotency_key": self.idempotency_key,
            "reason": self.reason.code(),
            "expected_record_version": self.expected_record_version,
            "current_record_version": self.current_record_version,
            "current_values": self.current_values,
            "stored_outcome": self.stored_outcome.as_ref().map(CommandOutcome::document),
        })
    }
}

/// ADR-007 D7.9. `Committed`/`Replayed` → 200; `Conflict` → 409 with the
/// recovery view; `Stale` → 412 with the current revision; `InFlight` → 409
/// `in_flight`; `Busy` → 409 `busy` (D7.9's "55P03 on an authorization fence"
/// row, typed here so that no lock wait becomes a 503).
// D7.9 names this shape; the recovery view is the largest variant by design.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Eq, PartialEq)]
pub enum CommandResult {
    Committed(CommandOutcome),
    Replayed(CommandOutcome),
    Conflict(RecoveryView),
    Stale(CurrentRevision),
    InFlight,
    Busy,
}

/// One field of a record as the record page shows it: the published schema's
/// field, its current value (`{"type":"null"}` when unset) and whether the
/// shown command may write it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordField {
    pub id: String,
    /// The schema field's `value_type` object.
    pub value_type: Value,
    pub nullable: bool,
    pub value: Value,
    pub writable: bool,
}
/// A declared command of the record's schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordCommand {
    pub id: String,
    pub operation: String,
    pub writable_fields: Vec<String>,
    /// The declared transitions, `(field_id, from, to)`.
    pub transitions: Vec<(String, String, String)>,
    /// APP-107: the declared guards and initial values (`storage::rules`).
    pub rules: super::rules::Rules,
}
/// A record read back for its page: current version (0 when absent), its
/// fields and the commands its schema declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDetail {
    pub record_id: String,
    pub collection: String,
    pub schema: String,
    pub schema_version: i32,
    pub publication: String,
    pub definition_digest: String,
    pub work_version: i64,
    pub version: i64,
    pub fields: Vec<RecordField>,
    pub commands: Vec<RecordCommand>,
    /// Whether the reader also holds `record_write` on the work now.
    pub can_write: bool,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn small(row: &Row, index: usize) -> Result<i32> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
/// Stored state that contradicts its own contract is unavailability, never a
/// denial a caller could probe.
fn require_stored(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Unavailable)
    }
}
pub(super) fn decode_stored(row: &Row, index: usize) -> Result<Value> {
    let bytes: Vec<u8> = row.try_get(index).map_err(|_| Error::Unavailable)?;
    require_stored(bytes.len() <= MAX_CANONICAL_BYTES)?;
    admission::decode(&bytes).map_err(|_| Error::Unavailable)
}

/// A field of the published record schema.
pub(super) struct FieldSpec {
    pub(super) id: String,
    pub(super) value_type: Value,
    pub(super) nullable: bool,
}
/// A declared command of the published definition.
pub(super) struct Declaration {
    pub(super) operation: String,
    pub(super) writable: Vec<String>,
    /// `(field_id, from, to)`.
    pub(super) transitions: Vec<(String, String, String)>,
    /// APP-107: the declared guards and initial values.
    pub(super) rules: super::rules::Rules,
}
/// The published definition's record schema `schema` at `version` and the
/// commands declared on it (stored content is admitted content: a shape that
/// does not parse is unavailability).
pub(super) struct Published {
    pub(super) definition_digest: String,
    pub(super) fields: Vec<FieldSpec>,
    pub(super) commands: BTreeMap<String, Declaration>,
}

fn string_list(value: Option<&Value>) -> Result<Vec<String>> {
    value
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or(Error::Unavailable))
        .collect()
}

pub(super) async fn published(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    publication: &str,
    schema: &str,
    version: i64,
) -> Result<Published> {
    let rows = transaction.query("SELECT p.definition_revision_id::text,p.definition_digest::text,d.canonical_content FROM genui.publications p JOIN genui.definition_revisions d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid AND p.publication_id=$3::text::uuid", &[&organization,&app,&publication]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    let content = decode_stored(row, 2)?;
    let schemas = content
        .get("record_schemas")
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?;
    let found = schemas
        .iter()
        .find(|s| s.get("id").and_then(Value::as_str) == Some(schema))
        .ok_or(Error::Denied)?;
    require(found.get("version").and_then(Value::as_i64) == Some(version))?;
    let fields = found
        .get("fields")
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?
        .iter()
        .map(|field| {
            Ok(FieldSpec {
                id: field
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or(Error::Unavailable)?
                    .to_owned(),
                value_type: field.get("value_type").cloned().ok_or(Error::Unavailable)?,
                nullable: field
                    .get("nullable")
                    .and_then(Value::as_bool)
                    .ok_or(Error::Unavailable)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut commands = BTreeMap::new();
    for declared in content
        .get("commands")
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?
    {
        if declared.get("record_schema_id").and_then(Value::as_str) != Some(schema) {
            continue;
        }
        let id = declared
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Error::Unavailable)?;
        let transitions = declared
            .get("transitions")
            .and_then(Value::as_array)
            .ok_or(Error::Unavailable)?
            .iter()
            .map(|t| {
                let part = |key: &str| -> Result<String> {
                    Ok(t.get(key)
                        .and_then(Value::as_str)
                        .ok_or(Error::Unavailable)?
                        .to_owned())
                };
                Ok((part("field_id")?, part("from")?, part("to")?))
            })
            .collect::<Result<Vec<_>>>()?;
        commands.insert(
            id.to_owned(),
            Declaration {
                operation: declared
                    .get("operation")
                    .and_then(Value::as_str)
                    .ok_or(Error::Unavailable)?
                    .to_owned(),
                writable: string_list(declared.get("writable_fields"))?,
                transitions,
                rules: super::rules::Rules::from_declaration(declared).ok_or(Error::Unavailable)?,
            },
        );
    }
    Ok(Published {
        definition_digest: text(row, 1)?,
        fields,
        commands,
    })
}

/// The work's collection of `schema` at `version`.
pub(super) async fn collection(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    schema: &str,
    version: i64,
) -> Result<String> {
    let version = i32::try_from(version).map_err(|_| Error::Denied)?;
    let rows = transaction.query("SELECT collection_id::text FROM genui.record_collections WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND schema_id=$4::text AND schema_version=$5", &[&organization,&app,&work,&schema,&version]).await?;
    text(rows.first().ok_or(Error::Denied)?, 0)
}

/// The record's current version (0 when absent) and state.
pub(super) async fn current_state(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    collection: &str,
    record: &str,
    lock: bool,
) -> Result<(i64, Option<Value>, Option<String>)> {
    let sql = if lock {
        "SELECT current_version::bigint FROM genui.internal_records WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid FOR UPDATE"
    } else {
        "SELECT current_version::bigint FROM genui.internal_records WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid"
    };
    let rows = transaction
        .query(sql, &[&organization, &app, &work, &collection, &record])
        .await?;
    let Some(row) = rows.first() else {
        return Ok((0, None, None));
    };
    let version = integer(row, 0)?;
    let revision = transaction.query("SELECT canonical_values,creator_id::text FROM genui.internal_record_revisions WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid AND version=$6::bigint", &[&organization,&app,&work,&collection,&record,&version]).await?;
    let revision = revision.first().ok_or(Error::Unavailable)?;
    Ok((
        version,
        Some(decode_stored(revision, 0)?),
        Some(text(revision, 1)?),
    ))
}

/// The committed outcome of this principal under the scoped key, with
/// whether its retry horizon is still open.
async fn stored_outcome(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
    command: &str,
    key: &str,
) -> Result<Option<(CommandOutcome, bool)>> {
    let rows = transaction.query("SELECT payload_digest::text,record_id::text,record_version::bigint,work_version::bigint,commit_sequence::bigint,audit_event_id::text,to_char(committed_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),to_char(retry_expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),retry_expires_at>clock_timestamp() FROM genui.command_outcomes WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND app_id=$3::text::uuid AND work_id=$4::text::uuid AND command_id=$5::text AND idempotency_key=$6::text", &[&request.organization,&request.principal,&app,&work,&command,&key]).await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    Ok(Some((
        CommandOutcome {
            organization: request.organization.clone(),
            app: app.to_owned(),
            work: work.to_owned(),
            principal: request.principal.clone(),
            command: command.to_owned(),
            idempotency_key: key.to_owned(),
            payload_digest: text(row, 0)?,
            record_id: text(row, 1)?,
            record_version: integer(row, 2)?,
            work_version: integer(row, 3)?,
            commit_sequence: integer(row, 4)?,
            audit_event_id: text(row, 5)?,
            committed_at: text(row, 6)?,
            retry_expires_at: text(row, 7)?,
        },
        boolean(row, 8)?,
    )))
}

/// The saved work's pin, as the work fence sees it.
pub(super) async fn work_pin(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
) -> Result<CurrentRevision> {
    let rows = transaction.query("SELECT w.publication_id::text,w.version::bigint,p.definition_revision_id::text,p.definition_digest::text FROM genui.saved_work w JOIN genui.publications p USING(organization_id,app_id,publication_id) WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid", &[&organization,&app,&work]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    Ok(CurrentRevision {
        publication: Some(text(row, 0)?),
        revision: Some(text(row, 2)?),
        definition_digest: Some(text(row, 3)?),
        work_version: Some(integer(row, 1)?),
    })
}

/// `lock_work_revision` with the caller's premise: `Ok(true)` when the work
/// pins exactly that publication and version (and is now `FOR SHARE`
/// locked), `Ok(false)` when the premise has moved.
pub(super) async fn work_fence(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    publication: &str,
    version: i64,
) -> Result<bool> {
    let rows = transaction.query("SELECT publication_id::text,work_version FROM genui_private.lock_work_revision($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5)", &[&organization,&app,&work,&publication,&version]).await?;
    Ok(match rows.first() {
        Some(row) => text(row, 0)? == publication && integer(row, 1)? == version,
        None => false,
    })
}

// --- value typing (contracts/validate.py `compatible`, edition 1 VALUES) ---

fn integer_text(value: Option<&Value>) -> bool {
    value.and_then(Value::as_str).is_some_and(|s| {
        let digits = s.strip_prefix('-').unwrap_or(s);
        !digits.is_empty()
            && digits.len() <= 38
            && digits.bytes().all(|b| b.is_ascii_digit())
            && (digits == "0" && !s.starts_with('-') || !digits.starts_with('0'))
    })
}
fn fixed_digits(value: &str, pattern: &str) -> bool {
    value.len() == pattern.len()
        && value.bytes().zip(pattern.bytes()).all(|(v, p)| match p {
            b'9' => v.is_ascii_digit(),
            _ => v == p,
        })
}
fn keys_are(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|k| object.contains_key(*k))
}
/// The contract rule: a value's tag matches the field's type, its members are
/// exactly the tag's, and the typed extras (unit, scale, currency, enum
/// member, reference target) equal the field's.
fn compatible(value: &Value, field: &FieldSpec) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        return false;
    };
    if kind == "null" {
        return field.nullable && keys_are(object, &["type"]);
    }
    let expected = &field.value_type;
    if expected.get("type").and_then(Value::as_str) != Some(kind) {
        return false;
    }
    let same = |key: &str| object.get(key) == expected.get(key) && object.get(key).is_some();
    match kind {
        "text" => {
            keys_are(object, &["type", "value"])
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.chars().count() <= MAX_TEXT_CHARACTERS)
        }
        "boolean" => {
            keys_are(object, &["type", "value"])
                && object.get("value").is_some_and(Value::is_boolean)
        }
        "integer" => {
            keys_are(object, &["type", "value", "unit"])
                && integer_text(object.get("value"))
                && same("unit")
        }
        "decimal" => {
            keys_are(object, &["type", "coefficient", "scale", "unit"])
                && integer_text(object.get("coefficient"))
                && same("scale")
                && same("unit")
        }
        "date" => {
            keys_are(object, &["type", "value"])
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|s| fixed_digits(s, "9999-99-99"))
        }
        "timestamp" => {
            keys_are(object, &["type", "value"])
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|s| fixed_digits(s, "9999-99-99T99:99:99Z"))
        }
        "enum" => {
            keys_are(object, &["type", "value"])
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|member| {
                        expected
                            .get("values")
                            .and_then(Value::as_array)
                            .is_some_and(|values| values.iter().any(|v| v.as_str() == Some(member)))
                    })
        }
        "reference" => {
            keys_are(object, &["type", "input_id", "record_id"])
                && object.get("input_id") == expected.get("target_input")
                && object
                    .get("record_id")
                    .and_then(Value::as_str)
                    .is_some_and(valid_id)
        }
        "money" => {
            keys_are(
                object,
                &["type", "minor_units", "currency", "minor_unit_exponent"],
            ) && integer_text(object.get("minor_units"))
                && same("currency")
                && same("minor_unit_exponent")
                && object.get("currency").and_then(Value::as_str) == Some("USD")
                && object.get("minor_unit_exponent").and_then(Value::as_i64) == Some(2)
        }
        _ => false,
    }
}

/// The record's state as stored: a `{field_id, value}` array in schema order.
pub(super) fn state_map(state: Option<&Value>) -> Result<BTreeMap<String, Value>> {
    let mut map = BTreeMap::new();
    for item in state.and_then(Value::as_array).into_iter().flatten() {
        let id = item
            .get("field_id")
            .and_then(Value::as_str)
            .ok_or(Error::Unavailable)?;
        let value = item.get("value").cloned().ok_or(Error::Unavailable)?;
        require_stored(map.insert(id.to_owned(), value).is_none())?;
    }
    Ok(map)
}

/// The new record state: the declaration's writable, type-compatible values
/// over the current state (or nulls for a new record); every non-nullable
/// field present. On an existing record (contract edition impl-2026-10-29.3
/// amendment, APP-013 review M1) the state must change, every changed enum
/// field follows a declared transition of this command or the command is an
/// `update`, and a `create` command must fire one of its declared
/// transitions: a note-only edit or a resubmission of the current values
/// through a `create` is refused, and no command writes a no-op revision.
pub(super) fn compose(
    spec: &[FieldSpec],
    declaration: &Declaration,
    current: Option<&Value>,
    values: &Value,
) -> Result<Vec<u8>> {
    let current = state_map(current)?;
    let creating = current.is_empty();
    let mut supplied = BTreeMap::new();
    for item in values.as_array().ok_or(Error::Denied)? {
        let id = item
            .get("field_id")
            .and_then(Value::as_str)
            .ok_or(Error::Denied)?;
        let value = item.get("value").ok_or(Error::Denied)?;
        let field = spec.iter().find(|f| f.id == id).ok_or(Error::Denied)?;
        require(
            declaration.writable.iter().any(|w| w == id)
                && compatible(value, field)
                && supplied.insert(id.to_owned(), value.clone()).is_none(),
        )?;
    }
    let null = json!({"type": "null"});
    let mut state = Vec::with_capacity(spec.len());
    let mut composed = Map::new();
    let mut moves = Vec::new();
    let mut changed = false;
    for field in spec {
        let value = supplied
            .get(&field.id)
            .or_else(|| current.get(&field.id))
            .cloned()
            .unwrap_or_else(|| null.clone());
        require(field.nullable || value != null)?;
        if !creating {
            let before = current.get(&field.id).unwrap_or(&null);
            if *before != value {
                changed = true;
                let is_enum = field.value_type.get("type").and_then(Value::as_str) == Some("enum");
                if is_enum {
                    let from = before.get("value").and_then(Value::as_str);
                    let to = value.get("value").and_then(Value::as_str);
                    moves.push((
                        field.id.as_str(),
                        from.map(str::to_owned),
                        to.map(str::to_owned),
                    ));
                }
            }
        }
        composed.insert(field.id.clone(), value.clone());
        state.push(json!({"field_id": field.id, "value": value}));
    }
    // APP-107: a declared transition fires only when its guard holds on the
    // record as composed; a create carries the declared initial value of
    // every field the command names one for.
    let mut fired = false;
    for (field, from, to) in moves {
        let declared = declaration
            .transitions
            .iter()
            .any(|(f, a, b)| f == field && Some(a) == from.as_ref() && Some(b) == to.as_ref())
            && match (&from, &to) {
                (Some(from), Some(to)) => declaration.rules.allows(field, from, to, &composed),
                _ => false,
            };
        require(declared || declaration.operation == "update")?;
        fired |= declared;
    }
    if creating {
        for field in declaration.rules.initial_fields() {
            let (value, _) = declaration
                .rules
                .initial_value(field, &composed)
                .ok_or(Error::Denied)?;
            require(composed.get(field) == Some(&json!({"type": "enum", "value": value})))?;
        }
    }
    if !creating {
        require(changed && (fired || declaration.operation == "update"))?;
    }
    let bytes = admission::encode(&Value::Array(state)).map_err(|_| Error::Denied)?;
    require(bytes.len() <= MAX_CANONICAL_BYTES)?;
    Ok(bytes)
}

/// How one command transaction ended, before COMMIT or ROLLBACK is chosen.
#[allow(clippy::large_enum_variant)] // Transient, one per command; the result's shape is D7.9's.
enum Done {
    Committed(CommandOutcome),
    Other(CommandResult),
}

/// A statement that may wait on a row lock: `Ok(None)` is a wait past
/// `lock_timeout` (or a lost deadlock); the transaction is then aborted.
async fn locked(
    transaction: &Transaction<'_>,
    sql: &str,
    params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) -> Result<Option<Vec<Row>>> {
    match transaction.query(sql, params).await {
        Ok(rows) => Ok(Some(rows)),
        Err(Error::Unavailable) if transaction.lock_timed_out() => Ok(None),
        Err(error) => Err(error),
    }
}

impl Data {
    /// Commits one declared command (see the module notes) within the D7.14
    /// handler deadline.
    pub async fn commit_record(
        &self,
        request: &TrustedRequest,
        command: &CommandRequest,
    ) -> Result<CommandResult> {
        self.commit_record_until(request, command, Instant::now() + COMMAND_HANDLER_DEADLINE)
            .await
    }

    /// `commit_record` within an explicit handler deadline.
    pub async fn commit_record_until(
        &self,
        request: &TrustedRequest,
        command: &CommandRequest,
        deadline: Instant,
    ) -> Result<CommandResult> {
        require(command.well_formed())?;
        let payload = payload::digest(command).ok_or(Error::Denied)?;
        let mut transaction = Transaction::begin_until(&self.inner, request, deadline).await?;
        let result = commit(&mut transaction, request, command, &payload).await;
        let commit = matches!(result, Ok(Done::Committed(_)));
        // Anything but a commit rolls back, releasing the claim (D7.9). A lost
        // COMMIT answer is UnknownOutcome from `finish`; the caller re-reads by key.
        transaction.finish(commit).await?;
        match result? {
            Done::Committed(outcome) => Ok(CommandResult::Committed(outcome)),
            Done::Other(other) => Ok(other),
        }
    }

    /// The caller's own outcome under the scoped key, under current
    /// authority (`record_write` on the work, judged now and under the fence):
    /// the client's lookup after a lost response (ADR-002). Another principal's
    /// outcome under the same key is `None`: the scope includes the principal.
    pub async fn read_outcome(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        command: &str,
        key: &str,
    ) -> Result<Option<CommandOutcome>> {
        require(
            valid_id(app) && valid_id(work) && local_id(command) && idempotency_key_shaped(key),
        )?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordWrite, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            Ok(
                stored_outcome(permit.transaction(), request, app, work, command, key)
                    .await?
                    .map(|(outcome, _)| outcome),
            )
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// A record of a work's collection for its page: the published schema's
    /// fields with the current values (version 0 and nulls when the record
    /// does not exist yet) and the commands declared on that schema. Needs
    /// `record_read` on the work (by grant or as its unrevoked creator); the
    /// work fence pins the publication the schema comes from.
    pub async fn read_record_detail(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        collection: &str,
        record: &str,
    ) -> Result<RecordDetail> {
        require(valid_id(app) && valid_id(work) && valid_id(collection) && valid_id(record))?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let org = &request.organization;
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let can_write = permit
                .also(Action::RecordWrite, Resource::Work { app, work })
                .await
                .is_ok();
            let transaction = permit.transaction();
            let pin = work_pin(transaction, org, app, work).await?;
            let (publication, work_version) = (
                pin.publication.clone().ok_or(Error::Unavailable)?,
                pin.work_version.ok_or(Error::Unavailable)?,
            );
            require(work_fence(transaction, org, app, work, &publication, work_version).await?)?;
            let rows = transaction.query("SELECT schema_id::text,schema_version FROM genui.record_collections WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid", &[org,&app,&work,&collection]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let (schema, schema_version) = (text(row, 0)?, small(row, 1)?);
            let published = published(
                transaction,
                org,
                app,
                &publication,
                &schema,
                i64::from(schema_version),
            )
            .await?;
            let (version, state, _) =
                current_state(transaction, org, app, work, collection, record, false).await?;
            let state = state_map(state.as_ref())?;
            let writable: BTreeSet<&str> = published
                .commands
                .values()
                .flat_map(|d| d.writable.iter().map(String::as_str))
                .collect();
            let null = json!({"type": "null"});
            Ok(RecordDetail {
                record_id: record.to_owned(),
                collection: collection.to_owned(),
                schema,
                schema_version,
                publication,
                definition_digest: published.definition_digest,
                work_version,
                version,
                fields: published
                    .fields
                    .iter()
                    .map(|field| RecordField {
                        id: field.id.clone(),
                        value_type: field.value_type.clone(),
                        nullable: field.nullable,
                        value: state.get(&field.id).cloned().unwrap_or_else(|| null.clone()),
                        writable: writable.contains(field.id.as_str()),
                    })
                    .collect(),
                commands: published
                    .commands
                    .iter()
                    .map(|(id, declaration)| RecordCommand {
                        id: id.clone(),
                        operation: declaration.operation.clone(),
                        writable_fields: declaration.writable.clone(),
                        transitions: declaration.transitions.clone(),
                            rules: declaration.rules.clone(),
                    })
                    .collect(),
                can_write,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

async fn commit<'a>(
    transaction: &mut Transaction<'a>,
    request: &'a TrustedRequest,
    command: &'a CommandRequest,
    payload: &str,
) -> Result<Done> {
    let org = request.organization.as_str();
    let (app, work) = (command.app.as_str(), command.work.as_str());
    let precheck = transaction
        .precheck(Action::RecordWrite, Resource::Work { app, work })
        .await?;
    #[cfg(test)]
    transaction.lease.checkpoint("precheck").await;
    // D7.9: the claim, right after the precheck and before the fences; a wait
    // on another transaction's uncommitted claim past lock_timeout is InFlight.
    let Some(claimed) = locked(transaction, "INSERT INTO genui.command_claims(organization_id,principal_id,app_id,work_id,command_id,idempotency_key,payload_digest,claimed_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6::text,$7::text,now()) ON CONFLICT DO NOTHING RETURNING 1",
        &[&org,&request.principal,&app,&work,&command.command,&command.idempotency_key,&payload]).await? else {
        return Ok(Done::Other(CommandResult::InFlight));
    };
    let fresh_claim = claimed.len() == 1;
    #[cfg(test)]
    transaction.lease.checkpoint("outcome").await;
    let fenced = transaction.fence(precheck).await;
    let permit = match fenced {
        Ok(permit) => permit,
        Err(error) => {
            // A 55P03 on the member fence: a conflicting operator or sign-out
            // operation holds the row (D7.9 busy).
            if error == Error::Unavailable && transaction.lock_timed_out() {
                return Ok(Done::Other(CommandResult::Busy));
            }
            return Err(error);
        }
    };
    #[cfg(test)]
    permit.transaction().lease.checkpoint("fence").await;
    let transaction = permit.transaction();
    if !fresh_claim {
        // The key is taken by a committed command of this principal (a claim
        // outlives its transaction only with its outcome). Delivered only now,
        // after the fence re-read current visibility (ADR-004).
        let (stored, live) = stored_outcome(
            transaction,
            request,
            app,
            work,
            &command.command,
            &command.idempotency_key,
        )
        .await?
        .ok_or(Error::Unavailable)?;
        if stored.payload_digest == payload && live {
            return Ok(Done::Other(CommandResult::Replayed(stored)));
        }
        let reason = if stored.payload_digest == payload {
            ConflictReason::RetryExpired
        } else {
            ConflictReason::PayloadMismatch
        };
        let pin = work_pin(transaction, org, app, work).await?;
        let collection = collection(
            transaction,
            org,
            app,
            work,
            &command.schema,
            command.schema_version_expected,
        )
        .await?;
        let (current, values, _) = current_state(
            transaction,
            org,
            app,
            work,
            &collection,
            &command.record,
            false,
        )
        .await?;
        return Ok(Done::Other(CommandResult::Conflict(RecoveryView {
            organization: org.to_owned(),
            app: app.to_owned(),
            work: work.to_owned(),
            publication: pin.publication.unwrap_or_default(),
            work_version: pin.work_version.unwrap_or_default(),
            record_id: command.record.clone(),
            command: command.command.clone(),
            idempotency_key: command.idempotency_key.clone(),
            reason,
            expected_record_version: command.expected_record_version,
            current_record_version: current,
            current_values: values,
            stored_outcome: Some(stored),
        })));
    }
    // The work fence with the caller's premise: a moved publication or
    // version is Stale (412 naming the current revision, D7.9).
    let fenced = match locked(transaction, "SELECT publication_id::text,work_version FROM genui_private.lock_work_revision($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5)",
        &[&org,&app,&work,&command.publication,&command.expected_work_version]).await? {
        Some(rows) => rows.first().map(|row| Ok::<_, Error>((text(row, 0)?, integer(row, 1)?))).transpose()?,
        None => return Ok(Done::Other(CommandResult::Busy)),
    };
    if fenced != Some((command.publication.clone(), command.expected_work_version)) {
        return Ok(Done::Other(CommandResult::Stale(
            work_pin(transaction, org, app, work).await?,
        )));
    }
    // The command against the publication the work pins.
    let published = published(
        transaction,
        org,
        app,
        &command.publication,
        &command.schema,
        command.schema_version_expected,
    )
    .await?;
    let declaration = published
        .commands
        .get(&command.command)
        .ok_or(Error::Denied)?;
    let collection = collection(
        transaction,
        org,
        app,
        work,
        &command.schema,
        command.schema_version_expected,
    )
    .await?;
    // D7.7: the organization's commit sequence, FOR UPDATE after both fences.
    let Some(sequence) = locked(transaction, "SELECT next_commit_sequence::bigint FROM genui.organization_sequences WHERE organization_id=$1::text::uuid FOR UPDATE", &[&org]).await? else {
        return Ok(Done::Other(CommandResult::InFlight));
    };
    let sequence = integer(sequence.first().ok_or(Error::Configuration)?, 0)?;
    require_stored(sequence < SAFE_MAX)?;
    // The record row FOR UPDATE (absent: version 0), then the premise.
    let Some(current) = locked(transaction, "SELECT current_version::bigint FROM genui.internal_records WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid FOR UPDATE",
        &[&org,&app,&work,&collection,&command.record]).await? else {
        return Ok(Done::Other(CommandResult::InFlight));
    };
    #[cfg(test)]
    transaction.lease.checkpoint("record_lock").await;
    let current_version = current
        .first()
        .map(|row| integer(row, 0))
        .transpose()?
        .unwrap_or(0);
    let (_, current_values, creator) = if current_version == 0 {
        (0, None, None)
    } else {
        current_state(
            transaction,
            org,
            app,
            work,
            &collection,
            &command.record,
            false,
        )
        .await?
    };
    if current_version != command.expected_record_version {
        return Ok(Done::Other(CommandResult::Conflict(RecoveryView {
            organization: org.to_owned(),
            app: app.to_owned(),
            work: work.to_owned(),
            publication: command.publication.clone(),
            work_version: command.expected_work_version,
            record_id: command.record.clone(),
            command: command.command.clone(),
            idempotency_key: command.idempotency_key.clone(),
            reason: ConflictReason::ExpectedVersion,
            expected_record_version: command.expected_record_version,
            current_record_version: current_version,
            current_values,
            stored_outcome: None,
        })));
    }
    // A `create` command creates (version 0); an existing record takes an
    // `update` command or a `create` command that declares transitions, and
    // `compose` requires such a `create` to fire one (contract edition
    // impl-2026-10-29.3 amendment; APP-013 review M1).
    require(if current_version == 0 {
        declaration.operation == "create"
    } else {
        declaration.operation == "update" || !declaration.transitions.is_empty()
    })?;
    let state = compose(
        &published.fields,
        declaration,
        current_values.as_ref(),
        &command.values,
    )?;
    let version = current_version + 1;
    require_stored(version <= SAFE_MAX)?;
    let creator = creator.unwrap_or_else(|| request.principal.clone());
    let (_, committed_at) = fresh(transaction, 1).await?;
    transaction.query("INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::bigint,$7::text::uuid,$8::text::uuid,$9::bigint,$10::text::timestamptz,$11)",
        &[&org,&app,&work,&collection,&command.record,&version,&creator,&request.principal,&sequence,&committed_at,&state]).await?;
    if current_version == 0 {
        transaction.query("INSERT INTO genui.internal_records(organization_id,app_id,work_id,collection_id,record_id,current_version) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,1)",
            &[&org,&app,&work,&collection,&command.record]).await?;
    } else {
        let updated = transaction.query("UPDATE genui.internal_records SET current_version=$6::bigint WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid AND current_version=$7::bigint RETURNING 1",
            &[&org,&app,&work,&collection,&command.record,&version,&current_version]).await?;
        require_stored(updated.len() == 1)?;
    }
    // C07: the revision names the source revisions the work binds now.
    transaction.query("INSERT INTO genui.record_source_dependencies(organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id,source_digest) SELECT organization_id,app_id,work_id,$4::text::uuid,$5::text::uuid,$6::bigint,input_id,source_id,source_revision_id,source_digest FROM genui.work_source_dependencies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid",
        &[&org,&app,&work,&collection,&command.record,&version]).await?;
    let event = append(
        transaction,
        Chain::Data,
        request,
        Event {
            kind: COMMAND_COMMITTED,
            id: None,
            app: Some(app),
            work: Some(work),
            record: Some(&command.record),
            outcome: Some(payload),
        },
    )
    .await?;
    let advanced = transaction.query("UPDATE genui.organization_sequences SET next_commit_sequence=$2::bigint WHERE organization_id=$1::text::uuid AND next_commit_sequence=$3::bigint RETURNING 1", &[&org,&(sequence+1),&sequence]).await?;
    require_stored(advanced.len() == 1)?;
    let outcome = transaction.query("INSERT INTO genui.command_outcomes(organization_id,app_id,work_id,principal_id,command_id,idempotency_key,payload_digest,state,record_id,record_version,work_version,commit_sequence,audit_event_id,committed_at,retry_expires_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6::text,$7::text,'committed',$8::text::uuid,$9::bigint,$10::bigint,$11::bigint,$12::text::uuid,$13::text::timestamptz,$13::text::timestamptz+interval '24 hours') RETURNING to_char(retry_expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        &[&org,&app,&work,&request.principal,&command.command,&command.idempotency_key,&payload,&command.record,&version,&command.expected_work_version,&sequence,&event,&committed_at]).await?;
    let retry_expires_at = text(outcome.first().ok_or(Error::Unavailable)?, 0)?;
    Ok(Done::Committed(CommandOutcome {
        organization: org.to_owned(),
        app: app.to_owned(),
        work: work.to_owned(),
        principal: request.principal.clone(),
        command: command.command.clone(),
        idempotency_key: command.idempotency_key.clone(),
        payload_digest: payload.to_owned(),
        record_id: command.record.clone(),
        record_version: version,
        work_version: command.expected_work_version,
        commit_sequence: sequence,
        audit_event_id: event,
        committed_at,
        retry_expires_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(id: &str, value_type: Value, nullable: bool) -> FieldSpec {
        FieldSpec {
            id: id.into(),
            value_type,
            nullable,
        }
    }
    fn decision() -> Vec<FieldSpec> {
        vec![
            field(
                "status",
                json!({"type": "enum", "values": ["needs_context", "reviewed"]}),
                false,
            ),
            field("note", json!({"type": "text"}), false),
        ]
    }
    fn record_decision() -> Declaration {
        Declaration {
            operation: "create".into(),
            writable: vec!["status".into(), "note".into()],
            transitions: vec![("status".into(), "needs_context".into(), "reviewed".into())],
            rules: Default::default(),
        }
    }
    fn values(status: &str, note: &str) -> Value {
        json!([
            {"field_id": "status", "value": {"type": "enum", "value": status}},
            {"field_id": "note", "value": {"type": "text", "value": note}},
        ])
    }

    #[test]
    fn values_are_typed_against_the_published_field_exactly() {
        let spec = decision();
        assert!(compatible(
            &json!({"type": "enum", "value": "reviewed"}),
            &spec[0]
        ));
        assert!(!compatible(
            &json!({"type": "enum", "value": "approved"}),
            &spec[0]
        ));
        assert!(!compatible(
            &json!({"type": "text", "value": "x"}),
            &spec[0]
        ));
        assert!(!compatible(&json!({"type": "null"}), &spec[1]));
        assert!(compatible(
            &json!({"type": "null"}),
            &field("optional", json!({"type": "text"}), true)
        ));
        assert!(!compatible(
            &json!({"type": "enum", "value": "reviewed", "extra": 1}),
            &spec[0]
        ));
        let money = field(
            "amount",
            json!({"type": "money", "currency": "USD", "minor_unit_exponent": 2}),
            false,
        );
        assert!(compatible(
            &json!({"type": "money", "minor_units": "4500000", "currency": "USD", "minor_unit_exponent": 2}),
            &money
        ));
        assert!(!compatible(
            &json!({"type": "money", "minor_units": "045", "currency": "USD", "minor_unit_exponent": 2}),
            &money
        ));
        let bytes = field("bytes", json!({"type": "integer", "unit": "byte"}), false);
        assert!(compatible(
            &json!({"type": "integer", "value": "-12", "unit": "byte"}),
            &bytes
        ));
        assert!(!compatible(
            &json!({"type": "integer", "value": "12", "unit": "bit"}),
            &bytes
        ));
        assert!(!compatible(
            &json!({"type": "integer", "value": 12, "unit": "byte"}),
            &bytes
        ));
        assert!(integer_text(Some(&json!("0"))) && !integer_text(Some(&json!("-0"))));
        assert!(!integer_text(Some(&json!(&"9".repeat(39)))));
        assert!(fixed_digits("2026-10-29T12:00:00Z", "9999-99-99T99:99:99Z"));
        assert!(!fixed_digits(
            "2026-10-29 12:00:00Z",
            "9999-99-99T99:99:99Z"
        ));
    }

    #[test]
    fn a_create_composes_the_whole_record_and_a_transition_governs_the_second_version() {
        let spec = decision();
        let declaration = record_decision();
        let first = compose(
            &spec,
            &declaration,
            None,
            &values("needs_context", "Review required"),
        )
        .unwrap();
        let first_state: Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(first_state, values("needs_context", "Review required"));
        // S05 ik-2: needs_context -> reviewed follows the declared transition.
        let second = compose(
            &spec,
            &declaration,
            Some(&first_state),
            &values("reviewed", "Approved"),
        )
        .unwrap();
        let second_state: Value = serde_json::from_slice(&second).unwrap();
        assert_eq!(second_state, values("reviewed", "Approved"));
        // APP-013 review M1: a create command on an existing record must fire
        // a declared transition. A note-only edit and a resubmission of the
        // current values under a new key are refused, before and after the
        // transition fired.
        let note_only = json!([{"field_id": "note", "value": {"type": "text", "value": "n"}}]);
        for current in [&first_state, &second_state] {
            assert_eq!(
                compose(&spec, &declaration, Some(current), &note_only).err(),
                Some(Error::Denied)
            );
            assert_eq!(
                compose(&spec, &declaration, Some(current), current).err(),
                Some(Error::Denied)
            );
        }
        // An update command edits any writable field, the unchanged status
        // carried from the current state, but never writes a no-op revision.
        let mut edit = record_decision();
        edit.operation = "update".into();
        let third: Value = serde_json::from_slice(
            &compose(&spec, &edit, Some(&second_state), &note_only).unwrap(),
        )
        .unwrap();
        assert_eq!(third, values("reviewed", "n"));
        assert_eq!(
            compose(&spec, &edit, Some(&third), &note_only).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            compose(&spec, &edit, Some(&third), &third).err(),
            Some(Error::Denied)
        );
        // reviewed -> needs_context is not declared on a create command.
        assert_eq!(
            compose(
                &spec,
                &declaration,
                Some(&second_state),
                &values("needs_context", "back")
            )
            .err(),
            Some(Error::Denied)
        );
        let mut update = record_decision();
        update.operation = "update".into();
        update.transitions.clear();
        assert!(compose(
            &spec,
            &update,
            Some(&second_state),
            &values("needs_context", "back")
        )
        .is_ok());
        // A new record needs every non-nullable field; an undeclared or
        // non-writable field and an incompatible value are refused.
        assert_eq!(
            compose(&spec, &declaration, None, &note_only).err(),
            Some(Error::Denied)
        );
        let mut narrow = record_decision();
        narrow.writable = vec!["status".into()];
        assert_eq!(
            compose(&spec, &narrow, None, &values("needs_context", "x")).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            compose(&spec, &declaration, None, &json!([{"field_id": "status", "value": {"type": "enum", "value": "later"}}, {"field_id": "note", "value": {"type": "text", "value": "x"}}])).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            compose(
                &spec,
                &declaration,
                None,
                &json!([{"field_id": "amount", "value": {"type": "text", "value": "x"}}])
            )
            .err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn the_request_shape_is_the_contracts_and_the_key_is_32_hex() {
        let request = CommandRequest {
            app: "00000000-0000-4000-8000-0000000000c9".into(),
            work: "00000000-0000-4000-8000-0000000000cd".into(),
            publication: "00000000-0000-4000-8000-0000000000cc".into(),
            command: "record_decision".into(),
            schema: "decision".into(),
            schema_version_expected: 1,
            record: "00000000-0000-4000-8000-0000000000f0".into(),
            expected_record_version: 0,
            expected_work_version: 1,
            values: values("needs_context", "Review required — synthetic only"),
            idempotency_key: "0123456789abcdef0123456789abcdef".into(),
        };
        assert!(request.well_formed());
        for (label, mutate) in [
            (
                "uppercase key",
                Box::new(|r: &mut CommandRequest| {
                    r.idempotency_key = "0123456789ABCDEF0123456789abcdef".into()
                }) as Box<dyn Fn(&mut CommandRequest)>,
            ),
            (
                "short key",
                Box::new(|r: &mut CommandRequest| r.idempotency_key = "0123456789abcdef".into()),
            ),
            (
                "work version 0",
                Box::new(|r: &mut CommandRequest| r.expected_work_version = 0),
            ),
            (
                "negative record version",
                Box::new(|r: &mut CommandRequest| r.expected_record_version = -1),
            ),
            (
                "command id",
                Box::new(|r: &mut CommandRequest| r.command = "Record".into()),
            ),
            (
                "record id",
                Box::new(|r: &mut CommandRequest| r.record = "not-a-uuid".into()),
            ),
            (
                "empty values",
                Box::new(|r: &mut CommandRequest| r.values = json!([])),
            ),
            (
                "duplicate field",
                Box::new(|r: &mut CommandRequest| {
                    r.values =
                        json!([{"field_id": "a", "value": {}}, {"field_id": "a", "value": {}}])
                }),
            ),
            (
                "extra member",
                Box::new(|r: &mut CommandRequest| {
                    r.values = json!([{"field_id": "a", "value": {}, "x": 1}])
                }),
            ),
        ] {
            let mut changed = request.clone();
            mutate(&mut changed);
            assert!(!changed.well_formed(), "{label}");
        }
        assert!(idempotency_key_shaped("00000000000000000000000000000000"));
        assert!(!idempotency_key_shaped("0000000000000000000000000000000g"));
        assert_eq!(ConflictReason::PayloadMismatch.code(), "payload_mismatch");
    }
}
