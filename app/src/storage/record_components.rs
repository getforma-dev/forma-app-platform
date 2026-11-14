//! APP-158/159: exact-work, bounded, read-only record snapshots. A run is an
//! opaque authorization product, never a deserialized request. All components
//! in one run see one PostgreSQL REPEATABLE READ snapshot. Release uses a fresh
//! transaction and current member/share authority; no guest owner impersonation.
use super::{
    authorization::Transaction,
    pool::Data,
    queries::{pinned_many_with, Premise},
    require,
    shared_access::check_guest,
    valid_id, Error, GuestAccess, Result, TrustedRequest, QUERY_HANDLER_DEADLINE,
};
use crate::admission;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use tokio::time::Instant;

pub const RECORD_COMPONENT_ROWS: usize = 2_000;
pub const RECORD_COMPONENT_BYTES: usize = 524_288;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Actor {
    Member { principal: String, epoch: i64 },
    Guest(Box<GuestAccess>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordComponentsRun {
    organization: String,
    app: String,
    work: String,
    publication: String,
    definition_digest: String,
    work_version: i64,
    policy_epoch: i64,
    actor: Actor,
    payload: Value,
    snapshot: Value,
    snapshot_digest: String,
}
impl RecordComponentsRun {
    pub fn organization(&self) -> &str {
        &self.organization
    }
    pub fn app(&self) -> &str {
        &self.app
    }
    pub fn work(&self) -> &str {
        &self.work
    }
    pub fn publication(&self) -> &str {
        &self.publication
    }
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }
    pub fn work_version(&self) -> i64 {
        self.work_version
    }
    pub fn policy_epoch(&self) -> i64 {
        self.policy_epoch
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    pub fn snapshot(&self) -> &Value {
        &self.snapshot
    }
    pub fn snapshot_digest(&self) -> &str {
        &self.snapshot_digest
    }
    pub fn is_empty(&self) -> bool {
        self.payload["components"]
            .as_array()
            .is_none_or(Vec::is_empty)
    }
    pub fn attribution(&self) -> Value {
        match &self.actor {
            Actor::Member { principal, epoch } => {
                json!({"kind":"member","principal_id":principal,"membership_epoch":epoch})
            }
            Actor::Guest(a) => {
                json!({"kind":"guest","account_id":a.account_id(),"share_id":a.share_id(),"share_version":a.share_version()})
            }
        }
    }
}
pub fn record_component_digest(domain: &str, value: &Value) -> Result<String> {
    if !matches!(
        domain,
        "forma:record-components-snapshot:v1"
            | "forma:record-components-result:v1"
            | "forma:record-components-receipt:v1"
    ) {
        return Err(Error::Unavailable);
    }
    let encoded = admission::encode(value).map_err(|_| Error::Unavailable)?;
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    hash.update(encoded);
    Ok(format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}
fn text(row: &tokio_postgres::Row, i: usize) -> Result<String> {
    row.try_get(i).map_err(|_| Error::Unavailable)
}
fn number(row: &tokio_postgres::Row, i: usize) -> Result<i64> {
    row.try_get(i).map_err(|_| Error::Unavailable)
}
fn same(pin: &Premise, run: &RecordComponentsRun) -> bool {
    pin.publication == run.publication
        && pin.definition_digest == run.definition_digest
        && pin.work_version == run.work_version
        && pin.policy_epoch == run.policy_epoch
}

impl Data {
    pub async fn record_components(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<RecordComponentsRun> {
        require(valid_id(app) && valid_id(work))?;
        let tx = Transaction::begin_read_only(
            &self.inner,
            request,
            Instant::now() + QUERY_HANDLER_DEADLINE,
        )
        .await?;
        let result = async {
            let grants = tx.read_only_grants(&[(app, work)]).await?;
            require(grants.first().is_some_and(|g| g.record_read))?;
            capture(
                &tx,
                &request.organization,
                app,
                work,
                Actor::Member {
                    principal: request.principal.clone(),
                    epoch: request.epoch,
                },
            )
            .await
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn guest_record_components(
        &self,
        access: &GuestAccess,
    ) -> Result<RecordComponentsRun> {
        let tx = Transaction::begin_shared_read_only(
            &self.inner,
            &access.organization,
            access.account_id(),
        )
        .await?;
        let result = async {
            check_guest(&tx, access).await?;
            capture(
                &tx,
                &access.organization,
                &access.app,
                &access.work,
                Actor::Guest(Box::new(access.clone())),
            )
            .await
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn release_record_components(
        &self,
        request: &TrustedRequest,
        run: &RecordComponentsRun,
    ) -> Result<()> {
        require(
            run.organization == request.organization
                && matches!(&run.actor,Actor::Member{principal,epoch} if principal==&request.principal && *epoch==request.epoch),
        )?;
        let tx = Transaction::begin_read_only(
            &self.inner,
            request,
            Instant::now() + QUERY_HANDLER_DEADLINE,
        )
        .await?;
        let result = async {
            require(
                tx.read_only_grants(&[(&run.app, &run.work)])
                    .await?
                    .first()
                    .is_some_and(|g| g.record_read),
            )?;
            let pin = pinned_many_with(&tx, &run.organization, &[(&run.app, &run.work)], false)
                .await?
                .pop()
                .ok_or(Error::Unavailable)??;
            require(same(&pin.0, run))
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn release_guest_record_components(
        &self,
        access: &GuestAccess,
        run: &RecordComponentsRun,
    ) -> Result<()> {
        require(matches!(&run.actor,Actor::Guest(a) if a.as_ref()==access))?;
        self.release_guest_access(access).await
    }
}
async fn capture(
    tx: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    actor: Actor,
) -> Result<RecordComponentsRun> {
    let (pin, content) = pinned_many_with(tx, org, &[(app, work)], false)
        .await?
        .pop()
        .ok_or(Error::Unavailable)??;
    let (admitted, bytes) = content?;
    require(admitted == pin.definition_digest)?;
    let content = admission::decode(&bytes).map_err(|_| Error::Unavailable)?;
    let components: Vec<Value> = content["views"]
        .as_array()
        .ok_or(Error::Unavailable)?
        .iter()
        .filter(|v| v.get("records").is_some())
        .cloned()
        .collect();
    let filters = content
        .get("record_filters")
        .cloned()
        .unwrap_or_else(|| json!([]));
    if let Actor::Guest(a) = &actor {
        require(
            a.publication == pin.publication
                && a.work_version == pin.work_version
                && a.definition_digest == pin.definition_digest
                && a.policy_epoch == pin.policy_epoch,
        )?;
        require(components.iter().all(|v| {
            v["id"]
                .as_str()
                .is_some_and(|id| a.views.iter().any(|v| v == id))
        }))?;
    }
    let inputs: BTreeSet<&str> = components
        .iter()
        .filter_map(|v| v["records"]["input_id"].as_str())
        .collect();
    let mut datasets = Vec::new();
    let mut total_rows = 0usize;
    let mut total_bytes = 0usize;
    let mut provenance = Vec::new();
    for input in inputs {
        let binding=tx.query("SELECT collection_id::text,schema_id::text,schema_version::bigint FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND input_id=$4::text AND kind='records' AND revision_policy='committed_snapshot'", &[&org,&app,&work,&input]).await?;
        let b = binding.first().ok_or(Error::Denied)?;
        let collection = text(b, 0)?;
        let schema = text(b, 1)?;
        let version = number(b, 2)?;
        require(
            components
                .iter()
                .filter(|v| v["records"]["input_id"] == input)
                .all(|v| v["records"]["schema_id"] == schema),
        )?;
        if let Actor::Guest(a) = &actor {
            require(a.collections.contains(&collection))?;
        }
        let declared = content["record_schemas"]
            .as_array()
            .and_then(|s| {
                s.iter()
                    .find(|s| s["id"] == schema && s["version"] == version)
            })
            .ok_or(Error::Unavailable)?;
        // Aggregate bounds before materializing bytes. No result is calculated
        // from a prefix and called a total. Same snapshot as the row read below.
        // APP-191: the row rule on the bounds and the rows alike.
        let own = super::authorization::own_rows(
            "v",
            "$2::text::uuid",
            "$3::text::uuid",
            super::authorization::SCOPED,
        );
        let bounds=tx.query(&format!("SELECT count(*)::bigint,coalesce(sum(octet_length(v.canonical_values)),0)::bigint FROM genui.internal_records r JOIN genui.internal_record_revisions v USING(organization_id,app_id,work_id,collection_id,record_id) WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND v.version=r.current_version AND {own}"), &[&org,&app,&work,&collection]).await?;
        let count = usize::try_from(number(&bounds[0], 0)?).map_err(|_| Error::Unavailable)?;
        total_rows = total_rows.checked_add(count).ok_or(Error::Unavailable)?;
        total_bytes = total_bytes
            .checked_add(usize::try_from(number(&bounds[0], 1)?).map_err(|_| Error::Unavailable)?)
            .ok_or(Error::Unavailable)?;
        if total_rows > RECORD_COMPONENT_ROWS || total_bytes > RECORD_COMPONENT_BYTES {
            return Err(Error::ProjectionLimit);
        }
        let rows=tx.query(&format!("SELECT r.record_id::text,r.current_version::bigint,v.commit_sequence::bigint,v.canonical_values FROM genui.internal_records r JOIN genui.internal_record_revisions v USING(organization_id,app_id,work_id,collection_id,record_id) WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND v.version=r.current_version AND {own} ORDER BY v.commit_sequence DESC,r.record_id LIMIT 2001"), &[&org,&app,&work,&collection]).await?;
        require(rows.len() == count)?;
        let mut records = Vec::new();
        let mut revisions = Vec::new();
        for row in &rows {
            let bytes: Vec<u8> = row.try_get(3).map_err(|_| Error::Unavailable)?;
            let values = admission::decode(&bytes).map_err(|_| Error::Unavailable)?;
            let id = text(row, 0)?;
            let revision = number(row, 1)?;
            let sequence = number(row, 2)?;
            records.push(json!({"id":id,"version":revision,"sequence":sequence,"values":values}));
            revisions.push(json!({"record_id":id,"version":revision,"commit_sequence":sequence}));
        }
        provenance.push(json!({"input_id":input,"collection_id":collection,"schema_id":schema,"schema_version":version,"records":revisions}));
        datasets.push(
            json!({"input_id":input,"schema_id":schema,"fields":declared["fields"],"rows":records}),
        );
    }
    let snapshot_digest =
        record_component_digest("forma:record-components-snapshot:v1", &json!(datasets))?;
    let payload = json!({"components":components,"filters":filters,"datasets":datasets});
    Ok(RecordComponentsRun {
        organization: org.into(),
        app: app.into(),
        work: work.into(),
        publication: pin.publication,
        definition_digest: pin.definition_digest,
        work_version: pin.work_version,
        policy_epoch: pin.policy_epoch,
        actor,
        payload,
        snapshot: json!(provenance),
        snapshot_digest,
    })
}
