//! APP-073 admission campaign. Isolated test executable only: local Docker,
//! OpenSSL, a throwaway CA and the locked Python environment, exactly as the
//! APP-008 native campaign. Admission runs through the storage module as
//! genui_workspace over verified TLS; issuance, listing, revocation, extension
//! and backfill run through the real operator command
//! (`app/runtime/admission_operator.py`) as genui_migration over verify-full
//! TLS. Fixture-only operator SQL (the local superuser) is used for the
//! writer-first race orders and for moving clocks, never as a product path.
use super::{
    admission::{
        code_digest, Admission, PendingAdmission, PendingReason, Reservation, SignOut,
        VerifiedNonmember,
    },
    authorization::{Resource, Transaction},
    pool::{Lease, TestHooks},
    qualification::{docker, fixture_python, Fixture},
    qualification_cases::{check, APP, COLLECTION, ORG, OTHER, PUBLICATION, RECORD, WORK},
    *,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::Arc,
    time::{Duration, Instant as WallInstant},
};
use tokio::time::timeout;

const ISSUER: &str = "app008-fixture-only";

fn sha(label: &str) -> String {
    let hash = Sha256::digest(label.as_bytes());
    format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

fn nonmember(stores: &Stores, organization: &str, label: &str) -> VerifiedNonmember {
    VerifiedNonmember {
        issuer: ISSUER.into(),
        provider_tenant: format!("ten_{organization}"),
        provider_subject: format!("usr_admission_{label}"),
        registry_sha256: stores.registry_identity().sha256.clone(),
        email_verified: true,
    }
}

fn member(stores: &Stores, label: &str, principal: &str, epoch: i64) -> TrustedRequest {
    TrustedRequest {
        organization: ORG.into(),
        principal: principal.into(),
        epoch,
        issuer: ISSUER.into(),
        provider_tenant: format!("ten_{ORG}"),
        provider_subject: format!("usr_admission_{label}"),
        registry_sha256: stores.registry_identity().sha256.clone(),
    }
}

/// The operator command as an operator runs it: environment-only connection
/// settings, genui_migration (or a forged identity) over verify-full TLS.
fn cli_command(fixture: &Fixture, user: &str, password: &str, args: &[&str]) -> Command {
    let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cmd = Command::new(fixture_python());
    cmd.current_dir(app.join("runtime"))
        .arg("admission_operator.py")
        .args(args)
        .env("PGHOST", "localhost")
        .env("PGPORT", fixture.port.to_string())
        .env("PGUSER", user)
        .env("PGPASSWORD", password)
        .env("PGDATABASE", "genui")
        .env("PGSSLROOTCERT", &fixture.ca)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}
fn wait(child: Child, label: &str) -> Output {
    let end = WallInstant::now() + Duration::from_secs(45);
    let mut child = child;
    while child.try_wait().expect("operator command status").is_none() {
        if WallInstant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("operator command deadline: {label}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().expect("operator command output")
}
fn operator_as(fixture: &Fixture, user: &str, password: &str, args: &[&str]) -> Output {
    wait(
        cli_command(fixture, user, password, args)
            .spawn()
            .expect("operator command available"),
        args[0],
    )
}
fn accepted(output: Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "operator command refused: {label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "operator stderr not empty: {label}"
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8 operator output");
    assert_eq!(text.matches('\n').count(), 1, "one JSON line: {label}");
    serde_json::from_str(&text).expect("operator JSON")
}
fn operator(fixture: &Fixture, args: &[&str]) -> Value {
    accepted(
        operator_as(fixture, "genui_migration", "synthetic-migration", args),
        args[0],
    )
}
fn refused(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
/// A refusal for the stated fixed reason code (never a value).
fn refused_for(output: &Output, reason: &str) {
    refused(output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).expect("refusal JSON"),
        json!({ "refused": reason })
    );
}

/// The whole database (or its data) as the local superuser sees it; read
/// concurrently, since a dump outgrows a pipe buffer.
fn pg_dump(fixture: &Fixture, data_only: bool) -> String {
    let mut cmd = Command::new("docker");
    cmd.args([
        "exec",
        &fixture.name,
        "pg_dump",
        "--username=postgres",
        "--dbname=genui",
    ]);
    if data_only {
        cmd.arg("--data-only");
    }
    let output = cmd
        .stdin(Stdio::null())
        .output()
        .expect("pg_dump available");
    assert!(output.status.success(), "pg_dump refused");
    String::from_utf8(output.stdout).expect("UTF-8 dump")
}
async fn superuser(fixture: &Fixture, sql: &str) -> Vec<tokio_postgres::Row> {
    let operator = fixture.operator().await;
    operator.client.query(sql, &[]).await.unwrap()
}
async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    superuser(fixture, sql).await[0].get::<_, i64>(0)
}
/// Observe an actual PostgreSQL lock wait, not a sleep.
async fn lock_wait(fixture: &Fixture, waiters: i64) {
    let operator = fixture.operator().await;
    timeout(Duration::from_secs(10), async {
        loop {
            let row = operator.client.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname='genui' AND wait_event_type='Lock'", &[]).await.unwrap();
            if row.get::<_, i64>(0) >= waiters {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("observed lock wait");
}
async fn paused(hooks: &TestHooks) {
    timeout(Duration::from_secs(5), hooks.reached.notified())
        .await
        .expect("admission barrier reached");
}

struct Issued {
    code_id: String,
    code: String,
    digest: String,
}
fn issue(fixture: &Fixture, max: &str, grants: &[String]) -> Issued {
    let mut args = vec![
        "issue",
        "--organization",
        ORG,
        "--issued-by",
        "admission-campaign",
        "--max-redemptions",
        max,
    ];
    for grant in grants {
        args.push("--grant");
        args.push(grant);
    }
    let out = operator(fixture, &args);
    let code = out["code"].as_str().unwrap().to_owned();
    Issued {
        code_id: out["code_id"].as_str().unwrap().to_owned(),
        digest: code_digest(&code).expect("issued code normalizes"),
        code,
    }
}
fn listed(fixture: &Fixture, code_id: &str) -> Value {
    let out = operator(fixture, &["list", "--organization", ORG]);
    out["codes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code_id"] == code_id)
        .expect("listed code")
        .clone()
}

async fn reserved(stores: &Stores, digest: &str, context: &str) -> String {
    match stores
        .workspace
        .reserve_code(ORG, digest, &sha(context))
        .await
        .unwrap()
    {
        Reservation::Reserved {
            reservation,
            expires_at,
        } => {
            assert!(expires_at > 0);
            reservation
        }
        other => panic!("reservation refused: {other:?}"),
    }
}
async fn pending(stores: &Stores, identity: &VerifiedNonmember, reservation: &str) -> String {
    match stores
        .workspace
        .record_pending_admission(identity, ORG, reservation)
        .await
        .unwrap()
    {
        PendingAdmission::Pending {
            pending_admission, ..
        } => pending_admission,
        other => panic!("pending admission refused: {other:?}"),
    }
}
async fn admitted(
    stores: &Stores,
    identity: &VerifiedNonmember,
    pending_admission: &str,
    payload: &str,
) -> (String, i64, String) {
    match stores
        .workspace
        .admit(identity, ORG, pending_admission, payload)
        .await
        .unwrap()
    {
        Admission::Admitted {
            principal,
            epoch,
            entitlement,
        } => (principal, epoch, entitlement),
        other => panic!("admission refused: {other:?}"),
    }
}
/// One full invited path for a fresh identity: reserve, pending, admit.
async fn admit_new(stores: &Stores, digest: &str, label: &str) -> (String, i64, String) {
    let identity = nonmember(stores, ORG, label);
    let reservation = reserved(stores, digest, label).await;
    let pending_admission = pending(stores, &identity, &reservation).await;
    admitted(stores, &identity, &pending_admission, &sha(label)).await
}
async fn reads(stores: &Stores, request: &TrustedRequest) -> bool {
    stores
        .workspace
        .read_publication(request, APP, PUBLICATION)
        .await
        .is_ok()
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_admission_campaign() {
    std::thread::Builder::new()
        .name("app073-admission-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn campaign() {
    let mut fixture = Fixture::new();
    // Local password authentication for the operator identity over TLS; it is
    // explicitly not the managed migration secret.
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    let mut codes = Vec::new();

    // --- D7.17 defaults are recorded before any code is issued -------------
    let defaults = operator(&fixture, &["defaults"]);
    assert_eq!(
        defaults,
        json!({"trial_entitlement_days":30,"code_expiry_days_default":14,"code_expiry_days_max":30,
            "max_redemptions_default":1,"max_redemptions_max":10,"presignup_reservation_minutes":10,
            "presignup_reservations_live_max":10,"pending_admission_hours":24,"list_rows_max":100,
            "code_secret_bytes":32,"handle_bytes":16})
    );
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM genui.access_codes").await,
        0
    );
    check(
        &mut checks,
        "operator defaults (D7.17) recorded before any code exists",
    );
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["backfill", "--organization", org]);
        assert_eq!(out["created"].as_array().unwrap().len(), 5);
        let again = operator(&fixture, &["backfill", "--organization", org]);
        assert!(again["created"].as_array().unwrap().is_empty());
    }
    check(
        &mut checks,
        "operator backfill creates the five genesis audit heads for both existing organizations once",
    );

    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    let sources: Vec<String> = superuser(
        &fixture,
        &format!("SELECT source_id::text FROM genui.sources WHERE organization_id='{ORG}' ORDER BY source_id"),
    )
    .await
    .iter()
    .map(|row| row.get(0))
    .collect();
    let mut grants = vec![
        format!("discover:{APP}"),
        format!("run:{APP}"),
        format!("record_read:{APP}:{WORK}"),
    ];
    grants.extend(sources.iter().map(|s| format!("source_read:{s}")));

    // --- issuance, digest-only storage, list --------------------------------
    let a = issue(&fixture, "1", &grants);
    codes.push(a.code.clone());
    let dump = pg_dump(&fixture, true);
    assert!(!dump.contains(&a.code) && !dump.contains(&a.code.replace('-', "")));
    assert!(dump.contains(&a.digest));
    let entry = listed(&fixture, &a.code_id);
    assert_eq!(
        (
            entry["status"].as_str(),
            entry["redeemed_count"].as_i64(),
            entry["max_redemptions"].as_i64()
        ),
        (Some("active"), Some(0), Some(1))
    );
    assert!(!entry.to_string().contains(&a.digest));
    check(&mut checks, "operator issues a 256-bit code shown once on stdout; storage, dump and list hold only its digest, identifiers, status and counts");

    // --- pre-scope lookup ----------------------------------------------------
    assert_eq!(
        stores.workspace.resolve_code(&a.digest).await,
        Ok(Some(ORG.into()))
    );
    assert_eq!(
        stores.workspace.resolve_code(&sha("unknown-code")).await,
        Ok(None)
    );
    assert_eq!(
        stores.workspace.resolve_code("not-a-digest").await,
        Ok(None)
    );
    assert_eq!(code_digest("not a code"), None);
    check(
        &mut checks,
        "resolve_code before scope: known digest names its organization; unknown and malformed resolve to nothing",
    );

    // --- pre-signup validation consumes no slot ------------------------------
    let mut reservations = Vec::new();
    for i in 0..3 {
        reservations.push(reserved(&stores, &a.digest, &format!("a-presignup-{i}")).await);
    }
    assert_eq!(listed(&fixture, &a.code_id)["redeemed_count"], 0);
    assert_eq!(
        stores
            .workspace
            .reserve_code(OTHER, &a.digest, &sha("foreign"))
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .reserve_code("00000000-0000-0000-0000-000000000001", &a.digest, &sha("x"))
            .await,
        Err(Error::Denied)
    );
    check(&mut checks, "three pre-signup reservations consume no redemption slot; a foreign or malformed organization refuses without disclosure");

    // --- pending admission and the verified-mailbox gate ---------------------
    let x = nonmember(&stores, ORG, "x");
    let mut foreign = nonmember(&stores, ORG, "x");
    foreign.provider_tenant = format!("ten_{OTHER}");
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&foreign, ORG, &reservations[0])
            .await,
        Err(Error::Denied)
    );
    let mut forged = nonmember(&stores, ORG, "x");
    forged.registry_sha256 = "0".repeat(64);
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&forged, ORG, &reservations[0])
            .await,
        Err(Error::Denied)
    );
    let x_pending = pending(&stores, &x, &reservations[0]).await;
    let mut unverified = nonmember(&stores, ORG, "x");
    unverified.email_verified = false;
    assert_eq!(
        stores
            .workspace
            .admit(&unverified, ORG, &x_pending, &sha("x"))
            .await,
        Ok(Admission::Pending {
            reason: PendingReason::EmailUnverified
        })
    );
    assert_eq!(listed(&fixture, &a.code_id)["redeemed_count"], 0);
    assert_eq!(listed(&fixture, &a.code_id)["live_pending_admissions"], 1);
    check(&mut checks, "pending admission requires the registry-bound verified identity; an unverified mailbox commits no slot, membership or entitlement");

    // --- rollback: a cancelled redemption leaves nothing ---------------------
    hooks.arm("admit");
    let x_payload = sha("x");
    {
        let operation = stores.workspace.admit(&x, ORG, &x_pending, &x_payload);
        tokio::pin!(operation);
        tokio::select! {_=&mut operation=>panic!("admit barrier not reached"),_=paused(&hooks)=>{}}
    }
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.principals WHERE organization_id='{ORG}' AND provider_subject='usr_admission_x'")).await, 0);
    assert_eq!(listed(&fixture, &a.code_id)["redeemed_count"], 0);
    check(
        &mut checks,
        "a redemption cancelled before commit rolls back: no principal, slot or audit event",
    );

    // --- atomic admission, identical retry, mismatches -----------------------
    let (x_principal, x_epoch, x_entitlement) = admitted(&stores, &x, &x_pending, &sha("x")).await;
    assert_eq!(x_epoch, 1);
    let facts = superuser(&fixture, &format!("SELECT (SELECT count(*) FROM genui.memberships WHERE principal_id='{x_principal}' AND status='active' AND membership_epoch=1), (SELECT count(*) FROM genui.entitlements WHERE entitlement_id='{x_entitlement}' AND status='active' AND expires_at-starts_at=interval '30 days'), (SELECT count(*) FROM genui.code_redemptions WHERE principal_id='{x_principal}'), (SELECT count(*) FROM genui.app_permissions WHERE principal_id='{x_principal}'), (SELECT count(*) FROM genui.audit_events WHERE actor_principal_id='{x_principal}' AND kind='admission.redeemed')")).await;
    let facts: Vec<i64> = (0..5).map(|i| facts[0].get(i)).collect();
    assert_eq!(facts, vec![1, 1, 1, 2, 1]);
    let entry = listed(&fixture, &a.code_id);
    assert_eq!(
        (entry["redeemed_count"].as_i64(), entry["status"].as_str()),
        (Some(1), Some("exhausted"))
    );
    let x_member = member(&stores, "x", &x_principal, 1);
    assert!(reads(&stores, &x_member).await);
    assert!(stores
        .data
        .read_record(&x_member, APP, WORK, COLLECTION, RECORD)
        .await
        .is_ok());
    check(&mut checks, "verified redemption commits principal, epoch-1 membership, template grants, 30-day entitlement, redemption, audit and exhaustion atomically; the member reads");
    assert_eq!(
        admitted(&stores, &x, &x_pending, &sha("x")).await,
        (x_principal.clone(), 1, x_entitlement.clone())
    );
    assert_eq!(listed(&fixture, &a.code_id)["redeemed_count"], 1);
    assert_eq!(
        stores
            .workspace
            .admit(&x, ORG, &x_pending, &sha("x-changed"))
            .await,
        Ok(Admission::Refused)
    );
    let y = nonmember(&stores, ORG, "y");
    assert_eq!(
        stores.workspace.admit(&y, ORG, &x_pending, &sha("x")).await,
        Ok(Admission::Refused)
    );
    assert_eq!(
        stores
            .workspace
            .reserve_code(ORG, &a.digest, &sha("late"))
            .await,
        Ok(Reservation::Exhausted)
    );
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&x, ORG, &reservations[1])
            .await,
        Ok(PendingAdmission::Exhausted)
    );
    check(&mut checks, "identical retry returns the stored admission without a second slot; changed payload or another principal refuses; the exhausted code refuses new holds");

    // --- restart on the actual database --------------------------------------
    stores.close().await.unwrap();
    drop(stores);
    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    assert_eq!(
        admitted(&stores, &x, &x_pending, &sha("x")).await,
        (x_principal.clone(), 1, x_entitlement.clone())
    );
    assert!(reads(&stores, &x_member).await);
    check(
        &mut checks,
        "after a store restart the admission persists and its identical retry still replays",
    );

    // --- entitlement expiry between precheck and fence -----------------------
    let mut tx = Transaction::begin(&stores.data.inner, &x_member)
        .await
        .unwrap();
    let pre = tx
        .precheck(
            Action::RecordRead,
            Resource::Work {
                app: APP,
                work: WORK,
            },
        )
        .await
        .unwrap();
    superuser(&fixture, &format!("UPDATE genui.entitlements SET expires_at=starts_at+interval '1 millisecond' WHERE entitlement_id='{x_entitlement}'")).await;
    assert!(matches!(tx.fence(pre).await, Err(Error::Denied)));
    tx.finish(false).await.unwrap();
    assert!(!reads(&stores, &x_member).await);
    assert_eq!(
        stores
            .data
            .read_record(&x_member, APP, WORK, COLLECTION, RECORD)
            .await,
        Err(Error::Denied)
    );
    // Fixture-only restoration so the revocation order below starts active.
    superuser(&fixture, &format!("UPDATE genui.entitlements SET expires_at=starts_at+interval '30 days' WHERE entitlement_id='{x_entitlement}'")).await;
    assert!(reads(&stores, &x_member).await);
    check(&mut checks, "an entitlement that expires after the precheck is refused by the fence's re-read, and an expired entitlement denies the next precheck");

    // --- entitlement use vs revocation: operation first (operator command) ---
    let mut tx = Transaction::begin(&stores.data.inner, &x_member)
        .await
        .unwrap();
    let pre = tx
        .precheck(
            Action::RecordRead,
            Resource::Work {
                app: APP,
                work: WORK,
            },
        )
        .await
        .unwrap();
    // The fence's member FOR SHARE lock is held until this transaction ends.
    let _permit = tx.fence(pre).await.unwrap();
    let revoke = cli_command(
        &fixture,
        "genui_migration",
        "synthetic-migration",
        &[
            "revoke-entitlement",
            "--organization",
            ORG,
            "--principal",
            &x_principal,
        ],
    )
    .spawn()
    .unwrap();
    lock_wait(&fixture, 1).await;
    tx.finish(true).await.unwrap();
    let out = accepted(
        tokio::task::spawn_blocking(move || wait(revoke, "revoke-entitlement"))
            .await
            .unwrap(),
        "revoke-entitlement",
    );
    assert_eq!(
        (out["changed"].as_bool(), out["membership_epoch"].as_i64()),
        (Some(true), Some(2))
    );
    assert!(!reads(&stores, &x_member).await);
    assert!(!reads(&stores, &member(&stores, "x", &x_principal, 2)).await);
    check(&mut checks, "operation-first: the operator's entitlement revocation waits on the member fence until the operation commits; the next request (old or new epoch) is refused");
    // Review m2: the identical retry still finds the stored redemption, but the
    // revoked entitlement makes it Refused, never a late Admitted.
    assert_eq!(
        stores.workspace.admit(&x, ORG, &x_pending, &sha("x")).await,
        Ok(Admission::Refused)
    );
    check(
        &mut checks,
        "an identical retry after the member's entitlement was revoked is refused, not replayed as admitted",
    );

    // --- code revocation and entitlement revocation have distinct effects ---
    let b = issue(&fixture, "4", &grants);
    codes.push(b.code.clone());
    let (w_principal, _, w_entitlement) = admit_new(&stores, &b.digest, "w").await;
    let w_member = member(&stores, "w", &w_principal, 1);
    let z = nonmember(&stores, ORG, "z");
    let z_reservation = reserved(&stores, &b.digest, "z").await;
    let z_pending = pending(&stores, &z, &z_reservation).await;
    let out = operator(
        &fixture,
        &["revoke", "--organization", ORG, "--code-id", &b.code_id],
    );
    assert_eq!(out["changed"], true);
    assert_eq!(
        stores.workspace.admit(&z, ORG, &z_pending, &sha("z")).await,
        Ok(Admission::Refused)
    );
    assert_eq!(
        stores
            .workspace
            .reserve_code(ORG, &b.digest, &sha("after-revoke"))
            .await,
        Ok(Reservation::Revoked)
    );
    assert!(reads(&stores, &w_member).await);
    check(&mut checks, "code revocation blocks unused pending admissions and new reservations while an already-admitted member's entitlement stays active");

    // --- entitlement use vs revocation: revocation first ----------------------
    let writer = fixture.operator().await;
    writer.client.batch_execute(&format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{w_principal}' FOR UPDATE; UPDATE genui.entitlements SET status='revoked',revoked_at=now() WHERE entitlement_id='{w_entitlement}'; UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id='{ORG}' AND principal_id='{w_principal}';")).await.unwrap();
    let mut tx = Transaction::begin(&stores.workspace.inner, &w_member)
        .await
        .unwrap();
    let pre = tx
        .precheck(Action::Discover, Resource::App(APP))
        .await
        .unwrap();
    {
        let fence = tx.fence(pre);
        tokio::pin!(fence);
        tokio::select! {_=&mut fence=>panic!("fence bypassed the operator revocation"),_=lock_wait(&fixture, 1)=>{}}
        writer.client.batch_execute("COMMIT").await.unwrap();
        assert!(matches!(fence.await, Err(Error::Denied)));
    }
    tx.finish(false).await.unwrap();
    drop(writer);
    assert!(!reads(&stores, &w_member).await);
    assert!(!reads(&stores, &member(&stores, "w", &w_principal, 2)).await);
    check(&mut checks, "revocation-first: the fence waits behind the operator's entitlement revocation and then refuses; entitlement revocation alone denies the member");

    // --- redemption vs code revocation: redemption first ---------------------
    let c = issue(&fixture, "3", &grants);
    codes.push(c.code.clone());
    let u = nonmember(&stores, ORG, "u");
    let u_reservation = reserved(&stores, &c.digest, "u").await;
    let u_pending = pending(&stores, &u, &u_reservation).await;
    hooks.arm("admit");
    let redemption = {
        let stores = stores.clone();
        let (u_pending, payload) = (u_pending.clone(), sha("u"));
        let u = nonmember(&stores, ORG, "u");
        tokio::spawn(async move { stores.workspace.admit(&u, ORG, &u_pending, &payload).await })
    };
    paused(&hooks).await;
    let revoke = cli_command(
        &fixture,
        "genui_migration",
        "synthetic-migration",
        &["revoke", "--organization", ORG, "--code-id", &c.code_id],
    )
    .spawn()
    .unwrap();
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let outcome = redemption.await.unwrap().unwrap();
    let Admission::Admitted {
        principal: u_principal,
        ..
    } = outcome
    else {
        panic!("redemption-first did not admit: {outcome:?}");
    };
    let out = accepted(
        tokio::task::spawn_blocking(move || wait(revoke, "revoke"))
            .await
            .unwrap(),
        "revoke",
    );
    assert_eq!(out["changed"], true);
    let entry = listed(&fixture, &c.code_id);
    assert_eq!(
        (entry["status"].as_str(), entry["redeemed_count"].as_i64()),
        (Some("revoked"), Some(1))
    );
    let u_member = member(&stores, "u", &u_principal, 1);
    assert!(reads(&stores, &u_member).await);
    check(&mut checks, "redemption-first: the operator's code revocation waits on the code row until the redemption commits; the admission stands and the code is then revoked");

    // --- redemption vs code revocation: revocation first ---------------------
    let d = issue(&fixture, "3", &grants);
    codes.push(d.code.clone());
    let v = nonmember(&stores, ORG, "v");
    let v_reservation = reserved(&stores, &d.digest, "v").await;
    let v_pending = pending(&stores, &v, &v_reservation).await;
    let writer = fixture.operator().await;
    writer.client.batch_execute(&format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; UPDATE genui.access_codes SET status='revoked',revoked_at=now() WHERE organization_id='{ORG}' AND code_id='{}';", d.code_id)).await.unwrap();
    let redemption = {
        let stores = stores.clone();
        let (v_pending, payload) = (v_pending.clone(), sha("v"));
        let v = nonmember(&stores, ORG, "v");
        tokio::spawn(async move { stores.workspace.admit(&v, ORG, &v_pending, &payload).await })
    };
    lock_wait(&fixture, 1).await;
    writer.client.batch_execute("COMMIT").await.unwrap();
    drop(writer);
    assert_eq!(redemption.await.unwrap(), Ok(Admission::Refused));
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.principals WHERE organization_id='{ORG}' AND provider_subject='usr_admission_v'")).await, 0);
    check(&mut checks, "revocation-first: the redemption waits on the code row behind the revocation and then refuses with no principal, membership or entitlement");

    // --- two principals race for the last slot -------------------------------
    let e = issue(&fixture, "1", &grants);
    codes.push(e.code.clone());
    let p1 = nonmember(&stores, ORG, "p1");
    let p2 = nonmember(&stores, ORG, "p2");
    let r1 = reserved(&stores, &e.digest, "p1").await;
    let r2 = reserved(&stores, &e.digest, "p2").await;
    hooks.arm("pending");
    let first = {
        let stores = stores.clone();
        let r1 = r1.clone();
        let p1 = nonmember(&stores, ORG, "p1");
        tokio::spawn(async move {
            stores
                .workspace
                .record_pending_admission(&p1, ORG, &r1)
                .await
        })
    };
    paused(&hooks).await;
    let second = {
        let stores = stores.clone();
        let r2 = r2.clone();
        let p2 = nonmember(&stores, ORG, "p2");
        tokio::spawn(async move {
            stores
                .workspace
                .record_pending_admission(&p2, ORG, &r2)
                .await
        })
    };
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let PendingAdmission::Pending {
        pending_admission: p1_pending,
        ..
    } = first.await.unwrap().unwrap()
    else {
        panic!("first holder refused");
    };
    assert_eq!(second.await.unwrap(), Ok(PendingAdmission::Exhausted));
    admitted(&stores, &p1, &p1_pending, &sha("p1")).await;
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&p2, ORG, &r2)
            .await,
        Ok(PendingAdmission::Exhausted)
    );
    let entry = listed(&fixture, &e.code_id);
    assert_eq!(
        (entry["redeemed_count"].as_i64(), entry["status"].as_str()),
        (Some(1), Some("exhausted"))
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.principals WHERE organization_id='{ORG}' AND provider_subject IN ('usr_admission_p1','usr_admission_p2')")).await, 1);
    check(&mut checks, "two principals racing for the last slot (observed lock wait): exactly one holds it and is admitted, the other is refused as exhausted");

    // --- concurrent identical retries consume one slot -----------------------
    let f = issue(&fixture, "1", &grants);
    codes.push(f.code.clone());
    let q = nonmember(&stores, ORG, "q");
    let q_reservation = reserved(&stores, &f.digest, "q").await;
    let q_pending = pending(&stores, &q, &q_reservation).await;
    hooks.arm("admit");
    let spawn_admit = |stores: Arc<Stores>, q_pending: String| {
        tokio::spawn(async move {
            let q = nonmember(&stores, ORG, "q");
            stores.workspace.admit(&q, ORG, &q_pending, &sha("q")).await
        })
    };
    let first = spawn_admit(stores.clone(), q_pending.clone());
    paused(&hooks).await;
    let second = spawn_admit(stores.clone(), q_pending.clone());
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let first = first.await.unwrap().unwrap();
    let second = second.await.unwrap().unwrap();
    let q_principal = match &first {
        Admission::Admitted { principal, .. } => principal.clone(),
        other => panic!("concurrent identical redemption did not admit: {other:?}"),
    };
    assert_eq!(first, second);
    assert_eq!(listed(&fixture, &f.code_id)["redeemed_count"], 1);
    check(&mut checks, "two concurrent identical redemptions return one stored admission and consume exactly one slot");

    // --- review m2: an identical retry racing a membership revocation --------
    hooks.arm("admit");
    let replay = spawn_admit(stores.clone(), q_pending.clone());
    paused(&hooks).await;
    // The definer has answered `replayed` from the stored redemption; the
    // operator's membership revocation commits before the retry's recheck.
    let out = operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            &q_principal,
        ],
    );
    assert_eq!(out["changed"], true);
    hooks.release.notify_one();
    assert_eq!(replay.await.unwrap(), Ok(Admission::Refused));
    assert_eq!(listed(&fixture, &f.code_id)["redeemed_count"], 1);
    check(&mut checks, "a membership revocation that commits while an identical retry is in flight (after the stored redemption was found) makes the retry refused; no slot is consumed");

    // --- expiry and exhaustion refusals; bounded extension -------------------
    let g = issue(&fixture, "2", &grants);
    codes.push(g.code.clone());
    let s = nonmember(&stores, ORG, "s");
    let s_reservation = reserved(&stores, &g.digest, "s").await;
    superuser(&fixture, &format!("UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE reservation_id='{s_reservation}'")).await;
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&s, ORG, &s_reservation)
            .await,
        Ok(PendingAdmission::ReservationExpired)
    );
    let s_reservation = reserved(&stores, &g.digest, "s-again").await;
    let s_pending = pending(&stores, &s, &s_reservation).await;
    superuser(&fixture, &format!("UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE reservation_id='{s_pending}'")).await;
    assert_eq!(
        stores.workspace.admit(&s, ORG, &s_pending, &sha("s")).await,
        Ok(Admission::Refused)
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.code_reservations WHERE reservation_id='{s_pending}' AND state='denied_for_reconciliation'")).await, 1);
    let out = operator(
        &fixture,
        &[
            "extend",
            "--organization",
            ORG,
            "--code-id",
            &g.code_id,
            "--expires-days",
            "20",
        ],
    );
    assert_eq!(out["changed"], true);
    superuser(&fixture, &format!("UPDATE genui.access_codes SET expires_at=issued_at+interval '1 millisecond' WHERE code_id='{}'", g.code_id)).await;
    assert_eq!(
        stores
            .workspace
            .reserve_code(ORG, &g.digest, &sha("expired"))
            .await,
        Ok(Reservation::Expired)
    );
    refused(&operator_as(
        &fixture,
        "genui_migration",
        "synthetic-migration",
        &[
            "extend",
            "--organization",
            ORG,
            "--code-id",
            &g.code_id,
            "--expires-days",
            "5",
        ],
    ));
    assert_eq!(listed(&fixture, &g.code_id)["status"], "expired");
    check(&mut checks, "reservation expiry, pending-admission expiry (denied for reconciliation) and code expiry refuse; extension is audited and an expired code is never resurrected");

    // --- sign-out advances the member's own epoch -----------------------------
    assert_eq!(
        stores
            .workspace
            .advance_epoch(&u_member, SignOut::One)
            .await,
        Ok(2)
    );
    assert!(!reads(&stores, &u_member).await);
    let u_epoch2 = member(&stores, "u", &u_principal, 2);
    assert!(reads(&stores, &u_epoch2).await);
    assert_eq!(
        stores
            .workspace
            .advance_epoch(&u_member, SignOut::All)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .advance_epoch(&u_epoch2, SignOut::All)
            .await,
        Ok(3)
    );
    // Review m1: two concurrent sign-outs of one member queue on the member row
    // (advance_member_epoch is the fence) instead of deadlocking.
    let u_epoch3 = member(&stores, "u", &u_principal, 3);
    hooks.arm("sign_out");
    let spawn_sign_out = |stores: Arc<Stores>, principal: String| {
        tokio::spawn(async move {
            let request = member(&stores, "u", &principal, 3);
            stores.workspace.advance_epoch(&request, SignOut::One).await
        })
    };
    let first = spawn_sign_out(stores.clone(), u_principal.clone());
    paused(&hooks).await;
    let second = spawn_sign_out(stores.clone(), u_principal.clone());
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    assert_eq!(first.await.unwrap(), Ok(4));
    assert_eq!(second.await.unwrap(), Err(Error::Denied));
    assert!(!reads(&stores, &u_epoch3).await);
    check(&mut checks, "sign-out advances only the member's own current epoch; the stale request is refused afterwards; two concurrent sign-outs queue on the member row (one advances, one refuses) without deadlock");

    // --- review m3: an inactive provider binding refuses admission -----------
    let h = issue(&fixture, "2", &grants);
    codes.push(h.code.clone());
    let k = nonmember(&stores, ORG, "k");
    let k_reservation = reserved(&stores, &h.digest, "k").await;
    superuser(
        &fixture,
        &format!(
            "UPDATE genui.provider_tenant_bindings SET active=false WHERE organization_id='{ORG}'"
        ),
    )
    .await;
    assert_eq!(
        stores
            .workspace
            .record_pending_admission(&k, ORG, &k_reservation)
            .await,
        Err(Error::Denied)
    );
    superuser(
        &fixture,
        &format!(
            "UPDATE genui.provider_tenant_bindings SET active=true WHERE organization_id='{ORG}'"
        ),
    )
    .await;
    let k_pending = pending(&stores, &k, &k_reservation).await;
    superuser(
        &fixture,
        &format!(
            "UPDATE genui.provider_tenant_bindings SET active=false WHERE organization_id='{ORG}'"
        ),
    )
    .await;
    assert_eq!(
        stores.workspace.admit(&k, ORG, &k_pending, &sha("k")).await,
        Err(Error::Denied)
    );
    superuser(
        &fixture,
        &format!(
            "UPDATE genui.provider_tenant_bindings SET active=true WHERE organization_id='{ORG}'"
        ),
    )
    .await;
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.principals WHERE organization_id='{ORG}' AND provider_subject='usr_admission_k'")).await, 0);
    assert_eq!(listed(&fixture, &h.code_id)["redeemed_count"], 0);
    check(&mut checks, "an inactive provider binding refuses both the pending admission and the redemption before any definer call");

    // --- ordinary member roles and forged operator identities ----------------
    let before = scalar(&fixture, "SELECT count(*) FROM genui.access_codes").await;
    for (role, password) in [
        ("genui_workspace", "synthetic-workspace"),
        ("genui_control", "synthetic-control"),
        ("genui_data", "synthetic-data"),
        ("genui_worker", "synthetic-worker"),
    ] {
        let grant = grants[0].clone();
        refused_for(
            &operator_as(
                &fixture,
                role,
                password,
                &[
                    "issue",
                    "--organization",
                    ORG,
                    "--issued-by",
                    "forged",
                    "--grant",
                    &grant,
                ],
            ),
            "operator_identity_refused",
        );
        refused_for(
            &operator_as(
                &fixture,
                role,
                password,
                &["revoke", "--organization", ORG, "--code-id", &g.code_id],
            ),
            "operator_identity_refused",
        );
    }
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "wrong-password",
            &["list", "--organization", ORG],
        ),
        "admission_operator_refused",
    );
    for pool in [
        &stores.workspace.inner,
        &stores.control.inner,
        &stores.data.inner,
    ] {
        for statement in [
            "SET LOCAL ROLE genui_owner".to_owned(),
            format!(
                "INSERT INTO genui_private.code_index VALUES ('{}','{ORG}')",
                sha("forged")
            ),
            format!("UPDATE genui.access_codes SET redeemed_count=0 WHERE organization_id='{ORG}'"),
            format!("UPDATE genui.entitlements SET status='active' WHERE organization_id='{ORG}'"),
            format!(
                "INSERT INTO genui.audit_events(organization_id,event_id) VALUES ('{ORG}','{}')",
                "00000000-0000-4000-8000-0000000000ee"
            ),
        ] {
            let tx = Transaction::begin(pool, &u_epoch2).await.unwrap();
            let error = tx
                .lease
                .client()
                .batch_execute(&statement)
                .await
                .unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
                "{statement}"
            );
            tx.finish(false).await.unwrap();
        }
    }
    let lease = Lease::acquire(&stores.workspace.inner).await.unwrap();
    assert!(lease
        .client()
        .query("SELECT genui_private.resolve_code($1)", &[&"x"])
        .await
        .is_ok());
    lease
        .finish(false, tokio::time::Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM genui.access_codes").await,
        before
    );
    check(&mut checks, "application roles as forged operators are refused by the command and by PostgreSQL (42501); a wrong operator password is refused; nothing was written");

    // --- audit chains verify and no raw code reached storage ----------------
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["verify", "--organization", org]);
        assert_eq!(out["verified"], true);
    }
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    // Eight issuances, two revocations, one extension, one entitlement revocation,
    // one membership revocation (review m2 race).
    assert_eq!(out["chains"]["operator"]["events"].as_i64(), Some(13));
    assert!(out["chains"]["admission"]["events"].as_i64().unwrap() >= 10);
    let dump = pg_dump(&fixture, false);
    for code in &codes {
        assert!(!dump.contains(code.as_str()) && !dump.contains(&code.replace('-', "")));
    }
    check(&mut checks, "operator and admission audit chains recompute from genesis to their heads in both organizations; no issued code appears in a full database dump");

    stores.close().await.unwrap();
    docker(
        &["rm", "--force", "--volumes", &fixture.name],
        b"",
        "remove isolated database",
    );
    fixture.removed = true;
    std::fs::remove_dir_all(&fixture.directory.0).unwrap();
    check(
        &mut checks,
        "local database and private fixture files removed",
    );
    if let Some(path) = std::env::var_os("GENUI_ADMISSION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"APP-073 isolated native PostgreSQL TLS admission and operator-command qualification","managed_acceptance":false,"server_version":fixture.server_version,"defaults":defaults,"codes_issued":codes.len(),"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Identities are synthetic registry-bound fixtures; no provider, session or browser flow is exercised (APP-009/074).","Raw codes are never written to this report."]})).unwrap()).unwrap();
    }
}
