//! Real PostgreSQL + confined child qualification for APP-158/159. Definitions
//! are admitted through the shipped template path; records are saved by the
//! production command transaction. Provider proof is synthetic test input only.
use super::{
    admission_qualification::operator,
    definition_qualification::request,
    qualification::{fixture_python, Fixture},
    qualification_cases::{check, ORG, OTHER, PRINCIPAL},
    *,
};
use crate::{
    composer::{
        routes::{compose_instant, First, Pin},
        Composer,
    },
    query::{
        record_components as projections,
        tests::{real_engine, Directory},
    },
};
use genui_auth_verifier::VerifiedIdentity;
use serde_json::{json, Value};
use std::time::Duration;

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
    let guest = stores.data.guest_record_components(&access).await.unwrap();
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
    let report = json!({"campaign":"APP-158-159-record-components","passed":true,"checks":checks,"record_operator_version":1,"bounds":{"rows":RECORD_COMPONENT_ROWS,"bytes":RECORD_COMPONENT_BYTES},"provider_identity":"synthetic-test-proof-only","definition":template.content(),"initial_command_outcomes":command_outcomes,"snapshot_datasets":run.payload()["datasets"],"parameters":parameters,"initial_result":result.document,"member_receipt":receipt,"guest_receipt":projections::receipt(&guest,&guest_result,&parameters).unwrap()});
    if let Some(path) = std::env::var_os("GENUI_RECORD_COMPONENTS_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
