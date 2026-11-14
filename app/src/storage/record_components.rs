//! APP-158/159: exact-work, bounded, read-only record snapshots. A run is an
//! opaque authorization product, never a deserialized request. All components
//! in one run see one PostgreSQL REPEATABLE READ snapshot. Release uses a fresh
//! transaction and current member/share authority; no guest owner impersonation.
//!
//! ADR-020 (APP-204): every row carries its revision's `creator` and the
//! payload a `creators` label map (an id-sorted `[{id, label}]` array), so a
//! ranking can group by person; the key input a `matches` measure names is
//! captured like any other input. Every
//! input is read under the viewer's own rows (`authorization::own_rows`), so
//! a member who may read only their own entries is ranked alone and scores
//! against no key row they cannot read. Labels are `directory::label` for a
//! member; a guest (an account outside the organization) sees only the
//! neutral handle, never a member's chosen name (ADR-020 clarification).
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
use std::collections::{BTreeMap, BTreeSet};
use tokio::time::Instant;

pub const RECORD_COMPONENT_ROWS: usize = 2_000;
pub const RECORD_COMPONENT_BYTES: usize = 524_288;
/// The child reads at most this many request bytes (`query_engine.py`); a
/// payload the child would refuse is a projection limit, refused before launch.
pub const RECORD_COMPONENT_PAYLOAD_BYTES: usize = 1_000_000;
/// ADR-020 C4: the snapshot digest's domain since rows carry `creator`.
pub const SNAPSHOT_DOMAIN: &str = "forma:record-components-snapshot:v2";

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
    /// Unit tests of the projection protocol: a run over a synthetic payload,
    /// never a capture. Test builds only.
    #[cfg(test)]
    pub fn for_tests(payload: Value) -> Self {
        let id = "00000000-0000-4000-8000-000000000001".to_owned();
        Self {
            organization: id.clone(),
            app: id.clone(),
            work: id.clone(),
            publication: id.clone(),
            definition_digest: format!("sha256:{}", "0".repeat(64)),
            work_version: 1,
            policy_epoch: 1,
            actor: Actor::Member {
                principal: id,
                epoch: 1,
            },
            payload,
            snapshot: json!([]),
            snapshot_digest: format!("sha256:{}", "0".repeat(64)),
        }
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
        SNAPSHOT_DOMAIN
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
/// The inputs one capture reads: every component's input and (ADR-020 C4)
/// every key input a `matches` measure names. Each is read under the same
/// row rule; a key input is never read past the viewer's own rows.
fn captured_inputs(components: &[Value]) -> BTreeSet<&str> {
    components
        .iter()
        .flat_map(|v| {
            [
                v["records"]["input_id"].as_str(),
                v["records"]["measure"]["key_input"].as_str(),
            ]
        })
        .flatten()
        .collect()
}
/// A payload the confined child could not read whole is refused as a
/// projection limit before any child is launched; nothing partial is shown.
fn within_payload_bound(payload: &Value) -> Result<()> {
    let encoded = admission::encode(payload).map_err(|_| Error::ProjectionLimit)?;
    if encoded.len() > RECORD_COMPONENT_PAYLOAD_BYTES {
        return Err(Error::ProjectionLimit);
    }
    Ok(())
}
/// ADR-020 C4: a creator's label as this viewer may see it. A member sees
/// the organization's label (a display name, else the neutral handle); a
/// guest is outside the organization and sees the neutral handle only.
fn creator_label(guest: bool, organization: &str, principal: &str) -> String {
    if guest {
        crate::directory::neutral_label(principal)
    } else {
        crate::directory::label(organization, principal)
    }
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
    let inputs = captured_inputs(&components);
    let mut creators = BTreeMap::new();
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
        let bounds=tx.query(&format!("SELECT count(*)::bigint,coalesce(sum(octet_length(v.canonical_values)),0)::bigint FROM genui.internal_records r {} WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND {own}", super::record_query::CURRENT, own = super::authorization::own_rows("v", "$2::text::uuid", "$3::text::uuid", super::authorization::SCOPED)), &[&org,&app,&work,&collection]).await?;
        let count = usize::try_from(number(&bounds[0], 0)?).map_err(|_| Error::Unavailable)?;
        total_rows = total_rows.checked_add(count).ok_or(Error::Unavailable)?;
        total_bytes = total_bytes
            .checked_add(usize::try_from(number(&bounds[0], 1)?).map_err(|_| Error::Unavailable)?)
            .ok_or(Error::Unavailable)?;
        if total_rows > RECORD_COMPONENT_ROWS || total_bytes > RECORD_COMPONENT_BYTES {
            return Err(Error::ProjectionLimit);
        }
        let rows=tx.query(&format!("SELECT r.record_id::text,r.current_version::bigint,v.commit_sequence::bigint,v.canonical_values,v.creator_id::text FROM genui.internal_records r {} WHERE r.organization_id=$1::text::uuid AND r.app_id=$2::text::uuid AND r.work_id=$3::text::uuid AND r.collection_id=$4::text::uuid AND {own} ORDER BY v.commit_sequence DESC,r.record_id LIMIT 2001", super::record_query::CURRENT, own = super::authorization::own_rows("v", "$2::text::uuid", "$3::text::uuid", super::authorization::SCOPED)), &[&org,&app,&work,&collection]).await?;
        require(rows.len() == count)?;
        let mut records = Vec::new();
        let mut revisions = Vec::new();
        for row in &rows {
            let bytes: Vec<u8> = row.try_get(3).map_err(|_| Error::Unavailable)?;
            let values = admission::decode(&bytes).map_err(|_| Error::Unavailable)?;
            let id = text(row, 0)?;
            let revision = number(row, 1)?;
            let sequence = number(row, 2)?;
            let creator = text(row, 4)?;
            if !creators.contains_key(&creator) {
                let label = creator_label(matches!(actor, Actor::Guest(_)), org, &creator);
                creators.insert(creator.clone(), label);
            }
            records.push(json!({"id":id,"version":revision,"sequence":sequence,"values":values,"creator":creator}));
            revisions.push(json!({"record_id":id,"version":revision,"commit_sequence":sequence}));
        }
        provenance.push(json!({"input_id":input,"collection_id":collection,"schema_id":schema,"schema_version":version,"records":revisions}));
        datasets.push(
            json!({"input_id":input,"schema_id":schema,"fields":declared["fields"],"rows":records}),
        );
    }
    let snapshot_digest = record_component_digest(SNAPSHOT_DOMAIN, &json!(datasets))?;
    // An id-sorted array: forma-json-v1 object keys are local ids, never
    // principal ids (ADR-020 clarification, APP-204).
    let creators: Vec<Value> = creators
        .into_iter()
        .map(|(id, label)| json!({"id":id,"label":label}))
        .collect();
    let payload =
        json!({"components":components,"filters":filters,"datasets":datasets,"creators":creators});
    within_payload_bound(&payload)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_matches_key_input_is_captured_beside_the_component_inputs() {
        let components = vec![
            json!({"records": {"input_id": "items"}}),
            json!({"records": {"input_id": "responses", "measure": {"op": "matches", "key_input": "expected", "fields": ["item"]}}}),
            json!({"records": {"input_id": "responses", "measure": {"op": "count"}}}),
        ];
        assert_eq!(
            captured_inputs(&components).into_iter().collect::<Vec<_>>(),
            ["expected", "items", "responses"]
        );
    }
    #[test]
    fn a_guest_never_sees_a_members_chosen_name() {
        let org = "00000000-0000-4000-8000-0000000000f1";
        let principal = "c0000f02-0000-4000-8000-0000000000f2";
        crate::directory::set_member_name(org, principal, Some("Dana Kim"));
        assert_eq!(creator_label(false, org, principal), "Dana Kim");
        assert_eq!(creator_label(true, org, principal), "Member c0000f02");
        crate::directory::set_member_name(org, principal, None);
        assert_eq!(creator_label(false, org, principal), "Member c0000f02");
    }
    #[test]
    fn a_payload_the_child_cannot_read_whole_is_a_projection_limit() {
        let label = "n".repeat(80);
        let creators = |n: usize| -> Vec<Value> {
            (0..n)
                .map(|i| json!({"id": format!("00000000-0000-4000-8000-{i:012}"), "label": label}))
                .collect()
        };
        let size = |n: usize| {
            admission::encode(&json!({"creators": creators(n)}))
                .unwrap()
                .len()
        };
        // Just under the bound, and just over it while still encodable: the
        // bound, not the encoder's own 1 MiB cap, refuses.
        assert!(size(7_200) <= RECORD_COMPONENT_PAYLOAD_BYTES);
        assert!((RECORD_COMPONENT_PAYLOAD_BYTES + 1..=1_048_576).contains(&size(7_400)));
        assert_eq!(
            within_payload_bound(&json!({"creators": creators(7_200)})),
            Ok(())
        );
        assert_eq!(
            within_payload_bound(&json!({"creators": creators(7_400)})),
            Err(Error::ProjectionLimit)
        );
        assert_eq!(
            within_payload_bound(&json!({"creators": creators(9_000)})),
            Err(Error::ProjectionLimit)
        );
    }
    #[test]
    fn the_snapshot_domain_is_v2_only() {
        assert!(record_component_digest(SNAPSHOT_DOMAIN, &json!([])).is_ok());
        assert_eq!(
            record_component_digest("forma:record-components-snapshot:v1", &json!([])),
            Err(Error::Unavailable)
        );
    }
}
