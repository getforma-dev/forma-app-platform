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
/// The result of a share: the grantee (none for an organization-wide share,
/// APP-192) and the granted subset, in order; `removed` names the work
/// permissions a lower role took away from the grantee (review f1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shared {
    pub grantee: Option<String>,
    pub granted: Vec<Granted>,
    pub removed: Vec<String>,
}
/// The owner's revocation of a member's personal share (APP-191 review M1):
/// the work permissions removed, in `record_read`, `record_review`,
/// `record_write` order (empty when the member held none).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Revoked {
    pub grantee: String,
    pub removed: Vec<String>,
}
impl Revoked {
    /// Whether this call wrote anything (and therefore appended its event).
    pub fn changed(&self) -> bool {
        !self.removed.is_empty()
    }
}
/// Who a share reaches (APP-192, D9): one member of the sharer's
/// organization, or every current and future active member of it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Audience<'a> {
    Member(&'a str),
    Organization,
}
/// The role a share grants (APP-192, D9): a viewer holds `record_read`, a
/// contributor also `record_write`, a reviewer also `record_review` (APP-191:
/// every row of an `own` collection and the `reviewer_only` commands). Each
/// permission is copied only when the sharer holds it; a reviewer grant needs
/// the sharer to be a reviewer (the owner always is).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShareRole {
    Viewer,
    Contributor,
    Reviewer,
}
impl ShareRole {
    /// The definer's role literal.
    pub fn name(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Contributor => "contributor",
            Self::Reviewer => "reviewer",
        }
    }
    /// The role the earlier share request's `record_write` flag maps to.
    pub fn from_record_write(record_write: bool) -> Self {
        if record_write {
            Self::Contributor
        } else {
            Self::Viewer
        }
    }
}
impl Shared {
    /// Whether this call wrote anything (and therefore appended its event).
    pub fn changed(&self) -> bool {
        self.granted.iter().any(|g| g.inserted) || !self.removed.is_empty()
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
    #[cfg(test)]
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
/// The member label: the operator-set display name when one exists
/// (`crate::directory`, ADR-011 scenarios), else the neutral `Member` and the
/// first eight hex digits.
// DS-17, DS-44: the directory's name (demo organizations only), else the neutral label.
pub fn member_label(organization: &str, principal: &str) -> String {
    crate::directory::label(organization, principal)
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
                        label: member_label(&request.organization, &principal),
                        principal,
                    })
                })
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The member's organization display name (the canvas top bar), under
    /// the request's scope and the member fence, like `members`. Nothing is
    /// written; a refused member is `Denied`.
    pub async fn organization_name(&self, request: &TrustedRequest) -> Result<String> {
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let permit = transaction.fence_member().await?;
            let rows = permit.transaction().query("SELECT display_name FROM genui.organizations WHERE organization_id=$1::text::uuid", &[&request.organization]).await?;
            match rows.first() {
                Some(row) => text(row, 0),
                None => Err(Error::Denied),
            }
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// `POST /v1/work/{app}/{work}/share`: shares `work` of `app` with the
    /// `audience` in `role` (APP-192, D9), granting exactly the sharer's
    /// subset (module notes). A member grantee receives permission rows
    /// through the definer `genui_private.share_work`; the organization
    /// receives one live `work_shares` row with no grantee through
    /// `genui_private.set_work_everyone`, which every work-level clause
    /// honors for every active member (`authorization::organization_share`),
    /// so a member admitted later is covered and one revocation
    /// (`revoke_organization_share`) ends it for all. An organization-wide
    /// share is refused for a work with source dependencies or folder files
    /// (the v1 limit: those are personal grants). `Denied` covers a work the
    /// sharer cannot open now, a sharer who is neither the unrevoked creator
    /// nor a `share_work` holder, a role the sharer does not hold (a
    /// contributor needs `record_write`, a reviewer `record_review`), a
    /// grantee who is the sharer, not an active member, or of another
    /// organization, and a malformed id, alike.
    pub async fn share_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        audience: Audience<'_>,
        role: ShareRole,
    ) -> Result<Shared> {
        require(valid_id(app) && valid_id(work))?;
        if let Audience::Member(grantee) = audience {
            require(valid_id(grantee) && grantee != request.principal)?;
        }
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            if role != ShareRole::Viewer {
                permit
                    .also(Action::RecordWrite, Resource::Work { app, work })
                    .await?;
            }
            if role == ShareRole::Reviewer {
                permit
                    .also(Action::RecordReview, Resource::Work { app, work })
                    .await?;
            }
            let transaction = permit.transaction();
            let rows = match audience {
                Audience::Member(grantee) => transaction.query("SELECT scope,resource_id::text,permission,inserted FROM genui_private.share_work($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text)", &[&request.organization,&request.principal,&grantee,&app,&work,&role.name()]).await?,
                Audience::Organization => {
                    require(personal_only(transaction, request, app, work).await?.is_none())?;
                    transaction.query("SELECT scope,resource_id::text,permission,inserted FROM genui_private.set_work_everyone($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text)", &[&request.organization,&request.principal,&app,&work,&role.name()]).await?
                }
            };
            require(!rows.is_empty())?;
            // The definer answers a stripped permission as scope `revoked`.
            let mut granted = Vec::new();
            let mut removed = Vec::new();
            for row in &rows {
                let scope = text(row, 0)?;
                if scope == "revoked" {
                    removed.push(text(row, 2)?);
                } else {
                    granted.push(Granted {
                        scope,
                        resource: text(row, 1)?,
                        permission: text(row, 2)?,
                        inserted: boolean(row, 3)?,
                    });
                }
            }
            // APP-108: a work built from a folder carries read access to
            // exactly the folder files it was built from (every share path:
            // canvas, Slack, MCP), audited, in this same transaction (APP-113
            // review: a share never commits without its file grants); a work
            // not built from a folder writes nothing.
            if let Audience::Member(grantee) = audience {
                super::boxes::grant_box_files_for_work(transaction, request, app, work, grantee)
                    .await?;
            }
            Ok(Shared {
                grantee: match audience {
                    Audience::Member(grantee) => Some(grantee.to_owned()),
                    Audience::Organization => None,
                },
                granted,
                removed,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The app owner's revocation of `grantee`'s personal share of `work`
    /// (APP-191 review M1): every `record_read`, `record_write` and
    /// `record_review` row the member holds on the work goes in one unit
    /// through `genui_private.revoke_work_share`, the member's share records
    /// are marked revoked and one `permission.revoked` is appended; a removed
    /// grant leaves no row that a later organization-wide share could revive.
    /// The organization-wide share, the app-level `discover`/`run` rows and
    /// the owner's own standing are untouched. `Denied` for anyone but the
    /// app's owner, for the owner naming themselves, for a work that does not
    /// exist and for a malformed id alike; a member who held nothing answers
    /// an unchanged [`Revoked`].
    pub async fn revoke_work_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        grantee: &str,
    ) -> Result<Revoked> {
        require(valid_id(app) && valid_id(work) && valid_id(grantee))?;
        require(grantee != request.principal)?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let permit = transaction.fence_member().await?;
            let rows = permit.transaction().query("SELECT permission,removed FROM genui_private.revoke_work_share($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid)", &[&request.organization,&request.principal,&app,&work,&grantee]).await?;
            require(!rows.is_empty())?;
            let mut removed = Vec::new();
            for row in &rows {
                if boolean(row, 1)? {
                    removed.push(text(row, 0)?);
                }
            }
            Ok(Revoked {
                grantee: grantee.to_owned(),
                removed,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Ends the live organization-wide share of `work` of `app` (APP-192) in
    /// one unit, for every member at once: the `work_shares` row is marked
    /// revoked and one `permission.revoked` event is appended. `Ok(true)`
    /// when a live share was revoked, `Ok(false)` when none was live;
    /// `Denied` for a sharer without share authority over the work.
    pub async fn revoke_organization_share(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<bool> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let precheck = transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let permit = transaction.fence(precheck).await?;
            let rows = permit.transaction().query("SELECT scope,resource_id::text,permission,inserted FROM genui_private.set_work_everyone($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,'none')", &[&request.organization,&request.principal,&app,&work]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            require(text(row, 2)? == "none")?;
            boolean(row, 3)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

/// Why a work can be shared only person by person (APP-192 v1 limit): it has
/// source dependencies (`source_read` is a personal, organization-wide
/// copy) or folder files (`box_file_grants` are personal). `None` when an
/// organization-wide share may proceed.
async fn personal_only(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<Option<&'static str>> {
    let deps = transaction.query("SELECT 1 FROM genui.work_source_dependencies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid LIMIT 1", &[&request.organization,&app,&work]).await?;
    if !deps.is_empty() {
        return Ok(Some("source dependencies"));
    }
    let files = transaction.query("SELECT 1 FROM genui.box_derivations WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid LIMIT 1", &[&request.organization,&app,&work]).await?;
    Ok((!files.is_empty()).then_some("folder files"))
}

// --- APP-192: what the share dialog shows (`GET /v1/work/{app}/{work}/share`)

impl ShareRole {
    /// The role named by a request or a `work_shares` row; anything else is
    /// no role.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "viewer" => Some(Self::Viewer),
            "contributor" => Some(Self::Contributor),
            "reviewer" => Some(Self::Reviewer),
            _ => None,
        }
    }
}

/// One member a work is shared with person by person: the label others see
/// (`member_label`, never the principal id) and the role their live
/// permission rows on the work carry now (review of W2b, item 1): a reviewer
/// with `record_review`, else a contributor with `record_write`, else a
/// viewer. An operator revocation or a demotion shows at once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShareEntry {
    pub principal: String,
    pub label: String,
    pub role: ShareRole,
}
/// A sharer's own standing on one work: what both share POSTs judge before
/// any grantee is considered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShareStanding {
    /// The roles this sharer may grant now, weakest first: viewer always, a
    /// contributor with the sharer's `record_write`, a reviewer when the
    /// sharer is also a reviewer of the work (`Action::RecordReview`): the
    /// same clauses `share_work` re-reads after its fence.
    pub roles: Vec<ShareRole>,
    /// False when the work may be shared only person by person (source
    /// dependencies or folder files: `personal_only`).
    pub organization_available: bool,
    /// The role of the live organization-wide share, if one is live.
    pub organization: Option<ShareRole>,
}
/// The sharing of one work as a member with share authority over it sees it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSharing {
    pub standing: ShareStanding,
    /// Whether the work has a private (`visibility: "own"`) list, where a
    /// member sees and changes only their own entries unless a reviewer.
    pub private_lists: bool,
    /// The other active members it is shared with personally, by label.
    pub members: Vec<ShareEntry>,
}

impl Workspace {
    /// The sharing of `work` of `app` for its share dialog (APP-192): the
    /// member's [`ShareStanding`], whether the work has private lists, and
    /// the members the work is shared with. One `REPEATABLE READ READ ONLY`
    /// snapshot with no fence (review m4: a listing writes nothing and takes
    /// no `genui_lock` row lock). `Denied` as [`Workspace::share_standing`].
    ///
    /// A member is listed while they have a live personal share row of the
    /// work **and** still hold `record_read` on it; the role shown is that of
    /// their live permission rows ([`ShareEntry`]), never the share rows'
    /// (a pre-0024 row without a role needs no special case). Only active
    /// members are listed, never the member asking, at most [`MEMBER_LIMIT`].
    pub async fn work_sharing(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<WorkSharing> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_read_only(&self.inner, request, cap).await?;
        let result = async {
            let standing = standing_in(&transaction, request, app, work).await?;
            let private_lists = super::authorization::own_collections(
                &transaction,
                &request.organization,
                app,
                work,
            )
            .await?
                > 0;
            let held = |permission: &str| format!("EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=$1::text::uuid AND p.principal_id=g.grantee_id AND p.app_id=$2::text::uuid AND p.work_id=$3::text::uuid AND p.permission='{permission}')");
            let rows = transaction.query(&format!("SELECT g.grantee_id::text,{},{} FROM (SELECT DISTINCT s.grantee_id FROM genui.work_shares s JOIN genui.memberships m ON m.organization_id=s.organization_id AND m.principal_id=s.grantee_id AND m.status='active' WHERE s.organization_id=$1::text::uuid AND s.app_id=$2::text::uuid AND s.work_id=$3::text::uuid AND s.grantee_id IS NOT NULL AND s.grantee_id<>$4::text::uuid AND s.revoked_at IS NULL) g WHERE {} ORDER BY g.grantee_id LIMIT $5", held("record_review"), held("record_write"), held("record_read")), &[&request.organization,&app,&work,&request.principal,&MEMBER_LIMIT]).await?;
            let mut members = rows
                .iter()
                .map(|row| {
                    let principal = text(row, 0)?;
                    // The strongest exposure shows: `record_review` alone
                    // already lets a member see every private entry.
                    let role = if boolean(row, 1)? {
                        ShareRole::Reviewer
                    } else if boolean(row, 2)? {
                        ShareRole::Contributor
                    } else {
                        ShareRole::Viewer
                    };
                    Ok(ShareEntry {
                        label: member_label(&request.organization, &principal),
                        principal,
                        role,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            members.sort_by(|a, b| {
                a.label
                    .to_lowercase()
                    .cmp(&b.label.to_lowercase())
                    .then_with(|| a.principal.cmp(&b.principal))
            });
            Ok(WorkSharing {
                standing,
                private_lists,
                members,
            })
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The member's [`ShareStanding`] on `work` of `app` alone (both share
    /// POSTs; review of W2b, NIT-5: no member list is read). One read-only
    /// snapshot with no fence: the member precheck (`record_read` on the
    /// work), then share authority as the definers judge it (the unrevoked
    /// creator or a `share_work` holder, with `run` on the app). `Denied`
    /// for anyone else, for a work the member cannot open and for a
    /// malformed id alike (the uniform 404).
    pub async fn share_standing(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<ShareStanding> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_read_only(&self.inner, request, cap).await?;
        let result = standing_in(&transaction, request, app, work).await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

/// [`ShareStanding`] inside a member transaction (module notes on
/// `work_sharing`).
async fn standing_in(
    transaction: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<ShareStanding> {
    transaction
        .precheck(Action::RecordRead, Resource::Work { app, work })
        .await?;
    let authority = transaction.query("SELECT 1 FROM genui.saved_work w WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid AND ((w.created_by=$4::text::uuid AND w.creator_revoked_at IS NULL) OR EXISTS(SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=w.organization_id AND p.principal_id=$4::text::uuid AND p.app_id=w.app_id AND p.work_id=w.work_id AND p.permission='share_work')) AND EXISTS(SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=w.organization_id AND a.principal_id=$4::text::uuid AND a.app_id=w.app_id AND a.permission='run')", &[&request.organization,&app,&work,&request.principal]).await?;
    require(!authority.is_empty())?;
    let grants = transaction.read_only_grants(&[(app, work)]).await?;
    let held = grants.first().ok_or(Error::Unavailable)?;
    let mut roles = vec![ShareRole::Viewer];
    if held.record_write {
        roles.push(ShareRole::Contributor);
        if held.record_review {
            roles.push(ShareRole::Reviewer);
        }
    }
    let organization_available = personal_only(transaction, request, app, work)
        .await?
        .is_none();
    let live = transaction.query("SELECT role FROM genui.work_shares WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND grantee_id IS NULL AND revoked_at IS NULL", &[&request.organization,&app,&work]).await?;
    let organization = match live.first() {
        Some(row) => Some(ShareRole::from_name(&text(row, 0)?).ok_or(Error::Unavailable)?),
        None => None,
    };
    Ok(ShareStanding {
        roles,
        organization_available,
        organization,
    })
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
        revision: Option<i64>,
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
            // The listing's clauses per member (`discover_clause`, the
            // record_read `work_clause`; APP-192's organization-wide share
            // included), and (APP-191) when the event names a committed
            // revision, `own_rows` on that row for that member: an `own`
            // collection's commit reaches its creator and the reviewers only.
            let principal = "m.principal_id";
            let rows = transaction.query(&format!("SELECT m.principal_id::text,m.membership_epoch,(EXISTS(SELECT 1 FROM genui.saved_work w WHERE w.organization_id=m.organization_id AND w.app_id=$3::text::uuid AND w.work_id=$4::text::uuid AND {} AND {}) AND ($6::bigint IS NULL OR EXISTS(SELECT 1 FROM genui.internal_record_revisions v WHERE v.organization_id=m.organization_id AND v.app_id=$3::text::uuid AND v.work_id=$4::text::uuid AND v.commit_sequence=$6::bigint AND {}))) FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND o.status='active' AND m.status='active' AND m.principal_id=ANY($2::text[]::uuid[]) AND {LIVE_ENTITLEMENT} ORDER BY m.principal_id LIMIT $5", super::authorization::discover_clause(principal), super::authorization::record_read_clause(principal), super::authorization::own_rows("v", "$3::text::uuid", "$4::text::uuid", principal)), &[&request.organization,&principals,&app,&work,&limit,&revision]).await?;
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
            .openable_principals(
                request,
                app,
                work,
                None,
                std::slice::from_ref(&request.principal),
            )
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
            member_label(
                "00000000-0000-4000-8000-0000000000d2",
                "00000000-0000-4000-8000-00000000006d"
            ),
            "Member 00000000"
        );
        assert_eq!(
            member_label("00000000-0000-4000-8000-0000000000d2", "abc"),
            "Member abc"
        );
        let shared = Shared {
            grantee: Some("g".into()),
            granted: vec![Granted {
                scope: "work".into(),
                resource: "w".into(),
                permission: "record_read".into(),
                inserted: false,
            }],
            removed: Vec::new(),
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
