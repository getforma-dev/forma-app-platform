//! Reusable assertions for the isolated test executable only. Callers own
//! fixture setup, credentials, operator mutations and transport cleanup.
//! This module has no endpoint, Docker command, SQL input or production hook.
use super::{
    authorization::{Resource, Transaction},
    pool::Lease,
    *,
};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::Instant;

pub(super) const ORG: &str = "00000000-0000-4000-8000-000000000001";
pub(super) const OTHER: &str = "00000000-0000-4000-8000-000000000002";
pub(super) const PRINCIPAL: &str = "00000000-0000-4000-8000-000000000066";
pub(super) const REVIEWER: &str = "00000000-0000-4000-8000-00000000006d";
pub(super) const APP: &str = "00000000-0000-4000-8000-000000000065";
pub(super) const WORK: &str = "00000000-0000-4000-8000-000000000069";
pub(super) const PUBLICATION: &str = "00000000-0000-4000-8000-000000000068";
pub(super) const COLLECTION: &str = "00000000-0000-4000-8000-00000000006a";
pub(super) const RECORD: &str = "00000000-0000-4000-8000-00000000008c";

pub(super) async fn pid(lease: &Lease) -> i32 {
    lease
        .client()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0)
}
pub(super) fn check(checks: &mut Vec<Value>, name: &str) {
    checks.push(json!({"name":name,"passed":true}));
    println!("APP008 check: {name}");
}

pub(super) async fn readiness(stores: &Stores, checks: &mut Vec<Value>) {
    stores.readiness().await.unwrap();
    check(
        checks,
        "aggregate fixed-role native readiness verifies identity, schema and blank scope",
    );
}

pub(super) async fn read_only_and_reuse(
    stores: &Stores,
    make_request: impl Fn(&str, &str, i64) -> TrustedRequest,
    checks: &mut Vec<Value>,
) {
    let request = make_request(ORG, PRINCIPAL, 1);
    let other = make_request(OTHER, PRINCIPAL, 1);
    let reviewer = make_request(ORG, REVIEWER, 1);
    assert!(stores
        .workspace
        .read_publication(&request, APP, PUBLICATION)
        .await
        .is_ok());
    assert!(stores
        .workspace
        .read_publication(&other, APP, PUBLICATION)
        .await
        .is_ok());
    assert!(stores
        .control
        .read_publication(&reviewer, APP, PUBLICATION)
        .await
        .is_ok());
    assert!(stores
        .data
        .read_record(&request, APP, WORK, COLLECTION, RECORD)
        .await
        .is_ok());
    assert!(stores
        .data
        .read_record(&other, APP, WORK, COLLECTION, RECORD)
        .await
        .is_ok());
    check(
        checks,
        "TLS role identity and scoped two-organization publication/record reads",
    );
    for pool in [
        &stores.workspace.inner,
        &stores.control.inner,
        &stores.data.inner,
        &stores.worker.inner,
    ] {
        let tx = Transaction::begin(pool, &request).await.unwrap();
        let rows = tx
            .query("SELECT organization_id::text FROM genui.organizations", &[])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<_, String>(0), ORG);
        assert!(tx
            .batch("UPDATE genui.organizations SET status='suspended'")
            .await
            .is_err());
        tx.finish(false).await.unwrap();
        let lease = Lease::acquire(pool).await.unwrap();
        assert_eq!(
            lease
                .client()
                .query_one("SELECT count(*) FROM genui.organizations", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        lease
            .finish(false, Instant::now() + Duration::from_secs(3))
            .await
            .unwrap();
    }
    check(
        checks,
        "all four fixed roles enforce RLS, blank scope and direct write refusal",
    );
    for (principal, epoch) in [
        ("00000000-0000-4000-8000-00000000006e", 2),
        ("00000000-0000-4000-8000-00000000006f", 1),
        (PRINCIPAL, 2),
    ] {
        let invalid = make_request(ORG, principal, epoch);
        assert_eq!(
            stores
                .workspace
                .read_publication(&invalid, APP, PUBLICATION)
                .await,
            Err(Error::Denied)
        );
        assert_eq!(
            stores
                .data
                .read_record(&invalid, APP, WORK, COLLECTION, RECORD)
                .await,
            Err(Error::Denied)
        );
    }
    let mut invalid = make_request(ORG, PRINCIPAL, 1);
    invalid.provider_tenant = format!("ten_{OTHER}");
    assert_eq!(
        stores
            .workspace
            .read_publication(&invalid, APP, PUBLICATION)
            .await,
        Err(Error::Denied)
    );
    invalid = make_request(ORG, PRINCIPAL, 1);
    invalid.provider_subject = "usr_unmapped".into();
    assert_eq!(
        stores
            .workspace
            .read_publication(&invalid, APP, PUBLICATION)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .control
            .read_publication(&request, APP, PUBLICATION)
            .await,
        Err(Error::Denied)
    );
    check(
        checks,
        "revocation, epoch, missing grants and registry/DB binding mismatches deny",
    );
    for action in [Action::Discover, Action::Run, Action::EditDefinition] {
        let mut tx = Transaction::begin(&stores.workspace.inner, &request)
            .await
            .unwrap();
        let pre = tx.precheck(action, Resource::App(APP)).await.unwrap();
        let _permit = tx.fence(pre).await.unwrap();
        tx.finish(true).await.unwrap();
    }
    for action in [Action::RecordRead, Action::RecordWrite, Action::ShareWork] {
        let mut tx = Transaction::begin(&stores.workspace.inner, &request)
            .await
            .unwrap();
        let pre = tx
            .precheck(
                action,
                Resource::Work {
                    app: APP,
                    work: WORK,
                },
            )
            .await
            .unwrap();
        let _permit = tx.fence(pre).await.unwrap();
        tx.finish(true).await.unwrap();
    }
    let mut tx = Transaction::begin(&stores.workspace.inner, &request)
        .await
        .unwrap();
    let pre = tx
        .precheck(
            Action::SourceRead,
            Resource::Source("00000000-0000-4000-8000-00000000006e"),
        )
        .await
        .unwrap();
    let _permit = tx.fence(pre).await.unwrap();
    tx.finish(true).await.unwrap();
    let admin = make_request(ORG, "00000000-0000-4000-8000-000000000f00", 1);
    let mut tx = Transaction::begin(&stores.workspace.inner, &admin)
        .await
        .unwrap();
    let pre = tx
        .precheck(Action::AdminPolicy, Resource::Organization)
        .await
        .unwrap();
    let _permit = tx.fence(pre).await.unwrap();
    tx.finish(true).await.unwrap();
    let tx = Transaction::begin(&stores.workspace.inner, &request)
        .await
        .unwrap();
    assert!(matches!(
        tx.precheck(
            Action::Export,
            Resource::Work {
                app: APP,
                work: WORK
            }
        )
        .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        tx.precheck(Action::AdminPolicy, Resource::Organization)
            .await,
        Err(Error::Denied)
    ));
    tx.finish(false).await.unwrap();
    check(
        checks,
        "closed action matrix evaluates current scoped grants without granting business writes",
    );
    // Same physical backend is demonstrated, rather than inferred from a pool count.
    let tx = Transaction::begin(&stores.data.inner, &request)
        .await
        .unwrap();
    let first = pid(&tx.lease).await;
    tx.batch("SELECT pg_advisory_lock(888008); LISTEN app008_fixture; SET SESSION row_security=off; SET SESSION search_path=public; SELECT set_config('genui.organization_id','poison',false); SET ROLE rds_iam").await.unwrap();
    tx.finish(true).await.unwrap();
    let tx = Transaction::begin(&stores.data.inner, &other)
        .await
        .unwrap();
    assert_eq!(pid(&tx.lease).await, first);
    let facts=tx.query("SELECT current_setting('row_security'),current_setting('search_path'),(SELECT count(*) FROM pg_listening_channels()),(SELECT count(*) FROM pg_locks WHERE pid=pg_backend_pid() AND locktype='advisory'),current_setting('genui.organization_id')",&[]).await.unwrap();
    assert_eq!(facts[0].get::<_, String>(0), "on");
    assert_eq!(facts[0].get::<_, String>(1), "pg_catalog, pg_temp");
    assert_eq!(facts[0].get::<_, i64>(2), 0);
    assert_eq!(facts[0].get::<_, i64>(3), 0);
    assert_eq!(facts[0].get::<_, String>(4), OTHER);
    assert!(tx.query("SELECT 1/0", &[]).await.is_err());
    tx.finish(false).await.unwrap();
    let tx = Transaction::begin(&stores.data.inner, &request)
        .await
        .unwrap();
    assert_eq!(pid(&tx.lease).await, first);
    tx.finish(true).await.unwrap();
    check(checks,"same-backend commit/error-rollback reuse clears poisoned role/settings, listeners and advisory locks");
}
