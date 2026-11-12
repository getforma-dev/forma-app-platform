//! Named, view-only sharing. Account authority is independent of membership.
//! A locator is never a bearer grant. Every release checks the account session,
//! accepted recipient, grant version, frozen publication/work configuration and
//! the sponsor's current authority. No recipient organization grants are copied.
use super::{
    authorization::{record_read_clause, Action, Resource, Transaction},
    definitions::{append, Chain, Event},
    pool::{Data, RolePool, Workspace},
    queries::{outline_of, pinned_many_with},
    require, valid_id, Error, Result, ShareAccount, TrustedRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio_postgres::Row;

pub const MAX_LIVE_SHARES: i64 = 100;
pub const MAX_LINKED_APPS: i64 = 100;

/// Invitation matching deliberately uses one conservative, server-owned policy.
/// No aliases, plus-address stripping, or identity merging by email.
pub fn normalize_share_email(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() > 320 || !value.is_ascii() || value.bytes().any(|b| b <= 32 || b == 127) {
        return None;
    }
    let (local, domain) = value.split_once('@')?;
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

fn default_share_days() -> i64 {
    7
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewLiveShare {
    pub recipient_email: String,
    #[serde(default)]
    pub allow_add_to_canvas: bool,
    #[serde(default = "default_share_days")]
    pub expires_in_days: i64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LiveShare {
    pub share_id: String,
    pub key: String,
    pub recipient_email: String,
    pub allow_add_to_canvas: bool,
    pub version: i64,
    pub expires_at: String,
    pub accepted: bool,
    pub revoked: bool,
    pub status: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LinkedApp {
    pub key: String,
    pub share_id: String,
    pub title: String,
    pub organization_id: String,
    pub app_id: String,
    pub work_id: String,
    pub can_add_to_canvas: bool,
}
/// Server-side provider routing only. Never serialized into the public landing
/// page: an invitation locator does not disclose its named recipient.
#[derive(Clone, Debug)]
pub struct SharedAuthTarget {
    pub organization: String,
    pub share: String,
    pub recipient_email: String,
}
/// Constructible only by the account/session plus accepted-grant resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestAccess {
    pub(super) account: ShareAccount,
    pub(super) organization: String,
    pub(super) app: String,
    pub(super) work: String,
    pub(super) publication: String,
    pub(super) definition_digest: String,
    pub(super) work_version: i64,
    pub(super) policy_epoch: i64,
    pub(super) share: String,
    pub(super) version: i64,
    pub(super) sponsor: String,
    pub(super) sponsor_epoch: i64,
    pub(super) views: Vec<String>,
    pub(super) collections: Vec<String>,
    pub(super) can_add: bool,
    pub(super) expires_at: String,
    pub(super) owner_label: String,
}
impl GuestAccess {
    pub fn organization(&self) -> &str {
        &self.organization
    }
    pub fn app(&self) -> &str {
        &self.app
    }
    pub fn work(&self) -> &str {
        &self.work
    }
    pub fn publication(&self) -> &str {
        &self.publication
    }
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }
    pub fn work_version(&self) -> i64 {
        self.work_version
    }
    pub fn share_id(&self) -> &str {
        &self.share
    }
    pub fn share_version(&self) -> i64 {
        self.version
    }
    pub fn account_id(&self) -> &str {
        self.account.account_id()
    }
    pub fn can_add_to_canvas(&self) -> bool {
        self.can_add
    }
    pub fn view_ids(&self) -> &[String] {
        &self.views
    }
    pub fn collection_ids(&self) -> &[String] {
        &self.collections
    }
    pub fn expires_at(&self) -> &str {
        &self.expires_at
    }
    pub fn owner_label(&self) -> &str {
        &self.owner_label
    }
    pub fn key(&self) -> String {
        format!("{}.{}", self.organization, self.share)
    }
}
fn text(row: &Row, i: usize) -> Result<String> {
    row.try_get(i).map_err(|_| Error::Unavailable)
}
fn number(row: &Row, i: usize) -> Result<i64> {
    row.try_get(i).map_err(|_| Error::Unavailable)
}
fn flag(row: &Row, i: usize) -> Result<bool> {
    row.try_get(i).map_err(|_| Error::Unavailable)
}
pub(super) fn share_locator(key: &str) -> Result<(&str, &str)> {
    let (org, share) = key.split_once('.').ok_or(Error::Denied)?;
    require(valid_id(org) && valid_id(share))?;
    Ok((org, share))
}
const SHARE_COLUMNS: &str = "share_id::text,recipient_email,allow_add_to_canvas,version::bigint,to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),accepted_account_id IS NOT NULL,revoked_at IS NOT NULL,CASE WHEN revoked_at IS NOT NULL THEN 'revoked' WHEN expires_at<=clock_timestamp() THEN 'expired' WHEN accepted_account_id IS NULL THEN 'pending' ELSE 'active' END";
async fn share_row(tx: &Transaction<'_>, org: &str, row: &Row) -> Result<LiveShare> {
    let id = text(row, 0)?;
    let mut share = LiveShare {
        key: format!("{org}.{id}"),
        share_id: id,
        recipient_email: text(row, 1)?,
        allow_add_to_canvas: flag(row, 2)?,
        version: number(row, 3)?,
        expires_at: text(row, 4)?,
        accepted: flag(row, 5)?,
        revoked: flag(row, 6)?,
        status: text(row, 7)?,
    };
    if matches!(share.status.as_str(), "pending" | "active") {
        let live=tx.query(&format!("SELECT 1 {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND {}",live_clause()), &[&org,&share.share_id]).await?;
        if live.is_empty() {
            share.status = "paused".into();
        }
    }
    Ok(share)
}
fn sha(value: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(value)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
async fn audit_member(
    tx: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
    kind: &'static str,
    share: &str,
    version: i64,
) -> Result<()> {
    let outcome = sha(format!("forma:live-share:v1\0{share}\0{version}").as_bytes());
    append(
        tx,
        Chain::Workspace,
        request,
        Event {
            kind,
            id: None,
            app: Some(app),
            work: Some(work),
            record: None,
            outcome: Some(&outcome),
        },
    )
    .await?;
    Ok(())
}
/// The account row is fenced for account-side writes, serializing link limits
/// and the append-only account event chain. It never locks a tenant member.
pub(super) async fn check_account(
    tx: &Transaction<'_>,
    account: &ShareAccount,
    lock: bool,
) -> Result<()> {
    let suffix = if lock { " FOR UPDATE OF a" } else { "" };
    let rows = tx.query(&format!("SELECT 1 FROM genui_identity.accounts a JOIN genui_identity.shared_sessions s USING(account_id) WHERE a.account_id=$1::text::uuid AND a.status='active' AND a.account_epoch=$2::bigint AND s.session_id=$3::text::uuid AND s.account_epoch=a.account_epoch AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '1 hour'{suffix}"), &[&account.account_id(), &account.account_epoch(), &account.session_id()]).await?;
    require(!rows.is_empty())
}
async fn audit_account(
    tx: &Transaction<'_>,
    account: &ShareAccount,
    org: &str,
    share: &str,
    kind: &str,
    version: i64,
) -> Result<()> {
    let rows=tx.query("SELECT audit_sequence::bigint,audit_hash FROM genui_identity.accounts WHERE account_id=$1::text::uuid FOR UPDATE", &[&account.account_id()]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    let sequence = number(row, 0)?.checked_add(1).ok_or(Error::Unavailable)?;
    let previous = text(row, 1)?;
    let body = serde_json::json!({"account_id":account.account_id(),"organization_id":org,"share_id":share,"kind":kind,"share_version":version,"sequence":sequence,"previous_hash":previous});
    let canonical = crate::admission::encode(&body).map_err(|_| Error::Unavailable)?;
    let mut bytes = b"forma:share-account-event:v1\0".to_vec();
    bytes.extend_from_slice(&canonical);
    let hash = sha(&bytes);
    tx.query("INSERT INTO genui_identity.share_events(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at) VALUES($1::text::uuid,gen_random_uuid(),$2::bigint,$3::text::uuid,$4::text::uuid,$5,$6::bigint,$7,$8::text,$9::text,clock_timestamp())", &[&account.account_id(),&sequence,&org,&share,&kind,&version,&canonical,&previous,&hash]).await?;
    tx.query("UPDATE genui_identity.accounts SET audit_sequence=$2::bigint,audit_hash=$3::text WHERE account_id=$1::text::uuid", &[&account.account_id(),&sequence,&hash]).await?;
    Ok(())
}

/// Same current authority as member readers, judged for the sponsor only. This
/// is delegation attenuation, not an actor substitution or recipient grant.
/// The sponsor organization's state is a parameter: `live_clause` requires an
/// active one, and the paused-link check (APP-178) asks about a suspended one.
fn sponsor_clause_for(organization: &str) -> String {
    format!("{organization} AND m.status='active' AND m.membership_epoch=s.sponsor_epoch AND EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=m.organization_id AND e.principal_id=m.principal_id AND e.status='active' AND e.expires_at>clock_timestamp()) AND ({}) AND ((w.created_by=s.sponsor_id AND w.creator_revoked_at IS NULL) OR EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.principal_id=s.sponsor_id AND p.permission='share_work'))", record_read_clause("m.principal_id"))
}
const SHARE_FROM: &str = "FROM genui.live_work_shares s JOIN genui.saved_work w ON (w.organization_id,w.app_id,w.work_id)=(s.organization_id,s.app_id,s.work_id) JOIN genui.publications p ON (p.organization_id,p.app_id,p.publication_id)=(w.organization_id,w.app_id,w.publication_id) JOIN genui.memberships m ON (m.organization_id,m.principal_id)=(s.organization_id,s.sponsor_id) JOIN genui.organizations o ON o.organization_id=s.organization_id";
fn live_clause() -> String {
    live_clause_for("o.status='active'")
}
fn live_clause_for(organization: &str) -> String {
    format!("s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND w.publication_id=s.publication_id AND w.version=s.work_version AND p.definition_digest=s.definition_digest AND p.operator_version=1 AND {}",sponsor_clause_for(organization))
}
/// APP-178 (ADR-014 addendum, 2026-11-12): a connected link whose grant is
/// live in every respect apart from the sponsor organization's state is paused,
/// not removed. It is hidden and unreadable while the organization is suspended,
/// and comes back when it is restored. Either state counts here, so a restore
/// that commits between the listing's access check and this check keeps the
/// link instead of removing it. Revocation, a policy change and expiry still
/// remove it.
async fn link_paused(
    tx: &Transaction<'_>,
    account: &ShareAccount,
    org: &str,
    share: &str,
    grant_version: i64,
) -> Result<bool> {
    check_account(tx, account, false).await?;
    let rows=tx.query(&format!("SELECT 1 {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND s.accepted_account_id=$3::text::uuid AND s.version=$4::bigint AND s.allow_add_to_canvas AND {}",live_clause_for("o.status IN ('active','suspended')")), &[&org,&share,&account.account_id(),&grant_version]).await?;
    Ok(!rows.is_empty())
}

pub(super) async fn check_guest(tx: &Transaction<'_>, access: &GuestAccess) -> Result<()> {
    check_account(tx, &access.account, false).await?;
    let rows=tx.query(&format!("SELECT 1 {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND s.accepted_account_id=$3::text::uuid AND s.version=$4::bigint AND s.app_id=$5::text::uuid AND s.work_id=$6::text::uuid AND s.publication_id=$7::text::uuid AND s.definition_digest=$8 AND s.work_version=$9::bigint AND s.sponsor_id=$10::text::uuid AND s.sponsor_epoch=$11::bigint AND s.view_ids=$12::text[] AND s.collection_ids=$13::text[] AND s.allow_add_to_canvas=$14 AND p.policy_version=$17::bigint AND EXISTS(SELECT 1 FROM genui_identity.accounts a JOIN genui_identity.shared_sessions ss USING(account_id) WHERE a.account_id=$3::text::uuid AND a.status='active' AND a.account_epoch=$15::bigint AND ss.session_id=$16::text::uuid AND ss.account_epoch=a.account_epoch AND ss.revoked_at IS NULL AND ss.expires_at>clock_timestamp() AND ss.last_seen_at>clock_timestamp()-interval '1 hour') AND {}",live_clause()), &[&access.organization,&access.share,&access.account.account_id(),&access.version,&access.app,&access.work,&access.publication,&access.definition_digest,&access.work_version,&access.sponsor,&access.sponsor_epoch,&access.views,&access.collections,&access.can_add,&access.account.account_epoch(),&access.account.session_id(),&access.policy_epoch]).await?;
    require(!rows.is_empty())
}
pub(super) async fn guest_transaction<'a>(
    pool: &Arc<RolePool>,
    access: &GuestAccess,
) -> Result<Transaction<'a>> {
    let tx = Transaction::begin_shared(
        pool,
        Some(&access.organization),
        Some(access.account.account_id()),
    )
    .await?;
    if let Err(error) = check_guest(&tx, access).await {
        tx.finish(false).await?;
        return Err(error);
    }
    Ok(tx)
}
impl Data {
    pub async fn release_guest_access(&self, access: &GuestAccess) -> Result<()> {
        let tx = guest_transaction(&self.inner, access).await?;
        tx.finish(true).await
    }
}
impl Workspace {
    pub async fn can_share_live_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<bool> {
        match self.list_live_shares(request, app, work).await {
            Ok(_) => Ok(true),
            Err(Error::Denied) => Ok(false),
            Err(error) => Err(error),
        }
    }
    pub async fn pending_share(&self, account: &ShareAccount, key: &str) -> Result<bool> {
        let (org, share) = share_locator(key)?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result=async {
            check_account(&tx,account,false).await?;
            let rows=tx.query(&format!("SELECT s.accepted_account_id IS NULL {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND ((s.accepted_account_id IS NULL AND s.recipient_email=(SELECT verified_email FROM genui_identity.accounts WHERE account_id=$3::text::uuid)) OR s.accepted_account_id=$3::text::uuid) AND {}",live_clause()), &[&org,&share,&account.account_id()]).await?;
            flag(rows.first().ok_or(Error::Denied)?,0)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn shared_auth_target(&self, key: &str) -> Result<SharedAuthTarget> {
        let (org, share) = share_locator(key)?;
        let tx = Transaction::begin_shared(&self.inner, Some(org), None).await?;
        let result=async {
            let rows=tx.query(&format!("SELECT s.recipient_email {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND {}",live_clause()), &[&org,&share]).await?;
            Ok(SharedAuthTarget { organization:org.into(),share:share.into(),recipient_email:text(rows.first().ok_or(Error::Denied)?,0)? })
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn create_live_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        input: &NewLiveShare,
    ) -> Result<LiveShare> {
        require(
            valid_id(app)
                && valid_id(work)
                && (1..=30).contains(&input.expires_in_days)
                && valid_id(&input.idempotency_key),
        )?;
        let email = normalize_share_email(&input.recipient_email).ok_or(Error::Denied)?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result=async {
            tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence_member().await?;
            permit.also(Action::RecordRead,Resource::Work{app,work}).await?;
            let tx=permit.transaction();
            owner_share_authority(tx,request,app,work).await?;
            tx.query("SELECT pg_advisory_xact_lock(hashtextextended('forma:live-shares:'||$1||':'||$2,0))", &[&request.organization,&request.principal]).await?;
            permit.recheck_entitlement().await?;
            let canonical=crate::admission::encode(&serde_json::json!({"email":email,"allow_add":input.allow_add_to_canvas,"days":input.expires_in_days})).map_err(|_| Error::Unavailable)?;
            let fingerprint=sha(&canonical);
            let rows=tx.query(&format!("SELECT {SHARE_COLUMNS},request_digest FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND sponsor_id=$4::text::uuid AND idempotency_key=$5"), &[&request.organization,&app,&work,&request.principal,&input.idempotency_key]).await?;
            if let Some(row)=rows.first() { require(text(row,8)?==fingerprint)?; return share_row(tx,&request.organization,row).await; }
            let count=tx.query("SELECT count(*)::bigint FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND sponsor_id=$2::text::uuid AND revoked_at IS NULL AND expires_at>clock_timestamp()", &[&request.organization,&request.principal]).await?;
            require(number(&count[0],0)?<MAX_LIVE_SHARES)?;
            let pins=pinned_many_with(tx,&request.organization,&[(app,work)],false).await?;
            let pin=pins.first().ok_or(Error::Unavailable)?;
            let outline=outline_of(pin)?;
            let premise=&pin.as_ref().map_err(|e|*e)?.0;
            // A share pins every admitted read component, including records
            // dashboards and notes. Command forms remain outside this scope.
            // Source-query execution still uses outline.views only.
            let views:Vec<String>=outline.parts.iter().filter(|v|v.kind!="form").map(|v|v.id.clone()).collect();
            let collections=tx.query("SELECT collection_id::text FROM genui.record_collections WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid ORDER BY collection_id", &[&request.organization,&app,&work]).await?.iter().map(|r|text(r,0)).collect::<Result<Vec<_>>>()?;
            let rows=tx.query(&format!("INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at) VALUES($1::text::uuid,gen_random_uuid(),$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::bigint,$6,$7::text::uuid,$8::text,$9::bigint,$10::text[],$11::text[],$12,$13,1,$14,$15::text,clock_timestamp(),clock_timestamp()+$16::bigint*interval '1 day') RETURNING {SHARE_COLUMNS}"), &[&request.organization,&app,&work,&request.principal,&request.epoch,&email,&premise.publication,&premise.definition_digest,&premise.work_version,&views,&collections,&outline.title,&input.allow_add_to_canvas,&input.idempotency_key,&fingerprint,&input.expires_in_days]).await?;
            let share=share_row(tx,&request.organization,rows.first().ok_or(Error::Unavailable)?).await?;
            require(share.status=="pending")?;
            audit_member(tx,request,app,work,"share.created",&share.share_id,share.version).await?;
            Ok(share)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn list_live_shares(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Vec<LiveShare>> {
        require(valid_id(app) && valid_id(work))?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result:Result<Vec<LiveShare>>=async {
            tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence_member().await?;
            permit.also(Action::RecordRead,Resource::Work{app,work}).await?; let tx=permit.transaction();
            owner_share_authority(tx,request,app,work).await?;
            let rows=tx.query(&format!("SELECT {SHARE_COLUMNS} FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND sponsor_id=$4::text::uuid ORDER BY (revoked_at IS NULL AND expires_at>clock_timestamp()) DESC,created_at DESC,share_id LIMIT 100"), &[&request.organization,&app,&work,&request.principal]).await?;
            let mut shares=Vec::with_capacity(rows.len());
            for row in &rows { shares.push(share_row(tx,&request.organization,row).await?); }
            Ok(shares)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn update_live_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        share: &str,
        expected_version: i64,
        allow_add: bool,
    ) -> Result<Option<LiveShare>> {
        self.change_live_share(request, app, work, share, expected_version, Some(allow_add))
            .await
    }
    pub async fn revoke_live_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        share: &str,
        expected_version: i64,
    ) -> Result<Option<LiveShare>> {
        self.change_live_share(request, app, work, share, expected_version, None)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn change_live_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        share: &str,
        expected_version: i64,
        allow_add: Option<bool>,
    ) -> Result<Option<LiveShare>> {
        require(valid_id(app) && valid_id(work) && valid_id(share) && expected_version > 0)?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result=async {
            tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence_member().await?;
            permit.also(Action::RecordRead,Resource::Work{app,work}).await?; let tx=permit.transaction();
            owner_share_authority(tx,request,app,work).await?;
            let rows=tx.query(&format!("SELECT {SHARE_COLUMNS} FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND share_id=$4::text::uuid AND sponsor_id=$5::text::uuid FOR UPDATE"), &[&request.organization,&app,&work,&share,&request.principal]).await?;
            let current=share_row(tx,&request.organization,rows.first().ok_or(Error::Denied)?).await?;
            if current.version!=expected_version { return Ok(None); }
            if (allow_add.is_none()&&current.revoked)||allow_add==Some(current.allow_add_to_canvas) { return Ok(Some(current)); }
            require(!current.revoked)?;
            permit.recheck_entitlement().await?;
            let rows=tx.query(&format!("UPDATE genui.live_work_shares SET allow_add_to_canvas=coalesce($6,allow_add_to_canvas),revoked_at=CASE WHEN $6::boolean IS NULL THEN clock_timestamp() ELSE revoked_at END,version=version+1 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND share_id=$4::text::uuid AND version=$5::bigint RETURNING {SHARE_COLUMNS}"), &[&request.organization,&app,&work,&share,&expected_version,&allow_add]).await?;
            let updated=share_row(tx,&request.organization,rows.first().ok_or(Error::Denied)?).await?;
            audit_member(tx,request,app,work,if allow_add.is_some(){"share.updated"}else{"share.revoked"},share,updated.version).await?;
            Ok(Some(updated))
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn accept_live_share(
        &self,
        account: &ShareAccount,
        key: &str,
    ) -> Result<GuestAccess> {
        let (org, share) = share_locator(key)?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result=async {
            check_account(&tx,account,true).await?;
            let rows=tx.query(&format!("SELECT s.accepted_account_id::text,s.version::bigint {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND s.recipient_email=(SELECT verified_email FROM genui_identity.accounts WHERE account_id=$3::text::uuid) AND {} FOR UPDATE OF s",live_clause()), &[&org,&share,&account.account_id()]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let accepted:Option<String>=row.try_get(0).map_err(|_|Error::Unavailable)?;
            require(accepted.as_deref().is_none_or(|a|a==account.account_id()))?;
            if accepted.is_none() {
                tx.query("UPDATE genui.live_work_shares SET accepted_account_id=$3::text::uuid,accepted_at=clock_timestamp() WHERE organization_id=$1::text::uuid AND share_id=$2::text::uuid AND accepted_account_id IS NULL", &[&org,&share,&account.account_id()]).await?;
                audit_account(&tx,account,org,share,"share.accepted",number(row,1)?).await?;
            }
            resolve_access(&tx,account,org,share).await
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn resolve_guest_access(
        &self,
        account: &ShareAccount,
        key: &str,
    ) -> Result<GuestAccess> {
        let (org, share) = share_locator(key)?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result = resolve_access(&tx, account, org, share).await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn link_shared_app(&self, account: &ShareAccount, key: &str) -> Result<LinkedApp> {
        let (org, share) = share_locator(key)?;
        // Reconcile invalid references before enforcing the active placement cap.
        self.linked_apps(account).await?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result=async {
            check_account(&tx,account,true).await?;
            // The grant lock makes permission changes and insertion serial.
            tx.query("SELECT share_id FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND share_id=$2::text::uuid FOR SHARE", &[&org,&share]).await?;
            let access=resolve_access(&tx,account,org,share).await?; require(access.can_add)?;
            let count=tx.query("SELECT count(*)::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND removed_at IS NULL", &[&account.account_id()]).await?;
            let existing=tx.query("SELECT removed_at IS NULL FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND organization_id=$2::text::uuid AND share_id=$3::text::uuid", &[&account.account_id(),&org,&share]).await?;
            if existing.first().is_none_or(|r|!flag(r,0).unwrap_or(false)) {
                require(number(&count[0],0)?<MAX_LINKED_APPS)?;
                tx.query("INSERT INTO genui_identity.linked_apps(account_id,organization_id,share_id,created_at,removed_at,grant_version) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,clock_timestamp(),NULL,$4::bigint) ON CONFLICT(account_id,organization_id,share_id) DO UPDATE SET removed_at=NULL,grant_version=EXCLUDED.grant_version,placement_generation=linked_apps.placement_generation+1,created_at=clock_timestamp()", &[&account.account_id(),&org,&share,&access.version]).await?;
                audit_account(&tx,account,org,share,"app.linked",access.version).await?;
            }
            linked_row(&tx,&access).await
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn unlink_shared_app(&self, account: &ShareAccount, key: &str) -> Result<()> {
        self.unlink_shared_app_if_current(account, key, None).await
    }
    pub(super) async fn unlink_shared_app_if_current(
        &self,
        account: &ShareAccount,
        key: &str,
        generation: Option<i64>,
    ) -> Result<()> {
        let (org, share) = share_locator(key)?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result=async {
            check_account(&tx,account,true).await?;
            let changed=tx.query("UPDATE genui_identity.linked_apps SET removed_at=clock_timestamp() WHERE account_id=$1::text::uuid AND organization_id=$2::text::uuid AND share_id=$3::text::uuid AND removed_at IS NULL AND ($4::bigint IS NULL OR placement_generation=$4::bigint) RETURNING share_id", &[&account.account_id(),&org,&share,&generation]).await?;
            if !changed.is_empty() { audit_account(&tx,account,org,share,"app.unlinked",0).await?; }
            Ok(())
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    async fn is_link_paused(
        &self,
        account: &ShareAccount,
        key: &str,
        grant_version: i64,
    ) -> Result<bool> {
        let (org, share) = share_locator(key)?;
        let tx =
            Transaction::begin_shared(&self.inner, Some(org), Some(account.account_id())).await?;
        let result = link_paused(&tx, account, org, share, grant_version).await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn linked_apps(&self, account: &ShareAccount) -> Result<Vec<LinkedApp>> {
        let tx = Transaction::begin_shared(&self.inner, None, Some(account.account_id())).await?;
        let keys=async {
            check_account(&tx,account,false).await?;
            tx.query("SELECT organization_id::text,share_id::text,grant_version::bigint,placement_generation::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND removed_at IS NULL ORDER BY created_at,share_id LIMIT 100", &[&account.account_id()]).await?.iter().map(|r|Ok((format!("{}.{}",text(r,0)?,text(r,1)?),number(r,2)?,number(r,3)?))).collect::<Result<Vec<_>>>()
        }.await;
        tx.finish(keys.is_ok()).await?;
        let mut result = Vec::new();
        for (key, grant_version, generation) in keys? {
            match self.resolve_guest_access(account, &key).await {
                Ok(access) if access.can_add && access.version == grant_version => {
                    let tx = guest_transaction(&self.inner, &access).await?;
                    let linked = linked_row(&tx, &access).await;
                    tx.finish(linked.is_ok()).await?;
                    result.push(linked?);
                }
                Ok(_) | Err(Error::Denied) => {
                    // A suspended sponsor organization pauses the link: hidden,
                    // kept, back on restore (ADR-014 addendum). Anything else
                    // that refuses it removes it.
                    if self.is_link_paused(account, &key, grant_version).await? {
                        continue;
                    }
                    // Re-check account liveness inside the mutation. A signed-out or
                    // expired account must never lose its saved references. Store
                    // failures never enter this branch. Unlink is idempotent/audited.
                    self.unlink_shared_app_if_current(account, &key, Some(generation))
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(result)
    }
}
async fn owner_share_authority(
    tx: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<()> {
    let rows=tx.query("SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid AND ((w.created_by=$4::text::uuid AND w.creator_revoked_at IS NULL) OR EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.principal_id=$4::text::uuid AND p.permission='share_work'))", &[&request.organization,&app,&work,&request.principal]).await?;
    require(!rows.is_empty())
}
async fn resolve_access(
    tx: &Transaction<'_>,
    account: &ShareAccount,
    org: &str,
    share: &str,
) -> Result<GuestAccess> {
    check_account(tx, account, false).await?;
    let rows=tx.query(&format!("SELECT s.app_id::text,s.work_id::text,s.publication_id::text,s.definition_digest::text,s.work_version::bigint,s.version::bigint,s.sponsor_id::text,s.sponsor_epoch::bigint,s.view_ids,s.collection_ids,s.allow_add_to_canvas,to_char(s.expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),o.display_name,p.policy_version::bigint {SHARE_FROM} WHERE s.organization_id=$1::text::uuid AND s.share_id=$2::text::uuid AND s.accepted_account_id=$3::text::uuid AND {}",live_clause()), &[&org,&share,&account.account_id()]).await?;
    let row = rows.first().ok_or(Error::Denied)?;
    Ok(GuestAccess {
        account: account.clone(),
        organization: org.into(),
        share: share.into(),
        app: text(row, 0)?,
        work: text(row, 1)?,
        publication: text(row, 2)?,
        definition_digest: text(row, 3)?,
        work_version: number(row, 4)?,
        version: number(row, 5)?,
        sponsor: text(row, 6)?,
        sponsor_epoch: number(row, 7)?,
        views: row.try_get(8).map_err(|_| Error::Unavailable)?,
        collections: row.try_get(9).map_err(|_| Error::Unavailable)?,
        can_add: flag(row, 10)?,
        expires_at: text(row, 11)?,
        owner_label: text(row, 12)?,
        policy_epoch: number(row, 13)?,
    })
}
async fn linked_row(tx: &Transaction<'_>, access: &GuestAccess) -> Result<LinkedApp> {
    check_guest(tx, access).await?;
    let rows=tx.query("SELECT title FROM genui.live_work_shares WHERE organization_id=$1::text::uuid AND share_id=$2::text::uuid", &[&access.organization,&access.share]).await?;
    Ok(LinkedApp {
        key: access.key(),
        share_id: access.share.clone(),
        title: text(rows.first().ok_or(Error::Denied)?, 0)?,
        organization_id: access.organization.clone(),
        app_id: access.app.clone(),
        work_id: access.work.clone(),
        can_add_to_canvas: access.can_add,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_email_and_locator_are_closed() {
        assert_eq!(
            normalize_share_email(" Alice+team@EXAMPLE.com ").as_deref(),
            Some("alice+team@example.com")
        );
        for bad in [
            "",
            "nobody",
            "a@@b.com",
            "a@b",
            "a\nb@example.com",
            "é@example.com",
            "a @example.com",
        ] {
            assert!(normalize_share_email(bad).is_none(), "{bad:?}");
        }
        assert!(share_locator(
            "00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-000000000002"
        )
        .is_ok());
        assert!(share_locator(
            "00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-000000000002.extra"
        )
        .is_err());
    }
}
