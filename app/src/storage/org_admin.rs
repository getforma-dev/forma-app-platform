//! Organization administration for the organization's own admins (D4 of the
//! trial plan; ADR-008 amendment 2026-11-14): who has joined, their consent,
//! activity and footprint, and member removal, through the `admin_policy`
//! permission and two `genui_admit` definer functions (migration 0023,
//! `genui_private.org_admin_overview` and
//! `genui_private.admin_remove_member`). Member display names (D6) live
//! here too: a member sets the name others see
//! (`genui_private.set_display_name`, audited as `member.named`), and the
//! process-wide directory cache that `crate::directory::label` reads is
//! refreshed.
//!
//! Every operation begins a member transaction on the workspace pool, so the
//! trusted request is the live session's. The admin operations precheck and
//! fence `admin_policy` on the organization (`authorization::permissions`),
//! and the definers re-check the same facts under that fence and return no
//! row when they disagree, so no path reaches member data without
//! `admin_policy` held at commit time. `Error::Denied` means "not an admin of
//! this organization" (or not a current member at all); the routes answer it
//! with the uniform 404.
use super::{
    authorization::{Action, Resource, Transaction},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use tokio_postgres::Row;

/// One member as the organization's admin sees it (`org_admin_overview`).
/// Never a provider subject or an email: the label is the member's chosen
/// display name, else `directory::label`'s fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberRow {
    pub principal: String,
    /// The display name the member chose, else `directory::label`.
    pub label: String,
    /// The operator's note on the code this member redeemed (0023
    /// `access_codes.note`), if any.
    pub note: Option<String>,
    /// `active` or `revoked` (the membership row).
    pub status: String,
    pub is_admin: bool,
    /// The latest notice version this member consented to, and when
    /// (RFC 3339, to the second).
    pub consent_version: Option<i32>,
    pub consented_at: Option<String>,
    /// The member's latest activity (RFC 3339, to the second): the latest
    /// audit event it caused or the latest session touch, whichever is later.
    pub last_active: Option<String>,
    pub apps: i64,
    pub works: i64,
    pub records: i64,
    pub uploads: i64,
    /// The status of the code this member redeemed, if any.
    pub code_status: Option<String>,
}

/// `admin_remove_member`'s answer: removed, or refused for one fixed reason
/// (`self`, `last_admin`, `not_member`). Authority refusals are `Error::Denied`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoveOutcome {
    Removed,
    Refused(&'static str),
}

/// RFC 3339 to the second, UTC, as every timestamp here is rendered.
const RFC3339: &str = "to_char($COL AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')";
fn rfc3339(column: &str) -> String {
    RFC3339.replace("$COL", column)
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

impl Workspace {
    /// Whether the request's member holds `admin_policy` in its organization
    /// (current membership, epoch and entitlement included). `Ok(false)` for
    /// every authority refusal; `Err` only for a store failure.
    pub async fn is_org_admin(&self, request: &TrustedRequest) -> Result<bool> {
        let transaction = Transaction::begin(&self.inner, request).await?;
        let result = transaction
            .precheck(Action::AdminPolicy, Resource::Organization)
            .await;
        transaction.finish(false).await?;
        match result {
            Ok(_) => Ok(true),
            Err(Error::Denied) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Every member of the admin's organization, as `MemberRow`s, ordered by
    /// name. `Denied` unless the caller holds `admin_policy`.
    pub async fn org_admin_overview(&self, request: &TrustedRequest) -> Result<Vec<MemberRow>> {
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::AdminPolicy, Resource::Organization)
                .await?;
            let permit = transaction.fence(precheck).await?;
            let sql = format!(
                "SELECT principal_id::text,display_name,status,is_admin,consent_version,{},{},apps,works,records,uploads,code_status,code_note FROM genui_private.org_admin_overview($1::text::uuid,$2::text::uuid)",
                rfc3339("consented_at"),
                rfc3339("last_active")
            );
            let rows = permit
                .transaction()
                .query(&sql, &[&request.organization, &request.principal])
                .await?;
            // The caller is a member, so an empty answer is the definer's refusal.
            require(!rows.is_empty())?;
            let mut members = Vec::with_capacity(rows.len());
            for row in &rows {
                let principal = text(row, 0)?;
                let label = optional(row, 1)?
                    .unwrap_or_else(|| crate::directory::label(&request.organization, &principal));
                members.push(MemberRow {
                    label,
                    status: text(row, 2)?,
                    is_admin: boolean(row, 3)?,
                    consent_version: row.try_get(4).map_err(|_| Error::Unavailable)?,
                    consented_at: optional(row, 5)?,
                    last_active: optional(row, 6)?,
                    apps: integer(row, 7)?,
                    works: integer(row, 8)?,
                    records: integer(row, 9)?,
                    uploads: integer(row, 10)?,
                    code_status: optional(row, 11)?,
                    note: optional(row, 12)?,
                    principal,
                });
            }
            Ok(members)
        }
        .await;
        transaction.finish(false).await?;
        result
    }

    /// Removes a member of the admin's organization: membership revoked,
    /// entitlements revoked, epoch advanced (every session dies at its next
    /// request), audited on the admission chain. Refuses the caller itself,
    /// the last admin and anyone who is not an active member. `Denied`
    /// unless the caller holds `admin_policy`; a malformed id is `Denied`
    /// too.
    pub async fn admin_remove_member(
        &self,
        request: &TrustedRequest,
        principal: &str,
    ) -> Result<RemoveOutcome> {
        require(valid_id(principal))?;
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::AdminPolicy, Resource::Organization)
                .await?;
            let permit = transaction.fence(precheck).await?;
            let rows = permit.transaction().query("SELECT outcome FROM genui_private.admin_remove_member($1::text::uuid,$2::text::uuid,$3::text::uuid)", &[&request.organization,&request.principal,&principal]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            Ok(match text(row, 0)?.as_str() {
                "removed" => RemoveOutcome::Removed,
                "self" => RemoveOutcome::Refused("self"),
                "last_admin" => RemoveOutcome::Refused("last_admin"),
                "not_member" => RemoveOutcome::Refused("not_member"),
                _ => return Err(Error::Unavailable),
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The display name this member chose, if any (`Denied` for a member who
    /// is no longer current).
    pub async fn display_name(&self, request: &TrustedRequest) -> Result<Option<String>> {
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let permit = transaction.fence_member().await?;
            let rows = permit.transaction().query("SELECT display_name FROM genui.memberships WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid", &[&request.organization,&request.principal]).await?;
            optional(rows.first().ok_or(Error::Denied)?, 0)
        }
        .await;
        transaction.finish(false).await?;
        result
    }

    /// Sets the display name others see for this member (bounded, printable:
    /// `directory::usable_name`, trimmed), audited as `member.named`, and
    /// refreshes the directory cache. The same name again changes nothing.
    pub async fn set_display_name(&self, request: &TrustedRequest, name: &str) -> Result<()> {
        require(crate::directory::usable_name(name))?;
        let name = name.trim();
        let mut transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let permit = transaction.fence_member().await?;
            let rows = permit.transaction().query("SELECT changed FROM genui_private.set_display_name($1::text::uuid,$2::text::uuid,$3)", &[&request.organization,&request.principal,&name]).await?;
            rows.first().ok_or(Error::Denied).map(|_| ())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result?;
        crate::directory::set_member_name(&request.organization, &request.principal, Some(name));
        Ok(())
    }

    /// The startup fill of the directory cache: one read of every stored
    /// display name per registry organization. Returns how many names were
    /// loaded. The beta runs one task (ADR-007 D7.18), so after this the
    /// cache follows this process's own writes.
    pub async fn load_display_names(&self) -> Result<usize> {
        let mut loaded = 0;
        for organization in self.inner.registry.organizations() {
            let transaction =
                Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
            let result = transaction.query("SELECT principal_id::text,display_name FROM genui.memberships WHERE organization_id=$1::text::uuid AND display_name IS NOT NULL ORDER BY principal_id", &[&organization]).await;
            transaction.finish(false).await?;
            let mut entries = Vec::new();
            for row in &result? {
                entries.push((text(row, 0)?, text(row, 1)?));
            }
            loaded += entries.len();
            crate::directory::replace_member_names(&organization, &entries);
        }
        Ok(loaded)
    }
}
