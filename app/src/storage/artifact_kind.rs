//! APP-169: a member's durable Card or App classification of one whole work.
//! Absence is ambiguous and is shown as an App. Component count is never read.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::confirm_identity,
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use std::collections::BTreeMap;

/// The two kinds a member can save: `card`, or `app`, the reversible step
/// back to an App. Anything else is refused. No row stays an ambiguous App.
pub(crate) fn parse_artifact_kind(value: &str) -> Option<&'static str> {
    match value {
        "card" => Some("card"),
        "app" => Some("app"),
        _ => None,
    }
}

impl Workspace {
    /// The kinds this member saved, `card` or `app`, by `(app, work)`. Other
    /// members' rows are not returned. A work without a row is not in the map.
    /// The caller shows a kind only on a work its own authorized listing
    /// returned, so a row for a work the member can no longer open is never
    /// rendered.
    pub async fn artifact_kinds(
        &self,
        request: &TrustedRequest,
    ) -> Result<BTreeMap<(String, String), &'static str>> {
        let tx = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&tx, request).await?;
            let rows = tx
                .query(
                    "SELECT app_id::text, work_id::text, kind FROM genui.work_artifact_kinds WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid",
                    &[&request.organization(), &request.principal()],
                )
                .await?;
            let mut kinds = BTreeMap::new();
            for row in rows {
                let app: String = row.try_get(0).map_err(|_| Error::Unavailable)?;
                let work: String = row.try_get(1).map_err(|_| Error::Unavailable)?;
                let kind: String = row.try_get(2).map_err(|_| Error::Unavailable)?;
                let kind = parse_artifact_kind(&kind).ok_or(Error::Unavailable)?;
                kinds.insert((app, work), kind);
            }
            Ok(kinds)
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }

    /// Record this member's classification of one whole work they can open:
    /// `card` or `app`, inserted or replaced in one statement, so an `app`
    /// before any row exists is saved too. The saved work, its components,
    /// grants, records and receipts stay. Returns the kind stored.
    pub async fn set_artifact_kind(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        kind: &str,
    ) -> Result<&'static str> {
        require(valid_id(app) && valid_id(work))?;
        let kind = parse_artifact_kind(kind).ok_or(Error::Denied)?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&tx, request).await?;
            // Same pair the canvas listing uses: discover on the app and
            // record_read on the work. A work they cannot open is Denied.
            let precheck = tx.precheck(Action::Discover, Resource::App(app)).await?;
            let permit = tx.fence(precheck).await?;
            permit
                .also(Action::RecordRead, Resource::Work { app, work })
                .await?;
            let t = permit.transaction();
            let found = t
                .query(
                    "SELECT 1 FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid",
                    &[&request.organization(), &app, &work],
                )
                .await?;
            require(!found.is_empty())?;
            permit.recheck_entitlement().await?;
            t.query(
                "INSERT INTO genui.work_artifact_kinds(organization_id,principal_id,app_id,work_id,kind) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5) ON CONFLICT (organization_id,principal_id,app_id,work_id) DO UPDATE SET kind=EXCLUDED.kind, updated_at=date_trunc('second',now())",
                &[&request.organization(), &request.principal(), &app, &work, &kind],
            )
            .await?;
            Ok(kind)
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::parse_artifact_kind;

    #[test]
    fn only_card_and_app_are_kinds() {
        assert_eq!(parse_artifact_kind("card"), Some("card"));
        assert_eq!(parse_artifact_kind("app"), Some("app"));
        assert_eq!(parse_artifact_kind("stack"), None);
        assert_eq!(parse_artifact_kind("Card"), None);
        assert_eq!(parse_artifact_kind(""), None);
    }
}
