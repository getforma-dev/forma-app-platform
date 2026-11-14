//! Organization administration for the organization's own admins (D4 of the
//! trial plan; ADR-008 amendment 2026-11-14): who has joined, their consent,
//! activity and footprint, and member removal, through the `admin_policy`
//! permission and two `genui_admit` definer functions (migration 0023,
//! `genui_private.org_admin_overview` and
//! `genui_private.admin_remove_member`). Member display names (D6) live
//! here too: a member sets the name others see
//! (`genui_private.set_display_name`, audited), and the process-wide
//! directory cache that `crate::directory::label` reads is refreshed.
//!
//! Every operation begins a member transaction on the workspace pool, so the
//! trusted request is the live session's. `Error::Denied` means "not an admin
//! of this organization" (or not a current member at all); the routes answer
//! it with the uniform 404. The definers re-check the same facts under the
//! member fence and return no row when they disagree, so no path reaches
//! member data without `admin_policy` held at commit time.
use super::{
    authorization::{Action, Resource, Transaction},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use tokio_postgres::Row;

/// One member as the organization's admin sees it (`org_admin_overview`).
/// Never a provider subject or an email: the label is the member's chosen
/// display name, else the neutral `Member <8 hex>` handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberRow {
    pub principal: String,
    /// The display name the member chose, else `directory::neutral_label`.
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
    /// The member's latest session activity (RFC 3339, to the second).
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
        let _ = request;
        Err(Error::Unavailable)
    }

    /// Every member of the admin's organization, as `MemberRow`s, ordered by
    /// label. `Denied` unless the caller holds `admin_policy`.
    pub async fn org_admin_overview(&self, request: &TrustedRequest) -> Result<Vec<MemberRow>> {
        let _ = request;
        Err(Error::Unavailable)
    }

    /// Removes a member of the admin's organization: membership revoked,
    /// entitlements revoked, epoch advanced (every session dies at its next
    /// request), audited on the admission chain. Refuses the caller itself,
    /// the last admin and anyone who is not an active member. `Denied`
    /// unless the caller holds `admin_policy`.
    pub async fn admin_remove_member(
        &self,
        request: &TrustedRequest,
        principal: &str,
    ) -> Result<RemoveOutcome> {
        let _ = (request, principal);
        Err(Error::Unavailable)
    }

    /// The display name this member chose, if any.
    pub async fn display_name(&self, request: &TrustedRequest) -> Result<Option<String>> {
        let _ = request;
        Err(Error::Unavailable)
    }

    /// Sets the display name others see for this member (bounded, printable:
    /// `directory::usable_name`), audited, and refreshes the directory cache.
    pub async fn set_display_name(&self, request: &TrustedRequest, name: &str) -> Result<()> {
        let _ = (request, name);
        Err(Error::Unavailable)
    }
}

// Referenced by the bodies that follow in the next commits; kept here so the
// interface compiles alone.
#[allow(dead_code)]
fn unused(row: &Row) -> Result<()> {
    let _ = (
        text(row, 0),
        optional(row, 0),
        integer(row, 0),
        boolean(row, 0),
    );
    let _ = (
        Action::AdminPolicy,
        Resource::Organization,
        valid_id(""),
        require(true),
    );
    let _: Option<Transaction<'_>> = None;
    Ok(())
}
