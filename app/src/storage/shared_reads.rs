//! Exact-work guest reads. The account/share capability is validated before
//! opening resource data and again after assembling it. No member request or
//! copied source permission is constructed anywhere in this module.
use super::{
    actions::{listing_of, records_many},
    pool::Data,
    queries::{self, collections_many, outline_of, pinned_many_with, QueryActor},
    require,
    shared_access::{check_guest, guest_transaction, GuestAccess},
    Error, QueryRun, Result, WorkReads,
};
use serde_json::Value;

/// An engine plan with explicit guest attribution and its release authority.
/// It is not accepted by the member receipt or member release paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestQueryRun {
    plan: QueryRun,
    access: GuestAccess,
}
impl GuestQueryRun {
    pub fn engine_plan(&self) -> &QueryRun {
        &self.plan
    }
    pub fn access(&self) -> &GuestAccess {
        &self.access
    }
}

impl Data {
    pub async fn guest_work_reads(&self, access: &GuestAccess) -> Result<WorkReads> {
        let tx = guest_transaction(&self.inner, access).await?;
        let result = async {
            let org = access.organization.as_str();
            let keys = [(access.app.as_str(), access.work.as_str())];
            // Member-only lock_work_revision is intentionally not called.
            // The share's immutable pins are checked before and after reads.
            let pinned = pinned_many_with(&tx, org, &keys, false)
                .await?
                .pop()
                .ok_or(Error::Unavailable)?;
            let (pin, _) = pinned.as_ref().map_err(|e| *e)?;
            require(
                pin.publication == access.publication
                    && pin.definition_digest == access.definition_digest
                    && pin.work_version == access.work_version
                    && pin.policy_epoch == access.policy_epoch,
            )?;
            let mut outline = outline_of(&pinned)?;
            if let Some(name) = super::work_labels::names(&tx, org, &keys)
                .await?
                .get(&(access.app.clone(), access.work.clone()))
                .and_then(|label| label.name.as_ref())
            {
                outline.title = name.clone();
            }
            require(outline.views.iter().all(|v| access.views.contains(&v.id)))?;
            outline.can_write = false;
            // APP-098 DS-84: the same check as the member's outline; the
            // bindings are read under the share's organization scope.
            let samples = super::queries::sample_works(&tx, org, &keys).await?;
            outline.sample_data = super::queries::reads_sample(
                &outline.query_inputs,
                samples.get(&(access.app.clone(), access.work.clone())),
            );
            let collections = collections_many(&tx, org, &keys)
                .await?
                .pop()
                .ok_or(Error::Unavailable)?;
            let records = records_many(&tx, org, &keys)
                .await?
                .pop()
                .ok_or(Error::Unavailable)?;
            // A guest is never a reviewer: no `reviewer_only` command is shown.
            let mut listing = listing_of(keys[0], false, false, &pinned, &collections, &records)?;
            require(
                listing
                    .collections
                    .iter()
                    .all(|c| access.collections.contains(&c.collection)),
            )?;
            outline.collections = listing
                .collections
                .iter()
                .map(|c| (c.collection.clone(), c.schema.clone()))
                .collect();
            // A read-only document never exposes declared commands or writable
            // field metadata; record hrefs are also absent from its HTTP props.
            for collection in &mut listing.collections {
                collection.commands.clear();
                for field in &mut collection.fields {
                    field.writable = false;
                }
            }
            check_guest(&tx, access).await?;
            Ok(WorkReads {
                outline: Ok(outline),
                records: Some(Ok(listing)),
            })
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }

    pub async fn guest_run_query(
        &self,
        access: &GuestAccess,
        view: &str,
        parameters: &Value,
    ) -> Result<GuestQueryRun> {
        require(access.views.iter().any(|id| id == view))?;
        let parameters = queries::parse_parameters(parameters).ok_or(Error::Denied)?;
        let tx = guest_transaction(&self.inner, access).await?;
        let result = async {
            let org = access.organization.as_str();
            let app = access.app.as_str();
            let work = access.work.as_str();
            let pinned = pinned_many_with(&tx, org, &[(app, work)], false).await?
                .pop().ok_or(Error::Unavailable)??;
            let (premise, content) = pinned;
            require(premise.publication == access.publication
                && premise.definition_digest == access.definition_digest
                && premise.work_version == access.work_version
                && premise.policy_epoch == access.policy_epoch)?;
            let (admitted, content) = content?;
            require(admitted == premise.definition_digest)?;
            let (stored, fields) = queries::view_query(&content, view)?;
            let input = stored["inputs"][0].as_str().ok_or(Error::Unavailable)?;
            let records=if queries::record_input(&content,input)? {
                let (collection,_,_)=super::record_query::binding(&tx,org,app,work,input).await?;
                require(access.collections.contains(&collection))?;
                let record=super::record_query::capture(&tx,org,app,work,input,&fields).await?;
                Some(record)
            } else {None};
            let source=if records.is_none(){Some(queries::bound_source(&tx,org,app,work,input).await?)}else{None};
            let dependencies=queries::dependencies(&tx,org,app,work).await?;
            if let Some(source)=&source {require(dependencies.contains(source))?;}
            let snapshot: i64 = if let Some(records) = &records { records.sequence } else {
                let own = super::authorization::own_rows("v", "$2::text::uuid", "$3::text::uuid", super::authorization::SCOPED);
                let rows = tx.query(&format!("SELECT coalesce(max(v.commit_sequence),0)::bigint FROM genui.internal_record_revisions v WHERE v.organization_id=$1::text::uuid AND v.app_id=$2::text::uuid AND v.work_id=$3::text::uuid AND {own}"), &[&org,&app,&work]).await?;
                rows.first().ok_or(Error::Unavailable)?.try_get(0).map_err(|_| Error::Unavailable)?
            };
            let query = parameters.apply(&stored, &fields).ok_or(Error::Denied)?;
            let plan = QueryRun {
                actor: QueryActor::Guest { account: access.account_id().into(), share: access.share.clone(), version: access.version },
                organization: org.into(),
                principal: access.account_id().into(),
                // There is no membership epoch for a guest. Member receipts
                // and release paths explicitly reject this actor variant.
                membership_epoch: 0,
                app: app.into(), work: work.into(), work_version: premise.work_version,
                publication: premise.publication, definition_digest: premise.definition_digest,
                policy_epoch: premise.policy_epoch, view: view.into(), view_query: stored,
                query, parameters: parameters.named_values(), filter_epoch: parameters.filter_epoch,
                record_snapshot_sequence: snapshot, source, records, dependencies, fields,
            };
            check_guest(&tx, access).await?;
            Ok(GuestQueryRun { plan, access: access.clone() })
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }

    pub async fn release_guest_result(
        &self,
        access: &GuestAccess,
        run: &GuestQueryRun,
    ) -> Result<()> {
        require(access == &run.access)?;
        let tx = guest_transaction(&self.inner, access).await?;
        let result = async {
            let plan = &run.plan;
            let (premise, content) = pinned_many_with(
                &tx,
                access.organization(),
                &[(access.app(), access.work())],
                false,
            )
            .await?
            .pop()
            .ok_or(Error::Unavailable)??;
            let (admitted, _) = content?;
            require(
                premise.publication == plan.publication
                    && premise.definition_digest == plan.definition_digest
                    && admitted == plan.definition_digest
                    && premise.work_version == plan.work_version
                    && premise.policy_epoch == plan.policy_epoch,
            )?;
            if let Some(expected) = plan.source() {
                let source = queries::bound_source(
                    &tx,
                    access.organization(),
                    access.app(),
                    access.work(),
                    expected.input(),
                )
                .await?;
                require(&source == expected)?;
            }
            if let Some(records) = plan.records() {
                require(access.collections.contains(&records.collection))?;
                super::record_query::release(
                    &tx,
                    access.organization(),
                    access.app(),
                    access.work(),
                    records,
                )
                .await?;
            }
            require(
                queries::dependencies(&tx, access.organization(), access.app(), access.work())
                    .await?
                    == plan.dependencies(),
            )?;
            check_guest(&tx, access).await
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
}
