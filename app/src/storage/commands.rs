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
//! ADR-020 (edition impl-2026-11-14.2) adds three rules inside that order,
//! after the version premise, on the commit and the dry run alike. The
//! transaction's instant (`now()` to the whole second, the same instant
//! stored as the revision's `committed_at`) stamps every `set_now` field in
//! `compose`; then every `requires` guard is judged on the composed record or,
//! through `via`, on the referenced record's current revision read under
//! `own_rows` (`judge_requires`: unreadable or absent is `Denied`, a failing
//! guard `Refused(RequirementNotMet)`); then a `create` of an absent record on
//! a schema with `one_per_creator` looks for the creator's own entry with the
//! same key (`keyed_entry`, after the sequence lock and the requested row's
//! lock, the matched row `FOR UPDATE`): none creates as before, a match with
//! the composed state is `Refused(Unchanged)`, any other match writes the next
//! revision of the matched record and the outcome names it.
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

/// ADR-020: a typed refusal of a command the committer may run, judged only
/// on records the committer may read, after the member fence (422 with the
/// code alone; nothing is written and the claim is released). A referenced
/// record the committer may not read is never a refusal: it is `Denied`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// C3: a `requires` guard does not hold.
    RequirementNotMet,
    /// C2: a replacing create found the creator's entry already holds
    /// exactly the composed values.
    Unchanged,
}
impl Refusal {
    /// The contract problem code (edition impl-2026-11-14.2, status 422).
    pub fn code(self) -> &'static str {
        match self {
            Self::RequirementNotMet => "requirement_not_met",
            Self::Unchanged => "unchanged",
        }
    }
}

/// ADR-007 D7.9. `Committed`/`Replayed` → 200; `Conflict` → 409 with the
/// recovery view; `Stale` → 412 with the current revision; `InFlight` → 409
/// `in_flight`; `Busy` → 409 `busy` (D7.9's "55P03 on an authorization fence"
/// row, typed here so that no lock wait becomes a 503); ADR-020 `Refused` →
/// 422 with the refusal's code.
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
    Refused(Refusal),
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
    /// ADR-020 (APP-205): the work's records bindings, `(input_id,
    /// collection_id)` in input order, so that a form can resolve a
    /// reference field's `target_input` to the collection it names.
    pub bindings: Vec<(String, String)>,
    /// ADR-020 §17.35 (APP-211): the collection's schema declares
    /// `visibility: "own"` (each member files their own entries), so a chooser
    /// over it names each record's creator. Presentation only; the row rule
    /// is `own_rows`.
    pub own: bool,
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
    /// APP-107: the declared transitions with their guards, and the initial
    /// values (the one parse of `transitions` and `initial`).
    pub(super) rules: super::rules::Rules,
    /// APP-113 review (storage 2): `(field_id, value)` pairs this command
    /// may not create a record at: every declared transition target of
    /// another command of the same record schema, on a field this command
    /// declares no `initial` value for. A filing create therefore never
    /// writes a decision outcome (an expense `submit` cannot file a claim as
    /// `approved`); a declared `initial` (Deal desk's guarded auto-approval)
    /// governs its field instead.
    pub(super) decided: Vec<(String, String)>,
    /// APP-191 (D8): the declaration carries `reviewer_only: true`; only a
    /// reviewer of the work (`Action::RecordReview`) may run it, and it is
    /// shown only to reviewers.
    pub(super) reviewer_only: bool,
    /// ADR-020 C3: the guards the command is accepted under, judged after
    /// `compose` by [`judge_requires`] (a `via` guard on the referenced
    /// record, a `now` literal at the transaction's instant).
    pub(super) requires: Vec<super::rules::Guard>,
    /// ADR-020 C3: the timestamp fields `compose` stamps with the
    /// transaction's instant (admission: none is writable).
    pub(super) set_now: Vec<String>,
}
impl Declaration {
    /// The declared transitions `(field_id, from, to)`, in declaration order.
    pub(super) fn transitions(&self) -> Vec<(String, String, String)> {
        self.rules
            .transitions
            .iter()
            .map(|(field, from, to, _)| (field.clone(), from.clone(), to.clone()))
            .collect()
    }
    pub(super) fn has_transitions(&self) -> bool {
        !self.rules.transitions.is_empty()
    }
}
/// The published definition's record schema `schema` at `version` and the
/// commands declared on it (stored content is admitted content: a shape that
/// does not parse is unavailability).
pub(super) struct Published {
    pub(super) definition_digest: String,
    pub(super) fields: Vec<FieldSpec>,
    pub(super) commands: BTreeMap<String, Declaration>,
    /// ADR-020 C2: the schema's `one_per_creator` key fields, in declaration
    /// order; empty when the schema declares none.
    pub(super) one_per_creator: Vec<String>,
    /// ADR-007 (D7): the schema declares `visibility: "own"`.
    pub(super) own: bool,
}

fn string_list(value: Option<&Value>) -> Result<Vec<String>> {
    value
        .and_then(Value::as_array)
        .ok_or(Error::Unavailable)?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or(Error::Unavailable))
        .collect()
}

/// Sets every command's `decided` (see `Declaration::decided`): the other
/// commands' transition targets on the fields it declares no `initial` for.
fn fence_starts(commands: &mut BTreeMap<String, Declaration>) {
    let targets: Vec<(String, String, String)> = commands
        .iter()
        .flat_map(|(id, declaration)| {
            declaration
                .rules
                .transitions
                .iter()
                .map(move |(field, _, to, _)| (id.clone(), field.clone(), to.clone()))
        })
        .collect();
    for (id, declaration) in commands.iter_mut() {
        let initial: Vec<String> = declaration
            .rules
            .initial_fields()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut decided: Vec<(String, String)> = targets
            .iter()
            .filter(|(owner, field, _)| owner != id && !initial.contains(field))
            .map(|(_, field, to)| (field.clone(), to.clone()))
            .collect();
        decided.sort();
        decided.dedup();
        declaration.decided = decided;
    }
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
    let content: Vec<u8> = row.try_get(2).map_err(|_| Error::Unavailable)?;
    published_content(&content, text(row, 1)?, schema, version)
}

/// [`published`] over the publication's stored definition bytes and digest
/// already read (the canvas batch reads them once per work).
pub(super) fn published_content(
    content: &[u8],
    definition_digest: String,
    schema: &str,
    version: i64,
) -> Result<Published> {
    require_stored(content.len() <= MAX_CANONICAL_BYTES)?;
    let content = admission::decode(content).map_err(|_| Error::Unavailable)?;
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
        commands.insert(
            id.to_owned(),
            Declaration {
                operation: declared
                    .get("operation")
                    .and_then(Value::as_str)
                    .ok_or(Error::Unavailable)?
                    .to_owned(),
                writable: string_list(declared.get("writable_fields"))?,
                rules: super::rules::Rules::from_declaration(declared).ok_or(Error::Unavailable)?,
                decided: Vec::new(),
                reviewer_only: declared.get("reviewer_only").and_then(Value::as_bool) == Some(true),
                requires: super::rules::requirements(declared).ok_or(Error::Unavailable)?,
                set_now: match declared.get("set_now") {
                    None => Vec::new(),
                    some => string_list(some)?,
                },
            },
        );
    }
    fence_starts(&mut commands);
    let one_per_creator = match found.get("one_per_creator") {
        None => Vec::new(),
        some => string_list(some)?,
    };
    // Admitted content: every key is a field of the schema (a key outside it
    // would match nothing and silently append).
    require_stored(
        one_per_creator
            .iter()
            .all(|key| fields.iter().any(|field| &field.id == key)),
    )?;
    Ok(Published {
        definition_digest,
        fields,
        commands,
        one_per_creator,
        own: found.get("visibility").and_then(Value::as_str) == Some("own"),
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
    // APP-191: the row rule as a column. A record of an `own` collection that
    // another member created is `Denied` here, before any value, version or
    // recovery view can reach the detail, commit, recovery or dry-run paths.
    let revision = transaction.query(&format!("SELECT canonical_values,creator_id::text,{} FROM genui.internal_record_revisions v WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid AND version=$6::bigint", super::authorization::own_rows("v", "$2::text::uuid", "$3::text::uuid", super::authorization::SCOPED)), &[&organization,&app,&work,&collection,&record,&version]).await?;
    let revision = revision.first().ok_or(Error::Unavailable)?;
    require(
        revision
            .try_get::<_, bool>(2)
            .map_err(|_| Error::Unavailable)?,
    )?;
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

/// Imported apps retain the same complete-data bounds after edits. Commit holds
/// the organization sequence lock before this check; concurrent adds cannot
/// both consume the final available row. Dry runs use it without writing.
#[allow(clippy::too_many_arguments)]
pub(super) async fn copy_capacity(
    t: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    collection: &str,
    record: &str,
    creating: bool,
    new_bytes: usize,
) -> Result<()> {
    let manifest=t.query("SELECT 1 FROM genui.work_data_copies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work]).await?;
    if manifest.is_empty() {
        return Ok(());
    }
    let rows=t.query(&format!("SELECT count(*)::bigint,coalesce(sum(octet_length(v.canonical_values)),0)::bigint,coalesce(max(CASE WHEN r.collection_id=$4::text::uuid AND r.record_id=$5::text::uuid THEN octet_length(v.canonical_values) ELSE 0 END),0)::bigint FROM genui.internal_records r {} WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid", super::record_query::CURRENT), &[&org,&app,&work,&collection,&record]).await?;
    let row = rows.first().ok_or(Error::Unavailable)?;
    let count: i64 = row.try_get(0).map_err(|_| Error::Unavailable)?;
    let bytes: i64 = row.try_get(1).map_err(|_| Error::Unavailable)?;
    let replaced: i64 = row.try_get(2).map_err(|_| Error::Unavailable)?;
    if count + i64::from(creating) > super::DATA_COPY_ROWS as i64
        || bytes - replaced + new_bytes as i64 > super::DATA_COPY_BYTES as i64
    {
        return Err(Error::ProjectionLimit);
    }
    Ok(())
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
fn keys_are(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|k| object.contains_key(*k))
}
/// The contract's bound on a `multi_enum` type and value (1..32 ids).
pub(crate) const MULTI_ENUM_VALUES: usize = 32;
/// ADR-020 §9: `picked` names declared values only, each once, in the
/// declared order (strictly increasing positions in `declared`).
pub(crate) fn ordered_subset(picked: &[Value], declared: Option<&Value>) -> bool {
    let Some(declared) = declared.and_then(Value::as_array) else {
        return false;
    };
    let mut last: Option<usize> = None;
    picked.iter().all(|value| {
        let Some(id) = value.as_str() else {
            return false;
        };
        let Some(position) = declared.iter().position(|d| d.as_str() == Some(id)) else {
            return false;
        };
        let ordered = last.is_none_or(|previous| previous < position);
        last = Some(position);
        ordered
    })
}
/// The contract rule: a value's tag matches the field's type, its members are
/// exactly the tag's, and the typed extras (unit, scale, currency, enum
/// member, reference target) equal the field's.
pub(super) fn compatible(value: &Value, field: &FieldSpec) -> bool {
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
                    .is_some_and(|s| {
                        s.len() == 10
                            && crate::admission::contract_timestamp(&format!("{s}T00:00:00Z"))
                    })
        }
        "timestamp" => {
            keys_are(object, &["type", "value"])
                && object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(crate::admission::contract_timestamp)
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
        // ADR-020 §9 (APP-208): a non-empty subset of the declared values in
        // declaration order (`contracts/validate.py` `compatible`); any other
        // order, a repeat, an unknown id or an empty array is refused. "None
        // selected" is the null value, judged above.
        "multi_enum" => {
            keys_are(object, &["type", "values"])
                && object
                    .get("values")
                    .and_then(Value::as_array)
                    .is_some_and(|picked| {
                        !picked.is_empty()
                            && picked.len() <= MULTI_ENUM_VALUES
                            && ordered_subset(picked, expected.get("values"))
                    })
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
///
/// APP-113 review (storage 1, 2): `by_filer` is whether the deciding
/// principal created the record; a declared transition on an existing record
/// is never taken by its filer (an approver decides other people's claims,
/// in every path: canvas, Slack, MCP, the command route). A create never
/// starts a field at another command's transition target unless this
/// command declares that field's `initial` value (`Declaration::decided`).
///
/// ADR-020 C3: `now` is the transaction's instant (a contract timestamp);
/// every `set_now` field of the declaration holds it, stamped before the
/// nullable check (so a non-nullable stamped field composes on a create, and
/// an update that changes nothing but a stamp within the same second is a
/// no-op revision, refused like any other).
pub(super) fn compose(
    spec: &[FieldSpec],
    declaration: &Declaration,
    current: Option<&Value>,
    values: &Value,
    by_filer: bool,
    reviewer: bool,
    now: &str,
) -> Result<Vec<u8>> {
    // APP-191 (D8): a `reviewer_only` command is refused for everyone but a
    // reviewer of the work, before any value is judged; `by_filer` still
    // applies to a reviewer deciding their own record.
    require(super::authorization::bypassed() || !declaration.reviewer_only || reviewer)?;
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
    let stamp = json!({"type": "timestamp", "value": now});
    for field in spec {
        let value = if declaration.set_now.contains(&field.id) {
            // Admitted content: a stamped field is a timestamp and is not
            // writable, so `supplied` cannot hold it.
            require_stored(compatible(&stamp, field) && !supplied.contains_key(&field.id))?;
            stamp.clone()
        } else {
            supplied
                .get(&field.id)
                .or_else(|| current.get(&field.id))
                .cloned()
                .unwrap_or_else(|| null.clone())
        };
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
        let declared =
            declaration.rules.transitions.iter().any(|(f, a, b, _)| {
                f == field && Some(a) == from.as_ref() && Some(b) == to.as_ref()
            }) && match (&from, &to) {
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
        for (field, value) in &declaration.decided {
            require(composed.get(field) != Some(&json!({"type": "enum", "value": value})))?;
        }
    }
    if !creating {
        require(changed && (fired || declaration.operation == "update"))?;
        require(!(fired && by_filer))?;
    }
    let bytes = admission::encode(&Value::Array(state)).map_err(|_| Error::Denied)?;
    require(bytes.len() <= MAX_CANONICAL_BYTES)?;
    Ok(bytes)
}

/// ADR-020 §3: the transaction's instant, `now()` truncated to the whole
/// second as a contract timestamp: the same instant `definitions::fresh`
/// stores as the revision's `committed_at` (PostgreSQL fixes `now()` for the
/// whole transaction), so every guard, every stamp and the stored commit time
/// agree and a judgment can be re-derived from stored data.
pub(super) async fn transaction_instant(transaction: &Transaction<'_>) -> Result<String> {
    let (_, now) = fresh(transaction, 1).await?;
    require_stored(admission::contract_timestamp(&now))?;
    Ok(now)
}

/// The composed record (`compose`'s canonical bytes) as a field map.
pub(super) fn composed_state(bytes: &[u8]) -> Result<Map<String, Value>> {
    let state = admission::decode(bytes).map_err(|_| Error::Unavailable)?;
    Ok(super::rules::state_of(&state))
}

/// A reference value's `(input_id, record_id)`; `None` for null or any
/// other value (a null reference satisfies no guard).
fn reference_of(value: Option<&Value>) -> Option<(String, String)> {
    let value = value?;
    if value.get("type").and_then(Value::as_str) != Some("reference") {
        return None;
    }
    Some((
        value.get("input_id")?.as_str()?.to_owned(),
        value.get("record_id")?.as_str()?.to_owned(),
    ))
}

/// ADR-020 C3: the current state of record `record` of the collection the
/// work binds records input `input` to, as the committer may read it:
/// `current_state` without a lock, which carries `own_rows`. An input bound
/// to nothing (or to a source), an absent record and a row the committer may
/// not read are all `Denied`, the uniform 404, so a guard never discloses
/// another member's row. The row is not locked: every commit of the
/// organization already queued on its commit sequence, so the read sees each
/// commit that reached the sequence before this one, and none after.
async fn referenced_state(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    input: &str,
    record: &str,
) -> Result<Map<String, Value>> {
    require(local_id(input) && valid_id(record))?;
    let rows = transaction.query("SELECT collection_id::text FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND input_id=$4::text AND kind='records' AND revision_policy='committed_snapshot'", &[&organization,&app,&work,&input]).await?;
    let collection = text(rows.first().ok_or(Error::Denied)?, 0)?;
    let (version, state, _) = current_state(
        transaction,
        organization,
        app,
        work,
        &collection,
        record,
        false,
    )
    .await?;
    require(version > 0)?;
    Ok(super::rules::state_of(&state.ok_or(Error::Denied)?))
}

/// ADR-020 C3: whether every `requires` guard of `declaration` holds on the
/// composed record at `now`: a guard without `via` on `composed`; one with
/// `via` on the current revision of the record its reference names
/// ([`referenced_state`], read once per record). A referenced record the
/// committer may not read is `Denied` whatever the other guards say; `false`
/// is the typed refusal `RequirementNotMet`.
pub(super) async fn judge_requires(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    declaration: &Declaration,
    composed: &Map<String, Value>,
    now: &str,
) -> Result<bool> {
    let mut held = true;
    let mut read: Vec<((String, String), Map<String, Value>)> = Vec::new();
    for guard in &declaration.requires {
        let Some(via) = &guard.via else {
            held &= guard.holds_at(composed, now);
            continue;
        };
        let Some(target) = reference_of(composed.get(via)) else {
            held = false;
            continue;
        };
        let index = match read.iter().position(|(key, _)| *key == target) {
            Some(index) => index,
            None => {
                let state =
                    referenced_state(transaction, organization, app, work, &target.0, &target.1)
                        .await?;
                read.push((target, state));
                read.len() - 1
            }
        };
        held &= guard.holds_at(&read[index].1, now);
    }
    Ok(held)
}

/// ADR-020 C2: the forma-json-v1 array of the key's `{field_id, value}`
/// pairs, taken from the composed record (so `initial` and `set_now` values
/// take part like any other, and a null key value is a key).
pub(super) fn key_document(keys: &[String], composed: &Map<String, Value>) -> Result<String> {
    let pairs = keys
        .iter()
        .map(|key| {
            let value = composed.get(key).ok_or(Error::Unavailable)?;
            // Set equality is not a key (admission refuses a multi_enum key).
            require_stored(value.get("type").and_then(Value::as_str) != Some("multi_enum"))?;
            Ok(json!({"field_id": key, "value": value}))
        })
        .collect::<Result<Vec<_>>>()?;
    let bytes = admission::encode(&Value::Array(pairs)).map_err(|_| Error::Unavailable)?;
    String::from_utf8(bytes).map_err(|_| Error::Unavailable)
}

/// The creator's existing entry a keyed create would replace.
#[derive(Debug)]
pub(super) struct KeyedEntry {
    pub(super) record: String,
    pub(super) version: i64,
    pub(super) values: Vec<u8>,
    pub(super) creator: String,
}

/// ADR-020 C2, test builds: how long each key-match statement took (the
/// participation campaign records the bound at 25 creators × 50 keys).
#[cfg(test)]
pub(crate) static KEY_MATCH_MICROS: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

/// ADR-020 C2: the committer's own current entry in `collection` whose
/// values contain `key` (jsonb containment of the `{field_id, value}` pairs:
/// exact for every scalar type), other than `requested`, newest first. The
/// statement reads only the committer's rows (`creator_id` is the scoped
/// principal) and carries `own_rows` like every revision read. With `lock`
/// (the commit, after the organization's sequence lock and the requested
/// row's, in the D7.9 order) the matched row is taken `FOR UPDATE`: `Ok(None)`
/// is then a wait past `lock_timeout`. The dry run reads without a lock.
#[allow(clippy::too_many_arguments)]
pub(super) async fn keyed_entry(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    work: &str,
    collection: &str,
    requested: &str,
    key: &str,
    lock: bool,
) -> Result<Option<Option<KeyedEntry>>> {
    use super::authorization::{bypassed, own_rows, SCOPED};
    // The creator rule is part of the enforcement the campaigns' bypass
    // disables (ADR-020 §14.1): with it off the match reads other creators'
    // rows, and the commit refuses to write over one (`require_stored`).
    let mine = if bypassed() {
        "TRUE".to_owned()
    } else {
        format!("coalesce(v.creator_id={SCOPED},false)")
    };
    let locking = if lock { " FOR UPDATE OF r" } else { "" };
    #[cfg(test)]
    let started = std::time::Instant::now();
    let rows = locked(transaction, &format!("SELECT r.record_id::text,r.current_version::bigint,v.canonical_values,v.creator_id::text FROM genui.internal_records r {current} WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND r.record_id<>$5::text::uuid AND {mine} AND {own} AND convert_from(v.canonical_values,'UTF8')::jsonb @> $6::text::jsonb ORDER BY v.commit_sequence DESC,r.record_id LIMIT 1{locking}", current = super::record_query::CURRENT, own = own_rows("v", "$2::text::uuid", "$3::text::uuid", SCOPED)), &[&organization,&app,&work,&collection,&requested,&key]).await?;
    #[cfg(test)]
    if let Ok(mut timings) = KEY_MATCH_MICROS.lock() {
        timings.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
    }
    let Some(rows) = rows else {
        return Ok(None);
    };
    let Some(row) = rows.first() else {
        return Ok(Some(None));
    };
    let values: Vec<u8> = row.try_get(2).map_err(|_| Error::Unavailable)?;
    require_stored(values.len() <= MAX_CANONICAL_BYTES)?;
    Ok(Some(Some(KeyedEntry {
        record: text(row, 0)?,
        version: integer(row, 1)?,
        values,
        creator: text(row, 3)?,
    })))
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
            let reviewer = permit
                .also(Action::RecordReview, Resource::Work { app, work })
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
            let bindings = transaction.query("SELECT input_id::text,collection_id::text FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND kind='records' AND revision_policy='committed_snapshot' ORDER BY input_id", &[org,&app,&work]).await?
                .iter()
                .map(|row| Ok((text(row, 0)?, text(row, 1)?)))
                .collect::<Result<Vec<_>>>()?;
            // APP-191 (D8), presentation only: a `reviewer_only` command is
            // shown to reviewers; `compose` is the judgment.
            let shown = |declaration: &&Declaration| !declaration.reviewer_only || reviewer;
            let writable: BTreeSet<&str> = published
                .commands
                .values()
                .filter(shown)
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
                    .filter(|(_, declaration)| shown(declaration))
                    .map(|(id, declaration)| RecordCommand {
                        id: id.clone(),
                        operation: declaration.operation.clone(),
                        writable_fields: declaration.writable.clone(),
                        transitions: declaration.transitions(),
                        rules: declaration.rules.clone(),
                    })
                    .collect(),
                can_write,
                bindings,
                own: published.own,
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
    // APP-191 (D8): whether the committer is a reviewer of the work, re-read
    // after the fence; `compose` refuses a `reviewer_only` command otherwise.
    let reviewer = permit
        .also(Action::RecordReview, Resource::Work { app, work })
        .await
        .is_ok();
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
        declaration.operation == "update" || declaration.has_transitions()
    })?;
    // ADR-020 §3: the transaction's instant, which every `set_now` stamp and
    // every `now` guard use and the revision stores as `committed_at`.
    let committed_at = transaction_instant(transaction).await?;
    let state = compose(
        &published.fields,
        declaration,
        current_values.as_ref(),
        &command.values,
        creator.as_deref() == Some(request.principal.as_str()),
        reviewer,
        &committed_at,
    )?;
    // ADR-020 C3: the guards on the composed record and, through `via`, on
    // the referenced record as the committer may read it (Denied otherwise).
    let composed = composed_state(&state)?;
    if !judge_requires(
        transaction,
        org,
        app,
        work,
        declaration,
        &composed,
        &committed_at,
    )
    .await?
    {
        return Ok(Done::Other(CommandResult::Refused(
            Refusal::RequirementNotMet,
        )));
    }
    // The record the revision is written to: the requested one, or (ADR-020
    // C2) the creator's own entry with the same key, which a create of an
    // absent record on a keyed schema replaces.
    let (mut target, mut base, mut creator) = (command.record.clone(), current_version, creator);
    if current_version == 0
        && declaration.operation == "create"
        && !published.one_per_creator.is_empty()
    {
        let key = key_document(&published.one_per_creator, &composed)?;
        let Some(found) = keyed_entry(
            transaction,
            org,
            app,
            work,
            &collection,
            &command.record,
            &key,
            true,
        )
        .await?
        else {
            return Ok(Done::Other(CommandResult::InFlight));
        };
        #[cfg(test)]
        transaction.lease.checkpoint("key_match").await;
        if let Some(entry) = found {
            if entry.values == state {
                return Ok(Done::Other(CommandResult::Refused(Refusal::Unchanged)));
            }
            // The statement reads only the committer's rows; anything else
            // is a broken store, never a write over another member's entry.
            require_stored(entry.creator == request.principal && entry.version > 0)?;
            target = entry.record;
            base = entry.version;
            creator = Some(entry.creator);
        }
    }
    copy_capacity(
        transaction,
        org,
        app,
        work,
        &collection,
        &target,
        base == 0,
        state.len(),
    )
    .await?;
    let version = base + 1;
    require_stored(version <= SAFE_MAX)?;
    let creator = creator.unwrap_or_else(|| request.principal.clone());
    transaction.query("INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::bigint,$7::text::uuid,$8::text::uuid,$9::bigint,$10::text::timestamptz,$11)",
        &[&org,&app,&work,&collection,&target,&version,&creator,&request.principal,&sequence,&committed_at,&state]).await?;
    if base == 0 {
        transaction.query("INSERT INTO genui.internal_records(organization_id,app_id,work_id,collection_id,record_id,current_version) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,1)",
            &[&org,&app,&work,&collection,&target]).await?;
    } else {
        let updated = transaction.query("UPDATE genui.internal_records SET current_version=$6::bigint WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND collection_id=$4::text::uuid AND record_id=$5::text::uuid AND current_version=$7::bigint RETURNING 1",
            &[&org,&app,&work,&collection,&target,&version,&base]).await?;
        require_stored(updated.len() == 1)?;
    }
    // C07: the revision names the source revisions the work binds now.
    transaction.query("INSERT INTO genui.record_source_dependencies(organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id,source_digest) SELECT organization_id,app_id,work_id,$4::text::uuid,$5::text::uuid,$6::bigint,input_id,source_id,source_revision_id,source_digest FROM genui.work_source_dependencies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid",
        &[&org,&app,&work,&collection,&target,&version]).await?;
    let event = append(
        transaction,
        Chain::Data,
        request,
        Event {
            kind: COMMAND_COMMITTED,
            id: None,
            app: Some(app),
            work: Some(work),
            record: Some(&target),
            outcome: Some(payload),
        },
    )
    .await?;
    let advanced = transaction.query("UPDATE genui.organization_sequences SET next_commit_sequence=$2::bigint WHERE organization_id=$1::text::uuid AND next_commit_sequence=$3::bigint RETURNING 1", &[&org,&(sequence+1),&sequence]).await?;
    require_stored(advanced.len() == 1)?;
    let outcome = transaction.query("INSERT INTO genui.command_outcomes(organization_id,app_id,work_id,principal_id,command_id,idempotency_key,payload_digest,state,record_id,record_version,work_version,commit_sequence,audit_event_id,committed_at,retry_expires_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6::text,$7::text,'committed',$8::text::uuid,$9::bigint,$10::bigint,$11::bigint,$12::text::uuid,$13::text::timestamptz,$13::text::timestamptz+interval '24 hours') RETURNING to_char(retry_expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        &[&org,&app,&work,&request.principal,&command.command,&command.idempotency_key,&payload,&target,&version,&command.expected_work_version,&sequence,&event,&committed_at]).await?;
    let retry_expires_at = text(outcome.first().ok_or(Error::Unavailable)?, 0)?;
    Ok(Done::Committed(CommandOutcome {
        organization: org.to_owned(),
        app: app.to_owned(),
        work: work.to_owned(),
        principal: request.principal.clone(),
        command: command.command.clone(),
        idempotency_key: command.idempotency_key.clone(),
        payload_digest: payload.to_owned(),
        record_id: target,
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
    impl Declaration {
        /// This declaration as an `update` command (tests).
        fn clone_for_update(&self) -> Declaration {
            Declaration {
                operation: "update".into(),
                writable: self.writable.clone(),
                rules: self.rules.clone(),
                decided: self.decided.clone(),
                reviewer_only: self.reviewer_only,
                requires: self.requires.clone(),
                set_now: self.set_now.clone(),
            }
        }
    }
    fn record_decision() -> Declaration {
        Declaration {
            operation: "create".into(),
            writable: vec!["status".into(), "note".into()],
            rules: super::super::rules::Rules {
                transitions: vec![(
                    "status".into(),
                    "needs_context".into(),
                    "reviewed".into(),
                    None,
                )],
                initial: Vec::new(),
            },
            decided: Vec::new(),
            reviewer_only: false,
            requires: Vec::new(),
            set_now: Vec::new(),
        }
    }
    /// The instant the unit tests compose at.
    const NOW: &str = "2026-11-29T15:00:00Z";
    /// `compose` as a decider who did not file the record.
    fn compose(
        spec: &[FieldSpec],
        declaration: &Declaration,
        current: Option<&Value>,
        values: &Value,
    ) -> Result<Vec<u8>> {
        super::compose(spec, declaration, current, values, false, true, NOW)
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
        assert!(!compatible(
            &json!({"type": "money", "minor_units": "12.50", "currency": "USD", "minor_unit_exponent": 2}),
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
    }

    /// ADR-020 §9 (APP-208): a stored multi-select is a non-empty subset of
    /// the declared values in declaration order; any other order, a repeat,
    /// an unknown id, an empty array, a non-string or an extra member is
    /// refused, and "none selected" is the null value (so a required
    /// multi-select is a non-nullable field).
    #[test]
    fn multi_select_values_are_an_ordered_subset_of_the_declared_values() {
        let declared = json!({"type": "multi_enum", "values": ["red", "green", "blue"]});
        let tags = field("tags", declared.clone(), false);
        let optional = field("tags", declared, true);
        let picked = |values: Value| json!({"type": "multi_enum", "values": values});
        for accepted in [
            json!(["red"]),
            json!(["blue"]),
            json!(["red", "blue"]),
            json!(["green", "blue"]),
            json!(["red", "green", "blue"]),
        ] {
            assert!(compatible(&picked(accepted.clone()), &tags), "{accepted}");
        }
        for refused in [
            json!(["blue", "red"]),
            json!(["red", "red"]),
            json!(["red", "purple"]),
            json!(["purple"]),
            json!([]),
            json!(["red", 1]),
            json!("red"),
            json!(null),
        ] {
            assert!(!compatible(&picked(refused.clone()), &tags), "{refused}");
        }
        assert!(!compatible(
            &json!({"type": "multi_enum", "values": ["red"], "value": "red"}),
            &tags
        ));
        assert!(!compatible(&json!({"type": "multi_enum"}), &tags));
        assert!(!compatible(&json!({"type": "enum", "value": "red"}), &tags));
        assert!(!compatible(
            &picked(json!(["red"])),
            &field(
                "tag",
                json!({"type": "enum", "values": ["red", "green", "blue"]}),
                false
            )
        ));
        assert!(!compatible(&json!({"type": "null"}), &tags));
        assert!(compatible(&json!({"type": "null"}), &optional));
    }

    #[test]
    fn date_and_timestamp_values_require_real_calendar_days_and_utc_clock_times() {
        let date = field("date", json!({"type":"date"}), false);
        for valid in [
            "0001-01-01",
            "2024-02-29",
            "2000-02-29",
            "2026-11-02",
            "9999-12-31",
        ] {
            assert!(
                compatible(&json!({"type":"date","value":valid}), &date),
                "refused {valid}"
            );
        }
        for invalid in [
            "2026-02-30",
            "2026-02-29",
            "1900-02-29",
            "2026-04-31",
            "0000-01-01",
            "2026-13-01",
            "2026-01-00",
            "2026-1-01",
            " 2026-11-02",
            "2026-11-02T00:00:00Z",
        ] {
            assert!(
                !compatible(&json!({"type":"date","value":invalid}), &date),
                "accepted {invalid}"
            );
        }
        let timestamp = field("at", json!({"type":"timestamp"}), false);
        for valid in ["2024-02-29T23:59:59Z", "2026-11-02T00:00:00Z"] {
            assert!(
                compatible(&json!({"type":"timestamp","value":valid}), &timestamp),
                "refused {valid}"
            );
        }
        for invalid in [
            "2026-02-30T12:00:00Z",
            "2026-11-02T24:00:00Z",
            "2026-11-02T12:60:00Z",
            "2026-11-02T12:00:60Z",
            "2026-11-02 12:00:00Z",
            "2026-11-02T12:00:00+00:00",
        ] {
            assert!(
                !compatible(&json!({"type":"timestamp","value":invalid}), &timestamp),
                "accepted {invalid}"
            );
        }
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
        update.rules.transitions.clear();
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

    /// APP-113 review (storage 1): the filer never takes a declared
    /// transition on their own record, in the dry run and the commit alike
    /// (both call `compose`); an `update` edit by the filer and a decision by
    /// anyone else are unchanged.
    #[test]
    fn the_filer_cannot_decide_their_own_record() {
        let spec = decision();
        let declaration = record_decision();
        let first: Value = serde_json::from_slice(
            &super::compose(
                &spec,
                &declaration,
                None,
                &values("needs_context", "x"),
                true,
                true,
                NOW,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            super::compose(
                &spec,
                &declaration,
                Some(&first),
                &values("reviewed", "mine"),
                true,
                true,
                NOW
            )
            .err(),
            Some(Error::Denied)
        );
        assert!(super::compose(
            &spec,
            &declaration,
            Some(&first),
            &values("reviewed", "theirs"),
            false,
            true,
            NOW
        )
        .is_ok());
        let mut edit = record_decision();
        edit.operation = "update".into();
        edit.rules.transitions.clear();
        let note_only = json!([{"field_id": "note", "value": {"type": "text", "value": "n"}}]);
        assert!(super::compose(&spec, &edit, Some(&first), &note_only, true, true, NOW).is_ok());
    }

    /// APP-191 (D8): a `reviewer_only` command is refused for a contributor
    /// before any value is judged, even on their own record and even for a
    /// plain update; a reviewer runs it; `by_filer` still refuses a reviewer
    /// deciding their own record; a shared command is unchanged for both.
    #[test]
    fn reviewer_only_commands_are_refused_for_contributors() {
        let spec = decision();
        let filed: Value = serde_json::from_slice(
            &super::compose(
                &spec,
                &record_decision(),
                None,
                &values("needs_context", "x"),
                true,
                false,
                NOW,
            )
            .unwrap(),
        )
        .unwrap();
        let mut approve = record_decision();
        approve.reviewer_only = true;
        let decide = values("reviewed", "ok");
        // A contributor deciding another's record: the gate, not the values.
        assert_eq!(
            super::compose(&spec, &approve, Some(&filed), &decide, false, false, NOW).err(),
            Some(Error::Denied)
        );
        // A contributor creating through a reviewer-only create: refused too.
        assert_eq!(
            super::compose(
                &spec,
                &approve,
                None,
                &values("needs_context", "x"),
                true,
                false,
                NOW
            )
            .err(),
            Some(Error::Denied)
        );
        // A reviewer deciding another's record: allowed.
        assert!(super::compose(&spec, &approve, Some(&filed), &decide, false, true, NOW).is_ok());
        // A reviewer deciding their own record: `by_filer` still refuses.
        assert_eq!(
            super::compose(&spec, &approve, Some(&filed), &decide, true, true, NOW).err(),
            Some(Error::Denied)
        );
        // A reviewer-only update on the filer's own record: refused for the
        // filer as a contributor, allowed for a reviewer.
        let mut edit = approve.clone_for_update();
        edit.rules.transitions.clear();
        let note_only = json!([{"field_id": "note", "value": {"type": "text", "value": "n"}}]);
        assert_eq!(
            super::compose(&spec, &edit, Some(&filed), &note_only, true, false, NOW).err(),
            Some(Error::Denied)
        );
        assert!(super::compose(&spec, &edit, Some(&filed), &note_only, true, true, NOW).is_ok());
        // A shared command is unchanged for a contributor.
        let shared = record_decision();
        assert!(super::compose(&spec, &shared, Some(&filed), &decide, false, false, NOW).is_ok());
    }

    /// APP-113 review (storage 2): a filing create without a declared
    /// `initial` never starts at another command's transition target (an
    /// expense `submit` cannot file a claim as `approved`); a declared
    /// `initial` governs its field instead (Deal desk's guarded approval).
    #[test]
    fn a_filing_create_never_starts_at_a_decision_outcome() {
        let spec = vec![
            field(
                "status",
                json!({"type": "enum", "values": ["submitted", "approved", "rejected"]}),
                false,
            ),
            field("note", json!({"type": "text"}), false),
        ];
        let submit = Declaration {
            operation: "create".into(),
            writable: vec!["status".into(), "note".into()],
            rules: Default::default(),
            decided: vec![
                ("status".into(), "approved".into()),
                ("status".into(), "rejected".into()),
            ],
            reviewer_only: false,
            requires: Vec::new(),
            set_now: Vec::new(),
        };
        let filed = |status: &str| {
            json!([
                {"field_id": "status", "value": {"type": "enum", "value": status}},
                {"field_id": "note", "value": {"type": "text", "value": "n"}},
            ])
        };
        assert!(compose(&spec, &submit, None, &filed("submitted")).is_ok());
        for outcome in ["approved", "rejected"] {
            assert_eq!(
                compose(&spec, &submit, None, &filed(outcome)).err(),
                Some(Error::Denied),
                "{outcome}"
            );
        }
        let auto = Declaration {
            rules: super::super::rules::Rules::from_declaration(&json!({"transitions": [], "initial": [
                {"field_id": "status", "value": "approved", "when": {"field_id": "note", "comparison": "eq", "value": {"type": "text", "value": "small"}}},
                {"field_id": "status", "value": "submitted"},
            ]}))
            .unwrap(),
            decided: Vec::new(),
            ..submit
        };
        let small = json!([
            {"field_id": "status", "value": {"type": "enum", "value": "approved"}},
            {"field_id": "note", "value": {"type": "text", "value": "small"}},
        ]);
        assert!(compose(&spec, &auto, None, &small).is_ok());
    }

    fn items() -> Vec<FieldSpec> {
        vec![
            field("prompt", json!({"type": "text"}), false),
            field(
                "state",
                json!({"type": "enum", "values": ["open", "closed"]}),
                false,
            ),
            field("opened_at", json!({"type": "timestamp"}), false),
        ]
    }
    fn item_command(operation: &str, writable: &[&str], set_now: &[&str]) -> Declaration {
        Declaration {
            operation: operation.into(),
            writable: writable.iter().map(|w| (*w).to_owned()).collect(),
            rules: Default::default(),
            decided: Vec::new(),
            reviewer_only: true,
            requires: Vec::new(),
            set_now: set_now.iter().map(|w| (*w).to_owned()).collect(),
        }
    }
    fn stamped(bytes: &[u8]) -> Value {
        composed_state(bytes).unwrap()["opened_at"].clone()
    }

    /// ADR-020 C3: `set_now` stamps the transaction's instant before the
    /// nullable check (a non-nullable stamped field composes on a create), a
    /// stamped field cannot be supplied, an update re-stamps, a stamp that
    /// changes nothing within the same second is a refused no-op, and a
    /// command without `set_now` leaves the stamp alone.
    #[test]
    fn set_now_stamps_the_instant_and_never_takes_a_supplied_value() {
        let spec = items();
        let post = item_command("create", &["prompt", "state"], &["opened_at"]);
        let open = json!([
            {"field_id": "prompt", "value": {"type": "text", "value": "Item one"}},
            {"field_id": "state", "value": {"type": "enum", "value": "open"}},
        ]);
        let posted = super::compose(&spec, &post, None, &open, true, true, NOW).unwrap();
        assert_eq!(stamped(&posted), json!({"type": "timestamp", "value": NOW}));
        // Without the stamp the non-nullable field refuses the create.
        let unstamped = item_command("create", &["prompt", "state"], &[]);
        assert_eq!(
            super::compose(&spec, &unstamped, None, &open, true, true, NOW).err(),
            Some(Error::Denied)
        );
        // A supplied stamp is a value for a field the command does not write.
        let mut forged = open.as_array().unwrap().clone();
        forged.push(json!({"field_id": "opened_at", "value": {"type": "timestamp", "value": "2030-01-01T00:00:00Z"}}));
        assert_eq!(
            super::compose(&spec, &post, None, &Value::Array(forged), true, true, NOW).err(),
            Some(Error::Denied)
        );
        // Stored content that would let a client write a stamped field is
        // not admitted content: unavailability, never the client's value.
        let both = item_command("create", &["prompt", "state", "opened_at"], &["opened_at"]);
        let mut with_stamp = open.as_array().unwrap().clone();
        with_stamp
            .push(json!({"field_id": "opened_at", "value": {"type": "timestamp", "value": NOW}}));
        assert_eq!(
            super::compose(
                &spec,
                &both,
                None,
                &Value::Array(with_stamp),
                true,
                true,
                NOW
            )
            .err(),
            Some(Error::Unavailable)
        );
        let posted: Value = serde_json::from_slice(&posted).unwrap();
        // close: no stamp, the opened_at carried.
        let close = item_command("update", &["state"], &[]);
        let closed_state =
            json!([{"field_id": "state", "value": {"type": "enum", "value": "closed"}}]);
        let later = "2026-11-29T15:00:40Z";
        let closed = super::compose(
            &spec,
            &close,
            Some(&posted),
            &closed_state,
            true,
            true,
            later,
        )
        .unwrap();
        assert_eq!(stamped(&closed), json!({"type": "timestamp", "value": NOW}));
        // reopen: re-stamps; the same second with the same state is a no-op.
        let reopen = item_command("update", &["state"], &["opened_at"]);
        let open_state = json!([{"field_id": "state", "value": {"type": "enum", "value": "open"}}]);
        assert_eq!(
            super::compose(&spec, &reopen, Some(&posted), &open_state, true, true, NOW).err(),
            Some(Error::Denied)
        );
        let reopened = super::compose(
            &spec,
            &reopen,
            Some(&posted),
            &open_state,
            true,
            true,
            later,
        )
        .unwrap();
        assert_eq!(
            stamped(&reopened),
            json!({"type": "timestamp", "value": later})
        );
    }

    /// ADR-020 C2: the key document is the forma-json-v1 array of the key
    /// pairs in declaration order, from the composed state; a null key value
    /// is a key; a multi_enum value is never one.
    #[test]
    fn the_key_is_the_composed_pairs_in_declaration_order() {
        let composed = super::super::rules::state_of(&json!([
            {"field_id": "item", "value": {"type": "reference", "input_id": "items", "record_id": "00000000-0000-4000-8000-0000000000f1"}},
            {"field_id": "choice", "value": {"type": "enum", "value": "b"}},
            {"field_id": "day", "value": {"type": "null"}},
        ]));
        assert_eq!(
            key_document(&["item".into()], &composed).unwrap(),
            r#"[{"field_id":"item","value":{"input_id":"items","record_id":"00000000-0000-4000-8000-0000000000f1","type":"reference"}}]"#
        );
        assert_eq!(
            key_document(&["day".into(), "choice".into()], &composed).unwrap(),
            r#"[{"field_id":"day","value":{"type":"null"}},{"field_id":"choice","value":{"type":"enum","value":"b"}}]"#
        );
        assert_eq!(
            key_document(&["absent".into()], &composed).err(),
            Some(Error::Unavailable)
        );
        let multi = super::super::rules::state_of(
            &json!([{"field_id": "tags", "value": {"type": "multi_enum", "values": ["a"]}}]),
        );
        assert_eq!(
            key_document(&["tags".into()], &multi).err(),
            Some(Error::Unavailable)
        );
        assert_eq!(Refusal::RequirementNotMet.code(), "requirement_not_met");
        assert_eq!(Refusal::Unchanged.code(), "unchanged");
    }

    #[test]
    fn decided_starts_are_the_other_commands_targets_without_an_initial() {
        let rules = |value: Value| super::super::rules::Rules::from_declaration(&value).unwrap();
        let declaration = |rules| Declaration {
            operation: "create".into(),
            writable: vec!["status".into()],
            rules,
            decided: Vec::new(),
            reviewer_only: false,
            requires: Vec::new(),
            set_now: Vec::new(),
        };
        let mut commands = BTreeMap::from([
            (
                "submit".to_owned(),
                declaration(rules(json!({"transitions": []}))),
            ),
            (
                "decide".to_owned(),
                declaration(rules(json!({"transitions": [
                    {"field_id": "status", "from": "submitted", "to": "approved"},
                    {"field_id": "status", "from": "submitted", "to": "rejected"},
                ]}))),
            ),
            (
                "auto".to_owned(),
                declaration(rules(
                    json!({"transitions": [], "initial": [{"field_id": "status", "value": "approved"}]}),
                )),
            ),
        ]);
        fence_starts(&mut commands);
        let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert_eq!(
            commands["submit"].decided,
            [pair("status", "approved"), pair("status", "rejected")]
        );
        assert!(commands["decide"].decided.is_empty());
        assert!(commands["auto"].decided.is_empty());
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
