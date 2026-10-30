//! APP-095 (ADR-011 pitch, `pivot/realtime`): sharing a saved work with a
//! teammate of the same organization, the members list the sharer picks
//! from, and the reads the event stream performs: one per event for the
//! whole organization (review M2) and one unfenced liveness read per stream
//! per re-check.
//!
//! `share_work` is one READ COMMITTED transaction of the workspace role in
//! ADR-004 order: the member precheck (`record_read` on the work, which is
//! the clause every reader of the work passes: the unrevoked creator's
//! authority or a work grant, `run` on the app and live `source_read` on
//! every dependency), the `genui_lock` member fence with the entitlement
//! re-read, `record_write` re-read when the sharer asks to share it, then one
//! call of the definer `genui_private.share_work` (migration 0008, owned by
//! `genui_admit`), which judges the sharer's authority to share (unrevoked
//! creator, or `share_work` on the work), the grantee's active membership in
//! the scoped organization (both membership rows `FOR SHARE`), and **copies**
//! exactly the subset of the sharer's own permissions (never more; no
//! `edit_definition`, `review_publish`, `share_work`, `export` or
//! `admin_policy`): the teammate then opens the work under the sharer's
//! permissions, copied, not under grants of its own (ADR-011 amendment
//! 2026-10-30, review M3). `source_read` is copied for every dependency the
//! sharer holds because the teammate needs it to open the work, and it is
//! organization-wide by the 0002 model, not scoped to the work. When a row
//! was inserted the definer writes one `work_shares` row naming exactly the
//! inserted set and one `permission.granted` event on the admission chain
//! that references it, so every share is reconstructable and reversible row
//! by row. The organization is the trusted request's: a grantee of another
//! organization is not a member under the scope and the function returns no
//! row, which is `Denied`, the uniform 404. Nothing a caller supplies names
//! a permission.
//!
//! The reads here take no `genui_lock` fence (review M2, m4): they write
//! nothing and must not wait on, or make wait, an operator's `FOR UPDATE` or
//! a command's fence. Each is `current` (an active organization and
//! membership at the request's epoch with a live entitlement, one statement)
//! and then a plain scoped SELECT.
use super::{
    authorization::{Action, Resource, Transaction},
    pool::{Data, Workspace},
    require, valid_id, Error, Result, TrustedRequest,
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline; each transaction here stays inside it.
pub const SHARE_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many members are listed.
const MEMBER_LIMIT: i64 = 100;
/// At most this many subscribed principals are judged per event (the hub
/// holds at most 256 streams, so this is the bound of one event's work).
pub const JUDGMENT_LIMIT: usize = 256;

/// One permission the share copied (or found already present) on the grantee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Granted {
    /// `app`, `work` or `source`.
    pub scope: String,
    pub resource: String,
    pub permission: String,
    /// False when the grantee already held it (a repeat share writes nothing).
    /// Kept server-side for [`Shared::changed`]; the route does not disclose
    /// it (review m1: a sharer learns nothing of the teammate's holdings).
    pub inserted: bool,
}
/// The result of a share: the grantee and the granted subset, in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shared {
    pub grantee: String,
    pub granted: Vec<Granted>,
}
impl Shared {
    /// Whether this call wrote anything (and therefore appended its event).
    pub fn changed(&self) -> bool {
        self.granted.iter().any(|g| g.inserted)
    }
}
/// A member the sharer may pick. Principals carry no display name in the
/// Phase 1 schema (0002), so the label is a neutral short handle derived
/// from the principal id; no provider identity is ever shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub principal: String,
    pub label: String,
}

/// One event's judgment for an organization's subscribed principals (review
/// M2): who is current (active organization and membership, at which epoch,
/// with a live entitlement) and who among them can open the work now. Made
/// in one lock-free scoped read; read for each subscriber without I/O.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Openable {
    /// Current subscribed principals and their membership epoch.
    current: HashMap<String, i64>,
    /// The current ones who can open `(app, work)` now.
    openable: HashSet<String>,
}
/// One subscriber's standing under an [`Openable`] judgment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Standing {
    /// Current at its epoch and able to open the work.
    Open,
    /// Current at its epoch, cannot open the work.
    Closed,
    /// Not an active member at the request's epoch, no live entitlement, or
    /// not among the judged principals.
    NotCurrent,
}
impl Openable {
    pub fn standing(&self, request: &TrustedRequest) -> Standing {
        match self.current.get(&request.principal) {
            Some(epoch) if *epoch == request.epoch => {
                if self.openable.contains(&request.principal) {
                    Standing::Open
                } else {
                    Standing::Closed
                }
            }
            _ => Standing::NotCurrent,
        }
    }
    /// How many principals the judgment found current (tests).
    pub fn current_count(&self) -> usize {
        self.current.len()
    }
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
/// The neutral member label: `Member` and the first eight hex digits.
pub fn member_label(principal: &str) -> String {
    format!("Member {}", &principal[..8.min(principal.len())])
}

/// A live entitlement of the member, judged on the statement's clock.
const LIVE_ENTITLEMENT: &str = "EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=m.organization_id AND e.principal_id=m.principal_id AND e.status='active' AND e.expires_at>clock_timestamp())";

/// The unfenced currency read: an active organization and an active
/// membership at the request's epoch with a live entitlement. `Denied`
/// otherwise. Nothing is locked.
async fn current(transaction: &Transaction<'_>, request: &TrustedRequest) -> Result<()> {
    let rows = transaction.query(&format!("SELECT 1 FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid AND o.status='active' AND m.status='active' AND m.membership_epoch=$3::bigint AND {LIVE_ENTITLEMENT}"), &[&request.organization,&request.principal,&request.epoch]).await?;
    require(!rows.is_empty())
}

impl Workspace {
    /// `GET /v1/members`: the other active members of the member's
    /// organization, under the request's scope, after the unfenced currency
    /// read (review m4: a listing takes no `genui_lock` fence). Nothing is
    /// written; a refused member is `Denied`.
    pub async fn members(&self, request: &TrustedRequest) -> Result<Vec<Member>> {
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            let rows = transaction.query("SELECT principal_id::text FROM genui.memberships WHERE organization_id=$1::text::uuid AND status='active' AND principal_id<>$2::text::uuid ORDER BY principal_id LIMIT $3", &[&request.organization,&request.principal,&MEMBER_LIMIT]).await?;
            rows.iter()
                .map(|row| {
                    let principal = text(row, 0)?;
                    Ok(Member {
                        label: member_label(&principal),
                        principal,
                    })
                })
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// `POST /v1/work/{app}/{work}/share`: shares `work` of `app` with
    /// `grantee`, a member of the sharer's own organization, granting
    /// exactly the sharer's subset (module notes). `Denied` covers a work the
    /// sharer cannot open now, a sharer who is neither the unrevoked creator
    /// nor a `share_work` holder, a `record_write` request the sharer does
    /// not hold, a grantee who is the sharer, not an active member, or of
    /// another organization, and a malformed id, alike.
    pub async fn share_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        grantee: &str,
        record_write: bool,
    ) -> Result<Shared> {
        require(valid_id(app) && valid_id(work) && valid_id(grantee))?;
        require(grantee != request.principal)?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            if record_write {
                permit
                    .also(Action::RecordWrite, Resource::Work { app, work })
                    .await?;
            }
            let rows = permit.transaction().query("SELECT scope,resource_id::text,permission,inserted FROM genui_private.share_work($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6)", &[&request.organization,&request.principal,&grantee,&app,&work,&record_write]).await?;
            require(!rows.is_empty())?;
            let granted = rows
                .iter()
                .map(|row| {
                    Ok(Granted {
                        scope: text(row, 0)?,
                        resource: text(row, 1)?,
                        permission: text(row, 2)?,
                        inserted: boolean(row, 3)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Shared {
                grantee: grantee.to_owned(),
                granted,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

impl Data {
    /// The event stream's one read per event (review M2): among
    /// `principals` (the organization's subscribed principals, at most
    /// [`JUDGMENT_LIMIT`]), who is current and who can open `work` of `app`
    /// now, judged exactly as the workspace listing judges each item
    /// (`discover` on the app and `record_read` on the work: the unrevoked
    /// creator's authority or a work grant, `run` on the app and live
    /// `source_read` on every dependency), on current state, in one scoped
    /// READ COMMITTED statement of the data role. Nothing is written and
    /// nothing is locked; the request only scopes the read (RLS) and must
    /// name the same organization. A principal absent from the result is not
    /// current.
    pub async fn openable_principals(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        principals: &[String],
    ) -> Result<Openable> {
        require(valid_id(app) && valid_id(work))?;
        require(principals.len() <= JUDGMENT_LIMIT && principals.iter().all(|p| valid_id(p)))?;
        if principals.is_empty() {
            return Ok(Openable::default());
        }
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let limit = JUDGMENT_LIMIT as i64;
        let result = async {
            let rows = transaction.query(&format!("SELECT m.principal_id::text,m.membership_epoch,(EXISTS(SELECT 1 FROM genui.app_permissions p JOIN genui.apps a USING(organization_id,app_id) WHERE p.organization_id=m.organization_id AND p.principal_id=m.principal_id AND p.app_id=$3::text::uuid AND p.permission='discover') AND EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=m.organization_id AND w.app_id=$3::text::uuid AND w.work_id=$4::text::uuid AND (EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.principal_id=m.principal_id AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.permission='record_read') OR (w.created_by=m.principal_id AND w.creator_revoked_at IS NULL)) AND EXISTS(SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=w.organization_id AND a.principal_id=m.principal_id AND a.app_id=w.app_id AND a.permission='run') AND NOT EXISTS(SELECT 1 FROM genui.work_source_dependencies d JOIN genui.source_revisions r ON r.organization_id=d.organization_id AND r.source_id=d.source_id AND r.revision_id=d.source_revision_id AND r.normalized_digest=d.source_digest WHERE d.organization_id=w.organization_id AND d.app_id=w.app_id AND d.work_id=w.work_id AND (r.deleted_at IS NOT NULL OR NOT EXISTS(SELECT 1 FROM genui.source_permissions s WHERE s.organization_id=d.organization_id AND s.principal_id=m.principal_id AND s.source_id=d.source_id AND s.permission='source_read'))))) FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND o.status='active' AND m.status='active' AND m.principal_id=ANY($2::text[]::uuid[]) AND {LIVE_ENTITLEMENT} ORDER BY m.principal_id LIMIT $5"), &[&request.organization,&principals,&app,&work,&limit]).await?;
            let mut judgment = Openable::default();
            for row in &rows {
                let principal = text(row, 0)?;
                if boolean(row, 2)? {
                    judgment.openable.insert(principal.clone());
                }
                judgment.current.insert(principal, integer(row, 1)?);
            }
            Ok(judgment)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Whether the member may open `work` of `app` now: the single-member
    /// form of [`Data::openable_principals`] (the campaign's check and the
    /// same clause). `Denied` for a work the member cannot open and for a
    /// member who is no longer current alike.
    pub async fn can_open(&self, request: &TrustedRequest, app: &str, work: &str) -> Result<()> {
        let judgment = self
            .openable_principals(request, app, work, std::slice::from_ref(&request.principal))
            .await?;
        require(judgment.standing(request) == Standing::Open)
    }

    /// Whether the member behind an open event stream is still current: an
    /// active organization and membership at the request's epoch (a sign-out
    /// or an operator revocation advances it) with a live entitlement, read
    /// without a fence (review M2: the re-check never takes `genui_lock` row
    /// locks). `Denied` closes the stream.
    pub async fn member_current(&self, request: &TrustedRequest) -> Result<()> {
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = current(&transaction, request).await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_labels_show_no_provider_identity() {
        assert_eq!(
            member_label("00000000-0000-4000-8000-00000000006d"),
            "Member 00000000"
        );
        assert_eq!(member_label("abc"), "Member abc");
        let shared = Shared {
            grantee: "g".into(),
            granted: vec![Granted {
                scope: "work".into(),
                resource: "w".into(),
                permission: "record_read".into(),
                inserted: false,
            }],
        };
        assert!(!shared.changed());
    }

    #[test]
    fn a_judgment_reads_without_io() {
        let mut judgment = Openable::default();
        judgment.current.insert("a".into(), 3);
        judgment.current.insert("b".into(), 1);
        judgment.openable.insert("a".into());
        let request = |principal: &str, epoch: i64| TrustedRequest {
            organization: String::new(),
            principal: principal.into(),
            epoch,
            issuer: String::new(),
            provider_tenant: String::new(),
            provider_subject: String::new(),
            registry_sha256: String::new(),
        };
        assert_eq!(judgment.standing(&request("a", 3)), Standing::Open);
        assert_eq!(judgment.standing(&request("a", 2)), Standing::NotCurrent);
        assert_eq!(judgment.standing(&request("b", 1)), Standing::Closed);
        assert_eq!(judgment.standing(&request("c", 1)), Standing::NotCurrent);
        assert_eq!(judgment.current_count(), 2);
    }
}
