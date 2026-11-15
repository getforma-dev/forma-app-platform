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
//!
//! ADR-020 §17.34 (APP-211): a `matches` component whose key input the viewer
//! cannot read is **withheld** here, once, before any child runs: the key's
//! collection is declared `own` and the viewer is not a reviewer of the work,
//! which is exactly the test `own_rows` applies to every row the viewer did
//! not create ([`super::authorization::hidden`]). A withheld component is not
//! in the engine payload (so it is never computed, and the receipt never
//! covers it); the run lists its declaration under [`RecordComponentsRun::withheld`]
//! so that every page shows the same note and no number.
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
    /// ADR-020 §17.34 (APP-211): the declarations of the components withheld
    /// from this viewer (their key input is unreadable), in definition order.
    /// Never in `payload`, so never computed and never in the receipt.
    withheld: Vec<Value>,
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
    /// Nothing to compute: no component reaches the engine. Withheld
    /// components do not count; they are shown from [`Self::withheld`].
    pub fn is_empty(&self) -> bool {
        self.payload["components"]
            .as_array()
            .is_none_or(Vec::is_empty)
    }
    /// ADR-020 §17.34 (APP-211): the components withheld from this viewer,
    /// as declared (id, title, kind, records), in definition order.
    pub fn withheld(&self) -> &[Value] {
        &self.withheld
    }
    /// Unit tests of the projection protocol: a run over a synthetic payload,
    /// never a capture. Test builds only.
    #[cfg(test)]
    pub fn for_tests(payload: Value) -> Self {
        Self::for_tests_withholding(payload, Vec::new())
    }
    /// [`Self::for_tests`] with components withheld from the viewer.
    #[cfg(test)]
    pub fn for_tests_withholding(payload: Value, withheld: Vec<Value>) -> Self {
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
            withheld,
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
/// The key input each `matches` component names (ADR-020 C4).
fn key_input(component: &Value) -> Option<&str> {
    component["records"]["measure"]["key_input"].as_str()
}
/// ADR-020 §17.34 (APP-211): splits the components into those the engine
/// computes and those withheld from the viewer, in definition order: a
/// component is withheld exactly when its key input is in `unreadable`.
fn partition_withheld(
    components: Vec<Value>,
    unreadable: &BTreeSet<String>,
) -> (Vec<Value>, Vec<Value>) {
    components
        .into_iter()
        .partition(|v| !key_input(v).is_some_and(|key| unreadable.contains(key)))
}
/// ADR-020 §17.34 (APP-211): whether `$3` (a collection of work `$2` of app
/// `$1`) is hidden from the scoped principal: declared `own` and the principal
/// is not a reviewer of the work. This is `own_rows`' own test for a row the
/// principal did not create ([`super::authorization::hidden`]), asked once
/// per key input rather than once per row; there is no second definition of
/// a reviewer here.
fn key_hidden_statement() -> String {
    format!(
        "SELECT $3::text::uuid = ANY({})",
        super::authorization::hidden(
            "$1::text::uuid",
            "$2::text::uuid",
            super::authorization::SCOPED
        )
    )
}
async fn key_hidden(tx: &Transaction<'_>, app: &str, work: &str, collection: &str) -> Result<bool> {
    // Test builds with the row rule bypassed (ADR-020 §14.1): nothing is
    // hidden, so nothing is withheld, and the campaign's check fails as every
    // D7 check must. (`hidden` is then a literal that names no parameter.)
    if super::authorization::bypassed() {
        return Ok(false);
    }
    let rows = tx
        .query(&key_hidden_statement(), &[&app, &work, &collection])
        .await?;
    rows.first()
        .ok_or(Error::Unavailable)?
        .try_get(0)
        .map_err(|_| Error::Unavailable)
}
/// The work's `records` binding of `input`: `(collection, schema, version)`.
/// An input the work does not bind is `Denied`, as any unreadable record is.
async fn binding(
    tx: &Transaction<'_>,
    org: &str,
    app: &str,
    work: &str,
    input: &str,
) -> Result<(String, String, i64)> {
    let binding=tx.query("SELECT collection_id::text,schema_id::text,schema_version::bigint FROM genui.work_bindings WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid AND input_id=$4::text AND kind='records' AND revision_policy='committed_snapshot'", &[&org,&app,&work,&input]).await?;
    let b = binding.first().ok_or(Error::Denied)?;
    Ok((text(b, 0)?, text(b, 1)?, number(b, 2)?))
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
/// ADR-020 C4: a creator's label as this viewer may see it
/// (`directory::creator_label`, shared with the records card since APP-215).
fn creator_label(guest: bool, organization: &str, principal: &str) -> String {
    crate::directory::creator_label(guest, organization, principal)
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
    // Every input the components name is bound once, before anything is read.
    let mut bindings = BTreeMap::new();
    for input in captured_inputs(&components) {
        bindings.insert(input.to_owned(), binding(tx, org, app, work, input).await?);
    }
    // ADR-020 §17.34 (APP-211): a key the viewer cannot read withholds every
    // component that scores against it. Decided here, once, so the canvas,
    // the pages, the live refresh and every other reader agree.
    let mut unreadable = BTreeSet::new();
    for key in components.iter().filter_map(key_input) {
        let (collection, _, _) = bindings.get(key).ok_or(Error::Unavailable)?;
        if key_hidden(tx, app, work, collection).await? {
            unreadable.insert(key.to_owned());
        }
    }
    let (components, withheld) = partition_withheld(components, &unreadable);
    let inputs = captured_inputs(&components);
    let mut creators = BTreeMap::new();
    let mut datasets = Vec::new();
    let mut total_rows = 0usize;
    let mut total_bytes = 0usize;
    let mut provenance = Vec::new();
    for input in inputs {
        let (collection, schema, version) = bindings.get(input).ok_or(Error::Unavailable)?.clone();
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
        withheld,
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
    /// ADR-020 §17.34 (APP-211): the withheld decision is `own_rows`' own
    /// test for a row the viewer did not create (`hidden`), asked once per
    /// key input: the `own` declaration and the one reviewer clause (the app's
    /// owner, the work's unrevoked creator, `record_review`, the
    /// organization-wide reviewer share), no second definition.
    #[test]
    fn the_key_test_is_the_row_rules_hidden_clause() {
        let statement = key_hidden_statement();
        assert!(statement.starts_with("SELECT $3::text::uuid = ANY(ARRAY(SELECT hc.collection_id"));
        for part in [
            "hd.owner_only",
            "ra.owner_principal_id=nullif(current_setting('genui.principal_id',true),'')::uuid",
            "rw.creator_revoked_at IS NULL",
            "rp.permission='record_review'",
            "rs.role='reviewer'",
        ] {
            assert!(statement.contains(part), "{part} is not in {statement}");
        }
        assert_eq!(
            statement,
            format!(
                "SELECT $3::text::uuid = ANY({})",
                super::super::authorization::hidden(
                    "$1::text::uuid",
                    "$2::text::uuid",
                    super::super::authorization::SCOPED
                )
            )
        );
    }
    #[test]
    fn a_component_is_withheld_exactly_when_its_key_input_is_unreadable() {
        let timer = json!({"id": "timer", "kind": "countdown", "records": {"input_id": "items"}});
        let board = json!({"id": "board", "kind": "record_ranking", "records": {"input_id": "responses", "group_by": "creator", "measure": {"op": "matches", "key_input": "expected", "fields": ["item"]}}});
        let count = json!({"id": "count", "kind": "record_summary", "records": {"input_id": "responses", "measure": {"op": "count"}}});
        let shared = json!({"id": "shared", "kind": "record_summary", "records": {"input_id": "responses", "measure": {"op": "matches", "key_input": "answers", "fields": ["item"]}}});
        let all = vec![timer.clone(), board.clone(), count.clone(), shared.clone()];
        // A readable key (a reviewer, the owner, a shared key list): nothing withheld.
        let (computed, withheld) = partition_withheld(all.clone(), &BTreeSet::new());
        assert_eq!((computed.len(), withheld.len()), (4, 0));
        // The `expected` key is unreadable: only the components scoring against it go.
        let unreadable = BTreeSet::from(["expected".to_owned()]);
        let (computed, withheld) = partition_withheld(all, &unreadable);
        assert_eq!(computed, vec![timer, count, shared]);
        assert_eq!(withheld, vec![board]);
        // The key input of a withheld component is no longer captured.
        assert_eq!(
            captured_inputs(&computed).into_iter().collect::<Vec<_>>(),
            ["answers", "items", "responses"]
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
