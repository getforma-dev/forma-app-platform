//! APP-010 typed definitions, publications and saved work (design section 6.2;
//! C01, C02, C04; ADR-004; ADR-007 D7.3, D7.6, D7.8, D7.9).
//!
//! Each operation is one transaction on its role's pool, in ADR-004 order:
//! the member precheck, the `genui_lock` fence, then the writes and one or
//! more audit events on the role's own chain (D7.8), committed together. The
//! workspace drafts definition revisions and saves work; only the control role
//! publishes, in a SERIALIZABLE transaction retried once on SQLSTATE 40001
//! within the D7.14 handler deadline (D7.6, A1). Each publication of an app
//! takes the next `publication_sequence` under the control-head lock, so the
//! newest publication is unambiguous, and its `publication.published` event
//! carries the publication id as its event id (ADR-007 amendment of
//! 2026-10-29, APP-010 review M1 and m6). A saved work records its creator,
//! whose authority on that work stands in for `record_read`/`record_write`
//! grants until the operator revokes it (review M2). Nothing a caller supplies
//! is trusted as authority or as
//! admitted content: every definition is re-admitted inside storage, its
//! organization and app come from the trusted request and the path, its input
//! contracts and record schemas are derived from the admitted bytes, and a
//! saved work's concrete sources are bound under the binder's own
//! `source_read` (design TH15 M30). A publication carries input contracts and
//! record schemas, never a source, revision or collection: those belong only
//! to saved work (C02). Readers return canonical forma-json-v1 bytes bounded
//! at 262,144 bytes.
use super::{
    authorization::{Action, Publication, Resource, Transaction},
    pool::{Control, Workspace},
    require, valid_id, Error, Result, TrustedRequest,
};
use crate::admission::{self, digest, AdmittedDefinition};
use serde_json::{json, Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::time::Instant;
use tokio_postgres::Row;

/// Canonical bytes a reader returns and a column stores (0002 CHECKs).
pub const MAX_CANONICAL_BYTES: usize = 262_144;
/// Inputs and record schemas in one definition (contract `maxItems`).
const MAX_BINDINGS: usize = 8;
/// Saved-work context, in Unicode scalar values (0002 CHECK, contract TEXT).
const MAX_CONTEXT_CHARACTERS: usize = 2000;
/// The only definition edition `definition_revisions` stores in Phase 1
/// (0002: `CHECK (schema_version=1)`); edition 2 waits for its migration.
const STORED_EDITION: u32 = 1;
/// ADR-007 D7.14 handler deadline. D7.6's single retry of a publication
/// happens within it (review m5): every publication transaction, and the
/// `Stale` read, is bounded by the deadline taken when `publish` starts.
pub const PUBLISH_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// The end of the handler budget kept back for the `Stale` read.
const STALE_READ: Duration = Duration::from_secs(1);
/// A second publication attempt starts only with at least this much budget
/// left before the `Stale` reserve; otherwise the publication is `Stale` now.
const RETRY_MINIMUM: Duration = Duration::from_secs(1);

/// Whether a second publication attempt may start at `now` (review m5).
fn retry_within(now: Instant, deadline: Instant) -> bool {
    deadline
        .checked_duration_since(now)
        .is_some_and(|left| left >= STALE_READ + RETRY_MINIMUM)
}

/// A drafted, immutable definition revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionRevision {
    pub revision: String,
    /// `forma:app-definition:v1` identity of the stored content.
    pub digest: String,
}

/// One typed input contract as stored with a revision and published with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputContract {
    pub input: String,
    /// `source` or `records`.
    pub kind: String,
    /// `forma:input-contract:v1` identity of `canonical`.
    pub digest: String,
    pub canonical: Vec<u8>,
}

/// One record schema as stored with a revision and published with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordSchema {
    pub schema: String,
    pub version: i32,
    /// `forma:record-schema:v1` identity of `canonical`.
    pub digest: String,
    pub canonical: Vec<u8>,
}

/// A stored revision read back: its content and the contracts derived from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionDetail {
    pub revision: String,
    pub digest: String,
    pub schema_version: i32,
    /// forma-json-v1 bytes of the definition content.
    pub canonical: Vec<u8>,
    pub inputs: Vec<InputContract>,
    pub record_schemas: Vec<RecordSchema>,
}

/// A publication read back. The reusable envelope names input contracts and
/// record schemas only; `inputs` carries each contract's canonical bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationDetail {
    pub publication: String,
    pub revision: String,
    pub definition_digest: String,
    pub admission_digest: String,
    pub policy_kind: String,
    /// forma-json-v1 bytes of the `publication` contract document.
    pub envelope: Vec<u8>,
    pub inputs: Vec<InputContract>,
    pub record_schemas: Vec<RecordSchema>,
}

/// The app's publication with the highest `publication_sequence` when a
/// publication lost to a concurrent writer and could not retry (D7.6, D7.9:
/// 412 naming the current revision). All `None` when the app has no
/// publication yet.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CurrentRevision {
    pub publication: Option<String>,
    pub revision: Option<String>,
    pub definition_digest: Option<String>,
    /// APP-013: the saved work's current version when a command's work
    /// premise has moved (D7.9 `Stale`); `None` for a publication's answer.
    pub work_version: Option<i64>,
}

/// D7.9. `Stale` maps to 412 with the current revision.
#[derive(Debug, Eq, PartialEq)]
pub enum PublishResult {
    Published(Publication),
    Stale(CurrentRevision),
}

/// One saved-work binding of a published input. A source input binds one
/// concrete revision of a source the binder may read; a records input binds
/// the work's collection of one published record schema whose fields equal
/// the input's. Contract digests are never supplied: storage copies them from
/// the publication and the source revision, and they must agree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkBinding {
    Source {
        input: String,
        source: String,
        revision: String,
    },
    Records {
        input: String,
        schema: String,
    },
}
impl WorkBinding {
    fn input(&self) -> &str {
        match self {
            Self::Source { input, .. } | Self::Records { input, .. } => input,
        }
    }
}

/// A newly saved work, at version 1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedWork {
    pub work: String,
    pub publication: String,
    pub version: i64,
}

/// A saved work read back, as the forma-json-v1 `saved_work` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedWorkDetail {
    pub work: String,
    pub publication: String,
    pub version: i64,
    pub definition_digest: String,
    pub canonical: Vec<u8>,
}

/// Lowercase local identifier (`genui.local_id`, contract `ID`).
pub(super) fn local_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
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
fn bytes(row: &Row, index: usize) -> Result<Vec<u8>> {
    let value: Vec<u8> = row.try_get(index).map_err(|_| Error::Unavailable)?;
    require_stored(value.len() <= MAX_CANONICAL_BYTES)?;
    Ok(value)
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
fn encode(value: &Value) -> Result<Vec<u8>> {
    let bytes = admission::encode(value).map_err(|_| Error::Unavailable)?;
    require_stored(bytes.len() <= MAX_CANONICAL_BYTES)?;
    Ok(bytes)
}

/// The content of an admitted definition, re-admitted inside storage, with
/// the input contracts and record schemas derived from its bytes.
struct Admitted {
    digest: String,
    canonical: Vec<u8>,
    order: Vec<String>,
    inputs: Vec<InputContract>,
    record_schemas: Vec<RecordSchema>,
}

/// Re-admits definition content as an envelope under the trusted scope.
/// `AdmittedDefinition` has public fields, so storage never takes one on
/// trust: the content is admitted again, whole, and must reproduce the same
/// identity, edition and registry versions. The revision identifier in the
/// envelope is syntax only and confers nothing (`admission::validate`).
fn readmit(
    content: &[u8],
    claimed: Option<&AdmittedDefinition>,
    stored_digest: Option<&str>,
    organization: &str,
    app: &str,
) -> Result<Admitted> {
    let value = admission::decode(content).map_err(|_| Error::Denied)?;
    let digest = digest::digest(digest::APP_DEFINITION, content);
    let envelope = json!({
        "kind": "app_definition",
        "schema_version": STORED_EDITION,
        "organization_id": organization,
        "app_id": app,
        "revision_id": app,
        "content": value,
        "digest": digest,
    });
    let bytes = admission::encode(&envelope).map_err(|_| Error::Denied)?;
    let admitted = admission::admit_definition(&bytes).map_err(|_| Error::Denied)?;
    require(
        admitted.canonical == content
            && admitted.schema_version == STORED_EDITION
            && admitted.renderer_version == 1
            && admitted.operator_version == 1
            && claimed.is_none_or(|claimed| *claimed == admitted)
            && stored_digest.is_none_or(|stored| stored == admitted.digest),
    )?;
    let member = |key: &str| -> Result<Vec<Value>> {
        Ok(value
            .get(key)
            .and_then(Value::as_array)
            .ok_or(Error::Denied)?
            .clone())
    };
    let mut order = Vec::new();
    let mut inputs = Vec::new();
    for item in member("inputs")? {
        let input = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Error::Denied)?;
        let kind = item
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(Error::Denied)?;
        let canonical = encode(&item)?;
        order.push(input.to_owned());
        inputs.push(InputContract {
            input: input.to_owned(),
            kind: kind.to_owned(),
            digest: digest::digest(digest::INPUT_CONTRACT, &canonical),
            canonical,
        });
    }
    let mut record_schemas = Vec::new();
    for item in member("record_schemas")? {
        let schema = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Error::Denied)?;
        let version = item
            .get("version")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(Error::Denied)?;
        let canonical = encode(&item)?;
        record_schemas.push(RecordSchema {
            schema: schema.to_owned(),
            version,
            digest: digest::digest(digest::RECORD_SCHEMA, &canonical),
            canonical,
        });
    }
    Ok(Admitted {
        digest: admitted.digest,
        canonical: admitted.canonical,
        order,
        inputs,
        record_schemas,
    })
}

/// The workspace confirms the request's provider identity against the
/// scoped principal and an active binding, as `read_publication` does.
async fn confirm_identity(transaction: &Transaction<'_>, request: &TrustedRequest) -> Result<()> {
    let rows = transaction.query("SELECT 1 FROM genui.principals p JOIN genui.provider_tenant_bindings b USING(organization_id,issuer,provider_tenant_id) WHERE p.organization_id=$1::text::uuid AND p.principal_id=$2::text::uuid AND p.issuer=$3 AND p.provider_tenant_id=$4 AND p.provider_subject=$5 AND b.active", &[&request.organization,&request.principal,&request.issuer,&request.provider_tenant,&request.provider_subject]).await?;
    require(rows.len() == 1)
}

/// `count` fresh UUIDv4 identifiers and the transaction's whole-second UTC
/// time in the contract's `YYYY-MM-DDTHH:MM:SSZ` form.
pub(super) async fn fresh(
    transaction: &Transaction<'_>,
    count: i32,
) -> Result<(Vec<String>, String)> {
    let rows = transaction
        .query(
            "SELECT gen_random_uuid()::text,to_char(date_trunc('second',now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM generate_series(1,$1::int)",
            &[&count],
        )
        .await?;
    require_stored(rows.len() == usize::try_from(count).map_err(|_| Error::Unavailable)?)?;
    let at = text(rows.first().ok_or(Error::Unavailable)?, 1)?;
    Ok((
        rows.iter().map(|row| text(row, 0)).collect::<Result<_>>()?,
        at,
    ))
}

/// The two application chains this module appends to (D7.8); the session
/// module appends `session.signed_in` to the workspace chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Chain {
    Workspace,
    Control,
    /// APP-013: the data role's `command.committed` chain.
    Data,
}
impl Chain {
    fn role(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Control => "control",
            Self::Data => "data",
        }
    }
}
pub(super) struct Event<'a> {
    pub kind: &'static str,
    /// The event id, when the event is the record of one identified object
    /// (a publication's event id is its publication id, review m6); else a
    /// fresh UUID from the database.
    pub id: Option<&'a str>,
    pub app: Option<&'a str>,
    pub work: Option<&'a str>,
    /// The record a `command.committed` event names (APP-013); else NULL.
    pub record: Option<&'a str>,
    pub outcome: Option<&'a str>,
}

/// The D7.8 event hash: SHA-256 over `forma:audit-event:v1`, NUL, the previous
/// hash and the forma-json-v1 bytes of every column except `hash`.
fn event_hash(previous: &str, body: &Value) -> Result<String> {
    let canonical = admission::encode(body).map_err(|_| Error::Unavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(b"forma:audit-event:v1\x00");
    hasher.update(previous.as_bytes());
    hasher.update(&canonical);
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for byte in hasher.finalize().iter() {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}
#[allow(clippy::too_many_arguments)]
fn event_body(
    chain: Chain,
    organization: &str,
    event: &str,
    sequence: i64,
    kind: &str,
    actor: &str,
    app: Option<&str>,
    work: Option<&str>,
    record: Option<&str>,
    outcome: Option<&str>,
    occurred: &str,
    previous: &str,
) -> Value {
    json!({
        "actor_principal_id": actor,
        "app_id": app,
        "chain_sequence": sequence,
        "code_id": null,
        "event_id": event,
        "kind": kind,
        "occurred_at": occurred,
        "organization_id": organization,
        "outcome_digest": outcome,
        "previous_hash": previous,
        "record_id": record,
        "work_id": work,
        "writer_role": chain.role(),
    })
}

/// D7.8 append protocol: lock the role's own head row, link the event to it,
/// insert the event and advance the head, all in the caller's transaction.
/// A missing head is a provisioning gap (D7.7, D7.19): `Configuration`.
pub(super) async fn append(
    transaction: &Transaction<'_>,
    chain: Chain,
    request: &TrustedRequest,
    event: Event<'_>,
) -> Result<String> {
    append_as(
        transaction,
        chain,
        &request.organization,
        &request.principal,
        event,
    )
    .await
}

/// As `append`, for an actor whose authority the caller's transaction has
/// already fenced (the session module, after `lock_member_authority`).
pub(super) async fn append_as(
    transaction: &Transaction<'_>,
    chain: Chain,
    organization: &str,
    actor: &str,
    event: Event<'_>,
) -> Result<String> {
    let role = chain.role();
    let head = transaction.query("SELECT h.chain_sequence::bigint,h.head_hash::text,gen_random_uuid()::text,to_char(date_trunc('second',now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.audit_heads h WHERE h.organization_id=$1::text::uuid AND h.writer_role=$2 FOR UPDATE", &[&organization,&role]).await?;
    let head = head.first().ok_or(Error::Configuration)?;
    let sequence = integer(head, 0)?
        .checked_add(1)
        .filter(|s| *s <= 9_007_199_254_740_991)
        .ok_or(Error::Unavailable)?;
    let previous = text(head, 1)?;
    let id = match event.id {
        Some(id) => id.to_owned(),
        None => text(head, 2)?,
    };
    let occurred = text(head, 3)?;
    let body = event_body(
        chain,
        organization,
        &id,
        sequence,
        event.kind,
        actor,
        event.app,
        event.work,
        event.record,
        event.outcome,
        &occurred,
        &previous,
    );
    let hash = event_hash(&previous, &body)?;
    transaction.query("INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash) VALUES ($1::text::uuid,$2::text::uuid,$3,$4::bigint,$5,$6::text::uuid,$7::text::uuid,$8::text::uuid,$13::text::uuid,NULL,$9::text,$10::text::timestamptz,$11::text,$12::text)",
        &[&organization,&id,&role,&sequence,&event.kind,&actor,&event.app,&event.work,&event.outcome,&occurred,&previous,&hash,&event.record]).await?;
    transaction.query("UPDATE genui.audit_heads SET chain_sequence=$1::bigint,head_hash=$2::text WHERE organization_id=$3::text::uuid AND writer_role=$4", &[&sequence,&hash,&organization,&role]).await?;
    Ok(id)
}

async fn stored_inputs(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    revision: &str,
) -> Result<Vec<InputContract>> {
    let rows = transaction.query("SELECT input_id::text,kind,contract_digest::text,canonical_contract FROM genui.definition_inputs WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND revision_id=$3::text::uuid ORDER BY input_id", &[&organization,&app,&revision]).await?;
    rows.iter()
        .map(|row| {
            Ok(InputContract {
                input: text(row, 0)?,
                kind: text(row, 1)?,
                digest: text(row, 2)?,
                canonical: bytes(row, 3)?,
            })
        })
        .collect()
}
async fn stored_schemas(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    revision: &str,
) -> Result<Vec<RecordSchema>> {
    let rows = transaction.query("SELECT schema_id::text,schema_version,digest::text,canonical_schema FROM genui.definition_record_schemas WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND revision_id=$3::text::uuid ORDER BY schema_id,schema_version", &[&organization,&app,&revision]).await?;
    rows.iter()
        .map(|row| {
            Ok(RecordSchema {
                schema: text(row, 0)?,
                version: small(row, 1)?,
                digest: text(row, 2)?,
                canonical: bytes(row, 3)?,
            })
        })
        .collect()
}
/// The stored revision, re-admitted; its stored contracts must equal the ones
/// its content derives (C02: a publication binds exactly what was admitted).
async fn stored_revision(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    revision: &str,
) -> Result<Admitted> {
    let rows = transaction.query("SELECT schema_version,digest::text,canonical_content FROM genui.definition_revisions WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND revision_id=$3::text::uuid", &[&organization,&app,&revision]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    require_stored(small(row, 0)? == 1)?;
    let digest = text(row, 1)?;
    let admitted = readmit(&bytes(row, 2)?, None, Some(&digest), organization, app)
        .map_err(|_| Error::Unavailable)?;
    let sorted = |mut inputs: Vec<InputContract>| {
        inputs.sort_by(|a, b| a.input.cmp(&b.input));
        inputs
    };
    let mut schemas = admitted.record_schemas.clone();
    schemas.sort_by(|a, b| (&a.schema, a.version).cmp(&(&b.schema, b.version)));
    require_stored(
        stored_inputs(transaction, organization, app, revision).await?
            == sorted(admitted.inputs.clone())
            && stored_schemas(transaction, organization, app, revision).await? == schemas,
    )?;
    Ok(admitted)
}

/// A stored revision's admitted identity and canonical content, re-admitted
/// as `stored_revision` does, for the query path (APP-012): the content a
/// publication pins is never taken from storage on trust.
pub(super) async fn stored_content(
    transaction: &Transaction<'_>,
    organization: &str,
    app: &str,
    revision: &str,
) -> Result<(String, Vec<u8>)> {
    let admitted = stored_revision(transaction, organization, app, revision).await?;
    Ok((admitted.digest, admitted.canonical))
}

impl Workspace {
    /// Drafts one immutable revision of `app` from admitted content. Needs
    /// `edit_definition` on the app; organization and app come from the
    /// request and the path, never from the document. Phase 1 stores edition
    /// 1 only (0002); any other content is `Denied` and nothing is written.
    pub async fn create_definition_revision(
        &self,
        request: &TrustedRequest,
        app: &str,
        admitted: &AdmittedDefinition,
    ) -> Result<DefinitionRevision> {
        require(valid_id(app) && admitted.canonical.len() <= MAX_CANONICAL_BYTES)?;
        let content = readmit(
            &admitted.canonical,
            Some(admitted),
            None,
            &request.organization,
            app,
        )?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::EditDefinition, Resource::App(app))
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let (ids, _) = fresh(transaction, 1).await?;
            let revision = ids[0].clone();
            transaction.query("INSERT INTO genui.definition_revisions(organization_id,app_id,revision_id,schema_version,digest,canonical_content) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,1,$4::text,$5)", &[org,&app,&revision,&content.digest,&content.canonical]).await?;
            for input in &content.inputs {
                transaction.query("INSERT INTO genui.definition_inputs(organization_id,app_id,revision_id,input_id,kind,contract_digest,canonical_contract) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text,$5,$6::text,$7)", &[org,&app,&revision,&input.input,&input.kind,&input.digest,&input.canonical]).await?;
            }
            for schema in &content.record_schemas {
                transaction.query("INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text,$5,$6::text,$7)", &[org,&app,&revision,&schema.schema,&schema.version,&schema.digest,&schema.canonical]).await?;
            }
            append(transaction, Chain::Workspace, request, Event {
                kind: "definition.drafted",
                id: None,
                app: Some(app),
                work: None,
                record: None,
                outcome: Some(&content.digest),
            })
            .await?;
            Ok(DefinitionRevision {
                revision,
                digest: content.digest.clone(),
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Reads a stored revision back, re-admitted. Needs `edit_definition`.
    pub async fn read_definition(
        &self,
        request: &TrustedRequest,
        app: &str,
        revision: &str,
    ) -> Result<DefinitionDetail> {
        require(valid_id(app) && valid_id(revision))?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::EditDefinition, Resource::App(app))
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let admitted =
                stored_revision(transaction, &request.organization, app, revision).await?;
            Ok(DefinitionDetail {
                revision: revision.to_owned(),
                digest: admitted.digest,
                schema_version: 1,
                canonical: admitted.canonical,
                inputs: stored_inputs(transaction, &request.organization, app, revision).await?,
                record_schemas: stored_schemas(transaction, &request.organization, app, revision)
                    .await?,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Reads a publication's reusable envelope and the input contracts and
    /// record schemas it publishes. Needs `discover` on the app.
    pub async fn read_publication_detail(
        &self,
        request: &TrustedRequest,
        app: &str,
        publication: &str,
    ) -> Result<PublicationDetail> {
        require(valid_id(app) && valid_id(publication))?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::Discover, Resource::App(app))
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let rows = transaction.query("SELECT definition_revision_id::text,definition_digest::text,admission_digest::text,policy_kind,canonical_envelope FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[org,&app,&publication]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let revision = text(row, 0)?;
            let inputs = transaction.query("SELECT p.input_id::text,p.kind,p.contract_digest::text,d.canonical_contract FROM genui.publication_inputs p JOIN genui.definition_inputs d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id AND d.input_id=p.input_id AND d.kind=p.kind AND d.contract_digest=p.contract_digest WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid AND p.publication_id=$3::text::uuid ORDER BY p.input_id", &[org,&app,&publication]).await?;
            let schemas = transaction.query("SELECT p.schema_id::text,p.schema_version,p.digest::text,d.canonical_schema FROM genui.publication_record_schemas p JOIN genui.definition_record_schemas d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id AND d.schema_id=p.schema_id AND d.schema_version=p.schema_version AND d.digest=p.digest WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid AND p.publication_id=$3::text::uuid ORDER BY p.schema_id,p.schema_version", &[org,&app,&publication]).await?;
            Ok(PublicationDetail {
                publication: publication.to_owned(),
                revision,
                definition_digest: text(row, 1)?,
                admission_digest: text(row, 2)?,
                policy_kind: text(row, 3)?,
                envelope: bytes(row, 4)?,
                inputs: inputs
                    .iter()
                    .map(|row| {
                        Ok(InputContract {
                            input: text(row, 0)?,
                            kind: text(row, 1)?,
                            digest: text(row, 2)?,
                            canonical: bytes(row, 3)?,
                        })
                    })
                    .collect::<Result<_>>()?,
                record_schemas: schemas
                    .iter()
                    .map(|row| {
                        Ok(RecordSchema {
                            schema: text(row, 0)?,
                            version: small(row, 1)?,
                            digest: text(row, 2)?,
                            canonical: bytes(row, 3)?,
                        })
                    })
                    .collect::<Result<_>>()?,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Saves a new work of `publication` at version 1 with one collection per
    /// published record schema and exactly one binding per published input.
    /// Needs `run` on the app and `source_read` on every bound source; each
    /// source revision must be live and carry the published input's contract
    /// digest. A source of another organization is unreachable (scope and
    /// organization-leading foreign keys). No permission row is written and
    /// no epoch advances: the work records the saving member as its creator,
    /// whose authority equals `record_read` and `record_write` on it until the
    /// operator revokes it (ADR-007 amendment, review M2; the RESTRICTIVE
    /// `saved_work_creator` policy admits only the scoped principal).
    pub async fn create_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        publication: &str,
        context: &str,
        bindings: &[WorkBinding],
    ) -> Result<SavedWork> {
        let mut inputs = BTreeSet::new();
        require(
            valid_id(app)
                && valid_id(publication)
                && context.chars().count() <= MAX_CONTEXT_CHARACTERS
                && bindings.len() <= MAX_BINDINGS
                && bindings.iter().all(|binding| {
                    inputs.insert(binding.input())
                        && local_id(binding.input())
                        && match binding {
                            WorkBinding::Source {
                                source, revision, ..
                            } => valid_id(source) && valid_id(revision),
                            WorkBinding::Records { schema, .. } => local_id(schema),
                        }
                }),
        )?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::Run, Resource::App(app))
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let published = transaction.query("SELECT 1 FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[org,&app,&publication]).await?;
            require(published.len() == 1)?;
            let published_inputs: BTreeMap<String, (String, String, Vec<u8>)> = transaction
                .query("SELECT p.input_id::text,p.kind,p.contract_digest::text,d.canonical_contract FROM genui.publication_inputs p JOIN genui.definition_inputs d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id AND d.input_id=p.input_id WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid AND p.publication_id=$3::text::uuid", &[org,&app,&publication])
                .await?
                .iter()
                .map(|row| Ok((text(row, 0)?, (text(row, 1)?, text(row, 2)?, bytes(row, 3)?))))
                .collect::<Result<_>>()?;
            let schemas: BTreeMap<String, (i32, Vec<u8>)> = transaction
                .query("SELECT p.schema_id::text,p.schema_version,d.canonical_schema FROM genui.publication_record_schemas p JOIN genui.definition_record_schemas d ON d.organization_id=p.organization_id AND d.app_id=p.app_id AND d.revision_id=p.definition_revision_id AND d.schema_id=p.schema_id AND d.schema_version=p.schema_version WHERE p.organization_id=$1::text::uuid AND p.app_id=$2::text::uuid AND p.publication_id=$3::text::uuid", &[org,&app,&publication])
                .await?
                .iter()
                .map(|row| Ok((text(row, 0)?, (small(row, 1)?, bytes(row, 2)?))))
                .collect::<Result<_>>()?;
            // Every published input is bound exactly once (contract: "work must
            // bind each input"); nothing else is bound.
            require(
                published_inputs.keys().map(String::as_str).collect::<BTreeSet<_>>() == inputs,
            )?;
            let count = i32::try_from(1 + schemas.len()).map_err(|_| Error::Denied)?;
            let (ids, _) = fresh(transaction, count).await?;
            let work = ids[0].clone();
            let collections: BTreeMap<&str, (&str, i32)> = schemas
                .iter()
                .zip(&ids[1..])
                .map(|((schema, (version, _)), id)| (schema.as_str(), (id.as_str(), *version)))
                .collect();
            transaction.query("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,1,$5,$6::text::uuid)", &[org,&app,&work,&publication,&context,&request.principal]).await?;
            for (schema, (collection, version)) in &collections {
                transaction.query("INSERT INTO genui.record_collections(organization_id,app_id,work_id,publication_id,collection_id,schema_id,schema_version) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text,$7)", &[org,&app,&work,&publication,collection,schema,version]).await?;
            }
            let mut bound = Vec::with_capacity(bindings.len());
            for binding in bindings {
                let (kind, contract, contract_bytes) =
                    published_inputs.get(binding.input()).ok_or(Error::Denied)?;
                let document = match binding {
                    WorkBinding::Source { input, source, revision } => {
                        require(kind == "source")?;
                        permit.also(Action::SourceRead, Resource::Source(source)).await?;
                        let rows = transaction.query("SELECT normalized_digest::text,contract_digest::text FROM genui.source_revisions WHERE organization_id=$1::text::uuid AND source_id=$2::text::uuid AND revision_id=$3::text::uuid AND deleted_at IS NULL", &[org,source,revision]).await?;
                        let row = rows.first().ok_or(Error::Denied)?;
                        let source_digest = text(row, 0)?;
                        require(&text(row, 1)? == contract)?;
                        transaction.query("INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,source_id,source_revision_id,source_digest) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,'source',$6::text,$7::text::uuid,$8::text::uuid,$9::text)", &[org,&app,&work,&publication,input,contract,source,revision,&source_digest]).await?;
                        transaction.query("INSERT INTO genui.work_source_dependencies(organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text,$5::text::uuid,$6::text::uuid,$7::text)", &[org,&app,&work,input,source,revision,&source_digest]).await?;
                        json!({"input_id": input, "kind": "source", "contract_digest": contract,
                            "source_id": source, "source_revision_id": revision, "source_digest": source_digest})
                    }
                    WorkBinding::Records { input, schema } => {
                        require(kind == "records")?;
                        let (collection, version) = collections.get(schema.as_str()).ok_or(Error::Denied)?;
                        let (_, schema_bytes) = schemas.get(schema).ok_or(Error::Denied)?;
                        // validate.py: a records input binds a schema with its fields.
                        let fields = |raw: &[u8]| {
                            admission::decode(raw)
                                .ok()
                                .and_then(|value| value.get("fields").cloned())
                        };
                        require(fields(contract_bytes).is_some() && fields(contract_bytes) == fields(schema_bytes))?;
                        transaction.query("INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,collection_id,schema_id,schema_version,revision_policy) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,'records',$6::text,$7::text::uuid,$8::text,$9,'committed_snapshot')", &[org,&app,&work,&publication,input,contract,collection,schema,version]).await?;
                        json!({"input_id": input, "kind": "records", "contract_digest": contract,
                            "collection_id": collection, "schema_id": schema, "schema_version": version,
                            "revision_policy": "committed_snapshot"})
                    }
                };
                bound.push(digest::digest(digest::SOURCE_BINDING, &encode(&document)?));
            }
            append(transaction, Chain::Workspace, request, Event {
                kind: "work.saved",
                id: None,
                app: Some(app),
                work: Some(&work),
                record: None,
                outcome: None,
            })
            .await?;
            for binding in &bound {
                append(transaction, Chain::Workspace, request, Event {
                    kind: "work.bound",
                    id: None,
                    app: Some(app),
                    work: Some(&work),
                    record: None,
                    outcome: Some(binding),
                })
                .await?;
            }
            Ok(SavedWork {
                work,
                publication: publication.to_owned(),
                version: 1,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Reads a saved work back as its `saved_work` document: bindings and
    /// source dependencies in input order, collections in schema order. Needs
    /// `record_read` on the work, by grant or as its unrevoked creator (which
    /// also needs `run` and `source_read` on every dependency, as every work
    /// read does).
    pub async fn read_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<SavedWorkDetail> {
        require(valid_id(app) && valid_id(work))?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let org = &request.organization;
            let rows = transaction.query("SELECT w.publication_id::text,w.version::bigint,w.context,p.definition_digest::text FROM genui.saved_work w JOIN genui.publications p USING(organization_id,app_id,publication_id) WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid", &[org,&app,&work]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let (publication, version, context, definition) =
                (text(row, 0)?, integer(row, 1)?, text(row, 2)?, text(row, 3)?);
            let collections = transaction.query("SELECT collection_id::text,schema_id::text,schema_version FROM genui.record_collections WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY schema_id,schema_version,collection_id", &[org,&app,&work]).await?;
            let collections = collections
                .iter()
                .map(|row| Ok(json!({"id": text(row, 0)?, "schema_id": text(row, 1)?, "schema_version": small(row, 2)?})))
                .collect::<Result<Vec<_>>>()?;
            let rows = transaction.query("SELECT input_id::text,kind,contract_digest::text,source_id::text,source_revision_id::text,source_digest::text,collection_id::text,schema_id::text,schema_version,revision_policy FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY input_id", &[org,&app,&work]).await?;
            let mut bindings = Vec::with_capacity(rows.len());
            for row in &rows {
                let optional = |index| -> Result<Option<String>> {
                    row.try_get(index).map_err(|_| Error::Unavailable)
                };
                let mut binding = Map::new();
                binding.insert("input_id".into(), text(row, 0)?.into());
                binding.insert("kind".into(), text(row, 1)?.into());
                binding.insert("contract_digest".into(), text(row, 2)?.into());
                match text(row, 1)?.as_str() {
                    "source" => {
                        for (index, key) in [(3, "source_id"), (4, "source_revision_id"), (5, "source_digest")] {
                            binding.insert(key.into(), optional(index)?.ok_or(Error::Unavailable)?.into());
                        }
                    }
                    "records" => {
                        binding.insert("collection_id".into(), optional(6)?.ok_or(Error::Unavailable)?.into());
                        binding.insert("schema_id".into(), optional(7)?.ok_or(Error::Unavailable)?.into());
                        let version: Option<i32> = row.try_get(8).map_err(|_| Error::Unavailable)?;
                        binding.insert("schema_version".into(), version.ok_or(Error::Unavailable)?.into());
                        binding.insert("revision_policy".into(), optional(9)?.ok_or(Error::Unavailable)?.into());
                    }
                    _ => return Err(Error::Unavailable),
                }
                bindings.push(Value::Object(binding));
            }
            let dependencies = transaction.query("SELECT input_id::text,source_id::text,source_revision_id::text,source_digest::text FROM genui.work_source_dependencies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY input_id,source_id,source_revision_id", &[org,&app,&work]).await?;
            let dependencies = dependencies
                .iter()
                .map(|row| Ok(json!({"input_id": text(row, 0)?, "source_id": text(row, 1)?, "source_revision_id": text(row, 2)?, "source_digest": text(row, 3)?})))
                .collect::<Result<Vec<_>>>()?;
            let document = json!({
                "kind": "saved_work",
                "schema_version": 1,
                "organization_id": org,
                "app_id": app,
                "work_id": work,
                "publication_id": publication,
                "version": version,
                "context": context,
                "bindings": bindings,
                "record_collections": collections,
                "source_dependencies": dependencies,
            });
            Ok(SavedWorkDetail {
                work: work.to_owned(),
                publication,
                version,
                definition_digest: definition,
                canonical: encode(&document)?,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

/// Publication attempts that ended in SQLSTATE 40001 (test executable only),
/// so the campaign can prove a retry happened rather than infer it.
#[cfg(test)]
pub(super) static SERIALIZATION_FAILURES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// One publication attempt's end (D7.6).
enum Attempt {
    Published(Publication),
    Serialization,
}

impl Control {
    /// Publishes a stored revision of `app` under the organization's policy
    /// `policy` (its newest version). `owner_publish`: the app's owner publishes
    /// with `edit_definition` and is author and approver. `independent_review`:
    /// a member other than the owner, with `review_publish`, approves the
    /// owner's revision. The stored content is re-admitted and its admitted
    /// digest becomes the publication's `admission_digest`. SERIALIZABLE; a
    /// 40001 at any statement or at COMMIT is retried once in a new
    /// transaction within the D7.14 handler deadline, and a second one (or one
    /// that leaves too little of the deadline for a retry) is `Stale` naming
    /// the app's publication with the highest sequence (D7.6, D7.9). Nothing
    /// else retries. The caller chooses `policy`; the server does not yet
    /// resolve the applicable policy itself (review m3, a Phase 1 limit while
    /// each organization has one policy row).
    pub async fn publish(
        &self,
        request: &TrustedRequest,
        app: &str,
        revision: &str,
        policy: &str,
    ) -> Result<PublishResult> {
        self.publish_until(
            request,
            app,
            revision,
            policy,
            Instant::now() + PUBLISH_HANDLER_DEADLINE,
        )
        .await
    }

    /// `publish` within an explicit handler deadline (review m5).
    pub(super) async fn publish_until(
        &self,
        request: &TrustedRequest,
        app: &str,
        revision: &str,
        policy: &str,
        deadline: Instant,
    ) -> Result<PublishResult> {
        require(valid_id(app) && valid_id(revision) && valid_id(policy))?;
        let attempts = deadline.checked_sub(STALE_READ).unwrap_or(deadline);
        for attempt in 0..2 {
            if attempt == 1 && !retry_within(Instant::now(), deadline) {
                break;
            }
            match self
                .publish_once(request, app, revision, policy, attempts)
                .await?
            {
                Attempt::Published(publication) => {
                    return Ok(PublishResult::Published(publication))
                }
                Attempt::Serialization => {
                    #[cfg(test)]
                    SERIALIZATION_FAILURES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        }
        Ok(PublishResult::Stale(
            self.current_revision(request, app, deadline).await?,
        ))
    }

    async fn publish_once(
        &self,
        request: &TrustedRequest,
        app: &str,
        revision: &str,
        policy: &str,
        cap: Instant,
    ) -> Result<Attempt> {
        let mut transaction = Transaction::begin_serializable(&self.inner, request, cap).await?;
        let result = async {
            let org = &request.organization;
            let rows = transaction.query("SELECT policy_version::bigint,kind FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND policy_id=$2::text::uuid ORDER BY policy_version DESC LIMIT 1", &[org,&policy]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let (policy_version, policy_kind) = (integer(row, 0)?, text(row, 1)?);
            let action = match policy_kind.as_str() {
                "owner_publish" => Action::EditDefinition,
                "independent_review" => Action::ReviewPublish,
                _ => return Err(Error::Denied),
            };
            let precheck = transaction.precheck(action, Resource::App(app)).await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            let owner = transaction.query("SELECT owner_principal_id::text FROM genui.apps WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid", &[org,&app]).await?;
            let owner = text(owner.first().ok_or(Error::Denied)?, 0)?;
            require(match action {
                Action::EditDefinition => owner == request.principal,
                _ => owner != request.principal,
            })?;
            let admitted = stored_revision(transaction, org, app, revision).await?;
            #[cfg(test)]
            transaction.lease.checkpoint("publish").await;
            // Review M1: the control head is locked before the publication is
            // ordered, so every publisher of the organization is serialized
            // here; one whose snapshot predates a committed publication fails
            // with 40001 on this lock (D7.6) instead of reusing its sequence.
            let head = transaction.query("SELECT 1 FROM genui.audit_heads WHERE organization_id=$1::text::uuid AND writer_role='control' FOR UPDATE", &[org]).await?;
            if head.is_empty() {
                return Err(Error::Configuration);
            }
            let sequence = transaction.query("SELECT coalesce(max(publication_sequence),0)::bigint+1 FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid", &[org,&app]).await?;
            let sequence = integer(sequence.first().ok_or(Error::Unavailable)?, 0)?;
            require_stored(sequence <= 9_007_199_254_740_991)?;
            let (ids, published_at) = fresh(transaction, 1).await?;
            let publication = ids[0].clone();
            let by_input: BTreeMap<&str, &InputContract> =
                admitted.inputs.iter().map(|i| (i.input.as_str(), i)).collect();
            let envelope = json!({
                "kind": "publication",
                "schema_version": 1,
                "organization_id": org,
                "app_id": app,
                "publication_id": publication,
                "definition_revision_id": revision,
                "definition_digest": admitted.digest,
                "input_contract_digests": admitted.order.iter()
                    .map(|id| json!({"input_id": id, "digest": by_input[id.as_str()].digest}))
                    .collect::<Vec<_>>(),
                "record_schema_digests": admitted.record_schemas.iter()
                    .map(|s| json!({"schema_id": s.schema, "digest": s.digest}))
                    .collect::<Vec<_>>(),
                "renderer_version": 1,
                "operator_version": 1,
                "admission_digest": admitted.digest,
                "policy": {"id": policy, "version": policy_version, "kind": policy_kind},
                "author_id": owner,
                "approver_id": request.principal,
                "approver_epoch": request.epoch,
                "published_at": published_at,
            });
            let envelope = encode(&envelope)?;
            transaction.query("INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope,publication_sequence) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6::text::uuid,$7::bigint,$8,$9::text::uuid,$10::text::uuid,$11::bigint,1,1,$12::text,$13::text::timestamptz,$14,$15::bigint)",
                &[org,&app,&publication,&revision,&admitted.digest,&policy,&policy_version,&policy_kind,&owner,&request.principal,&request.epoch,&admitted.digest,&published_at,&envelope,&sequence]).await?;
            for input in &admitted.inputs {
                transaction.query("INSERT INTO genui.publication_inputs(organization_id,app_id,publication_id,definition_revision_id,input_id,kind,contract_digest) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6,$7::text)", &[org,&app,&publication,&revision,&input.input,&input.kind,&input.digest]).await?;
            }
            for schema in &admitted.record_schemas {
                transaction.query("INSERT INTO genui.publication_record_schemas(organization_id,app_id,publication_id,definition_revision_id,schema_id,schema_version,digest) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6,$7::text)", &[org,&app,&publication,&revision,&schema.schema,&schema.version,&schema.digest]).await?;
            }
            append(transaction, Chain::Control, request, Event {
                kind: "publication.published",
                id: Some(&publication),
                app: Some(app),
                work: None,
                record: None,
                outcome: Some(&admitted.digest),
            })
            .await?;
            Ok(Publication {
                publication_id: publication,
                definition_digest: admitted.digest.clone(),
            })
        }
        .await;
        match result {
            Ok(publication) => Ok(if transaction.commit_serializable().await? {
                Attempt::Published(publication)
            } else {
                Attempt::Serialization
            }),
            Err(error) => {
                let serialization = transaction.serialization_failed();
                transaction.finish(false).await?;
                if serialization {
                    Ok(Attempt::Serialization)
                } else {
                    Err(error)
                }
            }
        }
    }

    /// The app's publication with the highest sequence, read under the
    /// reader's current authority (`discover` is not a control permission, so
    /// the same `edit_definition` or `review_publish` the publisher holds),
    /// within the handler deadline.
    async fn current_revision(
        &self,
        request: &TrustedRequest,
        app: &str,
        deadline: Instant,
    ) -> Result<CurrentRevision> {
        let mut transaction = Transaction::begin_until(&self.inner, request, deadline).await?;
        let result = async {
            let precheck = match transaction
                .precheck(Action::EditDefinition, Resource::App(app))
                .await
            {
                Ok(precheck) => precheck,
                Err(_) => {
                    transaction
                        .precheck(Action::ReviewPublish, Resource::App(app))
                        .await?
                }
            };
            let permit = transaction.fence(precheck).await?;
            let rows = permit.transaction().query("SELECT publication_id::text,definition_revision_id::text,definition_digest::text FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid ORDER BY publication_sequence DESC LIMIT 1", &[&request.organization,&app]).await?;
            Ok(match rows.first() {
                Some(row) => CurrentRevision {
                    publication: Some(text(row, 0)?),
                    revision: Some(text(row, 1)?),
                    definition_digest: Some(text(row, 2)?),
                    work_version: None,
                },
                None => CurrentRevision::default(),
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORGANIZATION: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn audit_event_hash_matches_the_operator_and_schema_campaign_definition() {
        // Vector computed by app/runtime/admission_operator.py (genesis and
        // event_hash), the verifier every chain is recomputed with (D7.8).
        let previous = "sha256:f66e7313fac697dfb11a0b84d722ef7d6ab4e08fa9eca9a2e1fb783a343d3c47";
        let mut genesis = Sha256::new();
        genesis.update(b"forma:audit-genesis:v1\x00");
        genesis.update(ORGANIZATION.as_bytes());
        genesis.update(b"\x00workspace");
        let genesis: String = genesis
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(format!("sha256:{genesis}"), previous);
        let outcome = format!("sha256:{}", "ab".repeat(32));
        let body = event_body(
            Chain::Workspace,
            ORGANIZATION,
            "00000000-0000-4000-8000-0000000000e9",
            2,
            "definition.drafted",
            "00000000-0000-4000-8000-0000000000ca",
            Some("00000000-0000-4000-8000-0000000000c9"),
            None,
            None,
            Some(&outcome),
            "2026-10-29T12:00:00Z",
            previous,
        );
        assert_eq!(
            event_hash(previous, &body).unwrap(),
            "sha256:61d8914f6080bf2292799178f16505878c5c96e34476c22f8a88d03eb64ede68"
        );
        assert_eq!(Chain::Control.role(), "control");
        assert_eq!(Chain::Data.role(), "data");
    }

    fn spend_review() -> (Value, AdmittedDefinition) {
        let bundle: Value = serde_json::from_str(include_str!(
            "../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json"
        ))
        .unwrap();
        let definition = bundle["definition"].clone();
        let admitted =
            admission::admit_definition(&admission::encode(&definition).unwrap()).unwrap();
        (definition, admitted)
    }

    #[test]
    fn a_publication_retry_starts_only_within_the_handler_deadline() {
        // Review m5: both attempts end by the deadline less the 1 s Stale
        // reserve, and the Stale read by the deadline, so the whole publish
        // fits the 5 s D7.14 handler deadline; a retry starts only with its
        // own 1 s minimum and the reserve still ahead of it.
        let now = Instant::now();
        assert_eq!(PUBLISH_HANDLER_DEADLINE, Duration::from_secs(5));
        assert!(retry_within(now, now + PUBLISH_HANDLER_DEADLINE));
        assert!(retry_within(now, now + Duration::from_secs(2)));
        assert!(!retry_within(now, now + Duration::from_millis(1999)));
        assert!(!retry_within(
            now + Duration::from_secs(6),
            now + PUBLISH_HANDLER_DEADLINE
        ));
    }

    #[test]
    fn readmission_derives_contracts_from_the_admitted_bytes_under_trusted_scope() {
        let (definition, admitted) = spend_review();
        let app = "00000000-0000-4000-8000-0000000000c9";
        let parts = readmit(
            &admitted.canonical,
            Some(&admitted),
            None,
            ORGANIZATION,
            app,
        )
        .unwrap();
        assert_eq!(parts.digest, definition["digest"].as_str().unwrap());
        assert_eq!(parts.order, ["expenses", "decisions"]);
        let content = &definition["content"];
        for (index, input) in parts.inputs.iter().enumerate() {
            let expected = admission::encode(&content["inputs"][index]).unwrap();
            assert_eq!(input.canonical, expected);
            assert_eq!(
                input.digest,
                digest::digest(digest::INPUT_CONTRACT, &expected)
            );
        }
        assert_eq!(parts.inputs[0].kind, "source");
        assert_eq!(parts.inputs[1].kind, "records");
        assert_eq!(parts.record_schemas.len(), 1);
        assert_eq!(parts.record_schemas[0].schema, "decision");
        assert_eq!(parts.record_schemas[0].version, 1);
        // The envelope's organization and app confer nothing: the same content
        // re-admits under any trusted scope with the same identity.
        let other = readmit(
            &admitted.canonical,
            Some(&admitted),
            None,
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-0000000000aa",
        )
        .unwrap();
        assert_eq!(other.digest, parts.digest);
    }

    #[test]
    fn forged_or_unstorable_admissions_are_refused_before_any_storage() {
        let (_, admitted) = spend_review();
        let app = "00000000-0000-4000-8000-0000000000c9";
        let mut forged = admitted.clone();
        forged.digest = format!("sha256:{}", "0".repeat(64));
        assert_eq!(
            readmit(&forged.canonical, Some(&forged), None, ORGANIZATION, app).err(),
            Some(Error::Denied)
        );
        let mut swapped = admitted.clone();
        let mut content = admission::decode(&swapped.canonical).unwrap();
        content["title"] = json!("Spend Review (patched after admission)");
        swapped.canonical = admission::encode(&content).unwrap();
        assert_eq!(
            readmit(&swapped.canonical, Some(&swapped), None, ORGANIZATION, app).err(),
            Some(Error::Denied)
        );
        let mut versions = admitted.clone();
        versions.operator_version = 2;
        assert_eq!(
            readmit(
                &versions.canonical,
                Some(&versions),
                None,
                ORGANIZATION,
                app
            )
            .err(),
            Some(Error::Denied)
        );
        // Edition 2 admits (APP-010 part 1) but Phase 1 stores edition 1 only.
        let current: Value = serde_json::from_str(include_str!(
            "../../../contracts/fixtures/spend-review.json"
        ))
        .unwrap();
        let edition_two =
            admission::admit_definition(&admission::encode(&current["definition"]).unwrap())
                .unwrap();
        assert_eq!(edition_two.schema_version, 2);
        assert_eq!(
            readmit(
                &edition_two.canonical,
                Some(&edition_two),
                None,
                ORGANIZATION,
                app
            )
            .err(),
            Some(Error::Denied)
        );
        assert!(local_id("expenses") && !local_id("Expenses") && !local_id("") && !local_id("1a"));
    }
}
