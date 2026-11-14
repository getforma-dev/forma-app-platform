//! Runs inside the isolated box campaign against a fully imported CSV app.
use super::{qualification::Fixture, *};
use genui_auth_verifier::VerifiedIdentity;
use serde_json::json;

pub(super) async fn qualify(
    fixture: &Fixture,
    stores: &Stores,
    owner: &TrustedRequest,
    app: &str,
    work: &str,
) {
    let proof = ShareIdentityProof::verified(
        &VerifiedIdentity {
            key_id: "synthetic-key".into(),
            issuer: "app008-fixture-only".into(),
            tenant_id: "record-query-guest-fixture".into(),
            subject: "record-query-guest".into(),
            issued_at: 1,
            expires_at: 2,
        },
        "record-query@example.test",
        true,
    )
    .unwrap();
    let cookie = stores
        .workspace
        .mint_shared_session(&proof, "00000000-0000-4000-8000-00000000c921")
        .await
        .unwrap();
    let guest = stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .unwrap();
    let share = stores
        .workspace
        .create_live_share(
            owner,
            app,
            work,
            &NewLiveShare {
                recipient_email: "record-query@example.test".into(),
                allow_add_to_canvas: false,
                expires_in_days: 1,
                idempotency_key: "00000000-0000-4000-8000-00000000c922".into(),
            },
        )
        .await
        .unwrap();
    let access = stores
        .workspace
        .accept_live_share(&guest, &share.key)
        .await
        .unwrap();
    let member_run = stores
        .data
        .run_query(owner, app, work, "editable_kpis", &json!({}))
        .await
        .unwrap();
    let guest_run = stores
        .data
        .guest_run_query(&access, "editable_kpis", &json!({}))
        .await
        .unwrap();
    assert!(member_run.source().is_none());
    assert_eq!(member_run.records(), guest_run.engine_plan().records());
    stores
        .data
        .release_result(owner, &member_run)
        .await
        .unwrap();
    stores
        .data
        .release_guest_result(&access, &guest_run)
        .await
        .unwrap();
    let mut omitted_collection = access.clone();
    omitted_collection.collections.clear();
    assert!(matches!(
        stores
            .data
            .guest_run_query(&omitted_collection, "editable_kpis", &json!({}))
            .await,
        Err(Error::Denied)
    ));
    // Whole snapshots remain reserved through execution/release. Saturation
    // (APP-200: the worker's admission, 24) refuses before materializing the
    // copy past the ceiling, and dropping one frees it.
    let mut capacity = Vec::new();
    for _ in 0..(super::record_query::SNAPSHOT_SLOTS - 2) {
        capacity.push(
            stores
                .data
                .run_query(owner, app, work, "editable_kpis", &json!({}))
                .await
                .unwrap(),
        );
    }
    assert!(matches!(
        stores
            .data
            .run_query(owner, app, work, "editable_kpis", &json!({}))
            .await,
        Err(Error::Busy)
    ));
    drop(capacity);

    let listing = stores.data.list_records(owner, app, work).await.unwrap();
    let record = &listing.records[0];
    let collection = listing
        .collections
        .iter()
        .find(|c| c.collection == record.collection)
        .unwrap();
    let command = collection
        .commands
        .iter()
        .find(|c| c.operation == "update")
        .unwrap();
    let mut amount = record
        .values
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["field_id"] == "amount")
        .unwrap()
        .clone();
    let before: i64 = amount["value"]["coefficient"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    amount["value"]["coefficient"] = json!((before + 1).to_string());
    let changed = stores
        .data
        .commit_record(
            owner,
            &CommandRequest {
                app: app.into(),
                work: work.into(),
                publication: listing.publication.clone(),
                command: command.id.clone(),
                schema: record.schema.clone(),
                schema_version_expected: i64::from(collection.schema_version),
                record: record.record_id.clone(),
                expected_record_version: record.version,
                expected_work_version: listing.work_version,
                values: json!([amount]),
                idempotency_key: "0000000000004000800000000000c923".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(changed, CommandResult::Committed(_)));
    // APP-198: a commit between capture and release makes the snapshot
    // `Stale` (typed; the route captures once more), not `Denied`: the member
    // reads the list under RLS already, so staleness discloses nothing.
    assert!(matches!(
        stores.data.release_result(owner, &member_run).await,
        Err(Error::Stale)
    ));
    assert!(matches!(
        stores.data.release_guest_result(&access, &guest_run).await,
        Err(Error::Stale)
    ));
    let previous_sequence = member_run.record_snapshot_sequence();
    drop((member_run, guest_run));
    let fresh = stores
        .data
        .run_query(owner, app, work, "editable_kpis", &json!({}))
        .await
        .unwrap();
    let fresh_guest = stores
        .data
        .guest_run_query(&access, "editable_kpis", &json!({}))
        .await
        .unwrap();
    assert!(fresh.record_snapshot_sequence() > previous_sequence);
    assert_eq!(fresh.records(), fresh_guest.engine_plan().records());
    stores.data.release_result(owner, &fresh).await.unwrap();
    stores
        .data
        .release_guest_result(&access, &fresh_guest)
        .await
        .unwrap();

    let source = fresh.dependencies()[0].source();
    let org = owner.organization();
    let principal = owner.principal();
    fixture.sql(&format!("DELETE FROM genui.source_permissions WHERE organization_id='{org}' AND principal_id='{principal}' AND source_id='{source}' AND permission='source_read'"), "genui");
    assert!(matches!(
        stores.data.release_result(owner, &fresh).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .data
            .release_guest_result(&access, &fresh_guest)
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .data
            .run_query(owner, app, work, "editable_kpis", &json!({}))
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .data
            .guest_run_query(&access, "editable_kpis", &json!({}))
            .await,
        Err(Error::Denied)
    ));
    fixture.sql(&format!("INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission) VALUES('{org}','{principal}','{source}','source_read') ON CONFLICT DO NOTHING"), "genui");
    stores.data.release_result(owner, &fresh).await.unwrap();
    stores
        .data
        .release_guest_result(&access, &fresh_guest)
        .await
        .unwrap();
}
