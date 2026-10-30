//! APP-095 (ADR-011 pitch, `pivot/realtime`): sharing a saved work with a
//! teammate of the same organization, the members list the sharer picks
//! from, and the two re-authorization reads the event stream performs at
//! delivery.
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
//! the scoped organization, and writes exactly the subset of the sharer's
//! own permissions (never more; no `edit_definition`, `review_publish`,
//! `share_work`, `export` or `admin_policy`), with one `permission.granted`
//! event on the admission chain when a row was inserted. The organization is
//! the trusted request's: a grantee of another organization is not a member
//! under the scope and the function returns no row, which is `Denied`, the
//! uniform 404. Nothing a caller supplies names a permission.
use super::{
    authorization::{Action, Resource, Transaction},
    pool::{Data, Workspace},
    require, valid_id, Error, Result, TrustedRequest,
};
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline; each transaction here stays inside it.
pub const SHARE_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many members are listed.
const MEMBER_LIMIT: i64 = 100;

/// One permission the share copied (or found already present) on the grantee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Granted {
    /// `app`, `work` or `source`.
    pub scope: String,
    pub resource: String,
    pub permission: String,
    /// False when the grantee already held it (a repeat share writes nothing).
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

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
/// The neutral member label: `Member` and the first eight hex digits.
pub fn member_label(principal: &str) -> String {
    format!("Member {}", &principal[..8.min(principal.len())])
}

impl Workspace {
    /// `GET /v1/members`: the other active members of the member's
    /// organization, under the request's scope and the member fence
    /// (`fence_member`: current membership, epoch and entitlement). Nothing
    /// is written; a refused member is `Denied`.
    pub async fn members(&self, request: &TrustedRequest) -> Result<Vec<Member>> {
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let permit = transaction.fence_member().await?;
            let rows = permit.transaction().query("SELECT principal_id::text FROM genui.memberships WHERE organization_id=$1::text::uuid AND status='active' AND principal_id<>$2::text::uuid ORDER BY principal_id LIMIT $3", &[&request.organization,&request.principal,&MEMBER_LIMIT]).await?;
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
    /// The event stream's delivery check (APP-095): whether the member may
    /// open `work` of `app` now, judged exactly as the workspace listing does
    /// (`discover` on the app and `record_read` on the work, on current
    /// state, at the request's epoch with a current entitlement). Nothing is
    /// written and nothing is locked. `Denied` for a work the member cannot
    /// open and for a member who is no longer current alike; the caller
    /// separates the two with [`Data::member_current`].
    pub async fn can_open(&self, request: &TrustedRequest, app: &str, work: &str) -> Result<()> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            transaction
                .precheck(Action::Discover, Resource::App(app))
                .await?;
            transaction
                .precheck(Action::RecordRead, Resource::Work { app, work })
                .await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Whether the member behind an open event stream is still current: an
    /// active organization and membership at the request's epoch (a sign-out
    /// or an operator revocation advances it) with a current entitlement,
    /// under the member fence. `Denied` closes the stream.
    pub async fn member_current(&self, request: &TrustedRequest) -> Result<()> {
        let cap = Instant::now() + SHARE_HANDLER_DEADLINE;
        let mut transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = transaction.fence_member().await.map(|_| ());
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
}
