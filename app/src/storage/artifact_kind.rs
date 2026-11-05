//! APP-169: a member's durable Card or App classification of one whole work.
//! Absence is ambiguous and is shown as an App. Component count is never read.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::confirm_identity,
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use std::collections::BTreeMap;

/// `card` is stored. `app` writes `app`, which is the reversible step back to
/// an App. Anything else is refused. Absence of a row stays an ambiguous App.
pub(crate) fn parse_artifact_kind(value: &str) -> Option<Option<&'static str>> {
    match value {
        "card" => Some(Some("card")),
        "app" => Some(None),
        _ => None,
    }
}

impl Workspace {
    /// Kinds this member admitted. Other members' rows are not returned.
    pub async fn artifact_kinds(
        &self,
        request: &TrustedRequest,
    ) -> Result<BTreeMap<(String, String), String>> {
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
                if kind == "card" {
                    kinds.insert((app, work), kind);
                }
            }
            Ok(kinds)
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }

    /// Record this member's classification of one whole work they can open.
    /// `Some("card")` stores a Card. `None` stores `app`. The saved work, its
    /// components, grants, records and receipts stay.
    pub async fn set_artifact_kind(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        kind: Option<&str>,
    ) -> Result<Option<&'static str>> {
        require(valid_id(app) && valid_id(work))?;
        if let Some(kind) = kind {
            require(kind == "card")?;
        }
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
            if kind == Some("card") {
                t.query(
                    "INSERT INTO genui.work_artifact_kinds(organization_id,principal_id,app_id,work_id,kind) VALUES ($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,'card') ON CONFLICT (organization_id,principal_id,app_id,work_id) DO UPDATE SET kind='card', updated_at=date_trunc('second',now())",
                    &[&request.organization(), &request.principal(), &app, &work],
                )
                .await?;
                Ok(Some("card"))
            } else {
                t.query(
                    "UPDATE genui.work_artifact_kinds SET kind='app', updated_at=date_trunc('second',now()) WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND app_id=$3::text::uuid AND work_id=$4::text::uuid",
                    &[&request.organization(), &request.principal(), &app, &work],
                )
                .await?;
                Ok(None)
            }
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
        assert_eq!(parse_artifact_kind("card"), Some(Some("card")));
        assert_eq!(parse_artifact_kind("app"), Some(None));
        assert_eq!(parse_artifact_kind("stack"), None);
        assert_eq!(parse_artifact_kind(""), None);
    }
}
