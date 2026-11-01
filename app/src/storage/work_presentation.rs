//! APP-155–157: adopt only presentation changes on an exact saved work.
//! Draft and owner publication keep their existing role-separated operations;
//! adoption is one fenced transaction. A losing race can leave an audited
//! unused immutable publication, but cannot alter work, bindings or records.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::{append, confirm_identity, stored_content, Chain, Event},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct WorkPresentation {
    pub publication: String,
    pub digest: String,
    pub version: i64,
    pub policy: String,
    pub can_edit: bool,
    pub content: Value,
    /// Private optimistic premise, never supplied by the browser.
    bindings: String,
    scope: (String, String, String),
}

async fn bindings(t: &Transaction<'_>, org: &str, app: &str, work: &str) -> Result<String> {
    let rows=t.query("SELECT json_build_object('bindings',(SELECT coalesce(jsonb_agg(to_jsonb(b)-'publication_id' ORDER BY input_id),'[]'::jsonb) FROM genui.work_bindings b WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid),'collections',(SELECT coalesce(jsonb_agg(to_jsonb(c)-'publication_id' ORDER BY collection_id),'[]'::jsonb) FROM genui.record_collections c WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid),'dependencies',(SELECT coalesce(jsonb_agg(to_jsonb(d) ORDER BY input_id,source_id,source_revision_id),'[]'::jsonb) FROM genui.work_source_dependencies d WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid))::text", &[&org,&app,&work]).await?;
    rows.first()
        .ok_or(Error::Unavailable)?
        .try_get(0)
        .map_err(|_| Error::Unavailable)
}
fn data_content(mut value: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.remove("presentation");
    }
    value
}

impl Workspace {
    pub async fn can_edit_presentation(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<bool> {
        Ok(self.work_presentation(request, app, work).await?.can_edit)
    }
    pub async fn work_presentation(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<WorkPresentation> {
        require(valid_id(app) && valid_id(work))?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result=async {
            confirm_identity(&tx,request).await?;
            let pre=tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence(pre).await?;
            let t=permit.transaction(); let org=request.organization();
            let rows=t.query("SELECT w.publication_id::text,w.version::bigint,p.definition_digest::text,p.definition_revision_id::text,p.policy_id::text,p.policy_kind,a.owner_principal_id::text,(SELECT kind FROM genui.publication_policies current_policy WHERE current_policy.organization_id=p.organization_id AND current_policy.policy_id=p.policy_id ORDER BY policy_version DESC LIMIT 1) FROM genui.saved_work w JOIN genui.publications p USING(organization_id,app_id,publication_id) JOIN genui.apps a USING(organization_id,app_id) WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid", &[&org,&app,&work]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let get=|i|row.try_get::<_,String>(i).map_err(|_|Error::Unavailable);
            let revision=get(3)?; let (digest,canonical)=stored_content(t,org,app,&revision).await?;
            require(digest==get(2)?)?;
            let mut can_edit=get(6)?==request.principal() && get(5)?=="owner_publish" && get(7)?=="owner_publish";
            if can_edit { match permit.also(Action::EditDefinition,Resource::App(app)).await { Ok(())=>{}, Err(Error::Denied)=>can_edit=false, Err(e)=>return Err(e) } }
            permit.recheck_entitlement().await?;
            Ok(WorkPresentation { scope:(org.to_owned(),app.to_owned(),work.to_owned()),publication:get(0)?,version:row.try_get(1).map_err(|_|Error::Unavailable)?,digest,policy:get(4)?,can_edit,content:crate::admission::decode(&canonical).map_err(|_|Error::Unavailable)?,bindings:bindings(t,org,app,work).await? })
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    /// Returns false on an optimistic race. All candidate bytes are re-admitted
    /// from storage and compared with the pinned content except presentation.
    pub async fn adopt_presentation(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        expected: &WorkPresentation,
        publication: &str,
    ) -> Result<bool> {
        require(
            valid_id(app)
                && valid_id(work)
                && valid_id(publication)
                && expected.can_edit
                && expected.scope
                    == (
                        request.organization().to_owned(),
                        app.to_owned(),
                        work.to_owned(),
                    ),
        )?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result=async {
            confirm_identity(&tx,request).await?;
            let pre=tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence(pre).await?;
            permit.also(Action::EditDefinition,Resource::App(app)).await?;
            let t=permit.transaction(); let org=request.organization();
            // Lock the work alone. A join to its former publication can lose
            // the row during PostgreSQL's post-wait EvalPlanQual recheck.
            let locked=t.query("SELECT 1 FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid FOR UPDATE", &[&org,&app,&work]).await?;
            permit.recheck_entitlement().await?;
            require(locked.len()==1)?;
            let rows=t.query("SELECT w.publication_id::text,w.version::bigint,p.definition_digest::text,p.definition_revision_id::text,a.owner_principal_id::text FROM genui.saved_work w JOIN genui.publications p USING(organization_id,app_id,publication_id) JOIN genui.apps a USING(organization_id,app_id) WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid", &[&org,&app,&work]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let get=|i|row.try_get::<_,String>(i).map_err(|_|Error::Unavailable);
            require(get(4)?==request.principal())?;
            let version:i64=row.try_get(1).map_err(|_|Error::Unavailable)?;
            if get(0)?!=expected.publication || get(2)?!=expected.digest || version!=expected.version || bindings(t,org,app,work).await?!=expected.bindings { return Ok(false); }
            require(version<9_007_199_254_740_991)?;
            let rows=t.query("SELECT definition_revision_id::text,definition_digest::text,author_id::text,policy_kind,policy_id::text,policy_version::bigint FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[&org,&app,&publication]).await?;
            let next=rows.first().ok_or(Error::Denied)?;
            let next_text=|i|next.try_get::<_,String>(i).map_err(|_|Error::Unavailable);
            require(next_text(2)?==request.principal() && next_text(3)?=="owner_publish" && next_text(4)?==expected.policy)?;
            let policies=t.query("SELECT policy_version::bigint,kind FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND policy_id=$2::text::uuid ORDER BY policy_version DESC LIMIT 1", &[&org,&expected.policy]).await?;
            let current_policy=policies.first().ok_or(Error::Denied)?;
            require(current_policy.try_get::<_,i64>(0).map_err(|_|Error::Unavailable)?==next.try_get::<_,i64>(5).map_err(|_|Error::Unavailable)? && current_policy.try_get::<_,String>(1).map_err(|_|Error::Unavailable)?=="owner_publish")?;
            let (old_digest,old)=stored_content(t,org,app,&get(3)?).await?;
            let (new_digest,new)=stored_content(t,org,app,&next_text(0)?).await?;
            require(old_digest==expected.digest && new_digest==next_text(1)?)?;
            let decode=|bytes:&[u8]|crate::admission::decode(bytes).map_err(|_|Error::Unavailable);
            require(data_content(decode(&old)?)==data_content(decode(&new)?))?;
            // Only the deferred saved-work publication FKs cross this brief
            // intermediate state. All input/schema FKs remain immediate.
            t.batch("SET CONSTRAINTS ALL DEFERRED").await?;
            t.query("UPDATE genui.saved_work SET publication_id=$4::text::uuid,version=version+1 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&publication]).await?;
            t.query("UPDATE genui.record_collections SET publication_id=$4::text::uuid WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&publication]).await?;
            t.query("UPDATE genui.work_bindings SET publication_id=$4::text::uuid WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&publication]).await?;
            append(t,Chain::Workspace,request,Event{kind:"work.bound",id:None,app:Some(app),work:Some(work),record:None,outcome:Some(&new_digest)}).await?;
            permit.recheck_entitlement().await?;
            Ok(true)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
}
