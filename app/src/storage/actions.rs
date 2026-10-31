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
        collection, compose, current_state, decode_stored, published, published_content, state_map,
        work_fence, work_pin, CommandRequest, Published, RecordCommand, RecordField,
    },
    pool::Data,
    queries::{collections_many, ordinal, outline_of, pinned_many, pinned_many_with, Pinned},
    require, valid_id, Error, Result, TrustedRequest, WorkOutline, COMMAND_HANDLER_DEADLINE,
    QUERY_HANDLER_DEADLINE,
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

/// The records listing's columns over `genui.internal_records r` joined with
/// each record's current revision `v`.
const RECORD_COLUMNS: &str = "r.record_id::text,r.collection_id::text,r.current_version::bigint,v.commit_sequence::bigint,to_char(v.committed_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),v.creator_id::text,v.updater_id::text,v.canonical_values";
const RECORDS_FROM: &str = "FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version";

/// One collection of the listing from its published schema and commands.
fn collection_summary(
    collection: String,
    schema: String,
    schema_version: i32,
    published: &Published,
) -> CollectionSummary {
    let writable: std::collections::BTreeSet<&str> = published
        .commands
        .values()
        .flat_map(|d| d.writable.iter().map(String::as_str))
        .collect();
    CollectionSummary {
        collection,
        schema,
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
                transitions: declaration.transitions(),
                rules: declaration.rules.clone(),
            })
            .collect(),
    }
}

/// APP-122: at most this many works in one Ask or Shape read.
pub const READ_ONLY_WORKS: usize = 12;

/// One current record from [`RECORD_COLUMNS`] starting at column `at`.
fn record_row(row: &Row, at: usize, collections: &[CollectionSummary]) -> Result<RecordRow> {
    let collection = text(row, at + 1)?;
    let schema = collections
        .iter()
        .find(|c| c.collection == collection)
        .map(|c| c.schema.clone())
        .ok_or(Error::Unavailable)?;
    let values = decode_stored(row, at + 7)?;
    // Stored state must be a `{field_id, value}` array.
    state_map(Some(&values))?;
    Ok(RecordRow {
        record_id: text(row, at)?,
        collection,
        schema,
        version: integer(row, at + 2)?,
        commit_sequence: integer(row, at + 3)?,
        committed_at: text(row, at + 4)?,
        creator: text(row, at + 5)?,
        updater: text(row, at + 6)?,
        values,
    })
}

/// For each `(app, work)`, its newest `RECORDS_LIMIT + 1` current records
/// (newest commit first, as `list_records` reads them), in one statement.
async fn records_many(
    transaction: &Transaction<'_>,
    organization: &str,
    works: &[(&str, &str)],
) -> Result<Vec<Vec<Row>>> {
    let mut records: Vec<Vec<Row>> = (0..works.len()).map(|_| Vec::new()).collect();
    let sent: Vec<usize> = (0..works.len())
        .filter(|&i| valid_id(works[i].0) && valid_id(works[i].1))
        .collect();
    if sent.is_empty() {
        return Ok(records);
    }
    let apps: Vec<&str> = sent.iter().map(|&i| works[i].0).collect();
    let ids: Vec<&str> = sent.iter().map(|&i| works[i].1).collect();
    let limit = RECORDS_LIMIT + 1;
    for row in transaction.query(&format!("SELECT c.n::bigint,x.* FROM unnest($2::text[],$3::text[]) WITH ORDINALITY AS c(app,work,n) CROSS JOIN LATERAL (SELECT {RECORD_COLUMNS},row_number() OVER (ORDER BY v.commit_sequence DESC,r.record_id) AS k {RECORDS_FROM} WHERE r.organization_id=$1::text::uuid AND r.app_id=c.app::uuid AND r.work_id=c.work::uuid ORDER BY v.commit_sequence DESC,r.record_id LIMIT $4) x ORDER BY c.n,x.k"), &[&organization,&apps,&ids,&limit]).await? {
        let i = ordinal(&row, &sent)?;
        records[i].push(row);
    }
    Ok(records)
}

/// `list_records`' listing of one batched work whose outline answered: its
/// premise is the work's pin under the lock the batch holds, and its
/// publication's content is the one the outline re-admitted.
fn listing_of(
    (app, work): (&str, &str),
    can_write: bool,
    pinned: &Pinned,
    collection_rows: &[Row],
    record_rows: &[Row],
) -> Result<RecordsListing> {
    let (premise, content) = pinned.as_ref().map_err(|error| *error)?;
    let (_, content) = content.as_ref().map_err(|error| *error)?;
    let mut collections = Vec::with_capacity(collection_rows.len());
    let mut definition_digest = String::new();
    for row in collection_rows {
        let (collection, schema, schema_version) = (text(row, 1)?, text(row, 2)?, small(row, 3)?);
        let published = published_content(
            content,
            premise.definition_digest.clone(),
            &schema,
            i64::from(schema_version),
        )?;
        definition_digest = published.definition_digest.clone();
        collections.push(collection_summary(
            collection,
            schema,
            schema_version,
            &published,
        ));
    }
    let truncated = record_rows.len() as i64 > RECORDS_LIMIT;
    let mut records = Vec::with_capacity(record_rows.len());
    for row in record_rows.iter().take(RECORDS_LIMIT as usize) {
        records.push(record_row(row, 1, &collections)?);
    }
    Ok(RecordsListing {
        app: app.to_owned(),
        work: work.to_owned(),
        publication: premise.publication.clone(),
        definition_digest,
        work_version: premise.work_version,
        can_write,
        collections,
        records,
        truncated,
    })
}

/// What the canvas page reads for one work of a batch ([`Data::work_reads`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkReads {
    /// What `Data::work_outline` answers for the work.
    pub outline: std::result::Result<WorkOutline, Error>,
    /// What `Data::list_records` answers, for a work the caller asked it for
    /// whose outline answered.
    pub records: Option<std::result::Result<RecordsListing, Error>>,
}

impl Data {
    /// The canvas page's reads of many works (APP-113's deferred batching)
    /// in one READ COMMITTED data-role transaction of a fixed number of
    /// statements: per work `(app, work, records)`, the outline
    /// `work_outline` answers and, when `records`, the listing `list_records`
    /// answers, under the same authorization (`Transaction::fence_works`:
    /// the member precheck, `record_read` on the work before and after the
    /// member fence, `record_write` after it), the work's pinned publication
    /// under `lock_work_revision` and its stored definition re-admitted.
    /// Nothing is written. A member refusal is `Denied` for the batch (each
    /// work's own read would be `Denied`); a store failure ends it.
    pub async fn work_reads(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str, bool)],
    ) -> Result<Vec<WorkReads>> {
        let cap = Instant::now() + QUERY_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let org = request.organization.as_str();
            let keys: Vec<(&str, &str)> =
                works.iter().map(|(app, work, _)| (*app, *work)).collect();
            let (permit, grants) = transaction.fence_works(&keys).await?;
            let transaction = permit.transaction();
            let readable: Vec<usize> = (0..keys.len())
                .filter(|&i| valid_id(keys[i].0) && valid_id(keys[i].1) && grants[i].record_read)
                .collect();
            let readable_keys: Vec<(&str, &str)> = readable.iter().map(|&i| keys[i]).collect();
            let pinned = pinned_many(transaction, org, &readable_keys).await?;
            let mut reads: Vec<WorkReads> = (0..keys.len())
                .map(|_| WorkReads {
                    outline: Err(Error::Denied),
                    records: None,
                })
                .collect();
            let mut listed = Vec::new();
            for (&i, pinned) in readable.iter().zip(pinned) {
                reads[i].outline = outline_of(&pinned);
                if reads[i].outline.is_ok() && works[i].2 {
                    listed.push((i, pinned));
                }
            }
            let listed_keys: Vec<(&str, &str)> = listed.iter().map(|(i, _)| keys[*i]).collect();
            let collections = collections_many(transaction, org, &listed_keys).await?;
            let records = records_many(transaction, org, &listed_keys).await?;
            for (((i, pinned), collections), records) in
                listed.into_iter().zip(collections).zip(records)
            {
                reads[i].records = Some(listing_of(
                    keys[i],
                    grants[i].record_write,
                    &pinned,
                    &collections,
                    &records,
                ));
            }
            Ok(reads)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

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
                collections.push(collection_summary(
                    collection,
                    schema,
                    schema_version,
                    &published,
                ));
            }
            let limit = RECORDS_LIMIT + 1;
            let rows = transaction.query(&format!("SELECT {RECORD_COLUMNS} {RECORDS_FROM} WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid ORDER BY v.commit_sequence DESC,r.record_id LIMIT $4"), &[org,&app,&work,&limit]).await?;
            let truncated = rows.len() as i64 > RECORDS_LIMIT;
            let mut records = Vec::with_capacity(rows.len());
            for row in rows.iter().take(RECORDS_LIMIT as usize) {
                records.push(record_row(row, 0, &collections)?);
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

    /// APP-122 (Ask and Shape): the records listings of `works` (at most
    /// [`READ_ONLY_WORKS`]), read in one `READ ONLY` snapshot
    /// (`Transaction::begin_read_only`): the member checks and every work's
    /// `record_read` (`Transaction::read_only_grants`), each work's pin and
    /// re-admitted publication, its collections and its newest records, as
    /// `work_reads` reads them, with no row lock. Every work must be readable,
    /// else `Denied` for all (the routes answer the uniform 404). The
    /// transaction cannot write: PostgreSQL refuses any write in it.
    pub async fn read_only_listings(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str)],
    ) -> Result<Vec<RecordsListing>> {
        self.read_only_listings_with(request, works, false)
            .await
            .map(|(listings, _)| listings)
    }

    /// APP-122 acceptance ("a forced write attempt fails"): the same reads in
    /// the same kind of transaction, then one write; answers the SQLSTATE
    /// PostgreSQL refused it with (none: the write ran). Campaigns only.
    #[cfg(test)]
    pub async fn read_only_forced_write(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str)],
    ) -> Result<Option<String>> {
        self.read_only_listings_with(request, works, true)
            .await
            .map(|(_, refused)| refused)
    }

    async fn read_only_listings_with(
        &self,
        request: &TrustedRequest,
        works: &[(&str, &str)],
        force_write: bool,
    ) -> Result<(Vec<RecordsListing>, Option<String>)> {
        require(
            (1..=READ_ONLY_WORKS).contains(&works.len())
                && works
                    .iter()
                    .all(|(app, work)| valid_id(app) && valid_id(work)),
        )?;
        let cap = Instant::now() + QUERY_HANDLER_DEADLINE;
        let transaction = Transaction::begin_read_only(&self.inner, request, cap).await?;
        let result = async {
            let org = request.organization.as_str();
            let grants = transaction.read_only_grants(works).await?;
            require(grants.iter().all(|grant| grant.record_read))?;
            let pinned = pinned_many_with(&transaction, org, works, false).await?;
            for pin in &pinned {
                outline_of(pin)?;
            }
            let collections = collections_many(&transaction, org, works).await?;
            let records = records_many(&transaction, org, works).await?;
            let mut listings = Vec::with_capacity(works.len());
            for (((key, pin), collections), records) in
                works.iter().zip(&pinned).zip(&collections).zip(&records)
            {
                listings.push(listing_of(*key, false, pin, collections, records)?);
            }
            let refused = if force_write {
                match transaction
                    .query_classified(
                        "UPDATE genui.internal_records SET current_version=current_version WHERE organization_id=$1::text::uuid",
                        &[&org],
                    )
                    .await?
                {
                    Ok(_) => None,
                    Err(state) => Some(state.code().to_owned()),
                }
            } else {
                None
            };
            Ok((listings, refused))
        }
        .await;
        // A refused write aborted the transaction: roll back (nothing to keep).
        transaction.finish(result.is_ok() && !force_write).await?;
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
            let (current_version, current_values, creator) = current_state(
                transaction,
                org,
                app,
                work,
                &collection,
                &command.record,
                false,
            )
            .await?;
            if current_version != command.expected_record_version {
                return Ok(DryRun::Conflict { current_version });
            }
            require(if current_version == 0 {
                declaration.operation == "create"
            } else {
                declaration.operation == "update" || declaration.has_transitions()
            })?;
            let bytes = compose(
                &published.fields,
                declaration,
                current_values.as_ref(),
                &command.values,
                creator.as_deref() == Some(request.principal.as_str()),
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
