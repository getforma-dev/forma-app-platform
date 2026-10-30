//! APP-108 (ADR-011 pitch, `pivot/boxes`): boxes, their files, the source
//! revisions registered from them, the apps derived from them and who may
//! read what (migration 0011).
//!
//! Every operation is one READ COMMITTED workspace transaction under the
//! member fence (`fence_member`: an active organization and membership at
//! the request's epoch, a live entitlement, the `genui_lock` fence) and the
//! request's RLS scope, so a foreign organization's rows are unreachable and
//! every refusal is `Denied` (the route's uniform 404). Who sees a box: its
//! owner, a member it was shared with (`box_shares`), and a member holding
//! a grant on one of its files (`box_file_grants`, written when a derived
//! app is shared), who then sees exactly those files and nothing else of the
//! box. A file grant counts only while the grantee can still open the work
//! it was granted through (`record_read` on that work, judged now; APP-113
//! review). Only the owner adds files, registers data or derives apps.
//! Writes append one workspace-chain audit event each (`box.created`,
//! `box.renamed`, `box.file_added`, `box.data_registered`,
//! `box.app_derived`, `box.shared`, and since 0012 `box.explained` and
//! `box.briefed`); the source ledger and the owner's `source_read` are
//! written only by the definer `genui_private.register_box_source` (0011,
//! idempotent per file and normalized digest since 0012).
use super::{
    authorization::{record_read_clause, Action, Resource, Transaction},
    definitions::{append, confirm_identity, Chain, Event},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use crate::admission;
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline for each transaction here.
pub const BOX_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many files in one box (ticket APP-108).
pub const MAX_BOX_FILES: i64 = 20;
/// At most this many boxes a member owns.
pub const MAX_OWNED_BOXES: i64 = 50;
/// Boxes listed for one member.
const BOX_LIST_LIMIT: i64 = 100;

/// One box as a member sees it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxRow {
    pub box_id: String,
    pub name: String,
    /// `box`, `uploads` or `app`.
    pub kind: String,
    pub app_id: Option<String>,
    /// The member owns it (else it was shared, or one of its files was).
    pub owner: bool,
    /// The files this member may see.
    pub file_count: i64,
    pub created_at: String,
}

/// A registered source revision of a box file (`box_file_sources`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileData {
    pub source_id: String,
    pub revision_id: String,
    pub normalized_digest: String,
    pub row_count: i64,
    /// The revision envelope's input contract (`id`, `contract_id`, `fields`, …).
    pub contract: Value,
    pub contract_digest: String,
    pub created_at: String,
}

/// One file of a box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxFile {
    pub box_id: String,
    pub file_id: String,
    pub name: String,
    /// `pdf`, `image`, `csv`, `xlsx` or `text`.
    pub kind: String,
    pub media_type: String,
    pub bytes: i64,
    pub content_digest: String,
    pub object_key: String,
    pub created_at: String,
    /// The newest registration, if any.
    pub data: Option<FileData>,
}

/// An app built from the box, or a file's data card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Derivation {
    pub app_id: String,
    pub work_id: String,
    /// `derived` or `data`.
    pub kind: String,
    pub file_ids: Vec<String>,
    pub file_digests: Vec<String>,
    pub created_at: String,
}

/// A box with the files and derived apps this member may see.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoxView {
    pub row: BoxRow,
    pub files: Vec<BoxFile>,
    pub apps: Vec<Derivation>,
}

/// A derived work's provenance (the "Built from box" chip, receipts' links).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lineage {
    pub box_id: String,
    pub box_name: String,
    /// `derived` or `data`.
    pub kind: String,
    /// `(file, registration)` of every file the derivation used that this
    /// member may read; the registration is the revision the work binds.
    pub files: Vec<(BoxFile, Option<FileData>)>,
    pub file_count: usize,
}

/// A new file's identity, as the route stored the object.
#[derive(Clone, Debug)]
pub struct NewFile<'a> {
    pub file_id: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub media_type: &'a str,
    pub bytes: i64,
    pub content_digest: &'a str,
    pub object_key: &'a str,
}

/// A registration the route prepared (objects written, envelope built).
#[derive(Clone, Debug)]
pub struct NewRegistration<'a> {
    pub source_id: &'a str,
    pub revision_id: &'a str,
    pub content_digest: &'a str,
    pub normalized_digest: &'a str,
    pub contract_digest: &'a str,
    pub envelope: &'a [u8],
    pub row_count: i64,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn texts(row: &Row, index: usize) -> Result<Vec<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// The domain-separated digest an audit event's outcome carries.
fn outcome(kind: &str, parts: &[&str]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("forma:box-{kind}:v1\u{0}").as_bytes());
    hasher.update(parts.join("\u{0}").as_bytes());
    format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// The SHA-256 of a stored body, as an audit outcome part.
fn body_digest(body: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Serializes one member's counted writes of one kind (APP-113 review,
/// storage 5): a transaction-scoped advisory lock on the scope, the member
/// and `what`, taken under the fence before counting, so two concurrent
/// requests cannot both pass a count of `limit - 1`. It needs no grant,
/// is released at COMMIT or ROLLBACK and waits under `lock_timeout`.
pub(super) async fn serialize_member(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    what: &str,
) -> Result<()> {
    let key = format!(
        "genui:{what}:{}:{}",
        request.organization, request.principal
    );
    transaction
        .query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))::text",
            &[&key],
        )
        .await?;
    Ok(())
}

/// A box name: 1–60 characters, no control characters, trimmed.
pub fn box_name(name: &str) -> Option<String> {
    let name = name.trim();
    ((1..=60).contains(&name.chars().count()) && !name.chars().any(char::is_control))
        .then(|| name.to_owned())
}

const TIME: &str =
    "to_char(date_trunc('second',{}) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')";
fn time(column: &str) -> String {
    TIME.replace("{}", column)
}

/// A grant on a file of box `b` held by `$2` (the principal) that still
/// counts: the grantee can open the work it was granted through now.
/// `file` narrows it to one file (`AND g.file_id=…`) or is empty.
fn granted(file: &str) -> String {
    format!(
        "EXISTS(SELECT 1 FROM genui.box_file_grants g JOIN genui.saved_work w ON w.organization_id=g.organization_id AND w.app_id=g.app_id AND w.work_id=g.work_id WHERE g.organization_id=b.organization_id AND g.box_id=b.box_id{file} AND g.principal_id=$2::text::uuid AND {})",
        record_read_clause("g.principal_id")
    )
}
/// Whether the principal sees the whole box (owner or shared).
const WHOLE: &str = "(b.owner_principal_id=$2::text::uuid OR EXISTS(SELECT 1 FROM genui.box_shares s WHERE s.organization_id=b.organization_id AND s.box_id=b.box_id AND s.principal_id=$2::text::uuid))";
/// Whether file `f` of box `b` is visible to `$2`: the whole box, or a live
/// grant on that file.
fn file_visible() -> String {
    format!("({WHOLE} OR {})", granted(" AND g.file_id=f.file_id"))
}

async fn box_rows(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    only: Option<&str>,
) -> Result<Vec<BoxRow>> {
    let sql = format!(
        "SELECT b.box_id::text,b.name,b.kind,b.app_id::text,b.owner_principal_id=$2::text::uuid,{created},(SELECT count(*) FROM genui.box_files f WHERE f.organization_id=b.organization_id AND f.box_id=b.box_id AND {visible}) FROM genui.boxes b WHERE b.organization_id=$1::text::uuid AND ({WHOLE} OR {any}) AND ($3::text IS NULL OR b.box_id=$3::text::uuid) ORDER BY b.created_at,b.box_id LIMIT $4",
        created = time("b.created_at"),
        visible = file_visible(),
        any = granted(""),
    );
    let rows = transaction
        .query(
            &sql,
            &[
                &request.organization,
                &request.principal,
                &only,
                &BOX_LIST_LIMIT,
            ],
        )
        .await?;
    rows.iter()
        .map(|row| {
            Ok(BoxRow {
                box_id: text(row, 0)?,
                name: text(row, 1)?,
                kind: text(row, 2)?,
                app_id: optional(row, 3)?,
                owner: boolean(row, 4)?,
                created_at: text(row, 5)?,
                file_count: integer(row, 6)?,
            })
        })
        .collect()
}

/// One `box_file_sources` row joined with its revision, as selected by
/// `registration_columns` (envelope decoded).
fn registered_row(row: &Row) -> Result<(String, FileData)> {
    let envelope: Vec<u8> = row.try_get(6).map_err(|_| Error::Unavailable)?;
    let envelope = admission::decode(&envelope).map_err(|_| Error::Unavailable)?;
    Ok((
        text(row, 0)?,
        FileData {
            source_id: text(row, 1)?,
            revision_id: text(row, 2)?,
            normalized_digest: text(row, 3)?,
            row_count: integer(row, 4)?,
            contract_digest: text(row, 5)?,
            contract: envelope
                .get("input_contract")
                .cloned()
                .ok_or(Error::Unavailable)?,
            created_at: text(row, 7)?,
        },
    ))
}
/// The columns `registration` reads, over `box_file_sources s` joined with
/// `source_revisions r`.
fn registration_columns() -> String {
    format!("s.file_id::text,s.source_id::text,s.revision_id::text,s.normalized_digest::text,s.row_count,r.contract_digest::text,r.canonical_envelope,{} FROM genui.box_file_sources s JOIN genui.source_revisions r ON r.organization_id=s.organization_id AND r.source_id=s.source_id AND r.revision_id=s.revision_id", time("s.created_at"))
}

/// The newest registration of each file of `box_id` (envelope decoded).
async fn registrations(
    transaction: &Transaction<'_>,
    organization: &str,
    box_id: &str,
) -> Result<Vec<(String, FileData)>> {
    let sql = format!("SELECT DISTINCT ON (s.file_id) {} WHERE s.organization_id=$1::text::uuid AND s.box_id=$2::text::uuid AND r.deleted_at IS NULL ORDER BY s.file_id,s.created_at DESC,s.revision_id", registration_columns());
    let rows = transaction.query(&sql, &[&organization, &box_id]).await?;
    rows.iter().map(registered_row).collect()
}

async fn file_rows(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    box_id: &str,
    only: Option<&str>,
) -> Result<Vec<BoxFile>> {
    let sql = format!("SELECT f.file_id::text,f.name,f.kind,f.media_type,f.byte_length,f.content_digest::text,f.object_key,{} FROM genui.box_files f JOIN genui.boxes b USING(organization_id,box_id) WHERE f.organization_id=$1::text::uuid AND f.box_id=$3::text::uuid AND ($4::text IS NULL OR f.file_id=$4::text::uuid) AND {} ORDER BY f.created_at,f.file_id LIMIT $5", time("f.created_at"), file_visible());
    let rows = transaction
        .query(
            &sql,
            &[
                &request.organization,
                &request.principal,
                &box_id,
                &only,
                &MAX_BOX_FILES,
            ],
        )
        .await?;
    let registered = registrations(transaction, &request.organization, box_id).await?;
    rows.iter()
        .map(|row| {
            let file_id = text(row, 0)?;
            let data = registered
                .iter()
                .find(|(file, _)| *file == file_id)
                .map(|(_, data)| data.clone());
            Ok(BoxFile {
                box_id: box_id.to_owned(),
                file_id,
                name: text(row, 1)?,
                kind: text(row, 2)?,
                media_type: text(row, 3)?,
                bytes: integer(row, 4)?,
                content_digest: text(row, 5)?,
                object_key: text(row, 6)?,
                created_at: text(row, 7)?,
                data,
            })
        })
        .collect()
}

async fn derivations(
    transaction: &Transaction<'_>,
    organization: &str,
    box_id: &str,
) -> Result<Vec<Derivation>> {
    let sql = format!("SELECT app_id::text,work_id::text,kind,file_ids::text[],file_digests,{} FROM genui.box_derivations WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid ORDER BY created_at,derivation_id LIMIT 64", time("created_at"));
    let rows = transaction.query(&sql, &[&organization, &box_id]).await?;
    rows.iter()
        .map(|row| {
            Ok(Derivation {
                app_id: text(row, 0)?,
                work_id: text(row, 1)?,
                kind: text(row, 2)?,
                file_ids: texts(row, 3)?,
                file_digests: texts(row, 4)?,
                created_at: text(row, 5)?,
            })
        })
        .collect()
}

/// After `share_work`, in the share's own transaction (APP-113 review,
/// storage 11): the grantee may read exactly the box files the shared work
/// was built from. `transaction` is already fenced with the sharer's
/// `record_read` on the work; the sharer must also see the whole box (else
/// nothing is granted). Returns how many grants were written (one
/// `box.shared` event when > 0); the grantee's active membership was judged
/// by `share_work`.
pub(super) async fn grant_box_files_for_work(
    t: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
    grantee: &str,
) -> Result<usize> {
    let rows = t.query(&format!("SELECT d.box_id::text,d.file_ids::text[] FROM genui.box_derivations d JOIN genui.boxes b USING(organization_id,box_id) WHERE d.organization_id=$1::text::uuid AND d.app_id=$3::text::uuid AND d.work_id=$4::text::uuid AND {WHOLE}"), &[&request.organization,&request.principal,&app,&work]).await?;
    let Some(row) = rows.first() else {
        return Ok(0);
    };
    let box_id = text(row, 0)?;
    let files = texts(row, 1)?;
    let mut inserted = 0;
    for file in &files {
        let rows = t.query("INSERT INTO genui.box_file_grants(organization_id,box_id,file_id,principal_id,app_id,work_id,granted_by,created_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text::uuid,$7::text::uuid,date_trunc('second',now())) ON CONFLICT DO NOTHING RETURNING 1", &[&request.organization,&box_id,file,&grantee,&app,&work,&request.principal]).await?;
        inserted += rows.len();
    }
    if inserted > 0 {
        let digest = outcome("shared", &[&box_id, grantee, &files.join(",")]);
        append(
            t,
            Chain::Workspace,
            request,
            Event {
                kind: "box.shared",
                id: None,
                app: Some(app),
                work: Some(work),
                record: None,
                outcome: Some(&digest),
            },
        )
        .await?;
    }
    Ok(inserted)
}

impl Workspace {
    async fn boxes_transaction<'a, T, F>(
        &'a self,
        request: &'a TrustedRequest,
        body: F,
    ) -> Result<T>
    where
        F: for<'t> FnOnce(
            &'t mut Transaction<'a>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T>> + Send + 't>,
        >,
    {
        let cap = Instant::now() + BOX_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = body(&mut transaction).await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// `GET /v1/boxes`: the boxes this member may see, oldest first.
    pub async fn list_boxes(&self, request: &TrustedRequest) -> Result<Vec<BoxRow>> {
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                box_rows(permit.transaction(), request, None).await
            })
        })
        .await
    }

    /// One box with its visible files and (owner or shared) derived apps.
    pub async fn read_box(&self, request: &TrustedRequest, box_id: &str) -> Result<BoxView> {
        require(valid_id(box_id))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let row = box_rows(t, request, Some(box_id))
                    .await?
                    .into_iter()
                    .next()
                    .ok_or(Error::Denied)?;
                let files = file_rows(t, request, box_id, None).await?;
                let whole = row.owner
                    || !t.query("SELECT 1 FROM genui.box_shares WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid AND principal_id=$3::text::uuid", &[&request.organization,&box_id,&request.principal]).await?.is_empty();
                let apps = if whole {
                    derivations(t, &request.organization, box_id).await?
                } else {
                    Vec::new()
                };
                Ok(BoxView { row, files, apps })
            })
        })
        .await
    }

    /// One file this member may read.
    pub async fn read_box_file(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        file_id: &str,
    ) -> Result<BoxFile> {
        require(valid_id(box_id) && valid_id(file_id))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                file_rows(permit.transaction(), request, box_id, Some(file_id))
                    .await?
                    .into_iter()
                    .next()
                    .ok_or(Error::Denied)
            })
        })
        .await
    }

    /// Creates a box the member owns. `kind` `uploads` and `app` are one per
    /// member (per app): an existing one is returned unchanged. An `app` box
    /// needs `run` on the app.
    pub async fn create_box(
        &self,
        request: &TrustedRequest,
        name: &str,
        kind: &str,
        app: Option<&str>,
    ) -> Result<BoxRow> {
        let name = box_name(name).ok_or(Error::Denied)?;
        require(matches!(
            (kind, app),
            ("box", None) | ("uploads", None) | ("app", Some(_))
        ))?;
        if let Some(app) = app {
            require(valid_id(app))?;
        }
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                if let Some(app) = app {
                    permit.also(Action::Run, Resource::App(app)).await?;
                }
                let t = permit.transaction();
                // The member's box writes in order: the one-per-kind lookup
                // and the count both see every earlier committed box.
                serialize_member(t, request, "boxes").await?;
                let existing = || async {
                    let rows = t.query("SELECT box_id::text FROM genui.boxes WHERE organization_id=$1::text::uuid AND owner_principal_id=$2::text::uuid AND kind=$3 AND ($4::text IS NULL OR app_id=$4::text::uuid)", &[&request.organization,&request.principal,&kind,&app]).await?;
                    match rows.first() {
                        Some(row) => Ok::<_, Error>(Some(text(row, 0)?)),
                        None => Ok(None),
                    }
                };
                if kind != "box" {
                    if let Some(id) = existing().await? {
                        return box_rows(t, request, Some(&id))
                            .await?
                            .into_iter()
                            .next()
                            .ok_or(Error::Unavailable);
                    }
                }
                let owned = t.query("SELECT count(*) FROM genui.boxes WHERE organization_id=$1::text::uuid AND owner_principal_id=$2::text::uuid", &[&request.organization,&request.principal]).await?;
                require(integer(owned.first().ok_or(Error::Unavailable)?, 0)? < MAX_OWNED_BOXES)?;
                let fresh = t.query("SELECT gen_random_uuid()::text", &[]).await?;
                let id = text(fresh.first().ok_or(Error::Unavailable)?, 0)?;
                // The one-per-member `uploads` and one-per-app boxes are
                // unique indexes: a racing creator's row wins and is returned
                // (review storage 13), never a unique-violation 503.
                let inserted = t.query("INSERT INTO genui.boxes(organization_id,box_id,owner_principal_id,name,kind,app_id,created_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4,$5,$6::text::uuid,date_trunc('second',now())) ON CONFLICT DO NOTHING RETURNING 1", &[&request.organization,&id,&request.principal,&name,&kind,&app]).await?;
                if inserted.is_empty() {
                    let id = existing().await?.ok_or(Error::Unavailable)?;
                    return box_rows(t, request, Some(&id))
                        .await?
                        .into_iter()
                        .next()
                        .ok_or(Error::Unavailable);
                }
                let digest = outcome("created", &[&id, kind]);
                append(t, Chain::Workspace, request, Event { kind: "box.created", id: None, app, work: None, record: None, outcome: Some(&digest) }).await?;
                box_rows(t, request, Some(&id))
                    .await?
                    .into_iter()
                    .next()
                    .ok_or(Error::Unavailable)
            })
        })
        .await
    }

    /// Whether the owner may add one more file to `box_id` now (the route
    /// checks before it stores anything).
    pub async fn box_has_room(&self, request: &TrustedRequest, box_id: &str) -> Result<bool> {
        let view = self.read_box(request, box_id).await?;
        require(view.row.owner)?;
        Ok((view.files.len() as i64) < MAX_BOX_FILES)
    }

    /// Records a file whose object the route has stored (owner only; at most
    /// `MAX_BOX_FILES`, re-counted under the fence: `Ok(None)` when full).
    pub async fn add_box_file(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        file: &NewFile<'_>,
    ) -> Result<Option<BoxFile>> {
        require(valid_id(box_id) && valid_id(file.file_id))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                // The box row FOR UPDATE (the owner's UPDATE(name) grant),
                // then the count in its own statement, so it sees every file
                // a concurrent add committed before this lock was granted.
                let owned = t.query("SELECT 1 FROM genui.boxes b WHERE b.organization_id=$1::text::uuid AND b.box_id=$2::text::uuid AND b.owner_principal_id=$3::text::uuid FOR UPDATE", &[&request.organization,&box_id,&request.principal]).await?;
                require(!owned.is_empty())?;
                let counted = t.query("SELECT count(*) FROM genui.box_files WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid", &[&request.organization,&box_id]).await?;
                let count = integer(counted.first().ok_or(Error::Unavailable)?, 0)?;
                if count >= MAX_BOX_FILES {
                    return Ok(None);
                }
                t.query("INSERT INTO genui.box_files(organization_id,box_id,file_id,name,kind,media_type,byte_length,content_digest,object_key,uploaded_by,created_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4,$5,$6,$7::bigint,$8::text,$9,$10::text::uuid,date_trunc('second',now()))", &[&request.organization,&box_id,&file.file_id,&file.name,&file.kind,&file.media_type,&file.bytes,&file.content_digest,&file.object_key,&request.principal]).await?;
                let digest = outcome("file", &[box_id, file.file_id, file.content_digest]);
                append(t, Chain::Workspace, request, Event { kind: "box.file_added", id: None, app: None, work: None, record: None, outcome: Some(&digest) }).await?;
                Ok(file_rows(t, request, box_id, Some(file.file_id))
                    .await?
                    .into_iter()
                    .next())
            })
        })
        .await
    }

    /// Registers a confirmed CSV/XLSX of a box the member owns as a new
    /// source revision (the definer), audited, and returns the registration
    /// as stored. Idempotent per file and normalized digest (0012): a repeat
    /// of the same normalized content returns the existing registration (its
    /// source, revision and contract; the caller's fresh ids are unused) and
    /// appends no second event.
    pub async fn register_box_data(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        file_id: &str,
        registration: &NewRegistration<'_>,
    ) -> Result<FileData> {
        require(
            valid_id(box_id)
                && valid_id(file_id)
                && valid_id(registration.source_id)
                && valid_id(registration.revision_id),
        )?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let rows = t.query("SELECT source_id::text,revision_id::text FROM genui_private.register_box_source($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text::uuid,$7,$8,$9,$10,$11::bigint)", &[&request.organization,&request.principal,&box_id,&file_id,&registration.source_id,&registration.revision_id,&registration.content_digest,&registration.normalized_digest,&registration.contract_digest,&registration.envelope,&registration.row_count]).await?;
                require(rows.len() == 1)?;
                let (source, revision) = (text(&rows[0], 0)?, text(&rows[0], 1)?);
                if source == registration.source_id && revision == registration.revision_id {
                    let digest = outcome("data", &[box_id, file_id, registration.source_id, registration.revision_id, registration.normalized_digest]);
                    append(t, Chain::Workspace, request, Event { kind: "box.data_registered", id: None, app: None, work: None, record: None, outcome: Some(&digest) }).await?;
                }
                let stored = t.query(&format!("SELECT {} WHERE s.organization_id=$1::text::uuid AND s.box_id=$2::text::uuid AND s.file_id=$3::text::uuid AND s.source_id=$4::text::uuid AND s.revision_id=$5::text::uuid", registration_columns()), &[&request.organization,&box_id,&file_id,&source,&revision]).await?;
                Ok(registered_row(stored.first().ok_or(Error::Unavailable)?)?.1)
            })
        })
        .await
    }

    /// Records that `work` of `app` was built from files of a box the member
    /// owns (`derived` or `data`), audited. The member must hold
    /// `record_write` on the work (its unrevoked creator does), and every
    /// `(file, digest)` pair must name a file of that box with exactly that
    /// content digest (paired, not two independent sets; APP-113 review).
    pub async fn record_box_derivation(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        app: &str,
        work: &str,
        kind: &str,
        files: &[(String, String)],
    ) -> Result<()> {
        require(
            valid_id(box_id)
                && valid_id(app)
                && valid_id(work)
                && matches!(kind, "derived" | "data")
                && (1..=MAX_BOX_FILES as usize).contains(&files.len())
                && files.iter().all(|(id, _)| valid_id(id))
                && files
                    .iter()
                    .enumerate()
                    .all(|(i, (id, _))| files[..i].iter().all(|(other, _)| other != id)),
        )?;
        let ids: Vec<String> = files.iter().map(|(id, _)| id.clone()).collect();
        let digests: Vec<String> = files.iter().map(|(_, d)| d.clone()).collect();
        let cap = Instant::now() + BOX_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::RecordWrite, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let t = permit.transaction();
            let owned = t.query("SELECT count(*) FROM unnest($4::text[],$5::text[]) AS u(file_id,digest) JOIN genui.box_files f ON f.organization_id=$1::text::uuid AND f.box_id=$2::text::uuid AND f.file_id::text=u.file_id AND f.content_digest::text=u.digest JOIN genui.boxes b ON b.organization_id=f.organization_id AND b.box_id=f.box_id WHERE b.owner_principal_id=$3::text::uuid", &[&request.organization,&box_id,&request.principal,&ids,&digests]).await?;
            require(integer(owned.first().ok_or(Error::Unavailable)?, 0)? == ids.len() as i64)?;
            t.query("INSERT INTO genui.box_derivations(organization_id,derivation_id,box_id,app_id,work_id,kind,file_ids,file_digests,created_by,created_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text::uuid,$3::text::uuid,$4::text::uuid,$5,$6::text[]::uuid[],$7,$8::text::uuid,date_trunc('second',now()))", &[&request.organization,&box_id,&app,&work,&kind,&ids,&digests,&request.principal]).await?;
            let mut parts = vec![box_id, kind];
            parts.extend(digests.iter().map(String::as_str));
            let digest = outcome("derived", &parts);
            append(t, Chain::Workspace, request, Event { kind: "box.app_derived", id: None, app: Some(app), work: Some(work), record: None, outcome: Some(&digest) }).await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The provenance of a work this member can open now; `Ok(None)` when it
    /// was not built from a box.
    pub async fn box_lineage(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Option<Lineage>> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + BOX_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let t = permit.transaction();
            let rows = t.query("SELECT d.box_id::text,b.name,d.kind,d.file_ids::text[] FROM genui.box_derivations d JOIN genui.boxes b USING(organization_id,box_id) WHERE d.organization_id=$1::text::uuid AND d.app_id=$2::text::uuid AND d.work_id=$3::text::uuid", &[&request.organization,&app,&work]).await?;
            let Some(row) = rows.first() else {
                return Ok(None);
            };
            let box_id = text(row, 0)?;
            let used = texts(row, 3)?;
            let bound = t.query("SELECT source_id::text,source_revision_id::text FROM genui.work_source_dependencies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&request.organization,&app,&work]).await?;
            let bound: Vec<(String, String)> = bound
                .iter()
                .map(|r| Ok((text(r, 0)?, text(r, 1)?)))
                .collect::<Result<_>>()?;
            let all = t.query(&format!("SELECT {} WHERE s.organization_id=$1::text::uuid AND s.box_id=$2::text::uuid", registration_columns()), &[&request.organization,&box_id]).await?;
            let all = all.iter().map(registered_row).collect::<Result<Vec<_>>>()?;
            let files = file_rows(t, request, &box_id, None).await?;
            let mut out = Vec::new();
            for file in files.into_iter().filter(|f| used.contains(&f.file_id)) {
                let data = all
                    .iter()
                    .find(|(id, data)| {
                        *id == file.file_id
                            && bound.contains(&(data.source_id.clone(), data.revision_id.clone()))
                    })
                    .map(|(_, data)| data.clone());
                out.push((file, data));
            }
            Ok(Some(Lineage {
                box_id,
                box_name: text(row, 1)?,
                kind: text(row, 2)?,
                file_count: used.len(),
                files: out,
            }))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Shares a whole box the member owns with an active teammate.
    pub async fn share_box(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        grantee: &str,
    ) -> Result<bool> {
        require(valid_id(box_id) && valid_id(grantee) && grantee != request.principal)?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let owned = t.query("SELECT 1 FROM genui.boxes WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid AND owner_principal_id=$3::text::uuid", &[&request.organization,&box_id,&request.principal]).await?;
                require(!owned.is_empty())?;
                let member = t.query("SELECT 1 FROM genui.memberships WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND status='active'", &[&request.organization,&grantee]).await?;
                require(!member.is_empty())?;
                let rows = t.query("INSERT INTO genui.box_shares(organization_id,box_id,principal_id,granted_by,created_at) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,date_trunc('second',now())) ON CONFLICT DO NOTHING RETURNING 1", &[&request.organization,&box_id,&grantee,&request.principal]).await?;
                if rows.is_empty() {
                    return Ok(false);
                }
                let digest = outcome("shared", &[box_id, grantee]);
                append(t, Chain::Workspace, request, Event { kind: "box.shared", id: None, app: None, work: None, record: None, outcome: Some(&digest) }).await?;
                Ok(true)
            })
        })
        .await
    }

    /// Renames a box the member owns (audited `box.renamed`).
    pub async fn rename_box(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        name: &str,
    ) -> Result<BoxRow> {
        let name = box_name(name).ok_or(Error::Denied)?;
        require(valid_id(box_id))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let rows = t.query("UPDATE genui.boxes SET name=$4 WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid AND owner_principal_id=$3::text::uuid AND kind<>'uploads' RETURNING 1", &[&request.organization,&box_id,&request.principal,&name]).await?;
                require(rows.len() == 1)?;
                let digest = outcome("renamed", &[box_id, &name]);
                append(t, Chain::Workspace, request, Event { kind: "box.renamed", id: None, app: None, work: None, record: None, outcome: Some(&digest) }).await?;
                box_rows(t, request, Some(box_id)).await?.into_iter().next().ok_or(Error::Unavailable)
            })
        })
        .await
    }

    /// Stores a folder's brief (owner only), audited `box.briefed` (0012).
    pub async fn save_box_brief(
        &self,
        request: &TrustedRequest,
        box_id: &str,
        body: &[u8],
    ) -> Result<String> {
        require(valid_id(box_id) && (1..=131072).contains(&body.len()))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let owned = t.query("SELECT 1 FROM genui.boxes WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid AND owner_principal_id=$3::text::uuid", &[&request.organization,&box_id,&request.principal]).await?;
                require(!owned.is_empty())?;
                let rows = t.query(&format!("INSERT INTO genui.box_briefs(organization_id,brief_id,box_id,body,created_by,created_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text::uuid,$3,$4::text::uuid,date_trunc('second',now())) RETURNING {},brief_id::text", time("created_at")), &[&request.organization,&box_id,&body,&request.principal]).await?;
                let row = rows.first().ok_or(Error::Unavailable)?;
                let digest = outcome("briefed", &[box_id, &text(row, 1)?, &body_digest(body)]);
                append(t, Chain::Workspace, request, Event { kind: "box.briefed", id: None, app: None, work: None, record: None, outcome: Some(&digest) }).await?;
                text(row, 0)
            })
        })
        .await
    }

    /// The newest brief of a box this member sees whole (owner or shared).
    pub async fn read_box_brief(
        &self,
        request: &TrustedRequest,
        box_id: &str,
    ) -> Result<Option<(Vec<u8>, String)>> {
        require(valid_id(box_id))?;
        self.boxes_transaction(request, |transaction| {
            Box::pin(async move {
                confirm_identity(transaction, request).await?;
                let permit = transaction.fence_member().await?;
                let t = permit.transaction();
                let visible = t.query(&format!("SELECT 1 FROM genui.boxes b WHERE b.organization_id=$1::text::uuid AND b.box_id=$3::text::uuid AND {WHOLE}"), &[&request.organization,&request.principal,&box_id]).await?;
                require(!visible.is_empty())?;
                let rows = t.query(&format!("SELECT body,{} FROM genui.box_briefs WHERE organization_id=$1::text::uuid AND box_id=$2::text::uuid ORDER BY created_at DESC,brief_id LIMIT 1", time("created_at")), &[&request.organization,&box_id]).await?;
                match rows.first() {
                    Some(row) => Ok(Some((row.try_get::<_, Vec<u8>>(0).map_err(|_| Error::Unavailable)?, text(row, 1)?))),
                    None => Ok(None),
                }
            })
        })
        .await
    }

    /// Stores a validated summary template for a work the member may write
    /// (`record_write`, which its unrevoked creator holds; APP-113 review),
    /// audited `box.explained` (0012). A reader without write authority
    /// reads the stored template and never replaces it.
    pub async fn save_box_explanation(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        template: &[u8],
    ) -> Result<String> {
        require(valid_id(app) && valid_id(work) && (1..=16384).contains(&template.len()))?;
        let cap = Instant::now() + BOX_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            confirm_identity(&transaction, request).await?;
            let precheck = transaction
                .precheck(Action::RecordWrite, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let t = permit.transaction();
            let rows = t.query(&format!("INSERT INTO genui.box_explanations(organization_id,explanation_id,app_id,work_id,template,created_by,created_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text::uuid,$3::text::uuid,$4,$5::text::uuid,date_trunc('second',now())) RETURNING {},explanation_id::text", time("created_at")), &[&request.organization,&app,&work,&template,&request.principal]).await?;
            let row = rows.first().ok_or(Error::Unavailable)?;
            let digest = outcome("explained", &[app, work, &text(row, 1)?, &body_digest(template)]);
            append(t, Chain::Workspace, request, Event { kind: "box.explained", id: None, app: Some(app), work: Some(work), record: None, outcome: Some(&digest) }).await?;
            text(row, 0)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The newest summary template of a work the member can open.
    pub async fn read_box_explanation(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Option<(Vec<u8>, String)>> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + BOX_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let rows = permit.transaction().query(&format!("SELECT template,{} FROM genui.box_explanations WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY created_at DESC,explanation_id LIMIT 1", time("created_at")), &[&request.organization,&app,&work]).await?;
            match rows.first() {
                Some(row) => Ok(Some((
                    row.try_get::<_, Vec<u8>>(0).map_err(|_| Error::Unavailable)?,
                    text(row, 1)?,
                ))),
                None => Ok(None),
            }
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_names_are_trimmed_bounded_and_plain() {
        assert_eq!(box_name("  Q3 board pack "), Some("Q3 board pack".into()));
        assert_eq!(box_name(""), None);
        assert_eq!(box_name("   "), None);
        assert_eq!(box_name("a\u{7}b"), None);
        assert_eq!(box_name(&"x".repeat(61)), None);
        assert!(box_name(&"é".repeat(60)).is_some());
    }

    #[test]
    fn outcome_digests_are_domain_separated() {
        assert_ne!(outcome("file", &["a", "b"]), outcome("data", &["a", "b"]));
        assert_ne!(outcome("file", &["ab", "c"]), outcome("file", &["a", "bc"]));
        assert!(outcome("file", &["a"]).starts_with("sha256:"));
    }
}
