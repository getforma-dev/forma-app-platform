//! Presentation names belong to a saved work, not its reused authoring app.
//! The member/work fence is held through the optimistic update and audit.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::{append, confirm_identity, Chain, Event},
    labels::{label_name, OWNER_OR_ADMIN},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use std::collections::BTreeMap;

/// Audit outcome identity, separate from admitted content/receipt identities.
fn outcome(work: &str, label: &WorkLabel) -> Result<String> {
    use sha2::{Digest as _, Sha256};
    let bytes = crate::admission::encode(&serde_json::json!({
        "work_id":work,"name":label.name,"version":label.version
    }))
    .map_err(|_| Error::Unavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(b"forma:work-label:v1\0");
    hasher.update(bytes);
    Ok(format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkLabel {
    pub name: Option<String>,
    pub version: i64,
    pub can_name: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkLabelResult {
    Saved(WorkLabel),
    Unchanged(WorkLabel),
    Conflict,
}

/// Call only inside an already authorized member or guest work transaction.
pub(super) async fn names(
    tx: &Transaction<'_>,
    organization: &str,
    keys: &[(&str, &str)],
) -> Result<BTreeMap<(String, String), WorkLabel>> {
    require(keys.len() <= 256)?;
    if keys.is_empty() {
        return Ok(BTreeMap::new());
    }
    let apps: Vec<_> = keys.iter().map(|(app, _)| *app).collect();
    let works: Vec<_> = keys.iter().map(|(_, work)| *work).collect();
    let rows = tx.query("SELECT w.app_id::text,w.work_id::text,w.display_name,w.name_version FROM genui.saved_work w JOIN unnest($2::text[],$3::text[]) k(app,work) ON w.app_id=k.app::uuid AND w.work_id=k.work::uuid WHERE w.organization_id=$1::text::uuid", &[&organization,&apps,&works]).await?;
    rows.iter()
        .map(|row| {
            Ok((
                (
                    row.try_get(0).map_err(|_| Error::Unavailable)?,
                    row.try_get(1).map_err(|_| Error::Unavailable)?,
                ),
                WorkLabel {
                    name: row.try_get(2).map_err(|_| Error::Unavailable)?,
                    version: row.try_get(3).map_err(|_| Error::Unavailable)?,
                    can_name: false,
                },
            ))
        })
        .collect()
}

impl Workspace {
    pub async fn work_label(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<WorkLabel> {
        require(valid_id(app) && valid_id(work))?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&tx, request).await?;
            let precheck = tx.precheck(Action::RecordRead, Resource::Work { app, work }).await?;
            let permit = tx.fence(precheck).await?;
            let t = permit.transaction();
            let mut label = names(t, request.organization(), &[(app, work)]).await?
                .remove(&(app.to_owned(), work.to_owned())).ok_or(Error::Denied)?;
            label.can_name = !t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {OWNER_OR_ADMIN}"), &[&request.organization(),&request.principal(),&app]).await?.is_empty();
            Ok(label)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }

    pub async fn label_work(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        name: Option<&str>,
        expected_version: i64,
    ) -> Result<WorkLabelResult> {
        require(
            valid_id(app)
                && valid_id(work)
                && (0..9_007_199_254_740_991).contains(&expected_version),
        )?;
        let name = match name {
            Some(value) => label_name(value).ok_or(Error::Denied)?,
            None => None,
        };
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result = async {
            confirm_identity(&tx, request).await?;
            let precheck = tx.precheck(Action::RecordRead, Resource::Work { app, work }).await?;
            let permit = tx.fence(precheck).await?;
            let t = permit.transaction();
            let org = request.organization();
            let principal = request.principal();
            let allowed = t.query(&format!("SELECT 1 FROM genui.apps a WHERE a.organization_id=$1::text::uuid AND a.app_id=$3::text::uuid AND {OWNER_OR_ADMIN}"), &[&org,&principal,&app]).await?;
            require(allowed.len() == 1)?;
            // Locks only this saved work; unrelated siblings keep their names.
            let rows = t.query("SELECT display_name,name_version FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid FOR UPDATE", &[&org,&app,&work]).await?;
            // The work row can wait behind another rename after the member
            // fence; expiry is judged again before any result or write.
            permit.recheck_entitlement().await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let current = WorkLabel { name: row.try_get(0).map_err(|_| Error::Unavailable)?, version: row.try_get(1).map_err(|_| Error::Unavailable)?, can_name: true };
            if current.version != expected_version { return Ok(WorkLabelResult::Conflict); }
            if current.name == name { return Ok(WorkLabelResult::Unchanged(current)); }
            let next = WorkLabel { name, version: current.version + 1, can_name: true };
            t.query("UPDATE genui.saved_work SET display_name=$4,name_version=$5 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&next.name,&next.version]).await?;
            let outcome = outcome(work, &next)?;
            append(t, Chain::Workspace, request, Event { kind: "app.labeled", id: None, app: Some(app), work: Some(work), record: None, outcome: Some(&outcome) }).await?;
            Ok(WorkLabelResult::Saved(next))
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
}
