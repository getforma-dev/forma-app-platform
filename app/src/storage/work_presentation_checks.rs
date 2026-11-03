//! Real storage regressions reused by the composer campaign.
use super::{
    definition_qualification::request,
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER, PRINCIPAL},
    *,
};
use crate::composer::{plan, routes::Composed};
use serde_json::{json, Value};

async fn publish(
    stores: &Stores,
    member: &TrustedRequest,
    app: &str,
    before: &WorkPresentation,
    content: Value,
) -> String {
    let admitted = plan::admit(&content, member.organization(), app).unwrap();
    let draft = stores
        .workspace
        .create_definition_revision(member, app, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(published) = stores
        .control
        .publish(member, app, &draft.revision, &before.policy)
        .await
        .unwrap()
    else {
        panic!("fixture publication")
    };
    published.publication_id
}
fn data(bytes: &[u8]) -> Value {
    let mut v = crate::admission::decode(bytes).unwrap();
    v.as_object_mut().unwrap().remove("publication_id");
    v.as_object_mut().unwrap().remove("version");
    v
}
pub(super) async fn run(
    fixture: &Fixture,
    stores: &Stores,
    member: &TrustedRequest,
    first: &Composed,
    second: &Composed,
    checks: &mut Vec<Value>,
) {
    let (app, work) = (&first.app, &first.work);
    let before = stores
        .workspace
        .work_presentation(member, app, work)
        .await
        .unwrap();
    assert!(before.can_edit);
    let command = CommandRequest {
        app: app.clone(),
        work: work.clone(),
        publication: before.publication.clone(),
        command: "add".into(),
        schema: "request".into(),
        schema_version_expected: 1,
        record: "00000000-0000-4000-8000-000000000155".into(),
        expected_record_version: 0,
        expected_work_version: before.version,
        values: json!([
            {"field_id":"item","value":{"type":"text","value":"Monitor"}},
            {"field_id":"requester","value":{"type":"text","value":"Synthetic requester"}},
            {"field_id":"notes","value":{"type":"null"}}
        ]),
        idempotency_key: "00000000000000000000000000000155".into(),
    };
    assert!(matches!(
        stores.data.commit_record(member, &command).await.unwrap(),
        CommandResult::Committed(_)
    ));
    let record_before = stores.data.list_records(member, app, work).await.unwrap();
    let saved = stores.workspace.read_work(member, app, work).await.unwrap();
    let sibling = stores
        .workspace
        .read_work(member, &second.app, &second.work)
        .await
        .unwrap();
    let mut content = before.content.clone();
    content["presentation"]["theme"]["accent"] = json!("violet");
    let left = publish(stores, member, app, &before, content.clone()).await;
    content["presentation"]["theme"]["accent"] = json!("amber");
    let right = publish(stores, member, app, &before, content.clone()).await;
    assert_eq!(
        stores
            .workspace
            .adopt_presentation(member, &second.app, &second.work, &before, &left)
            .await,
        Err(Error::Denied)
    );
    let foreign = request(stores, OTHER, PRINCIPAL, 1);
    assert!(matches!(
        stores
            .workspace
            .work_presentation(&foreign, app, work)
            .await,
        Err(Error::Denied)
    ));
    let (a, b) = tokio::join!(
        stores
            .workspace
            .adopt_presentation(member, app, work, &before, &left),
        stores
            .workspace
            .adopt_presentation(member, app, work, &before, &right)
    );
    assert!(
        matches!((&a, &b), (Ok(true), Ok(false)) | (Ok(false), Ok(true))),
        "adoption race: {a:?} / {b:?}"
    );
    let after = stores.workspace.read_work(member, app, work).await.unwrap();
    assert_eq!(after.version, before.version + 1);
    assert_eq!(
        stores
            .data
            .list_records(member, app, work)
            .await
            .unwrap()
            .records,
        record_before.records
    );
    assert_eq!(data(&saved.canonical), data(&after.canonical));
    assert_eq!(
        stores
            .workspace
            .read_work(member, &second.app, &second.work)
            .await
            .unwrap(),
        sibling
    );
    assert!(!stores
        .workspace
        .adopt_presentation(member, app, work, &before, &left)
        .await
        .unwrap());
    check(checks,"APP-155/157: two admitted presentation publications race for one exact work; one adoption wins, one remains unused, stale/sibling/foreign premises refuse; collections, bindings, source dependencies, sibling and record identity remain unchanged");
    let before = stores
        .workspace
        .work_presentation(member, app, work)
        .await
        .unwrap();
    let mut changed_data = before.content.clone();
    changed_data["title"] = json!("Not presentation");
    let wrong = publish(stores, member, app, &before, changed_data).await;
    assert_eq!(
        stores
            .workspace
            .adopt_presentation(member, app, work, &before, &wrong)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores.workspace.read_work(member, app, work).await.unwrap(),
        after
    );
    let candidate = publish(stores, member, app, &before, content.clone()).await;
    let operator = fixture.operator().await;
    let policy_version:i64=operator.client.query_one("SELECT (max(policy_version)+1)::bigint FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND policy_id=$2::text::uuid",&[&ORG,&before.policy]).await.unwrap().get(0);
    operator.client.execute("INSERT INTO genui.publication_policies(organization_id,policy_id,policy_version,kind) VALUES($1::text::uuid,$2::text::uuid,$3::bigint,'independent_review')",&[&ORG,&before.policy,&policy_version]).await.unwrap();
    assert!(
        !stores
            .workspace
            .work_presentation(member, app, work)
            .await
            .unwrap()
            .can_edit
    );
    assert_eq!(
        stores
            .workspace
            .adopt_presentation(member, app, work, &before, &candidate)
            .await,
        Err(Error::Denied)
    );
    operator.client.execute("DELETE FROM genui.publication_policies WHERE organization_id=$1::text::uuid AND policy_id=$2::text::uuid AND policy_version=$3::bigint",&[&ORG,&before.policy,&policy_version]).await.unwrap();
    assert_eq!(
        stores.workspace.read_work(member, app, work).await.unwrap(),
        after
    );
    check(checks,"APP-156: admission alone never authorizes adopting changed data/schema content; current publication policy change after publishing refuses adoption and hides the editor without modifying saved work");
    // Prove the post-lock entitlement recheck with an actually waiting writer.
    operator.client.batch_execute("BEGIN").await.unwrap();
    operator.client.query_one("SELECT version FROM genui.saved_work WHERE organization_id=$1::text::uuid AND app_id=$2::text::uuid AND work_id=$3::text::uuid FOR UPDATE",&[&ORG,app,work]).await.unwrap();
    let pending = stores
        .workspace
        .adopt_presentation(member, app, work, &before, &candidate);
    tokio::pin!(pending);
    let mut waiting = 0_i64;
    for _ in 0..40 {
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut pending)
                .await
                .is_err()
        );
        waiting=operator.client.query_one("SELECT count(*) FROM pg_locks waiting JOIN pg_locks held USING(locktype,transactionid) WHERE waiting.locktype='transactionid' AND NOT waiting.granted AND held.pid=pg_backend_pid() AND held.granted",&[]).await.unwrap().get(0);
        if waiting == 1 {
            break;
        }
    }
    assert_eq!(waiting, 1);
    operator.client.execute("UPDATE genui.entitlements SET expires_at=starts_at+interval '1 millisecond' WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid",&[&ORG,&member.principal()]).await.unwrap();
    operator.client.batch_execute("COMMIT").await.unwrap();
    assert_eq!(pending.await, Err(Error::Denied));
    operator.client.execute("UPDATE genui.entitlements SET expires_at='2036-10-01T00:00:00Z' WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid",&[&ORG,&member.principal()]).await.unwrap();
    assert_eq!(
        stores.workspace.read_work(member, app, work).await.unwrap(),
        after
    );
    check(checks,"APP-155: actual row-lock wait followed by entitlement expiry denies presentation adoption and leaves work unchanged");
}
