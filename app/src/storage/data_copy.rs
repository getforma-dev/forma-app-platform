//! Bounded, all-or-nothing initialization of an editable copy of source data.
use super::{
    authorization::{Action, Resource, Transaction},
    definitions::{append, confirm_identity, stored_content, Chain, Event},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest, WorkPresentation,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const DATA_COPY_ROWS: usize = 100_000;
pub const DATA_COPY_BYTES: usize = 64 * 1024 * 1024;
/// The installed migration's closed JSONB transport bound, including hex and
/// row identity overhead. This is checked before any new publication too.
pub const DATA_COPY_TRANSPORT_BYTES: usize = 140_000_000;
#[derive(Clone, Debug)]
pub struct DataCopyInput {
    pub source_input: String,
    pub input: String,
    pub schema: String,
    pub fields: Value,
}
#[derive(Clone, Debug)]
pub struct DataCopyRows {
    pub source_input: String,
    pub rows: Vec<Value>,
}

fn identifier(prefix: &str, name: &str, used: &mut BTreeSet<String>) -> String {
    let base = format!("{prefix}{name}");
    let mut id = base.chars().take(58).collect::<String>();
    let stem = id.clone();
    let mut suffix = 0;
    while !used.insert(id.clone()) {
        suffix += 1;
        id = format!("{stem}_{suffix}");
    }
    id
}
/// Deterministic trusted transformation: keep every source contract byte-for-byte,
/// preserve components/layout, and redirect only typed query references.
pub fn editable_definition(content: &Value) -> Result<(Value, Vec<DataCopyInput>)> {
    let mut next = content.clone();
    let inputs = content["inputs"].as_array().ok_or(Error::Denied)?;
    require(
        !inputs.is_empty()
            && inputs.iter().all(|i| i["kind"] == "source")
            && content["record_schemas"]
                .as_array()
                .is_some_and(Vec::is_empty)
            && content["commands"].as_array().is_some_and(Vec::is_empty),
    )?;
    let mut used: BTreeSet<String> = inputs
        .iter()
        .filter_map(|i| i["id"].as_str().map(str::to_owned))
        .collect();
    let mut copies = Vec::new();
    let mut map = BTreeMap::new();
    for source in inputs {
        let old = source["id"].as_str().ok_or(Error::Denied)?;
        let input = identifier("data_", old, &mut used);
        let schema = identifier("rows_", old, &mut used);
        let fields = source["fields"].clone();
        let ids: Vec<&str> = fields
            .as_array()
            .ok_or(Error::Denied)?
            .iter()
            .map(|f| f["id"].as_str().ok_or(Error::Denied))
            .collect::<Result<_>>()?;
        require((1..=32).contains(&ids.len()))?;
        next["inputs"].as_array_mut().ok_or(Error::Denied)?.push(json!({"id":input,"contract_id":schema,"contract_version":1,"kind":"records","fields":fields}));
        next["record_schemas"]
            .as_array_mut()
            .ok_or(Error::Denied)?
            .push(json!({"id":schema,"version":1,"fields":fields}));
        for (prefix, operation) in [("add_", "create"), ("edit_", "update")] {
            let command = identifier(prefix, &schema, &mut used);
            next["commands"].as_array_mut().ok_or(Error::Denied)?.push(json!({"id":command,"record_schema_id":schema,"operation":operation,"writable_fields":ids,"transitions":[]}));
        }
        map.insert(old.to_owned(), input.clone());
        copies.push(DataCopyInput {
            source_input: old.to_owned(),
            input,
            schema,
            fields,
        });
    }
    fn rewrite(value: &mut Value, map: &BTreeMap<String, String>) {
        match value {
            Value::Object(object) => {
                for (key, item) in object {
                    if key == "input_id" {
                        if let Some(next) = item.as_str().and_then(|s| map.get(s)) {
                            *item = json!(next);
                        }
                    } else if key == "inputs" {
                        if let Some(items) = item.as_array_mut() {
                            for input in items {
                                if let Some(next) = input.as_str().and_then(|s| map.get(s)) {
                                    *input = json!(next);
                                }
                            }
                        }
                    } else {
                        rewrite(item, map);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    rewrite(item, map)
                }
            }
            _ => {}
        }
    }
    for view in next["views"].as_array_mut().ok_or(Error::Denied)? {
        if let Some(query) = view.get_mut("query") {
            rewrite(query, &map);
        }
    }
    Ok((next, copies))
}

#[derive(Clone, Debug)]
pub struct DataCopyInit {
    pub plans: Vec<DataCopyInput>,
    pub copies: Vec<DataCopyRows>,
}
fn payload(
    plans: &[DataCopyInput],
    copies: &[DataCopyRows],
    collections: &[String],
) -> Result<(String, usize, String)> {
    require(plans.len() == copies.len() && plans.len() == collections.len())?;
    let mut payload = Vec::new();
    let mut bytes = 0;
    let mut total = 0;
    for (plan, collection) in plans.iter().zip(collections) {
        let data = copies
            .iter()
            .find(|r| r.source_input == plan.source_input)
            .ok_or(Error::Denied)?;
        let fields = plan.fields.as_array().ok_or(Error::Denied)?;
        for row in &data.rows {
            let values = row.as_array().ok_or(Error::Denied)?;
            require(values.len() == fields.len())?;
            for (value, field) in values.iter().zip(fields) {
                let spec = super::commands::FieldSpec {
                    id: field["id"].as_str().ok_or(Error::Denied)?.to_owned(),
                    value_type: field["value_type"].clone(),
                    nullable: field["nullable"].as_bool().unwrap_or(false),
                };
                require(
                    value.as_object().is_some_and(|m| m.len() == 2)
                        && value["field_id"] == spec.id
                        && super::commands::compatible(&value["value"], &spec),
                )?;
            }
            let canonical = crate::admission::encode(row).map_err(|_| Error::Denied)?;
            bytes += canonical.len();
            total += 1;
            require(bytes <= DATA_COPY_BYTES && total <= DATA_COPY_ROWS)?;
            let mut hex = String::with_capacity(canonical.len() * 2);
            const DIGITS: &[u8] = b"0123456789abcdef";
            for byte in &canonical {
                hex.push(DIGITS[(byte >> 4) as usize] as char);
                hex.push(DIGITS[(byte & 15) as usize] as char);
            }
            payload.push(json!({"collection":collection,"record":crate::auth::capability::random_uuid().ok_or(Error::Unavailable)?,"source_input":plan.source_input,"canonical":hex}));
        }
    }
    let payload = serde_json::to_string(&payload).map_err(|_| Error::Unavailable)?;
    require(payload.len().saturating_add(total * 8) <= DATA_COPY_TRANSPORT_BYTES)?;
    let digest = crate::boxes::store::sha256(payload.as_bytes());
    Ok((payload, total, digest))
}
pub(super) async fn initialize(
    t: &Transaction<'_>,
    request: &TrustedRequest,
    app: &str,
    work: &str,
    publication: &str,
    initial: &DataCopyInit,
    collections: &BTreeMap<&str, (&str, i32)>,
) -> Result<()> {
    let ids: Vec<String> = initial
        .plans
        .iter()
        .map(|p| {
            collections
                .get(p.schema.as_str())
                .map(|(id, _)| (*id).to_owned())
                .ok_or(Error::Denied)
        })
        .collect::<Result<_>>()?;
    let (payload, total, digest) = payload(&initial.plans, &initial.copies, &ids)?;
    let org = request.organization();
    let result=match t.query_classified("SELECT genui_private.import_work_records($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::jsonb)", &[&org,&request.principal(),&app,&work,&payload]).await? {
        Ok(rows)=>rows,Err(code)=>{tracing::warn!(sqlstate=code.code(),"data_copy.initialization_refused");return Err(Error::Unavailable);}
    };
    require(
        result
            .first()
            .and_then(|r| r.try_get::<_, Option<i64>>(0).ok())
            .flatten()
            == Some(total as i64),
    )?;
    let key = crate::auth::capability::random_uuid()
        .ok_or(Error::Unavailable)?
        .replace('-', "");
    t.query("INSERT INTO genui.work_data_copies(organization_id,app_id,work_id,actor_principal_id,idempotency_key,original_publication_id,original_work_version,publication_id,imported_rows,payload_digest) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5,$6::text::uuid,1,$6::text::uuid,$7::bigint,$8::text)", &[&org,&app,&work,&request.principal(),&key,&publication,&(total as i64),&digest]).await?;
    append(
        t,
        Chain::Workspace,
        request,
        Event {
            kind: "work.bound",
            id: None,
            app: Some(app),
            work: Some(work),
            record: None,
            outcome: Some(&digest),
        },
    )
    .await?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct DataCopyReceipt {
    pub rows: i64,
    pub key: String,
    pub publication: String,
    pub version: i64,
}

impl Workspace {
    pub async fn data_copy_rows(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Option<i64>> {
        Ok(self
            .data_copy_receipt(request, app, work)
            .await?
            .map(|r| r.rows))
    }
    pub async fn data_copy_receipt(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Option<DataCopyReceipt>> {
        require(valid_id(app) && valid_id(work))?;
        let mut tx = Transaction::begin(&self.inner, request).await?;
        let result=async {
            confirm_identity(&tx,request).await?;
            let pre=tx.precheck(Action::RecordRead,Resource::Work{app,work}).await?;
            let permit=tx.fence(pre).await?;
            let rows=permit.transaction().query("SELECT imported_rows,idempotency_key,original_publication_id::text,original_work_version::bigint FROM genui.work_data_copies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&request.organization(),&app,&work]).await?;
            permit.recheck_entitlement().await?;
            rows.first().map(|r|Ok(DataCopyReceipt{rows:r.try_get(0).map_err(|_|Error::Unavailable)?,key:r.try_get(1).map_err(|_|Error::Unavailable)?,publication:r.try_get(2).map_err(|_|Error::Unavailable)?,version:r.try_get(3).map_err(|_|Error::Unavailable)?})).transpose()
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn adopt_data_copy(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
        expected: &WorkPresentation,
        publication: &str,
        key: &str,
        copies: &[DataCopyRows],
    ) -> Result<bool> {
        require(
            valid_id(app)
                && valid_id(work)
                && valid_id(publication)
                && super::idempotency_key_shaped(key)
                && expected.can_edit,
        )?;
        let (next_content, plans) = editable_definition(&expected.content)?;
        require(copies.len() == plans.len())?;
        let collections = plans
            .iter()
            .map(|_| crate::auth::capability::random_uuid().ok_or(Error::Unavailable))
            .collect::<Result<Vec<_>>>()?;
        let (payload, total, digest) = payload(&plans, copies, &collections)?;
        let mut tx = Transaction::begin_import(&self.inner, request).await?;
        let result=async {
            confirm_identity(&tx,request).await?;
            let pre=tx.precheck(Action::RecordWrite,Resource::Work{app,work}).await?;
            let permit=tx.fence(pre).await?; permit.also(Action::EditDefinition,Resource::App(app)).await?;
            let t=permit.transaction(); let org=request.organization();
            t.query("SELECT 1 FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid FOR UPDATE", &[&org,&app,&work]).await?;
            permit.recheck_entitlement().await?;
            let already=t.query("SELECT idempotency_key,original_publication_id::text,original_work_version::bigint FROM genui.work_data_copies WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work]).await?;
            if let Some(row)=already.first(){require(row.try_get::<_,String>(0).map_err(|_|Error::Unavailable)?==key&&row.try_get::<_,String>(1).map_err(|_|Error::Unavailable)?==expected.publication&&row.try_get::<_,i64>(2).map_err(|_|Error::Unavailable)?==expected.version)?;return Ok(true);}
            let rows=t.query("SELECT w.publication_id::text,w.version::bigint,p.definition_revision_id::text,a.owner_principal_id::text,w.created_by::text FROM genui.saved_work w JOIN genui.publications p USING(organization_id,app_id,publication_id) JOIN genui.apps a USING(organization_id,app_id) WHERE w.organization_id=$1::text::uuid AND w.app_id=$2::text::uuid AND w.work_id=$3::text::uuid", &[&org,&app,&work]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let get=|i|row.try_get::<_,String>(i).map_err(|_|Error::Unavailable);
            require(get(3)?==request.principal()&&get(4)?==request.principal())?;
            if get(0)?!=expected.publication||row.try_get::<_,i64>(1).map_err(|_|Error::Unavailable)?!=expected.version||super::work_presentation::bindings(t,org,app,work).await?!=expected.bindings {return Ok(false);}
            let (old_digest,old)=stored_content(t,org,app,&get(2)?).await?;
            require(old_digest==expected.digest&&crate::admission::decode(&old).map_err(|_|Error::Unavailable)?==expected.content)?;
            let rows=t.query("SELECT definition_revision_id::text,definition_digest::text,author_id::text,policy_id::text,policy_kind,policy_version::bigint FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid", &[&org,&app,&publication]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let get=|i|row.try_get::<_,String>(i).map_err(|_|Error::Unavailable);
            require(get(2)?==request.principal()&&get(3)?==expected.policy&&get(4)?=="owner_publish")?;
            let policies=t.query("SELECT policy_version::bigint,kind FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND policy_id=$2::text::uuid ORDER BY policy_version DESC LIMIT 1", &[&org,&expected.policy]).await?;
            let policy=policies.first().ok_or(Error::Denied)?;
            require(policy.try_get::<_,String>(1).map_err(|_|Error::Unavailable)?=="owner_publish"&&policy.try_get::<_,i64>(0).map_err(|_|Error::Unavailable)?==row.try_get::<_,i64>(5).map_err(|_|Error::Unavailable)?)?;
            let (new_digest,new)=stored_content(t,org,app,&get(0)?).await?;
            require(new_digest==get(1)?&&crate::admission::decode(&new).map_err(|_|Error::Unavailable)?==next_content)?;
            t.batch("SET CONSTRAINTS ALL DEFERRED").await?;
            t.query("UPDATE genui.saved_work SET publication_id=$4::text::uuid,version=version+1 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&publication]).await?;
            t.query("UPDATE genui.work_bindings SET publication_id=$4::text::uuid WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid", &[&org,&app,&work,&publication]).await?;
            for (plan,collection) in plans.iter().zip(&collections) {
                let contract=&next_content["inputs"].as_array().ok_or(Error::Denied)?.iter().find(|i|i["id"]==plan.input).ok_or(Error::Denied)?;
                let contract_digest=crate::admission::digest::digest(crate::admission::digest::INPUT_CONTRACT,&crate::admission::encode(contract).map_err(|_|Error::Unavailable)?);
                t.query("INSERT INTO genui.record_collections(organization_id,app_id,work_id,publication_id,collection_id,schema_id,schema_version) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::text,1)", &[&org,&app,&work,&publication,&collection,&plan.schema]).await?;
                t.query("INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,collection_id,schema_id,schema_version,revision_policy) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,'records',$6::text,$7::text::uuid,$8::text,1,'committed_snapshot')", &[&org,&app,&work,&publication,&plan.input,&contract_digest,&collection,&plan.schema]).await?;
            }
            let result=match t.query_classified("SELECT genui_private.import_work_records($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text::jsonb)", &[&org,&request.principal(),&app,&work,&payload]).await? {
        Ok(rows)=>rows,Err(code)=>{tracing::warn!(sqlstate=code.code(),"data_copy.initialization_refused");return Err(Error::Unavailable);}
    };
            require(result.first().and_then(|r|r.try_get::<_,Option<i64>>(0).ok()).flatten()==Some(total as i64))?;
            t.query("INSERT INTO genui.work_data_copies(organization_id,app_id,work_id,actor_principal_id,idempotency_key,original_publication_id,original_work_version,publication_id,imported_rows,payload_digest) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5,$6::text::uuid,$10::bigint,$7::text::uuid,$8::bigint,$9::text)", &[&org,&app,&work,&request.principal(),&key,&expected.publication,&publication,&(total as i64),&digest,&expected.version]).await?;
            append(t,Chain::Workspace,request,Event{kind:"work.bound",id:None,app:Some(app),work:Some(work),record:None,outcome:Some(&digest)}).await?;
            permit.recheck_entitlement().await?; Ok(true)
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_copy_preserves_original_and_components() {
        let content = json!({"inputs":[{"id":"transactions","contract_id":"original_v1","contract_version":1,"kind":"source","fields":[{"id":"amount","value_type":{"type":"decimal","scale":2,"unit":"count"},"nullable":true}]}],"record_schemas":[],"commands":[],"views":[{"id":"total","kind":"summary","query":{"inputs":["transactions"],"operations":[{"type":"aggregate","measures":[{"field":{"input_id":"transactions","field_id":"amount"}}]}]}}],"default_layout":[{"view_id":"total","column":0,"row":0,"width":12}]});
        let (next, plans) = editable_definition(&content).unwrap();
        assert_eq!(next["inputs"][0], content["inputs"][0]);
        assert_eq!(next["default_layout"], content["default_layout"]);
        assert_eq!(next["views"][0]["id"], "total");
        assert_eq!(next["views"][0]["query"]["inputs"][0], plans[0].input);
        assert_eq!(
            next["views"][0]["query"]["operations"][0]["measures"][0]["field"]["input_id"],
            plans[0].input
        );
        assert_eq!(next["commands"].as_array().unwrap().len(), 2);
        assert!(editable_definition(&next).is_err());
        let copies = vec![DataCopyRows {
            source_input: "transactions".into(),
            rows: vec![
                json!([{"field_id":"amount","value":{"type":"decimal","coefficient":"1","scale":2,"unit":"count"}}]),
            ],
        }];
        let (encoded, count, _) = payload(
            &plans,
            &copies,
            &["00000000-0000-4000-8000-000000000001".into()],
        )
        .unwrap();
        assert_eq!(count, 1);
        assert!(encoded.contains("canonical"));
        let mut invalid = copies;
        invalid[0].rows[0][0]["value"]["scale"] = json!(3);
        assert!(payload(
            &plans,
            &invalid,
            &["00000000-0000-4000-8000-000000000001".into()]
        )
        .is_err());
    }
}
