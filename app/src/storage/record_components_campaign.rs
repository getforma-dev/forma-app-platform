//! Real PostgreSQL + confined child qualification for APP-158/159. Definitions
//! are admitted through the shipped template path; records are saved by the
//! production command transaction. Provider proof is synthetic test input only.
//!
//! APP-204 (ADR-020 C1, C4) adds a general participation app admitted through
//! the definition path: a shared `items` list, an `own` `responses` list and an
//! `own` `expected` key list, with a countdown, a ranking by creator and a
//! `matches` summary. Its D7 checks run twice: as shipped, where they must
//! hold, and with `authorization::BYPASS` set, where they must fail (the
//! contributor's ranking then shows everyone).
//!
//! APP-208 (ADR-020 §9) adds the contract specimen's multi-select app
//! (`contracts/fixtures/participation.json`, `multi_select`): an `own`
//! `choices` list whose `tags` is a `multi_enum`, picked through the JSON
//! route and through the native form's checkbox group; a ranking that counts
//! a row once per selected value, a list whose cell joins the selection, and
//! a typed query that sees the field as text; storage's refusals of any other
//! order, a repeat, an unknown id, an empty set and nothing on a required
//! field; and D7 on the ranking and the typed query (fails under bypass).
use super::{
    admission_qualification::operator,
    authorization::BYPASS,
    definition_qualification::{grant, request, POLICY},
    qualification::{fixture_python, Fixture},
    qualification_cases::{check, ORG, OTHER, PRINCIPAL},
    *,
};
use crate::{
    admission::{self, digest},
    composer::{
        routes::{compose_instant, First, Pin},
        Composer,
    },
    query::{
        record_components as projections,
        tests::{real_engine, Directory},
        Engine,
    },
};
use genui_auth_verifier::VerifiedIdentity;
use serde_json::{json, Value};
use std::{sync::atomic::Ordering, time::Duration};

#[test]
#[ignore = "requires local Docker, OpenSSL and locked migration/contract Python environments"]
fn native_record_components_campaign() {
    std::thread::Builder::new()
        .name("record-components-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign())
        })
        .unwrap()
        .join()
        .unwrap();
}
// Test fixture: keep each exact command scope and typed input explicit at call sites.
#[allow(clippy::too_many_arguments)]
async fn saved(
    stores: &Stores,
    owner: &TrustedRequest,
    app: &str,
    work: &str,
    publication: &str,
    n: u64,
    amount: &str,
    day: &str,
    restaurant: &str,
) -> Value {
    let command = CommandRequest {
        app: app.into(),
        work: work.into(),
        publication: publication.into(),
        command: "add".into(),
        schema: "meal".into(),
        schema_version_expected: 1,
        record: format!("00000000-0000-4000-8000-{n:012x}"),
        expected_record_version: 0,
        expected_work_version: 1,
        values: json!([
            {"field_id":"restaurant","value":{"type":"text","value":restaurant}},
            {"field_id":"amount","value":{"type":"money","minor_units":amount,"currency":"USD","minor_unit_exponent":2}},
            {"field_id":"date","value":{"type":"date","value":day}},
            {"field_id":"notes","value":{"type":"null"}}
        ]),
        idempotency_key: format!("{n:032x}"),
    };
    let CommandResult::Committed(outcome) =
        stores.data.commit_record(owner, &command).await.unwrap()
    else {
        panic!("record command must commit");
    };
    outcome.document()
}
fn value<'a>(result: &'a projections::RecordComponentsResult, id: &str) -> &'a Value {
    result.document["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id)
        .unwrap()
}
async fn campaign() {
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    operator(&fixture, &["backfill", "--organization", ORG]);
    let stores = Stores::new(fixture.settings()).unwrap();
    stores.readiness().await.unwrap();
    let owner = request(&stores, ORG, PRINCIPAL, 1);
    let foreign = request(&stores, OTHER, PRINCIPAL, 1);
    let template = crate::composer::templates::find("dining-tracker").unwrap();
    let pin = Pin::Template(template.id.into());
    let no_model = Composer::new(None, "unused", "unused");
    let First::Placed(placed, _) =
        compose_instant(&stores, &no_model, &owner, template.description, &pin)
            .await
            .unwrap()
    else {
        panic!("template must place");
    };
    let (app, work) = (&placed.app, &placed.work);
    let empty = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    let publication = empty.publication().to_owned();
    let root = Directory::new("records-projections");
    let engine = real_engine(&fixture_python(), &root.0);
    let parameters = json!({"filter_epoch":7,"filters":[{"id":"date_filter","start":"2026-11-01","end":"2026-12-01"}]});
    let zero = engine
        .execute_records(&empty, &parameters, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(value(&zero, "total")["value"], "0");
    assert_eq!(value(&zero, "total")["state"], "empty");
    stores
        .data
        .release_record_components(&owner, &empty)
        .await
        .unwrap();
    let mut checks = Vec::new();
    check(&mut checks,"real admitted empty template gives released zero with empty state, no seeded records or model values");
    let mut command_outcomes = Vec::new();
    for (n, amount, day, restaurant) in [
        (1, "9999", "2026-10-31", "Outside"),
        (2, "1250", "2026-11-02", "Cafe A"),
        (3, "750", "2026-11-03", "Cafe A"),
        (4, "2500", "2026-11-04", "Cafe B"),
        (5, "9999", "2026-12-01", "Outside"),
    ] {
        command_outcomes.push(
            saved(
                &stores,
                &owner,
                app,
                work,
                &publication,
                n,
                amount,
                day,
                restaurant,
            )
            .await,
        );
    }
    let run = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    let result = engine
        .execute_records(&run, &parameters, Duration::from_millis(1800))
        .await
        .unwrap();
    stores
        .data
        .release_record_components(&owner, &run)
        .await
        .unwrap();
    assert_eq!(value(&result, "total")["value"], "4500");
    assert_eq!(value(&result, "total")["population"], 3);
    assert_eq!(
        value(&result, "monthly")["groups"],
        json!([{"label":"2026-11","missing":false,"value":"4500","count":3}])
    );
    assert_eq!(
        value(&result, "restaurants")["groups"],
        json!([{"label":"Cafe B","missing":false,"value":"2500","count":1},{"label":"Cafe A","missing":false,"value":"2000","count":2}])
    );
    assert_eq!(
        value(&result, "budget")["budget"]["ratio_basis_points"],
        "4500"
    );
    assert_eq!(
        value(&result, "budget")["budget"]["remaining_minor_units"],
        "5500"
    );
    let receipt = projections::receipt(&run, &result, &parameters).unwrap();
    crate::admission::admit_record_components_receipt(&receipt).unwrap();
    assert!(super::query_qualification::contract_valid(&receipt));
    let budget_receipt = receipt["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["view_id"] == "budget")
        .unwrap();
    assert_eq!(budget_receipt["population"], 3);
    assert_eq!(
        budget_receipt["budget"],
        json!({"origin":"preset","minor_units":"10000","currency":"USD","minor_unit_exponent":2})
    );
    assert_eq!(receipt["actor"]["kind"], "member");
    assert_eq!(
        receipt["snapshot"][0]["records"].as_array().unwrap().len(),
        5
    );
    let mut forged = receipt.clone();
    forged["actor"]["membership_epoch"] = json!(2);
    assert!(crate::admission::admit_record_components_receipt(&forged).is_err());
    let props = projections::props(&run, &result, &parameters).unwrap();
    assert_eq!(
        props.iter().find(|p| p.id == "total").unwrap().summary,
        "$45.00"
    );
    check(&mut checks,"five real command commits produce November4500/count3, CafeB2500/CafeA2000, month4500 and preset10000 budget45percent/5500remaining; half-open dates exclude October31/December1; typed receipt pins all5 revisions");
    assert_eq!(
        stores.data.record_components(&foreign, app, work).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores.data.release_record_components(&foreign, &run).await,
        Err(Error::Denied)
    );
    let current = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    saved(
        &stores,
        &owner,
        app,
        work,
        &publication,
        6,
        "1000",
        "2026-11-05",
        "Cafe C",
    )
    .await;
    let before = engine
        .execute_records(&current, &parameters, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(value(&before, "total")["value"], "4500");
    stores
        .data
        .release_record_components(&owner, &current)
        .await
        .unwrap();
    let later = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    let after = engine
        .execute_records(&later, &parameters, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(value(&after, "total")["value"], "5500");
    assert_ne!(later.snapshot_digest(), current.snapshot_digest());
    check(&mut checks,"foreign tenant refuses snapshot/release; command after capture cannot mix rows in old result; subsequent snapshot includes commit and changes snapshot identity");
    // A real verified-account session and named grant, without tenant admission.
    let proof = ShareIdentityProof::verified(
        &VerifiedIdentity {
            key_id: "fixture-key".into(),
            issuer: "app008-fixture-only".into(),
            tenant_id: format!("ten_{OTHER}"),
            subject: "records-guest".into(),
            issued_at: 1,
            expires_at: 2,
        },
        "records-guest@example.test",
        true,
    )
    .unwrap();
    let cookie = stores
        .workspace
        .mint_shared_session(&proof, "00000000-0000-4000-8000-00000000e101")
        .await
        .unwrap();
    let account = stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .unwrap();
    let share = stores
        .workspace
        .create_live_share(
            &owner,
            app,
            work,
            &NewLiveShare {
                recipient_email: "records-guest@example.test".into(),
                allow_add_to_canvas: false,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-00000000e102".into(),
            },
        )
        .await
        .unwrap();
    stores
        .workspace
        .accept_live_share(&account, &share.key)
        .await
        .unwrap();
    let access = stores
        .workspace
        .resolve_guest_access(&account, &share.key)
        .await
        .unwrap();
    // ADR-020 C4: a guest is outside the organization and never receives a
    // member's chosen name, only the neutral handle; a member receives the name.
    stores
        .workspace
        .set_display_name(&owner, "Dana Owner")
        .await
        .unwrap();
    let guest = stores.data.guest_record_components(&access).await.unwrap();
    let named = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    stores
        .data
        .release_record_components(&owner, &named)
        .await
        .unwrap();
    assert_eq!(
        named.payload()["creators"],
        json!([{"id": PRINCIPAL, "label": "Dana Owner"}])
    );
    assert_eq!(
        guest.payload()["creators"],
        json!([{"id": PRINCIPAL, "label": crate::directory::neutral_label(PRINCIPAL)}])
    );
    assert!(guest.payload()["datasets"][0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["creator"] == PRINCIPAL));
    let guest_result = engine
        .execute_records(&guest, &parameters, Duration::from_millis(1800))
        .await
        .unwrap();
    stores
        .data
        .release_guest_record_components(&access, &guest)
        .await
        .unwrap();
    assert_eq!(value(&guest_result, "total")["value"], "5500");
    assert_eq!(
        projections::receipt(&guest, &guest_result, &parameters).unwrap()["actor"]["kind"],
        "guest"
    );
    assert_eq!(
        stores.data.release_record_components(&owner, &guest).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .release_guest_record_components(&access, &run)
            .await,
        Err(Error::Denied)
    );
    check(&mut checks,"named external account reads exact shared record components through same confined engine; receipt identifies actual account/share; member/guest release types cannot substitute");
    check(&mut checks,"APP-204: every captured row names its creator; a member's creators map carries the display name, a guest's only the neutral handle");
    let op = fixture.operator().await;
    op.client.execute("INSERT INTO genui.publication_policies(organization_id,policy_id,policy_version,kind) SELECT organization_id,policy_id,policy_version+1,policy_kind FROM genui.publications WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid ON CONFLICT DO NOTHING",&[&ORG,&app,&publication]).await.unwrap();
    op.client.execute("UPDATE genui.publications SET policy_version=policy_version+1 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid",&[&ORG,&app,&publication]).await.unwrap();
    assert_eq!(
        stores.data.release_record_components(&owner, &run).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .release_guest_record_components(&access, &guest)
            .await,
        Err(Error::Denied)
    );
    op.client.execute("UPDATE genui.publications SET policy_version=policy_version-1 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid",&[&ORG,&app,&publication]).await.unwrap();
    let refused=op.client.execute("UPDATE genui.publications SET operator_version=2 WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND publication_id=$3::text::uuid",&[&ORG,&app,&publication]).await.unwrap_err();
    assert_eq!(refused.code().map(|code| code.code()), Some("23514"));
    stores
        .data
        .release_record_components(&owner, &run)
        .await
        .unwrap();
    stores
        .workspace
        .revoke_live_share(&owner, app, work, &share.share_id, share.version)
        .await
        .unwrap();
    assert_eq!(
        stores
            .data
            .release_guest_record_components(&access, &guest)
            .await,
        Err(Error::Denied)
    );
    check(&mut checks,"policy changes after execution deny both member and guest release; share revocation withholds already-computed guest values");
    // Real saved commands extend beyond the legacy newest-200 listing page.
    for n in 10..=211 {
        saved(
            &stores,
            &owner,
            app,
            work,
            &publication,
            n,
            "1",
            "2026-11-15",
            "Newest",
        )
        .await;
    }
    let bounded = stores
        .data
        .record_components(&owner, app, work)
        .await
        .unwrap();
    let needle = json!({"filter_epoch":8,"filters":[{"id":"restaurant_filter","value":"Cafe B"}]});
    let filtered = engine
        .execute_records(&bounded, &needle, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(value(&filtered, "ledger")["population"], 1);
    assert_eq!(value(&filtered, "total")["value"], "2500");
    check(&mut checks,"filter-before-page finds the older CafeB saved record behind202 newer real command commits; total follows same explicit filter targets");
    let ids = bounded.snapshot()[0]["collection_id"].as_str().unwrap();
    // Boundary-only operator fixture: replicate valid immutable revision bytes.
    // This does not claim these rows came through user commands.
    op.client.batch_execute(&format!("BEGIN; INSERT INTO genui.internal_records(organization_id,app_id,work_id,collection_id,record_id,current_version) SELECT '{ORG}','{app}','{work}','{ids}',('00000000-0000-4000-9000-'||lpad(to_hex(n),12,'0'))::uuid,1 FROM generate_series(1,2001)n; INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,commit_sequence,canonical_values,creator_id,updater_id,committed_at) SELECT '{ORG}','{app}','{work}','{ids}',('00000000-0000-4000-9000-'||lpad(to_hex(n),12,'0'))::uuid,1,10000+n,v.canonical_values,'{PRINCIPAL}','{PRINCIPAL}',clock_timestamp() FROM generate_series(1,2001)n CROSS JOIN LATERAL(SELECT canonical_values FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND app_id='{app}' AND work_id='{work}' LIMIT1)v; COMMIT;").replace("LIMIT1","LIMIT 1")).await.unwrap();
    assert_eq!(
        stores.data.record_components(&owner, app, work).await,
        Err(Error::ProjectionLimit)
    );
    check(&mut checks,"over2000-row operator boundary fixture is refused before row materialization; no partial result or fabricated total");
    let launched = engine.stats().launched;
    assert!(matches!(
        engine
            .execute_records(&run, &parameters, Duration::ZERO)
            .await,
        Err(crate::query::QueryError::Failed(
            crate::query::Failure::QueueExpired
        ))
    ));
    assert_eq!(engine.stats().launched, launched);
    op.client.execute("UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1 WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid",&[&ORG,&PRINCIPAL]).await.unwrap();
    assert_eq!(
        stores.data.release_record_components(&owner, &run).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores.data.record_components(&owner, app, work).await,
        Err(Error::Denied)
    );
    check(&mut checks,"expired execution deadline launches no child and yields no values; member revocation denies previously computed release and new snapshot before bounds/data reads");
    let participation = participation(&fixture, &stores, &engine, &mut checks).await;
    let multi_select = multi_select(&fixture, &stores, &engine, &mut checks).await;
    let report = json!({"campaign":"APP-158-159-record-components","passed":true,"checks":checks,"participation":participation,"multi_select":multi_select,"record_operator_version":1,"bounds":{"rows":RECORD_COMPONENT_ROWS,"bytes":RECORD_COMPONENT_BYTES},"provider_identity":"synthetic-test-proof-only","definition":template.content(),"initial_command_outcomes":command_outcomes,"snapshot_datasets":run.payload()["datasets"],"parameters":parameters,"initial_result":result.document,"member_receipt":receipt,"guest_receipt":projections::receipt(&guest,&guest_result,&parameters).unwrap()});
    if let Some(path) = std::env::var_os("GENUI_RECORD_COMPONENTS_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

// --- APP-204: ADR-020 C1 (countdown) and C4 (ranking by creator, matches) ----

/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const QUIZ_APP: &str = "00000000-0000-4000-8000-000000000c10";
/// Distinct first eight hex digits, so each neutral handle is distinct.
const P_OWNER: &str = "c0000c01-0000-4000-8000-000000000c01";
const P_REV: &str = "c0000c02-0000-4000-8000-000000000c02";
const P_BEN: &str = "c0000c03-0000-4000-8000-000000000c03";
const P_ANA: &str = "c0000c04-0000-4000-8000-000000000c04";
const P_CAL: &str = "c0000c05-0000-4000-8000-000000000c05";
const ITEM_1: &str = "00000000-0000-4000-8000-000000000c21";
const ITEM_2: &str = "00000000-0000-4000-8000-000000000c22";
const OPENED_1: &str = "2026-11-29T15:00:00Z";
const OPENED_2: &str = "2026-11-29T15:05:00Z";

/// ADR-020 §10's shape without classroom words, using only what this base's
/// storage commits: `opened_at` is written by the reviewer's `post` (the
/// `set_now` stamp and the `requires` guards are APP-203's; a countdown reads
/// the stored instant whichever way it was written).
fn participation_definition() -> Value {
    let item_fields = json!([
        {"id": "prompt", "value_type": {"type": "text"}, "nullable": false},
        {"id": "state", "value_type": {"type": "enum", "values": ["open", "closed"]}, "nullable": false},
        {"id": "opened_at", "value_type": {"type": "timestamp"}, "nullable": false}
    ]);
    let answer_fields = json!([
        {"id": "item", "value_type": {"type": "reference", "target_input": "items"}, "nullable": false},
        {"id": "choice", "value_type": {"type": "enum", "values": ["a", "b", "c", "d"]}, "nullable": false}
    ]);
    let records = |id: &str, extra: Value| {
        let mut r = json!({"version": 1, "input_id": id, "schema_id": id});
        for (k, v) in extra.as_object().unwrap() {
            r[k] = v.clone();
        }
        r
    };
    let matches = json!({"op": "matches", "key_input": "expected", "fields": ["item", "choice"]});
    let views = json!([
        {"id": "respond", "kind": "form", "renderer_version": 1, "title": "Respond", "command_id": "respond"},
        {"id": "timer", "kind": "countdown", "renderer_version": 1, "title": "Time left", "records": records("items", json!({"timestamp_field": "opened_at", "offset_seconds": 30, "closed_when": {"field_id": "state", "comparison": "eq", "value": {"type": "enum", "value": "closed"}}}))},
        {"id": "item_list", "kind": "records_list", "renderer_version": 1, "title": "Items", "records": records("items", json!({"columns": ["prompt", "state", "opened_at"]}))},
        {"id": "right", "kind": "record_summary", "renderer_version": 1, "title": "Matching responses", "records": records("responses", json!({"measure": matches}))},
        {"id": "leaderboard", "kind": "record_ranking", "renderer_version": 1, "title": "Leaderboard", "records": records("responses", json!({"group_by": "creator", "measure": matches, "limit": 50}))},
        {"id": "top", "kind": "record_ranking", "renderer_version": 1, "title": "Top", "records": records("responses", json!({"group_by": "creator", "measure": matches, "limit": 1}))}
    ]);
    let layout: Vec<Value> = views
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(row, v)| json!({"view_id": v["id"], "column": 0, "row": row, "width": 12}))
        .collect();
    let content = json!({
        "title": "Participation",
        "description": "Synthetic campaign app: shared items, each member's own responses, the reviewers' own expected key.",
        "inputs": [
            {"id": "items", "contract_id": "items_v1", "contract_version": 1, "kind": "records", "fields": item_fields},
            {"id": "responses", "contract_id": "responses_v1", "contract_version": 1, "kind": "records", "fields": answer_fields},
            {"id": "expected", "contract_id": "expected_v1", "contract_version": 1, "kind": "records", "fields": answer_fields}
        ],
        "views": views,
        "record_schemas": [
            {"id": "items", "version": 1, "fields": item_fields},
            {"id": "responses", "version": 1, "visibility": "own", "fields": answer_fields},
            {"id": "expected", "version": 1, "visibility": "own", "fields": answer_fields}
        ],
        "commands": [
            {"id": "post", "record_schema_id": "items", "operation": "create", "writable_fields": ["prompt", "state", "opened_at"], "transitions": [], "initial": [{"field_id": "state", "value": "open"}], "reviewer_only": true},
            {"id": "close", "record_schema_id": "items", "operation": "update", "writable_fields": ["state"], "transitions": [], "reviewer_only": true},
            {"id": "respond", "record_schema_id": "responses", "operation": "create", "writable_fields": ["item", "choice"], "transitions": []},
            {"id": "add_expected", "record_schema_id": "expected", "operation": "create", "writable_fields": ["item", "choice"], "transitions": [], "reviewer_only": true}
        ],
        "links": [],
        "default_layout": layout
    });
    let canonical = admission::encode(&content).unwrap();
    json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": ORG,
        "app_id": QUIZ_APP,
        "revision_id": "00000000-0000-4000-8000-000000000c11",
        "content": content,
        "digest": digest::digest(digest::APP_DEFINITION, &canonical),
    })
}

/// Operator-channel stand-in (as tenant_seed writes fixture authority): one
/// active member of the organization with a live entitlement.
fn admit_member(fixture: &Fixture, principal: &str) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{ORG}','{principal}','active',1); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
}

struct Quiz {
    work: String,
    publication: String,
}
impl Quiz {
    fn command(
        &self,
        command: &str,
        schema: &str,
        record: &str,
        expected: i64,
        values: Value,
        key: usize,
    ) -> CommandRequest {
        CommandRequest {
            app: QUIZ_APP.into(),
            work: self.work.clone(),
            publication: self.publication.clone(),
            command: command.into(),
            schema: schema.into(),
            schema_version_expected: 1,
            record: record.into(),
            expected_record_version: expected,
            expected_work_version: 1,
            values,
            idempotency_key: format!("{:032x}", 0xc000 + key),
        }
    }
}
async fn commits(
    stores: &Stores,
    who: &TrustedRequest,
    request: &CommandRequest,
) -> CommandOutcome {
    match stores.data.commit_record(who, request).await {
        Ok(CommandResult::Committed(outcome)) => outcome,
        other => panic!(
            "{} by {} must commit: {other:?}",
            request.command,
            who.principal()
        ),
    }
}
fn reference(record: &str) -> Value {
    json!({"type": "reference", "input_id": "items", "record_id": record})
}
fn answer(record: &str, choice: &str) -> Value {
    json!([
        {"field_id": "item", "value": reference(record)},
        {"field_id": "choice", "value": {"type": "enum", "value": choice}}
    ])
}
/// One authorized capture, the confined child and the fresh release check.
async fn computed(
    stores: &Stores,
    engine: &Engine,
    who: &TrustedRequest,
    work: &str,
) -> (RecordComponentsRun, projections::RecordComponentsResult) {
    computed_in(stores, engine, who, QUIZ_APP, work).await
}
/// [`computed`] for any campaign app (APP-208's multi-select app too).
async fn computed_in(
    stores: &Stores,
    engine: &Engine,
    who: &TrustedRequest,
    app: &str,
    work: &str,
) -> (RecordComponentsRun, projections::RecordComponentsResult) {
    let run = stores.data.record_components(who, app, work).await.unwrap();
    let result = engine
        .execute_records(
            &run,
            &projections::default_parameters(),
            Duration::from_millis(1800),
        )
        .await
        .unwrap();
    stores
        .data
        .release_record_components(who, &run)
        .await
        .unwrap();
    (run, result)
}
fn groups(result: &projections::RecordComponentsResult, id: &str) -> Vec<(String, String, u64)> {
    value(result, id)["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["label"].as_str().unwrap().to_owned(),
                g["value"].as_str().unwrap().to_owned(),
                g["count"].as_u64().unwrap(),
            )
        })
        .collect()
}
fn group(label: &str, score: &str, count: u64) -> (String, String, u64) {
    (label.to_owned(), score.to_owned(), count)
}
/// APP-211: whether the engine's result has no outcome for component `id`.
fn absent(result: &projections::RecordComponentsResult, id: &str) -> bool {
    result.document["components"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["id"] != id)
}
/// APP-211: the ids of the components the run withheld, in definition order.
fn withheld_ids(run: &RecordComponentsRun) -> Vec<&str> {
    run.withheld()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect()
}
/// How many revisions of `input` the run's provenance (the receipt's
/// `snapshot`) lists; `usize::MAX` when the input was not captured.
fn listed(run: &RecordComponentsRun, input: &str) -> usize {
    run.snapshot()
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["input_id"] == input)
        .map(|s| s["records"].as_array().unwrap().len())
        .unwrap_or(usize::MAX)
}

/// A D7 check: holds as shipped, fails with the row rule bypassed.
macro_rules! judged {
    ($checks:expr, $name:expr, $body:block) => {{
        BYPASS.store(false, Ordering::SeqCst);
        let held: bool = (async $body).await;
        BYPASS.store(true, Ordering::SeqCst);
        let bypassed: bool = (async $body).await;
        BYPASS.store(false, Ordering::SeqCst);
        assert!(held, "{} does not hold as shipped", $name);
        assert!(!bypassed, "{} still holds with own_rows bypassed: the check does not depend on the rule", $name);
        check($checks, &format!("{} (fails under bypass)", $name));
    }};
}

async fn participation(
    fixture: &Fixture,
    stores: &Stores,
    engine: &Engine,
    checks: &mut Vec<Value>,
) -> Value {
    for member in [P_OWNER, P_REV, P_BEN, P_ANA, P_CAL] {
        admit_member(fixture, member);
    }
    let owner = request(stores, ORG, P_OWNER, 1);
    let rev = request(stores, ORG, P_REV, 1);
    let ben = request(stores, ORG, P_BEN, 1);
    let ana = request(stores, ORG, P_ANA, 1);
    let cal = request(stores, ORG, P_CAL, 1);
    let created = operator(
        fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            QUIZ_APP,
            "--space",
            SPACE,
            "--owner",
            P_OWNER,
        ],
    );
    assert_eq!(created["granted"], json!(["edit_definition"]));
    for permission in ["discover", "run"] {
        grant(fixture, P_OWNER, permission, &["--app", QUIZ_APP]);
    }
    let admitted =
        admission::admit_definition(&serde_json::to_vec(&participation_definition()).unwrap())
            .expect("the participation definition admits");
    let revision = stores
        .workspace
        .create_definition_revision(&owner, QUIZ_APP, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(&owner, QUIZ_APP, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let saved = stores
        .workspace
        .create_work(
            &owner,
            QUIZ_APP,
            &publication.publication_id,
            "Participation (synthetic campaign)",
            &["items", "responses", "expected"].map(|id| WorkBinding::Records {
                input: id.into(),
                schema: id.into(),
            }),
        )
        .await
        .unwrap();
    let quiz = Quiz {
        work: saved.work.clone(),
        publication: publication.publication_id.clone(),
    };
    let work = quiz.work.as_str();
    stores
        .workspace
        .share_work(
            &owner,
            QUIZ_APP,
            work,
            Audience::Member(P_REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    for member in [P_BEN, P_ANA, P_CAL] {
        stores
            .workspace
            .share_work(
                &owner,
                QUIZ_APP,
                work,
                Audience::Member(member),
                ShareRole::Contributor,
                ShareMode::Set,
            )
            .await
            .unwrap();
    }
    stores
        .workspace
        .set_display_name(&ben, "Ben Okafor")
        .await
        .unwrap();
    stores
        .workspace
        .set_display_name(&ana, "Ana Lima")
        .await
        .unwrap();

    // --- C1: nothing readable yet ---------------------------------------------
    let at = |text: &str| projections::instant(text).unwrap();
    let (run, empty) = computed(stores, engine, &ben, work).await;
    let timer = value(&empty, "timer");
    assert_eq!(
        (
            timer["state"].as_str(),
            timer["value"].is_null(),
            timer["label"].as_str(),
            timer["closed"].as_bool()
        ),
        (Some("empty"), true, Some(""), Some(true))
    );
    let mut props = projections::props(&run, &empty, &projections::default_parameters()).unwrap();
    projections::stamp(&mut props, at(OPENED_2));
    let shown = props.iter().find(|p| p.id == "timer").unwrap();
    assert!(!shown.has_countdown && shown.empty);
    assert_eq!(shown.message, "Nothing is open yet");
    // APP-211: the contributor's `matches` components are withheld, not empty.
    assert!(absent(&empty, "leaderboard"));
    assert_eq!(withheld_ids(&run), ["right", "leaderboard", "top"]);
    check(checks, "APP-204 C1: before any item a contributor's countdown is empty: no target, \"Nothing is open yet\", no timer");

    // --- the owner posts two items and files the expected key -----------------
    let post = |record: &str, prompt: &str, opened: &str, key: usize| {
        quiz.command(
            "post",
            "items",
            record,
            0,
            json!([
                {"field_id": "prompt", "value": {"type": "text", "value": prompt}},
                {"field_id": "state", "value": {"type": "enum", "value": "open"}},
                {"field_id": "opened_at", "value": {"type": "timestamp", "value": opened}}
            ]),
            key,
        )
    };
    commits(stores, &owner, &post(ITEM_1, "Which colour?", OPENED_1, 1)).await;
    commits(stores, &owner, &post(ITEM_2, "Which shape?", OPENED_2, 2)).await;
    // D8 as shipped: a contributor cannot post.
    assert!(!matches!(
        stores
            .data
            .commit_record(
                &ben,
                &post("00000000-0000-4000-8000-000000000c29", "Mine", OPENED_2, 3)
            )
            .await,
        Ok(CommandResult::Committed(_))
    ));
    for (n, (record, choice)) in [(ITEM_1, "a"), (ITEM_2, "c")].into_iter().enumerate() {
        commits(
            stores,
            &owner,
            &quiz.command(
                "add_expected",
                "expected",
                &format!("00000000-0000-4000-8000-000000000c3{n}"),
                0,
                answer(record, choice),
                10 + n,
            ),
        )
        .await;
    }
    let mut ben_response = None;
    for (n, (who, record, choice)) in [
        (&ben, ITEM_1, "a"),
        (&ben, ITEM_2, "c"),
        (&ana, ITEM_1, "b"),
        (&ana, ITEM_2, "c"),
        (&cal, ITEM_1, "a"),
    ]
    .into_iter()
    .enumerate()
    {
        let outcome = commits(
            stores,
            who,
            &quiz.command(
                "respond",
                "responses",
                &format!("00000000-0000-4000-8000-000000000c4{n}"),
                0,
                answer(record, choice),
                20 + n,
            ),
        )
        .await;
        ben_response.get_or_insert(outcome);
    }
    let ben_response = ben_response.unwrap();

    // --- C1: the current record, the server's instant, closing ----------------
    for who in [&owner, &ben] {
        let (run, result) = computed(stores, engine, who, work).await;
        let timer = value(&result, "timer");
        assert_eq!(
            (
                timer["state"].as_str(),
                timer["population"].as_u64(),
                timer["value"].as_str(),
                timer["label"].as_str(),
                timer["closed"].as_bool()
            ),
            (
                Some("ready"),
                Some(2),
                Some("2026-11-29T15:05:30Z"),
                Some("Which shape?"),
                Some(false)
            )
        );
        let mut props =
            projections::props(&run, &result, &projections::default_parameters()).unwrap();
        projections::stamp(&mut props, at("2026-11-29T15:05:05Z"));
        let shown = props.iter().find(|p| p.id == "timer").unwrap();
        assert_eq!(
            (
                shown.countdown_state.as_str(),
                shown.countdown_target.as_str(),
                shown.server_time.as_str(),
                shown.countdown_text.as_str(),
                shown.message.as_str()
            ),
            (
                "open",
                "2026-11-29T15:05:30Z",
                "2026-11-29T15:05:05Z",
                "0:25",
                "Which shape?"
            )
        );
        // The text follows the server's instant given to it, nothing else.
        projections::stamp(&mut props, at("2026-11-29T14:05:05Z"));
        assert_eq!(
            props
                .iter()
                .find(|p| p.id == "timer")
                .unwrap()
                .countdown_text,
            "1:00:25"
        );
    }
    check(checks, "APP-204 C1: after two real posts the countdown (owner's and contributor's page alike, a shared list) follows the item with the highest commit sequence: label \"Which shape?\", target opened_at+30 = 15:05:30Z; stamped at the server's 15:05:05Z it reads 0:25 (an hour earlier, 1:00:25)");
    let close = |record: &str, version: i64, key: usize| {
        quiz.command(
            "close",
            "items",
            record,
            version,
            json!([{"field_id": "state", "value": {"type": "enum", "value": "closed"}}]),
            key,
        )
    };
    commits(stores, &owner, &close(ITEM_2, 1, 40)).await;
    let (run, result) = computed(stores, engine, &ben, work).await;
    let timer = value(&result, "timer");
    assert_eq!(
        (timer["label"].as_str(), timer["closed"].as_bool()),
        (Some("Which shape?"), Some(true))
    );
    let mut props = projections::props(&run, &result, &projections::default_parameters()).unwrap();
    projections::stamp(&mut props, at("2026-11-29T15:05:05Z"));
    let shown = props.iter().find(|p| p.id == "timer").unwrap();
    assert_eq!(
        (
            shown.countdown_state.as_str(),
            shown.countdown_text.as_str()
        ),
        ("closed", "Closed")
    );
    commits(stores, &owner, &close(ITEM_1, 1, 41)).await;
    let (_, result) = computed(stores, engine, &ben, work).await;
    assert_eq!(value(&result, "timer")["label"], "Which colour?");
    assert_eq!(value(&result, "timer")["value"], "2026-11-29T15:00:30Z");
    check(checks, "APP-204 C1: the reviewer's close (closed_when state = closed) shows \"Closed\" with the time still well ahead; closing the older item makes it current (most recently touched, ADR-020 §3)");

    // --- C4: the ranking by creator, scored against the key --------------------
    let full = vec![
        group("Ben Okafor", "2", 2),
        group("Ana Lima", "1", 2),
        group("Member c0000c05", "1", 1),
    ];
    let mut owner_view = Value::Null;
    for who in [&owner, &rev] {
        let (run, result) = computed(stores, engine, who, work).await;
        assert_eq!(groups(&result, "leaderboard"), full);
        assert_eq!(
            (
                value(&result, "leaderboard")["population"].as_u64(),
                value(&result, "leaderboard")["truncated_page"].as_bool(),
                value(&result, "right")["value"].as_str()
            ),
            (Some(5), Some(false), Some("4"))
        );
        assert_eq!(groups(&result, "top"), vec![group("Ben Okafor", "2", 2)]);
        assert_eq!(value(&result, "top")["truncated_page"], true);
        let props = projections::props(&run, &result, &projections::default_parameters()).unwrap();
        let top = props.iter().find(|p| p.id == "top").unwrap();
        assert!(
            top.truncated && top.message.starts_with("Showing a bounded page"),
            "{}",
            top.message
        );
        // The receipt lists both inputs' revisions and validates.
        let receipt =
            projections::receipt(&run, &result, &projections::default_parameters()).unwrap();
        admission::admit_record_components_receipt(&receipt).unwrap();
        assert!(super::query_qualification::contract_valid(&receipt));
        assert_eq!(
            (
                listed(&run, "items"),
                listed(&run, "responses"),
                listed(&run, "expected")
            ),
            (2, 5, 2)
        );
        assert!(receipt["components"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["kind"] == "countdown"));
        // APP-211: a reader of the key withholds nothing.
        assert!(run.withheld().is_empty());
        if who.principal() == P_OWNER {
            owner_view = json!({"result": result.document, "receipt": receipt});
        }
    }
    check(checks, "APP-204 C4: on the owner's and a reviewer's page the ranking by creator lists every contributor by label (Ben Okafor 2, Ana Lima 1, the neutral Member c0000c05 1 for a member with no display name), sorted by score then label; the limit-1 ranking is truncated and says so; the receipt lists items, responses and expected revisions and validates (Rust and contracts); nothing is withheld");
    // ADR-020 §17.34 (APP-211): a contributor cannot read the owner's `own`
    // key list, so every component scoring against it is withheld: never
    // computed (no misleading zero), shown as the one note on every path.
    let mut contributor_view = Value::Null;
    let withheld_views = ["right", "leaderboard", "top"];
    for who in [&ben, &ana, &cal] {
        let (run, result) = computed(stores, engine, who, work).await;
        // The capture: the three declarations, in definition order; the key
        // input is not captured at all, nor the responses here, whose only
        // readers are the withheld components (the snapshot lists items only).
        assert_eq!(withheld_ids(&run), withheld_views);
        assert_eq!(
            run.snapshot()
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["input_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["items"]
        );
        assert!(run.payload()["components"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| !withheld_views.contains(&c["id"].as_str().unwrap())));
        // The engine: no outcome for them; the countdown and the list compute.
        assert!(withheld_views.iter().all(|id| absent(&result, id)));
        assert_eq!(value(&result, "timer")["state"], "ready");
        assert_eq!(value(&result, "item_list")["population"], 2);
        // The receipt: truthful, no coverage and no value for a withheld view.
        let receipt =
            projections::receipt(&run, &result, &projections::default_parameters()).unwrap();
        admission::admit_record_components_receipt(&receipt).unwrap();
        assert!(super::query_qualification::contract_valid(&receipt));
        assert!(receipt["components"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| !withheld_views.contains(&c["view_id"].as_str().unwrap())));
        // The page path (the work page's and the record page's props) and the
        // canvas path (the JSON batch the canvas and the live refresh receive)
        // agree: the state `withheld`, the note, and nothing else.
        let page = projections::props(&run, &result, &projections::default_parameters()).unwrap();
        let canvas = crate::query::record_routes::compute(
            Some(engine as &dyn crate::query::routes::Executor),
            &run,
            &projections::default_parameters(),
        )
        .await;
        let batch = serde_json::to_value(crate::query::record_routes::released(
            canvas.clone(),
            &projections::default_parameters(),
            run.clone(),
            at(OPENED_2),
        ))
        .unwrap();
        assert_eq!(page.len(), 5);
        assert_eq!(page, canvas);
        for id in withheld_views {
            let p = page.iter().find(|p| p.id == id).unwrap();
            assert_eq!(
                (p.state.as_str(), p.message.as_str()),
                ("withheld", projections::WITHHELD_NOTE)
            );
            assert_eq!(p.message, "Scores are shown to this app's reviewers.");
            assert!(
                !p.ready
                    && !p.empty
                    && !p.failed
                    && !p.has_summary
                    && !p.has_groups
                    && !p.has_table
                    && !p.has_receipt
                    && !p.has_filters
                    && !p.has_countdown
            );
            assert!(
                p.summary.is_empty()
                    && p.summary_raw.is_empty()
                    && p.groups.is_empty()
                    && p.rows.is_empty()
                    && p.population.is_empty()
                    && p.receipt_json.is_empty()
            );
            let wire = batch["components"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == id)
                .unwrap();
            assert_eq!(
                (
                    wire["state"].as_str(),
                    wire["message"].as_str(),
                    wire["has_summary"].as_bool(),
                    wire["has_groups"].as_bool(),
                    wire["has_receipt"].as_bool()
                ),
                (
                    Some("withheld"),
                    Some(projections::WITHHELD_NOTE),
                    Some(false),
                    Some(false),
                    Some(false)
                )
            );
            assert_eq!(
                (
                    wire["summary"].as_str(),
                    wire["groups"].as_array().map(Vec::len)
                ),
                (Some(""), Some(0))
            );
        }
        assert!(page.iter().filter(|p| p.state == "withheld").count() == 3);
        if who.principal() == P_BEN {
            contributor_view = json!({"result": result.document, "receipt": receipt, "withheld": run.withheld(), "props": page});
        }
    }
    check(checks, "APP-211 (ADR-020 §17.34): on each contributor's page the three components scoring against the owner's own key (the matches summary and both rankings) are withheld: the capture lists them and reads no key row, the engine never receives them, the result document and the receipt carry no value and no coverage for them, and the page props and the canvas batch alike show the state withheld with exactly \"Scores are shown to this app's reviewers.\" and no number, rows, groups or receipt; the countdown and the item list still compute");

    // --- APP-211 (ADR-020 §17.35): the record chooser names the creator ------
    // The labels an update or transition form's chooser shows, as
    // `query::routes::document_forms` builds them from the same detail and
    // listing: over the `own` responses each names its creator, over the
    // shared items none does.
    async fn chooser(
        stores: &Stores,
        who: &TrustedRequest,
        work: &str,
        schema: &str,
    ) -> (bool, Vec<String>) {
        let collection = stores
            .data
            .list_records(who, QUIZ_APP, work)
            .await
            .unwrap()
            .collections
            .iter()
            .find(|c| c.schema == schema)
            .unwrap()
            .collection
            .clone();
        let fresh = crate::auth::capability::random_uuid().unwrap();
        let detail = stores
            .data
            .read_record_detail(who, QUIZ_APP, work, &collection, &fresh)
            .await
            .unwrap();
        let listing = stores.data.list_records(who, QUIZ_APP, work).await.unwrap();
        let labels: Vec<String> = listing
            .records
            .iter()
            .filter(|row| row.collection == collection)
            .map(|row| {
                crate::query::routes::record_choice_label(
                    detail
                        .own
                        .then(|| crate::directory::label(ORG, &row.creator)),
                    &row.values,
                    row.version,
                )
            })
            .collect();
        (detail.own, labels)
    }
    let names = |labels: &[String]| {
        let mut seen: Vec<String> = labels
            .iter()
            .map(|l| l.split(" · ").next().unwrap().to_owned())
            .collect();
        seen.sort();
        seen.dedup();
        seen
    };
    for who in [&owner, &rev] {
        let (own, labels) = chooser(stores, who, work, "responses").await;
        assert!(own);
        assert_eq!(labels.len(), 5);
        assert_eq!(
            names(&labels),
            ["Ana Lima", "Ben Okafor", "Member c0000c05"]
        );
        assert!(
            labels
                .iter()
                .all(|l| l.split(" · ").count() == 3 && l.contains(" · revision ")),
            "{labels:?}"
        );
        let (shared, items) = chooser(stores, who, work, "items").await;
        assert!(!shared);
        assert_eq!(
            items,
            ["Which colour? · revision 2", "Which shape? · revision 2"]
        );
    }
    let (own, labels) = chooser(stores, &ben, work, "responses").await;
    assert!(own);
    assert_eq!(labels.len(), 2);
    assert_eq!(names(&labels), ["Ben Okafor"]);
    let (_, labels) = chooser(stores, &cal, work, "responses").await;
    assert_eq!(names(&labels), ["Member c0000c05"]);
    check(checks, "APP-211 (ADR-020 §17.35): the record chooser over the own responses labels every entry <creator> · <text> · revision n for the owner and the reviewer (Ana Lima, Ben Okafor and the neutral Member c0000c05, five entries), a contributor sees only their own entries under their own name, and the shared items keep today's label, newest commit first (\"Which colour? · revision 2\", \"Which shape? · revision 2\")");

    // --- labels, never principal ids -----------------------------------------
    let principals = [P_OWNER, P_REV, P_BEN, P_ANA, P_CAL];
    for who in [&owner, &ben] {
        let (run, result) = computed(stores, engine, who, work).await;
        let document = serde_json::to_string(&result.document).unwrap();
        assert!(
            principals.iter().all(|p| !document.contains(p)),
            "a result document carries a principal id"
        );
        // The page props carry the receipt, which names only the viewer.
        let props = serde_json::to_string(
            &projections::props(&run, &result, &projections::default_parameters()).unwrap(),
        )
        .unwrap();
        assert!(principals
            .iter()
            .filter(|p| **p != who.principal())
            .all(|p| !props.contains(p)));
    }
    check(checks, "APP-204 §14.4: the result document carries labels and no principal id; the page props name no member but the viewer (in their own receipt)");

    // --- D7 under bypass ------------------------------------------------------
    judged!(checks, "APP-211 D7: a contributor's components scoring against the owner's own key are withheld (no value, the one note) because the key is hidden from them by the row rule; a reviewer's compute", {
        let (run, result) = computed(stores, engine, &ben, work).await;
        let props = projections::props(&run, &result, &projections::default_parameters()).unwrap();
        let (_, reviewed) = computed(stores, engine, &rev, work).await;
        withheld_ids(&run) == ["right", "leaderboard", "top"]
            && absent(&result, "leaderboard") && absent(&result, "right") && absent(&result, "top")
            && props.iter().filter(|p| p.state == "withheld").count() == 3
            && props.iter().filter(|p| p.state == "withheld").all(|p| p.message == projections::WITHHELD_NOTE && !p.has_summary && !p.has_groups && !p.has_receipt)
            && groups(&reviewed, "leaderboard") == full
    });
    judged!(checks, "APP-204 D7: a contributor's capture reads none of the owner's own key list (APP-211: neither the key input nor the responses, whose only readers are withheld, is captured; the creators map names the owner alone, from the items); a reviewer's reads all", {
        let b = stores.data.record_components(&ben, QUIZ_APP, work).await.unwrap();
        let r = stores.data.record_components(&rev, QUIZ_APP, work).await.unwrap();
        let ben_creators: Vec<String> = b.payload()["creators"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect();
        listed(&b, "responses") == usize::MAX && listed(&b, "expected") == usize::MAX && listed(&b, "items") == 2 && listed(&r, "responses") == 5 && listed(&r, "expected") == 2
            && ben_creators == [P_OWNER.to_owned()]
    });
    judged!(checks, "APP-204 D7 (APP-191's delivery rule, the ranking card open): a contributor's response reaches its creator, the reviewers and the owner, never another contributor", {
        let judged = stores.data.openable_principals(&owner, QUIZ_APP, work, Some(ben_response.commit_sequence), &principals.map(str::to_owned)).await.unwrap();
        judged.standing(&owner) == Standing::Open
            && judged.standing(&rev) == Standing::Open
            && judged.standing(&ben) == Standing::Open
            && judged.standing(&ana) == Standing::Closed
            && judged.standing(&cal) == Standing::Closed
    });
    json!({"definition": participation_definition()["content"], "owner": owner_view, "contributor": contributor_view})
}

// --- APP-208: ADR-020 §9 (the multi-select) ----------------------------------

const MULTI_APP: &str = "00000000-0000-4000-8000-000000000c30";
const MULTI_REVISION: &str = "00000000-0000-4000-8000-000000000c31";
/// The contract specimen of edition impl-2026-11-14.2 (B0's
/// `make_fixtures.participation()`); its `multi_select` member is ADR-020 §9's
/// definition: an `own` `choices` list (`title` text, `tags` multi_enum
/// red|green|blue, `day` date) with a `pick` create, a reviewer-only `keys`
/// list, a `records_list`, a ranking by `tags`, a trend by `day`, a `matches`
/// summary and a typed query projecting `title` and `tags`.
const SPECIMEN: &str = include_str!("../../../contracts/fixtures/participation.json");

/// The specimen's multi-select definition under this campaign's organization
/// and app, at definition schema version 1 (storage stores edition 1 only,
/// `definitions::create_definition_revision`), derived as B0's
/// `make_fixtures.records_input` derives `definition_v1` from the version-2
/// content: every input drops its version-2-only `coverage`; nothing else
/// changes. Re-digested over that content.
fn multi_select_definition() -> Value {
    let specimen: Value = serde_json::from_str(SPECIMEN).unwrap();
    let mut content = specimen["multi_select"]["content"].clone();
    for input in content["inputs"].as_array_mut().unwrap() {
        input.as_object_mut().unwrap().remove("coverage");
    }
    let canonical = admission::encode(&content).unwrap();
    json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": ORG,
        "app_id": MULTI_APP,
        "revision_id": MULTI_REVISION,
        "content": content,
        "digest": digest::digest(digest::APP_DEFINITION, &canonical),
    })
}

fn tags(values: &[&str]) -> Value {
    json!({"type": "multi_enum", "values": values})
}
fn pick_values(title: &str, tags_value: Value, day: &str) -> Value {
    json!([
        {"field_id": "title", "value": {"type": "text", "value": title}},
        {"field_id": "tags", "value": tags_value},
        {"field_id": "day", "value": {"type": "date", "value": day}}
    ])
}
fn multi_record(n: u32) -> String {
    format!("00000000-0000-4000-8000-00000000c3{n:02x}")
}
/// The `pick` form a member is offered on record `n`, as the record page and
/// the canvas card build it (`view_for_command`).
async fn multi_form(
    stores: &Stores,
    who: &TrustedRequest,
    work: &str,
    choices: &str,
    n: u32,
) -> crate::commands::RecordView {
    let detail = stores
        .data
        .read_record_detail(who, MULTI_APP, work, choices, &multi_record(n))
        .await
        .unwrap();
    crate::commands::view_for_command(
        MULTI_APP,
        work,
        &detail,
        "csrf".into(),
        format!("{:032x}", 0xc380 + n),
        Some("pick"),
    )
}
/// The specimen's `tag_rows` typed query as `who` may run it: its rows,
/// sorted by title, each as a JSON array.
async fn typed_rows(
    stores: &Stores,
    engine: &Engine,
    who: &TrustedRequest,
    work: &str,
) -> Vec<Value> {
    let run = stores
        .data
        .run_query(who, MULTI_APP, work, "tag_rows", &json!({}))
        .await
        .unwrap();
    let result = engine
        .execute(&run, Duration::from_millis(1800))
        .await
        .unwrap();
    stores.data.release_result(who, &run).await.unwrap();
    let mut rows = result.rows.clone();
    rows.sort_by(|a, b| a[0].as_str().cmp(&b[0].as_str()));
    rows.into_iter().map(Value::Array).collect()
}

/// ADR-020 §9: the multi-select through the shipped paths. Returns the
/// report's `multi_select` member.
async fn multi_select(
    fixture: &Fixture,
    stores: &Stores,
    engine: &Engine,
    checks: &mut Vec<Value>,
) -> Value {
    // The actors are `participation`'s (admitted and named there).
    let owner = request(stores, ORG, P_OWNER, 1);
    let rev = request(stores, ORG, P_REV, 1);
    let ben = request(stores, ORG, P_BEN, 1);
    let ana = request(stores, ORG, P_ANA, 1);
    operator(
        fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            MULTI_APP,
            "--space",
            SPACE,
            "--owner",
            P_OWNER,
        ],
    );
    for permission in ["discover", "run"] {
        grant(fixture, P_OWNER, permission, &["--app", MULTI_APP]);
    }
    let admitted =
        admission::admit_definition(&serde_json::to_vec(&multi_select_definition()).unwrap())
            .expect("the specimen's multi-select definition admits");
    let revision = stores
        .workspace
        .create_definition_revision(&owner, MULTI_APP, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(&owner, MULTI_APP, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let saved = stores
        .workspace
        .create_work(
            &owner,
            MULTI_APP,
            &publication.publication_id,
            "Multi-select (synthetic campaign)",
            &["choices", "keys"].map(|id| WorkBinding::Records {
                input: id.into(),
                schema: id.into(),
            }),
        )
        .await
        .unwrap();
    let work = saved.work.as_str();
    stores
        .workspace
        .share_work(
            &owner,
            MULTI_APP,
            work,
            Audience::Member(P_REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    for member in [P_BEN, P_ANA] {
        stores
            .workspace
            .share_work(
                &owner,
                MULTI_APP,
                work,
                Audience::Member(member),
                ShareRole::Contributor,
                ShareMode::Set,
            )
            .await
            .unwrap();
    }
    let listing = stores
        .data
        .list_records(&owner, MULTI_APP, work)
        .await
        .unwrap();
    let choices = listing
        .collections
        .iter()
        .find(|c| c.schema == "choices")
        .expect("the choices collection")
        .collection
        .clone();
    let record = multi_record;
    let pick = |n: u32, values: Value| CommandRequest {
        app: MULTI_APP.into(),
        work: work.to_owned(),
        publication: publication.publication_id.clone(),
        command: "pick".into(),
        schema: "choices".into(),
        schema_version_expected: 1,
        record: record(n),
        expected_record_version: 0,
        expected_work_version: 1,
        values,
        idempotency_key: format!("{:032x}", 0xc300 + n),
    };
    check(checks, "APP-208: the contract specimen's multi-select definition (an own `choices` list with a multi_enum `tags`) admits, publishes and saves through the shipped path");

    // --- storage: the ordered subset and nothing else ---------------------
    commits(
        stores,
        &ben,
        &pick(1, pick_values("one", tags(&["red", "blue"]), "2026-11-14")),
    )
    .await;
    commits(
        stores,
        &ben,
        &pick(2, pick_values("two", tags(&["blue"]), "2026-11-14")),
    )
    .await;
    commits(
        stores,
        &ana,
        &pick(
            3,
            pick_values("three", tags(&["red", "green", "blue"]), "2026-11-15"),
        ),
    )
    .await;
    for (n, refused) in [
        (0x10, tags(&["blue", "red"])),
        (0x11, tags(&["red", "red"])),
        (0x12, tags(&["purple"])),
        (0x13, tags(&["red", "purple"])),
        (0x14, tags(&[])),
        (0x15, json!({"type": "null"})),
        (0x16, json!({"type": "enum", "value": "red"})),
        (
            0x17,
            json!({"type": "multi_enum", "values": ["red"], "value": "red"}),
        ),
    ] {
        let attempt = pick(n, pick_values("refused", refused.clone(), "2026-11-14"));
        assert_eq!(
            stores.data.commit_record(&ben, &attempt).await,
            Err(Error::Denied),
            "{refused}"
        );
    }
    let detail = stores
        .data
        .read_record_detail(&ben, MULTI_APP, work, &choices, &record(1))
        .await
        .unwrap();
    assert_eq!(
        detail.fields.iter().find(|f| f.id == "tags").unwrap().value,
        tags(&["red", "blue"])
    );
    check(checks, "APP-208 storage: a selection in declaration order commits and is stored canonically; another order, a repeat, an unknown id, an empty set, null on the required field, an enum literal and an extra member are each refused (the uniform refusal, nothing written)");

    // --- the native form: checkboxes under one input name -----------------
    let fresh = multi_form(stores, &ben, work, &choices, 4).await;
    let row = |view: &crate::commands::RecordView, id: &str| {
        view.fields.iter().find(|r| r.id == id).cloned().unwrap()
    };
    let tags_row = row(&fresh, "tags");
    assert_eq!(tags_row.kind, "multi_enum");
    assert_eq!(
        tags_row
            .choices
            .iter()
            .map(|c| (c.value.as_str(), c.selected))
            .collect::<Vec<_>>(),
        [("red", false), ("green", false), ("blue", false)]
    );
    assert!(tags_row.writable && !tags_row.nothing_to_choose && tags_row.value.is_empty());
    let card = crate::query::canvas::form_card(work, &fresh).unwrap();
    let card_tags = card.fields.iter().find(|f| f.id == "tags").unwrap();
    assert!(card_tags.has_checkboxes && !card_tags.has_choices && !card_tags.has_text);
    let field = |name: &str, value: &str| (name.to_owned(), value.to_owned());
    let premise = |view: &crate::commands::RecordView| {
        vec![
            field("csrf", "csrf"),
            field("idempotency_key", &view.idempotency_key),
            field("collection_id", &view.collection),
            field("publication_id", &view.publication),
            field("schema_id", &view.schema),
            field("schema_version_expected", &view.schema_version),
            field("record_id", &view.record_id),
            field("expected_record_version", &view.version),
            field("expected_work_version", &view.work_version),
            field(&row(view, "title").input, "four"),
            field(&row(view, "title").spec_input, &row(view, "title").spec),
            field(&row(view, "day").input, "2026-11-16"),
            field(&row(view, "day").spec_input, &row(view, "day").spec),
        ]
    };
    // As the browser posts the group: the empty sentinel, then each checked
    // box in document (declaration) order, then the spec echo.
    let mut posted = premise(&fresh);
    posted.extend([
        field(&tags_row.input, ""),
        field(&tags_row.input, "green"),
        field(&tags_row.input, "blue"),
        field(&tags_row.spec_input, &tags_row.spec),
    ]);
    let (submitted, collection) =
        crate::commands::routes::parse_form_for_tests(&posted, MULTI_APP, work, "pick").unwrap();
    assert_eq!(collection, choices);
    let posted_value = |request: &CommandRequest, id: &str| {
        request
            .values
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["field_id"] == id)
            .unwrap()["value"]
            .clone()
    };
    assert_eq!(posted_value(&submitted, "tags"), tags(&["green", "blue"]));
    let picked = commits(stores, &ben, &submitted).await;
    assert_eq!(picked.record_id, record(4));
    let held = multi_form(stores, &ben, work, &choices, 4).await;
    assert_eq!(row(&held, "tags").value, "green, blue");
    assert_eq!(
        row(&held, "tags")
            .choices
            .iter()
            .map(|c| c.selected)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    // Nothing checked: the sentinel alone is the null value, which the
    // required field refuses.
    let none = multi_form(stores, &ben, work, &choices, 5).await;
    let mut unchecked = premise(&none);
    unchecked.extend([
        field(&tags_row.input, ""),
        field(&tags_row.spec_input, &tags_row.spec),
    ]);
    let (nothing, _) =
        crate::commands::routes::parse_form_for_tests(&unchecked, MULTI_APP, work, "pick").unwrap();
    assert_eq!(posted_value(&nothing, "tags"), json!({"type": "null"}));
    assert_eq!(
        stores.data.commit_record(&ben, &nothing).await,
        Err(Error::Denied)
    );
    check(checks, "APP-208 forms: a contributor's pick form offers one checkbox per declared value in declaration order (none checked on a new record; the canvas card carries them as checkboxes, not a select or a text box); the boxes posted through the native form's parse compose one ordered multi_enum value storage accepts, and the record then reads \"green, blue\" with those two checked; nothing checked posts null, which the required field refuses");

    // --- the ranking counts a row once per selected value -----------------
    // Four rows: [red, blue], [blue], [red, green, blue], [green, blue].
    let (run, result) = computed_in(stores, engine, &rev, MULTI_APP, work).await;
    assert_eq!(
        groups(&result, "by_tag"),
        vec![
            group("blue", "4", 4),
            group("green", "2", 2),
            group("red", "2", 2)
        ]
    );
    let by_tag = value(&result, "by_tag");
    assert_eq!(
        (by_tag["population"].as_u64(), by_tag["value"].as_str()),
        (Some(4), Some("4"))
    );
    assert_eq!(value(&result, "list")["population"], 4);
    let list = value(&result, "list");
    let cell = |record_id: &str| {
        list["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == record_id)
            .map(|r| r["cells"][1].clone())
            .unwrap()
    };
    assert_eq!(cell(&record(1)), tags(&["red", "blue"]));
    assert_eq!(cell(&record(4)), tags(&["green", "blue"]));
    let props = projections::props(&run, &result, &projections::default_parameters()).unwrap();
    let shown = props.iter().find(|p| p.id == "list").unwrap();
    let texts: Vec<String> = shown
        .rows
        .iter()
        .filter(|r| r.id == record(1))
        .flat_map(|r| {
            r.cells
                .iter()
                .filter(|c| c.id == "tags")
                .map(|c| c.text.clone())
        })
        .collect();
    assert_eq!(texts, ["red, blue"]);
    // The trend buckets by month (APP-159): three November days, one group.
    assert_eq!(groups(&result, "by_day"), vec![group("2026-11", "4", 4)]);
    assert_eq!(value(&result, "matched")["value"], "0");
    check(checks, "APP-208 ranking: over four rows selecting [red, blue], [blue], [red, green, blue] and [green, blue], a reviewer's ranking by tags counts blue 4, green 2, red 2 (a row adds one to each option it selected) while its population and total count the four rows once; the list's cell carries the typed selection and reads \"red, blue\"; the trend and the matches summary run beside it");

    // --- the typed query sees the field as text ---------------------------
    assert_eq!(
        typed_rows(stores, engine, &rev, work).await,
        vec![
            json!(["four", "green, blue"]),
            json!(["one", "red, blue"]),
            json!(["three", "red, green, blue"]),
            json!(["two", "blue"]),
        ]
    );
    check(checks, "APP-208 typed query: the specimen's `tag_rows` view runs over the input with the multi_enum field projected as text joined by \", \"");

    // --- D7 on every path -------------------------------------------------
    judged!(checks, "APP-208 x D7: a contributor's ranking by tags, list and typed query count only their own rows (Ben: blue 3, green 1, red 1 over three rows), a reviewer's every member's", {
        let (_, mine) = computed_in(stores, engine, &ben, MULTI_APP, work).await;
        let rows = typed_rows(stores, engine, &ben, work).await;
        groups(&mine, "by_tag") == vec![group("blue", "3", 3), group("green", "1", 1), group("red", "1", 1)]
            && value(&mine, "by_tag")["population"] == 3
            && value(&mine, "list")["population"] == 3
            && rows.len() == 3
            && rows.iter().all(|r| r[0] != "three")
    });
    json!({
        "definition": multi_select_definition()["content"],
        "work": work,
        "reviewer_ranking": value(&result, "by_tag")["groups"],
        "reviewer_rows": typed_rows(stores, engine, &rev, work).await,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// APP-208: the specimen's multi-select app admits at the stored edition
    /// (version 1, `coverage` dropped from its inputs) with its `multi_enum`
    /// field, its ranking by that field and its typed query intact.
    #[test]
    fn the_multi_select_definition_admits_at_the_stored_edition() {
        let document = super::multi_select_definition();
        assert_eq!(document["schema_version"], 1);
        let admitted = crate::admission::admit_definition(&serde_json::to_vec(&document).unwrap())
            .expect("admits");
        assert_eq!(admitted.schema_version, 1);
        let content = crate::admission::decode(&admitted.canonical).unwrap();
        let inputs = content["inputs"].as_array().unwrap();
        assert!(inputs.iter().all(|i| i.get("coverage").is_none()));
        let tags = &content["record_schemas"][0]["fields"][1];
        assert_eq!(tags["value_type"]["type"], "multi_enum");
        let kinds: Vec<&Value> = content["views"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| &v["kind"])
            .collect();
        assert!(
            kinds.contains(&&Value::from("record_ranking"))
                && kinds.contains(&&Value::from("table"))
        );
    }
}
