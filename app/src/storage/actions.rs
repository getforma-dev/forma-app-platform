//! ADR-011 scenarios (APP-093/APP-095): the two storage reads behind the
//! action path. Nothing here writes.
//!
//! - [`Data::list_records`]: the current records of every collection of a
//!   saved work, newest commit first and bounded, with the published schema
//!   and the declared commands — the record page's authorization (`record_read`
//!   on the work by grant or as its unrevoked creator, under the member fence
//!   and the work fence), so the canvas's records card and the action path's
//!   target lookup see exactly what the member may read. A refused member, an
//!   unknown work and a foreign organization are `Denied` alike (the uniform
//!   404).
//! - [`Data::dry_run_command`]: a typed command judged by the same rules the
//!   commit applies (`record_write` precheck and fence, the work fence with
//!   the caller's premise, the declaration, the version premise, the
//!   create/transition rule and `compose`), then rolled back. The action path
//!   uses it to show a confirmation card that storage would accept; the
//!   member's confirmation still goes through the audited command route,
//!   which judges everything again.
use super::{
    authorization::{Action, Resource, Transaction},
    commands::{
        collection, compose, current_state, decode_stored, published, state_map, work_fence,
        work_pin, CommandRequest, RecordCommand, RecordField,
    },
    pool::Data,
    require, valid_id, Error, Result, TrustedRequest, COMMAND_HANDLER_DEADLINE,
};
use crate::commands::payload;
use serde_json::{json, Value};
use tokio::time::Instant;
use tokio_postgres::Row;

/// At most this many records are listed (newest first).
pub const RECORDS_LIMIT: i64 = 200;

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn small(row: &Row, index: usize) -> Result<i32> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// One current record: identity, version, provenance and typed values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordRow {
    pub record_id: String,
    pub collection: String,
    pub schema: String,
    pub version: i64,
    pub commit_sequence: i64,
    /// Whole-second UTC.
    pub committed_at: String,
    pub creator: String,
    pub updater: String,
    /// The stored `{field_id, value}` array, in schema order.
    pub values: Value,
}

/// One collection of the work: its published schema and declared commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSummary {
    pub collection: String,
    pub schema: String,
    pub schema_version: i32,
    pub fields: Vec<RecordField>,
    pub commands: Vec<RecordCommand>,
}

/// The records listing of a saved work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordsListing {
    pub app: String,
    pub work: String,
    pub publication: String,
    pub definition_digest: String,
    pub work_version: i64,
    /// Whether the reader also holds `record_write` on the work now.
    pub can_write: bool,
    pub collections: Vec<CollectionSummary>,
    /// Newest commit first, at most `RECORDS_LIMIT`.
    pub records: Vec<RecordRow>,
    pub truncated: bool,
}

/// How a dry run ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DryRun {
    /// Storage would compose this state (the full record after the command).
    Valid {
        collection: String,
        current_version: i64,
        state: Value,
    },
    /// The work moved past the caller's premise.
    Stale,
    /// `expected_record_version` is not the record's current version.
    Conflict { current_version: i64 },
}

impl Data {
    /// The records of `work` (module notes).
    pub async fn list_records(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<RecordsListing> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + COMMAND_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
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
            let rows = transaction.query("SELECT collection_id::text,schema_id::text,schema_version FROM genui.record_collections WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY collection_id", &[org,&app,&work]).await?;
            let mut collections = Vec::with_capacity(rows.len());
            let mut definition_digest = String::new();
            for row in &rows {
                let (collection, schema, schema_version) =
                    (text(row, 0)?, text(row, 1)?, small(row, 2)?);
                let published = published(
                    transaction,
                    org,
                    app,
                    &publication,
                    &schema,
                    i64::from(schema_version),
                )
                .await?;
                definition_digest = published.definition_digest.clone();
                let writable: std::collections::BTreeSet<&str> = published
                    .commands
                    .values()
                    .flat_map(|d| d.writable.iter().map(String::as_str))
                    .collect();
                collections.push(CollectionSummary {
                    collection,
                    schema: schema.clone(),
                    schema_version,
                    fields: published
                        .fields
                        .iter()
                        .map(|field| RecordField {
                            id: field.id.clone(),
                            value_type: field.value_type.clone(),
                            nullable: field.nullable,
                            value: json!({"type": "null"}),
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
                        })
                        .collect(),
                });
            }
            let limit = RECORDS_LIMIT + 1;
            let rows = transaction.query("SELECT r.record_id::text,r.collection_id::text,r.current_version::bigint,v.commit_sequence::bigint,to_char(v.committed_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),v.creator_id::text,v.updater_id::text,v.canonical_values FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid ORDER BY v.commit_sequence DESC,r.record_id LIMIT $4", &[org,&app,&work,&limit]).await?;
            let truncated = rows.len() as i64 > RECORDS_LIMIT;
            let mut records = Vec::with_capacity(rows.len());
            for row in rows.iter().take(RECORDS_LIMIT as usize) {
                let collection = text(row, 1)?;
                let schema = collections
                    .iter()
                    .find(|c| c.collection == collection)
                    .map(|c| c.schema.clone())
                    .ok_or(Error::Unavailable)?;
                let values = decode_stored(row, 7)?;
                // Stored state must be a `{field_id, value}` array.
                state_map(Some(&values))?;
                records.push(RecordRow {
                    record_id: text(row, 0)?,
                    collection,
                    schema,
                    version: integer(row, 2)?,
                    commit_sequence: integer(row, 3)?,
                    committed_at: text(row, 4)?,
                    creator: text(row, 5)?,
                    updater: text(row, 6)?,
                    values,
                });
            }
            Ok(RecordsListing {
                app: app.to_owned(),
                work: work.to_owned(),
                publication,
                definition_digest,
                work_version,
                can_write,
                collections,
                records,
                truncated,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Judges `command` as a commit would, without writing (module notes).
    /// `Denied` for anything the commit would refuse as a denial: no
    /// `record_write`, an undeclared command, a value outside the published
    /// schema or the declared transitions.
    pub async fn dry_run_command(
        &self,
        request: &TrustedRequest,
        command: &CommandRequest,
    ) -> Result<DryRun> {
        require(command.well_formed())?;
        require(payload::digest(command).is_some())?;
        let cap = Instant::now() + COMMAND_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let org = request.organization.as_str();
            let (app, work) = (command.app.as_str(), command.work.as_str());
            let precheck = transaction
                .precheck(Action::RecordWrite, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let transaction = permit.transaction();
            if !work_fence(
                transaction,
                org,
                app,
                work,
                &command.publication,
                command.expected_work_version,
            )
            .await?
            {
                return Ok(DryRun::Stale);
            }
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
            let (current_version, current_values, _) =
                current_state(transaction, org, app, work, &collection, &command.record, false)
                    .await?;
            if current_version != command.expected_record_version {
                return Ok(DryRun::Conflict { current_version });
            }
            require(if current_version == 0 {
                declaration.operation == "create"
            } else {
                declaration.operation == "update" || !declaration.transitions.is_empty()
            })?;
            let bytes = compose(
                &published.fields,
                declaration,
                current_values.as_ref(),
                &command.values,
            )?;
            let state = crate::admission::decode(&bytes).map_err(|_| Error::Unavailable)?;
            Ok(DryRun::Valid {
                collection,
                current_version,
                state,
            })
        }
        .await;
        // Never a commit: a dry run leaves no row and no claim.
        transaction.finish(false).await?;
        result
    }
}
