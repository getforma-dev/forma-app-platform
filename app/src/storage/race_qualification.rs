//! APP-014 retry, conflict and authorization race campaign (plan Task 11).
//! Isolated test executable only: local Docker, OpenSSL and the locked Python
//! environments, as the APP-013 command campaign, on the same Spend Review S05
//! fixture. Every race is driven by APP-013's test-only checkpoints in the
//! command transaction (`precheck`, `outcome`, `fence`, `record_lock`) and by
//! lock waits observed in `pg_stat_activity`, never by sleeps. Membership
//! revocation runs through the real operator command (`revoke-membership`)
//! where it does not need to hold its locks; a writer that must hold them
//! across a checkpoint is fixture-only SQL in the operator channel's order
//! (`SET LOCAL ROLE genui_owner` under explicit organization scope:
//! organization `FOR SHARE`, then the member row `FOR UPDATE`, then the
//! status and `membership_epoch + 1`, D7.1). The adoption writer is the
//! test-only writer of D7.4: organization `FOR SHARE`, the saved work `FOR
//! UPDATE`, `version + 1`, through the same owner channel. Each group asserts
//! the typed `CommandResult` on the storage path and the D7.9 HTTP row on the
//! command route with a minted workspace session, and every write (or its
//! absence) on the database itself.
use super::{
    admission_qualification::{accepted, cli_command, lock_wait, operator, wait},
    command_qualification::{
        committed, conflict, json_headers, paused, scalar, send, values, vault_token, Reply,
        TestPages,
    },
    definition_qualification::{
        grant, request, revoke, spend_review_s05, SpendReview, AUTHOR, EXPENSES, SPEND_APP,
    },
    pool::TestHooks,
    qualification::{Fixture, Operator},
    qualification_cases::{check, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    auth::{Auth, Pages},
    commands::{self, routes::Commands, RecordPage},
    http,
};
use axum::{
    http::{header, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant as WallInstant},
};
use tokio::{task::JoinHandle, time::timeout};

/// A record of the S05 decision collection per campaign group (client-named,
/// as the contract's `command_request` carries one).
fn record(group: u32) -> String {
    format!("00000000-0000-4000-8000-{:012x}", 0x1400 + group)
}
/// A distinct 32-hex idempotency key.
fn key(n: u32) -> String {
    format!("{:032x}", 0xa014_0000_u64 + u64::from(n))
}
fn command(
    s05: &SpendReview,
    record: &str,
    key: &str,
    expected_record_version: i64,
    expected_work_version: i64,
    values: Value,
) -> CommandRequest {
    CommandRequest {
        app: SPEND_APP.into(),
        work: s05.work.clone(),
        publication: s05.publication.clone(),
        command: "record_decision".into(),
        schema: "decision".into(),
        schema_version_expected: 1,
        record: record.into(),
        expected_record_version,
        expected_work_version,
        values,
        idempotency_key: key.into(),
    }
}
/// The JSON route's body for `command` (the key travels in the header).
fn body(command: &CommandRequest) -> String {
    let mut document = command.document();
    document.as_object_mut().unwrap().remove("idempotency_key");
    serde_json::to_string(&document).unwrap()
}

/// Revisions, outcomes and data-chain events naming `record`, as the
/// superuser counts them: one intended write is exactly `[n, n, n]`.
async fn tally(fixture: &Fixture, record: &str) -> [i64; 3] {
    [
        scalar(fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND record_id='{record}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.command_outcomes WHERE organization_id='{ORG}' AND record_id='{record}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='data' AND record_id='{record}'")).await,
    ]
}
async fn claims(fixture: &Fixture) -> i64 {
    scalar(fixture, "SELECT count(*) FROM genui.command_claims").await
}
async fn current_version(fixture: &Fixture, record: &str) -> i64 {
    scalar(fixture, &format!("SELECT coalesce(max(current_version),0) FROM genui.internal_records WHERE organization_id='{ORG}' AND record_id='{record}'")).await
}
async fn work_version(fixture: &Fixture, work: &str) -> i64 {
    scalar(
        fixture,
        &format!("SELECT version FROM genui.saved_work WHERE organization_id='{ORG}' AND work_id='{work}'"),
    )
    .await
}
async fn epoch_of(fixture: &Fixture, principal: &str) -> i64 {
    scalar(fixture, &format!("SELECT membership_epoch FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{principal}'")).await
}
async fn lock_waiters(fixture: &Fixture) -> i64 {
    scalar(
        fixture,
        "SELECT count(*) FROM pg_stat_activity WHERE datname='genui' AND wait_event_type='Lock'",
    )
    .await
}

/// A command of `principal` at `epoch` through the storage module, in its own task.
fn spawn_command(
    stores: &Arc<Stores>,
    principal: &'static str,
    epoch: i64,
    command: &CommandRequest,
) -> JoinHandle<Result<CommandResult>> {
    let (stores, command) = (stores.clone(), command.clone());
    tokio::spawn(async move {
        let trusted = request(&stores, ORG, principal, epoch);
        stores.data.commit_record(&trusted, &command).await
    })
}
async fn commit(
    stores: &Stores,
    principal: &str,
    epoch: i64,
    command: &CommandRequest,
) -> Result<CommandResult> {
    stores
        .data
        .commit_record(&request(stores, ORG, principal, epoch), command)
        .await
}
async fn lookup(
    stores: &Stores,
    principal: &str,
    epoch: i64,
    s05: &SpendReview,
    key: &str,
) -> Result<Option<CommandOutcome>> {
    stores
        .data
        .read_outcome(
            &request(stores, ORG, principal, epoch),
            SPEND_APP,
            &s05.work,
            "record_decision",
            key,
        )
        .await
}

/// The JSON command route, in its own task.
fn spawn_post(
    app: &Router,
    s05: &SpendReview,
    cookie: &str,
    command: &CommandRequest,
) -> JoinHandle<Reply> {
    let (app, cookie, command) = (app.clone(), cookie.to_owned(), command.clone());
    let action = action(s05);
    tokio::spawn(async move {
        send(
            &app,
            "POST",
            &action,
            Some(&cookie),
            &body(&command),
            &json_headers(&command.idempotency_key),
        )
        .await
    })
}
async fn post(app: &Router, s05: &SpendReview, cookie: &str, command: &CommandRequest) -> Reply {
    spawn_post(app, s05, cookie, command).await.unwrap()
}
async fn get_outcome(app: &Router, s05: &SpendReview, cookie: &str, key: &str) -> Reply {
    send(
        app,
        "GET",
        &format!("{}/outcomes/{key}", action(s05)),
        Some(cookie),
        "",
        &[],
    )
    .await
}
fn action(s05: &SpendReview) -> String {
    format!("/v1/work/{SPEND_APP}/{}/commands/record_decision", s05.work)
}
/// A D7.9 refusal row: the status, the problem code and, for the two lock
/// waits, `Retry-After: 2`. Never a 503.
fn problem(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_ne!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        reply.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(reply.json()["code"], code, "{}", reply.body);
    let retry = reply
        .headers
        .get(header::RETRY_AFTER)
        .map(|v| v.to_str().unwrap().to_owned());
    if matches!(code, "in_flight" | "busy") {
        assert_eq!(retry.as_deref(), Some("2"), "{code}");
    } else {
        assert_eq!(retry, None, "{code}");
    }
}
fn ok(reply: &Reply) -> Value {
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.headers[header::CONTENT_TYPE], "application/json");
    reply.json()
}

/// A fixture-only writer transaction in the operator channel (owner role,
/// explicit organization scope), left open after `statements`.
async fn hold(fixture: &Fixture, statements: &str) -> Operator {
    let writer = fixture.operator().await;
    writer
        .client
        .batch_execute(&format!(
            "BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); {statements}"
        ))
        .await
        .unwrap();
    writer
}
/// The same writer run to COMMIT in its own task (it may block on a lock).
fn spawn_writer(writer: Operator, statements: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        writer
            .client
            .batch_execute(&format!(
                "BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); {statements} COMMIT;"
            ))
            .await
            .unwrap();
    })
}
async fn finish(writer: Operator, sql: &str) {
    writer.client.batch_execute(sql).await.unwrap();
}
/// D7.1 revocation order, in the operator channel.
fn revoke_member(principal: &str) -> String {
    format!("SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{principal}' FOR UPDATE; UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1 WHERE organization_id='{ORG}' AND principal_id='{principal}';")
}
/// A member-row update that changes nothing (FOR NO KEY UPDATE only): an
/// operator member transaction that is not a revocation.
fn touch_member(principal: &str) -> String {
    format!("UPDATE genui.memberships SET membership_epoch=membership_epoch WHERE organization_id='{ORG}' AND principal_id='{principal}';")
}
/// D7.4: the test-only adoption writer through the owner channel.
fn adopt(work: &str) -> String {
    format!("SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.saved_work WHERE organization_id='{ORG}' AND app_id='{SPEND_APP}' AND work_id='{work}' FOR UPDATE; UPDATE genui.saved_work SET version=version+1 WHERE organization_id='{ORG}' AND app_id='{SPEND_APP}' AND work_id='{work}';")
}
/// A saved-work update that changes nothing (FOR NO KEY UPDATE only).
fn touch_work(work: &str) -> String {
    format!("UPDATE genui.saved_work SET version=version WHERE organization_id='{ORG}' AND app_id='{SPEND_APP}' AND work_id='{work}';")
}
/// Fixture-only restoration of a revoked member in the same D7.1 order; the
/// resulting epoch is a fresh trusted fixture (as the APP-008 campaign).
async fn restore_member(fixture: &Fixture, principal: &str) -> i64 {
    let writer = hold(fixture, &format!("SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{principal}' FOR UPDATE; UPDATE genui.memberships SET status='active',membership_epoch=membership_epoch+1 WHERE organization_id='{ORG}' AND principal_id='{principal}' AND status='revoked';")).await;
    finish(writer, "COMMIT").await;
    epoch_of(fixture, principal).await
}
/// The real operator command, run to completion in a blocking task.
fn spawn_operator(fixture: &Fixture, args: &[&str]) -> JoinHandle<Value> {
    let label = args[0].to_owned();
    let child = cli_command(fixture, "genui_migration", "synthetic-migration", args)
        .spawn()
        .expect("operator command available");
    tokio::task::spawn_blocking(move || accepted(wait(child, &label), &label))
}
async fn mint(stores: &Stores, auth: &Auth, principal: &str) -> String {
    let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
    stores
        .workspace
        .mint_session_for_tests(ORG, principal, &reference)
        .await
        .unwrap()
}
fn router(stores: &Arc<Stores>) -> (Arc<Auth>, Router) {
    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let record_page: Arc<dyn RecordPage> = pages;
    let app = commands::routes::router(Arc::new(Commands {
        auth: auth.clone(),
        page: record_page,
    }))
    .fallback(http::fallback);
    (auth, app)
}
fn stale(result: Result<CommandResult>) -> CurrentRevision {
    match result {
        Ok(CommandResult::Stale(current)) => current,
        other => panic!("expected Stale: {other:?}"),
    }
}

/// One campaign group: its checks and its wall time.
struct Group {
    name: &'static str,
    started: WallInstant,
    checks: Vec<Value>,
}
impl Group {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            started: WallInstant::now(),
            checks: Vec::new(),
        }
    }
    fn check(&mut self, name: &str) {
        check(&mut self.checks, &format!("{}: {name}", self.name));
    }
    fn close(self, report: &mut Vec<Value>) {
        report.push(json!({
            "group": self.name,
            "elapsed_ms": self.started.elapsed().as_millis() as u64,
            "checks": self.checks,
        }));
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_race_campaign() {
    std::thread::Builder::new()
        .name("app014-race-campaign".into())
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
    let started = WallInstant::now();
    let mut fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut groups = Vec::new();
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["backfill", "--organization", org]);
        assert_eq!(out["sequence"]["organization_sequences"], "created");
    }
    let settings = fixture.settings();
    let mut stores = Arc::new(Stores::new(settings).unwrap());
    stores.readiness().await.unwrap();
    let s05 = spend_review_s05(&fixture, &stores).await;
    assert_eq!(work_version(&fixture, &s05.work).await, 1);
    let (mut auth, mut app) = router(&stores);
    let mut author_cookie = mint(&stores, &auth, AUTHOR).await;
    let mut author_epoch = 1;

    // ===================================================================
    let mut group =
        Group::new("lost_response_retry_returns_original_outcome_with_stale_expected_version");
    let r1 = record(1);
    let k1 = command(
        &s05,
        &r1,
        &key(1),
        0,
        1,
        values("needs_context", "lost response — synthetic"),
    );
    // The response to the first submission is lost: the client keeps only
    // its key and payload (the body is retained here solely for comparison).
    let original = post(&app, &s05, &author_cookie, &k1).await;
    let original_document = ok(&original);
    assert_eq!(original_document["record_version"], 1);
    let o1 = lookup(&stores, AUTHOR, author_epoch, &s05, &key(1))
        .await
        .unwrap()
        .expect("the committed outcome by key");
    assert_eq!(o1.document(), original_document);
    assert_eq!(o1.record_version, 1);
    assert_eq!(tally(&fixture, &r1).await, [1, 1, 1]);
    // Another edit moves the record on, so the retried premise (version 0)
    // is stale twice over.
    let k2 = command(
        &s05,
        &r1,
        &key(2),
        1,
        1,
        values("reviewed", "moved on — synthetic"),
    );
    let o2 = committed(commit(&stores, AUTHOR, author_epoch, &k2).await);
    assert_eq!(o2.record_version, 2);
    assert_eq!(tally(&fixture, &r1).await, [2, 2, 2]);
    let claims_before = claims(&fixture).await;
    let retried = post(&app, &s05, &author_cookie, &k1).await;
    assert_eq!(
        (retried.status, retried.body.as_str()),
        (StatusCode::OK, original.body.as_str())
    );
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &k1).await,
        Ok(CommandResult::Replayed(o1.clone()))
    );
    let looked_up = get_outcome(&app, &s05, &author_cookie, &key(1)).await;
    assert_eq!(looked_up.body, original.body);
    assert_eq!(tally(&fixture, &r1).await, [2, 2, 2]);
    assert_eq!(claims(&fixture).await, claims_before);
    assert_eq!(current_version(&fixture, &r1).await, 2);
    group.check("a committed command whose response is lost, retried with the same scoped key and payload and its now-stale expected version 0 after another edit reached version 2, returns the original outcome (record version 1) byte-identically on the route (200) and as Replayed in storage; the lookup by key returns the same; one revision, one outcome and one command.committed event per command, no new claim");

    let mut edited = k1.clone();
    edited.values = values("needs_context", "lost response — edited");
    let reply = post(&app, &s05, &author_cookie, &edited).await;
    problem(&reply, StatusCode::CONFLICT, "conflict");
    let view = reply.json()["recovery_view"].clone();
    assert_eq!(view["reason"], "payload_mismatch");
    assert_eq!(view["stored_outcome"], original_document);
    assert_eq!(view["current_record_version"], 2);
    assert_eq!(
        view["current_values"],
        values("reviewed", "moved on — synthetic")
    );
    assert_eq!(tally(&fixture, &r1).await, [2, 2, 2]);
    group.check("the lost key reused with a changed payload conflicts (409 conflict) with the authorized recovery view naming the original outcome and the record's current version 2 and values; nothing is written");

    // Current authorization precedes disclosure: without `run` on the app the
    // creator holds no record_write, and the stored outcome is not delivered.
    let out = revoke(&fixture, AUTHOR, "run", &["--app", SPEND_APP]);
    let revoked_epoch = out["membership_epoch"].as_i64().unwrap();
    assert_eq!(revoked_epoch, author_epoch + 1);
    problem(
        &post(&app, &s05, &author_cookie, &k1).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    problem(
        &get_outcome(&app, &s05, &author_cookie, &key(1)).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &k1).await,
        Err(Error::Denied)
    );
    assert_eq!(
        commit(&stores, AUTHOR, revoked_epoch, &k1).await,
        Err(Error::Denied)
    );
    assert_eq!(
        lookup(&stores, AUTHOR, revoked_epoch, &s05, &key(1)).await,
        Err(Error::Denied)
    );
    author_epoch = revoked_epoch;
    grant(&fixture, AUTHOR, "run", &["--app", SPEND_APP]);
    assert_eq!(epoch_of(&fixture, AUTHOR).await, author_epoch);
    assert_eq!(
        post(&app, &s05, &author_cookie, &k1).await.body,
        original.body
    );
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &k1).await,
        Ok(CommandResult::Replayed(o1.clone()))
    );
    assert_eq!(tally(&fixture, &r1).await, [2, 2, 2]);
    group.check("current authorization precedes outcome disclosure: after the operator revokes the author's app run grant (epoch advanced) the retry and the lookup are the uniform 404 and Denied at the old and the new epoch; once run is granted again the same retry returns the original outcome; nothing is written either way");

    // Restart every pool and store, then retry the lost key again.
    drop(app);
    drop(auth);
    stores.close().await.unwrap();
    drop(stores);
    let settings = fixture.settings();
    let hooks: Arc<TestHooks> = settings.hooks.clone();
    stores = Arc::new(Stores::new(settings).unwrap());
    stores.readiness().await.unwrap();
    (auth, app) = router(&stores);
    let old_cookie = author_cookie;
    author_cookie = mint(&stores, &auth, AUTHOR).await;
    problem(
        &post(&app, &s05, &old_cookie, &k1).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(
        post(&app, &s05, &author_cookie, &k1).await.body,
        original.body
    );
    assert_eq!(
        get_outcome(&app, &s05, &author_cookie, &key(1)).await.body,
        original.body
    );
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &k1).await,
        Ok(CommandResult::Replayed(o1.clone()))
    );
    assert_eq!(tally(&fixture, &r1).await, [2, 2, 2]);
    group.check("after every pool and store is replaced (the old session ended with its vault), the lost key retried on a fresh session still returns the original outcome byte-identically and replays in storage; still exactly two revisions, outcomes and events for the record");
    group.close(&mut groups);

    // ===================================================================
    let mut group =
        Group::new("same_key_and_different_key_races_commit_exactly_one_intended_write");
    let r2 = record(2);
    // Same key, same payload: the second waits on the first's claim and, once
    // the first commits, replays its outcome.
    let a = command(
        &s05,
        &r2,
        &key(10),
        0,
        1,
        values("needs_context", "A — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &a);
    paused(&hooks).await;
    let second = spawn_command(&stores, AUTHOR, author_epoch, &a);
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let oa = committed(first.await.unwrap());
    assert_eq!(oa.record_version, 1);
    assert_eq!(
        second.await.unwrap(),
        Ok(CommandResult::Replayed(oa.clone()))
    );
    assert_eq!(tally(&fixture, &r2).await, [1, 1, 1]);
    group.check("two submissions of the same key and payload race: the second waits on the first's in-flight claim (observed in pg_stat_activity), and when the first commits version 1 the second returns that outcome as Replayed; exactly one revision, outcome and event");

    // Same key, changed payload, racing.
    let b = command(
        &s05,
        &r2,
        &key(11),
        1,
        1,
        values("reviewed", "B — synthetic"),
    );
    let mut b_changed = b.clone();
    b_changed.values = values("reviewed", "B changed — synthetic");
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &b);
    paused(&hooks).await;
    let second = spawn_command(&stores, AUTHOR, author_epoch, &b_changed);
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let ob = committed(first.await.unwrap());
    assert_eq!(ob.record_version, 2);
    let view = conflict(second.await.unwrap());
    assert_eq!(view.reason, ConflictReason::PayloadMismatch);
    assert_eq!(view.stored_outcome.as_ref(), Some(&ob));
    assert_eq!(
        (view.current_record_version, view.current_values.clone()),
        (2, Some(values("reviewed", "B — synthetic")))
    );
    assert_eq!(tally(&fixture, &r2).await, [2, 2, 2]);
    group.check("the same key with a changed payload racing an in-flight command waits on its claim, then conflicts (payload_mismatch) with the recovery view naming the committed outcome and the current values; exactly one write");

    // Different keys on the same expected version: the second waits on the
    // organization sequence, then conflicts on the moved version; its
    // authorized recovery (a new key on the current version) commits.
    let c = command(
        &s05,
        &r2,
        &key(12),
        2,
        1,
        values("reviewed", "C — synthetic"),
    );
    let d = command(
        &s05,
        &r2,
        &key(13),
        2,
        1,
        values("reviewed", "D — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &c);
    paused(&hooks).await;
    let second = spawn_command(&stores, AUTHOR, author_epoch, &d);
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let oc = committed(first.await.unwrap());
    assert_eq!(oc.record_version, 3);
    let view = conflict(second.await.unwrap());
    assert_eq!(view.reason, ConflictReason::ExpectedVersion);
    assert_eq!(
        (view.expected_record_version, view.current_record_version),
        (2, 3)
    );
    assert_eq!(
        view.current_values,
        Some(values("reviewed", "C — synthetic"))
    );
    assert!(view.stored_outcome.is_none());
    assert_eq!(tally(&fixture, &r2).await, [3, 3, 3]);
    let recovered = command(
        &s05,
        &r2,
        &key(14),
        view.current_record_version,
        1,
        values("reviewed", "D — synthetic"),
    );
    let od = committed(commit(&stores, AUTHOR, author_epoch, &recovered).await);
    assert_eq!(od.record_version, 4);
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &d).await.map(|r| matches!(r, CommandResult::Conflict(v) if v.reason == ConflictReason::ExpectedVersion && v.current_record_version == 4)),
        Ok(true)
    );
    assert_eq!(tally(&fixture, &r2).await, [4, 4, 4]);
    group.check("two different keys expecting version 2 race: the second waits on the organization sequence row, then conflicts (expected 2, current 3, the winner's values, no stored outcome) without writing; its authorized recovery on version 3 under a new key commits version 4, and the stale edit stays a conflict");

    // The same races through the route while the first command holds its
    // locks past lock_timeout: 409 in_flight with Retry-After, never 503.
    let e = command(
        &s05,
        &r2,
        &key(15),
        4,
        1,
        values("reviewed", "E — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &e);
    paused(&hooks).await;
    let reply = post(&app, &s05, &author_cookie, &e).await;
    problem(&reply, StatusCode::CONFLICT, "in_flight");
    assert_eq!(tally(&fixture, &r2).await, [4, 4, 4]);
    hooks.release.notify_one();
    let oe = committed(first.await.unwrap());
    assert_eq!(oe.record_version, 5);
    let retried = ok(&post(&app, &s05, &author_cookie, &e).await);
    assert_eq!(retried, oe.document());
    let f = command(
        &s05,
        &r2,
        &key(16),
        5,
        1,
        values("reviewed", "F — synthetic"),
    );
    let g = command(
        &s05,
        &r2,
        &key(17),
        5,
        1,
        values("reviewed", "G — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &f);
    paused(&hooks).await;
    let reply = post(&app, &s05, &author_cookie, &g).await;
    problem(&reply, StatusCode::CONFLICT, "in_flight");
    hooks.release.notify_one();
    let of = committed(first.await.unwrap());
    assert_eq!(of.record_version, 6);
    let reply = post(&app, &s05, &author_cookie, &g).await;
    problem(&reply, StatusCode::CONFLICT, "conflict");
    let view = reply.json()["recovery_view"].clone();
    assert_eq!(
        (
            view["reason"].as_str(),
            view["current_record_version"].as_i64()
        ),
        (Some("expected_version"), Some(6))
    );
    let recovered = command(
        &s05,
        &r2,
        &key(18),
        6,
        1,
        values("reviewed", "G — synthetic"),
    );
    assert_eq!(
        ok(&post(&app, &s05, &author_cookie, &recovered).await)["record_version"],
        7
    );
    assert_eq!(tally(&fixture, &r2).await, [7, 7, 7]);
    group.check("on the route, the same key behind a command holding its claim and a different key behind its sequence lock each wait past lock_timeout and answer 409 in_flight with Retry-After: 2 (never 503) with nothing written; after the holder commits, the same key returns its outcome (200) and the different key conflicts (409 expected_version) until its authorized recovery commits; seven intended writes, seven revisions, outcomes and events");
    group.close(&mut groups);

    // ===================================================================
    let mut group = Group::new("other_principal_cannot_retrieve_key");
    let r3 = record(3);
    for permission in ["discover", "run"] {
        grant(&fixture, REVIEWER, permission, &["--app", SPEND_APP]);
    }
    grant(&fixture, REVIEWER, "source_read", &["--source", EXPENSES]);
    grant(
        &fixture,
        REVIEWER,
        "record_write",
        &["--app", SPEND_APP, "--work", &s05.work],
    );
    let mut reviewer_epoch = epoch_of(&fixture, REVIEWER).await;
    let mut reviewer_cookie = mint(&stores, &auth, REVIEWER).await;
    let k20 = command(
        &s05,
        &r3,
        &key(20),
        0,
        1,
        values("needs_context", "author's — synthetic"),
    );
    let o20 = committed(commit(&stores, AUTHOR, author_epoch, &k20).await);
    let author_document = o20.document();
    // While the author's identical retry holds its fences, the reviewer
    // submits the author's key and payload: its own claim, its own premise.
    hooks.arm("fence");
    let author_retry = spawn_command(&stores, AUTHOR, author_epoch, &k20);
    paused(&hooks).await;
    let view = conflict(commit(&stores, REVIEWER, reviewer_epoch, &k20).await);
    assert_eq!(
        (view.reason, view.current_record_version),
        (ConflictReason::ExpectedVersion, 1)
    );
    assert!(view.stored_outcome.is_none());
    assert_eq!(
        lookup(&stores, REVIEWER, reviewer_epoch, &s05, &key(20)).await,
        Ok(None)
    );
    assert_eq!(lock_waiters(&fixture).await, 0);
    hooks.release.notify_one();
    assert_eq!(
        author_retry.await.unwrap(),
        Ok(CommandResult::Replayed(o20.clone()))
    );
    let reply = post(&app, &s05, &reviewer_cookie, &k20).await;
    problem(&reply, StatusCode::CONFLICT, "conflict");
    assert_eq!(reply.json()["recovery_view"]["stored_outcome"], Value::Null);
    assert_eq!(reply.json()["recovery_view"]["reason"], "expected_version");
    problem(
        &get_outcome(&app, &s05, &reviewer_cookie, &key(20)).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(
        ok(&get_outcome(&app, &s05, &author_cookie, &key(20)).await),
        author_document
    );
    assert_eq!(tally(&fixture, &r3).await, [1, 1, 1]);
    group.check("while the author's identical retry holds its fences, another principal with record_write submitting the author's key and payload gets its own fresh claim judged on its own premise (409 conflict expected_version, no stored outcome, no lock wait) and its lookup by that key finds nothing (404 on the route); the author's retry replays and the author's lookup returns the outcome");

    let bystander_cookie = mint(&stores, &auth, PRINCIPAL).await;
    problem(
        &get_outcome(&app, &s05, &bystander_cookie, &key(20)).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    problem(
        &post(&app, &s05, &bystander_cookie, &k20).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(
        stores
            .data
            .read_outcome(
                &request(&stores, OTHER, AUTHOR, 1),
                SPEND_APP,
                &s05.work,
                "record_decision",
                &key(20)
            )
            .await,
        Err(Error::Denied)
    );
    {
        let operator = fixture.operator().await;
        for (principal, expected) in [(REVIEWER, 0), (AUTHOR, 1)] {
            operator.client.batch_execute(&format!("BEGIN; SET LOCAL ROLE genui_data; SELECT set_config('genui.organization_id','{ORG}',true),set_config('genui.principal_id','{principal}',true),set_config('genui.membership_epoch','1',true);")).await.unwrap();
            for relation in ["command_claims", "command_outcomes"] {
                let visible: i64 = operator
                    .client
                    .query_one(
                        &format!(
                            "SELECT count(*) FROM genui.{relation} WHERE idempotency_key='{}'",
                            key(20)
                        ),
                        &[],
                    )
                    .await
                    .unwrap()
                    .get(0);
                assert_eq!(visible, expected, "{principal} {relation}");
            }
            operator.client.batch_execute("ROLLBACK").await.unwrap();
        }
    }
    group.check("a member without record_write gets the uniform 404 for the key's lookup and submission, the canary organization's member is Denied, and at the row level the data role under the reviewer's scope sees none of the author's claims or outcomes under the key (one under the author's)");

    // Both principals hold their own outcomes under the same key string.
    let reviewer_own = command(
        &s05,
        &r3,
        &key(20),
        1,
        1,
        values("reviewed", "reviewer's — synthetic"),
    );
    let or = committed(commit(&stores, REVIEWER, reviewer_epoch, &reviewer_own).await);
    assert_eq!((or.principal.as_str(), or.record_version), (REVIEWER, 2));
    assert_eq!(
        lookup(&stores, AUTHOR, author_epoch, &s05, &key(20)).await,
        Ok(Some(o20.clone()))
    );
    assert_eq!(
        lookup(&stores, REVIEWER, reviewer_epoch, &s05, &key(20)).await,
        Ok(Some(or.clone()))
    );
    assert_eq!(
        ok(&get_outcome(&app, &s05, &reviewer_cookie, &key(20)).await),
        or.document()
    );
    assert_eq!(
        ok(&get_outcome(&app, &s05, &author_cookie, &key(20)).await),
        author_document
    );
    // An in-flight claim of one principal never blocks or answers another's.
    let k21 = command(
        &s05,
        &r3,
        &key(21),
        2,
        1,
        values("reviewed", "author in flight — synthetic"),
    );
    hooks.arm("outcome");
    let author_in_flight = spawn_command(&stores, AUTHOR, author_epoch, &k21);
    paused(&hooks).await;
    let mut reviewer_k21 = k21.clone();
    reviewer_k21.values = values("reviewed", "reviewer beside — synthetic");
    let orr = committed(commit(&stores, REVIEWER, reviewer_epoch, &reviewer_k21).await);
    assert_eq!(orr.record_version, 3);
    hooks.release.notify_one();
    let view = conflict(author_in_flight.await.unwrap());
    assert_eq!(
        (view.reason, view.current_record_version),
        (ConflictReason::ExpectedVersion, 3)
    );
    assert!(view.stored_outcome.is_none());
    assert_eq!(
        lookup(&stores, AUTHOR, author_epoch, &s05, &key(21)).await,
        Ok(None)
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!(
                "SELECT count(*) FROM genui.command_claims WHERE idempotency_key='{}'",
                key(20)
            )
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!(
                "SELECT count(*) FROM genui.command_claims WHERE idempotency_key='{}'",
                key(21)
            )
        )
        .await,
        1
    );
    assert_eq!(tally(&fixture, &r3).await, [3, 3, 3]);
    group.check("the scope includes the principal: the reviewer's own command under the author's key string commits its own outcome and each principal's lookup returns only its own; while the author's fresh claim is in flight the reviewer's same-key command neither waits nor replays it and commits, and the author's command then conflicts on the moved version with its claim released");
    group.close(&mut groups);

    // ===================================================================
    let mut group = Group::new("adoption_vs_commit_both_orders");
    let r4 = record(4);
    let work = s05.work.clone();
    // Adoption first: the paused command's work premise has moved.
    let claims_before = claims(&fixture).await;
    let k30 = command(
        &s05,
        &r4,
        &key(30),
        0,
        1,
        values("needs_context", "adoption first — synthetic"),
    );
    hooks.arm("precheck");
    let paused_command = spawn_command(&stores, AUTHOR, author_epoch, &k30);
    paused(&hooks).await;
    finish(hold(&fixture, &adopt(&work)).await, "COMMIT").await;
    assert_eq!(work_version(&fixture, &work).await, 2);
    hooks.release.notify_one();
    let current = stale(paused_command.await.unwrap());
    assert_eq!(
        (current.publication.as_deref(), current.work_version),
        (Some(s05.publication.as_str()), Some(2))
    );
    assert_eq!(tally(&fixture, &r4).await, [0, 0, 0]);
    assert_eq!(claims(&fixture).await, claims_before);
    group.check("adoption first: a command paused after its precheck resumes after the D7.4 test-only adoption (saved work FOR UPDATE, version 1 → 2) committed, and its work fence answers Stale naming the current publication and version 2; no revision, outcome, event or claim remains");

    // Adoption first holding its lock; it commits while the command waits.
    let k31 = command(
        &s05,
        &r4,
        &key(31),
        0,
        2,
        values("needs_context", "adoption held — synthetic"),
    );
    hooks.arm("precheck");
    let reply = spawn_post(&app, &s05, &author_cookie, &k31);
    paused(&hooks).await;
    let writer = hold(&fixture, &adopt(&work)).await;
    hooks.release.notify_one();
    lock_wait(&fixture, 1).await;
    finish(writer, "COMMIT").await;
    let reply = reply.await.unwrap();
    problem(&reply, StatusCode::PRECONDITION_FAILED, "stale");
    assert_eq!(reply.json()["current_revision"]["work_version"], 3);
    assert_eq!(tally(&fixture, &r4).await, [0, 0, 0]);
    group.check("adoption first with its lock held: the resumed route command's claim waits on the adoption writer's saved-work lock (observed), the writer commits version 3 during the wait, and the route answers 412 stale naming version 3; nothing is written");

    // The adoption writer holds past lock_timeout: in_flight (its FOR UPDATE
    // blocks the claim's reference) or busy (a FOR NO KEY UPDATE lets the
    // claim pass and the work fence waits). Never 503.
    let k32 = command(
        &s05,
        &r4,
        &key(32),
        0,
        3,
        values("needs_context", "held past timeout — synthetic"),
    );
    for (statements, code) in [(adopt(&work), "in_flight"), (touch_work(&work), "busy")] {
        hooks.arm("precheck");
        let reply = spawn_post(&app, &s05, &author_cookie, &k32);
        paused(&hooks).await;
        let writer = hold(&fixture, &statements).await;
        hooks.release.notify_one();
        lock_wait(&fixture, 1).await;
        problem(&reply.await.unwrap(), StatusCode::CONFLICT, code);
        finish(writer, "ROLLBACK").await;
    }
    assert_eq!(work_version(&fixture, &work).await, 3);
    assert_eq!(tally(&fixture, &r4).await, [0, 0, 0]);
    assert_eq!(claims(&fixture).await, claims_before);
    group.check("an adoption writer holding the saved work past lock_timeout: FOR UPDATE blocks the claim's reference and the route answers 409 in_flight; a FOR NO KEY UPDATE holder passes the claim and blocks the work fence, answering 409 busy; both with Retry-After: 2, never 503, nothing written, and the writer's rollback leaves version 3");

    // Command first: the command holds the work FOR SHARE; adoption waits.
    let k34 = command(
        &s05,
        &r4,
        &key(34),
        0,
        3,
        values("needs_context", "command first — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &k34);
    paused(&hooks).await;
    let adoption = spawn_writer(fixture.operator().await, adopt(&work));
    lock_wait(&fixture, 1).await;
    assert_eq!(work_version(&fixture, &work).await, 3);
    hooks.release.notify_one();
    let o34 = committed(first.await.unwrap());
    assert_eq!((o34.record_version, o34.work_version), (1, 3));
    timeout(Duration::from_secs(5), adoption)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(work_version(&fixture, &work).await, 4);
    assert_eq!(tally(&fixture, &r4).await, [1, 1, 1]);
    // The earlier authorized commit stays recorded and replays; a new edit
    // on the old work premise is stale; its recovery on version 4 commits.
    assert_eq!(
        commit(&stores, AUTHOR, author_epoch, &k34).await,
        Ok(CommandResult::Replayed(o34.clone()))
    );
    assert_eq!(
        ok(&post(&app, &s05, &author_cookie, &k34).await),
        o34.document()
    );
    let k35 = command(
        &s05,
        &r4,
        &key(35),
        1,
        3,
        values("reviewed", "after adoption — synthetic"),
    );
    let reply = post(&app, &s05, &author_cookie, &k35).await;
    problem(&reply, StatusCode::PRECONDITION_FAILED, "stale");
    assert_eq!(reply.json()["current_revision"]["work_version"], 4);
    let mut k35_recovered = k35.clone();
    k35_recovered.idempotency_key = key(36);
    k35_recovered.expected_work_version = 4;
    let o36 = committed(commit(&stores, AUTHOR, author_epoch, &k35_recovered).await);
    assert_eq!((o36.record_version, o36.work_version), (2, 4));
    assert_eq!(tally(&fixture, &r4).await, [2, 2, 2]);
    group.check("command first: while the command holds the work FOR SHARE (paused at its record lock) the adoption writer waits (observed); the command commits version 1 on work version 3, then the adoption commits version 4; the committed outcome stays recorded and its identical retry replays it (200, work_version 3), a new edit on work version 3 is 412 stale naming 4, and its recovery on version 4 commits record version 2");

    // The claim is the earliest fence: a command that has only claimed its key
    // already holds the saved work's key share, so the adoption waits for it.
    let k37 = command(
        &s05,
        &r4,
        &key(37),
        2,
        4,
        values("reviewed", "claim first — synthetic"),
    );
    hooks.arm("outcome");
    let first = spawn_command(&stores, AUTHOR, author_epoch, &k37);
    paused(&hooks).await;
    let adoption = spawn_writer(fixture.operator().await, adopt(&work));
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let o37 = committed(first.await.unwrap());
    assert_eq!((o37.record_version, o37.work_version), (3, 4));
    timeout(Duration::from_secs(5), adoption)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(work_version(&fixture, &work).await, 5);
    assert_eq!(tally(&fixture, &r4).await, [3, 3, 3]);
    group.check("a command paused right after its claim already holds the saved work's key share: the adoption writer's FOR UPDATE waits on it (observed), the command's work fence then takes FOR SHARE ahead of the queued writer without deadlock and commits on work version 4, and the adoption commits version 5 afterwards");
    group.close(&mut groups);
    let work_now = work_version(&fixture, &work).await;

    // ===================================================================
    let mut group = Group::new("revocation_vs_commit_both_orders");
    let r5 = record(5);
    let claims_before = claims(&fixture).await;
    // Revocation first, through the operator command, while the command is
    // paused after its precheck.
    let k40 = command(
        &s05,
        &r5,
        &key(40),
        0,
        work_now,
        values("needs_context", "revocation first — synthetic"),
    );
    hooks.arm("precheck");
    let paused_command = spawn_command(&stores, REVIEWER, reviewer_epoch, &k40);
    paused(&hooks).await;
    let out = timeout(
        Duration::from_secs(45),
        spawn_operator(
            &fixture,
            &[
                "revoke-membership",
                "--organization",
                ORG,
                "--principal",
                REVIEWER,
            ],
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        (out["changed"].as_bool(), out["membership_epoch"].as_i64()),
        (Some(true), Some(reviewer_epoch + 1))
    );
    hooks.release.notify_one();
    assert_eq!(paused_command.await.unwrap(), Err(Error::Denied));
    assert_eq!(
        commit(&stores, REVIEWER, reviewer_epoch + 1, &k40).await,
        Err(Error::Denied)
    );
    problem(
        &post(&app, &s05, &reviewer_cookie, &k40).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(tally(&fixture, &r5).await, [0, 0, 0]);
    assert_eq!(claims(&fixture).await, claims_before);
    group.check("revocation first: a command paused after its precheck resumes after the operator's revoke-membership committed (epoch advanced); the fence refuses it (Denied), nothing is written and its claim is released; the next request at either epoch is Denied and the route answers the uniform 404");

    // Revocation first with its locks held; it commits while the command waits.
    restore_member(&fixture, REVIEWER).await;
    reviewer_cookie = mint(&stores, &auth, REVIEWER).await;
    let k41 = command(
        &s05,
        &r5,
        &key(41),
        0,
        work_now,
        values("needs_context", "revocation held — synthetic"),
    );
    hooks.arm("precheck");
    let reply = spawn_post(&app, &s05, &reviewer_cookie, &k41);
    paused(&hooks).await;
    let writer = hold(&fixture, &revoke_member(REVIEWER)).await;
    hooks.release.notify_one();
    lock_wait(&fixture, 1).await;
    finish(writer, "COMMIT").await;
    problem(&reply.await.unwrap(), StatusCode::NOT_FOUND, "not_found");
    assert_eq!(tally(&fixture, &r5).await, [0, 0, 0]);
    assert_eq!(claims(&fixture).await, claims_before);
    group.check("revocation first with its locks held (D7.1 order in the operator channel): the resumed route command's claim waits on the member row (observed), the revocation commits during the wait, and the fence refuses the command: the uniform 404, nothing written");

    // Held past lock_timeout: in_flight on the claim (member FOR UPDATE) or
    // busy on the authorization fence (FOR NO KEY UPDATE). Never 503.
    reviewer_epoch = restore_member(&fixture, REVIEWER).await;
    reviewer_cookie = mint(&stores, &auth, REVIEWER).await;
    let k42 = command(
        &s05,
        &r5,
        &key(42),
        0,
        work_now,
        values("needs_context", "held past timeout — synthetic"),
    );
    for (statements, code) in [
        (revoke_member(REVIEWER), "in_flight"),
        (touch_member(REVIEWER), "busy"),
    ] {
        hooks.arm("precheck");
        let reply = spawn_post(&app, &s05, &reviewer_cookie, &k42);
        paused(&hooks).await;
        let writer = hold(&fixture, &statements).await;
        hooks.release.notify_one();
        lock_wait(&fixture, 1).await;
        problem(&reply.await.unwrap(), StatusCode::CONFLICT, code);
        finish(writer, "ROLLBACK").await;
    }
    assert_eq!(epoch_of(&fixture, REVIEWER).await, reviewer_epoch);
    assert_eq!(tally(&fixture, &r5).await, [0, 0, 0]);
    assert_eq!(claims(&fixture).await, claims_before);
    group.check("a member writer holding the row past lock_timeout after the command's precheck: a revocation in progress (FOR UPDATE) blocks the claim's reference and the route answers 409 in_flight; a FOR NO KEY UPDATE holder passes the claim and blocks the authorization fence, answering 409 busy; both with Retry-After: 2, never 503, nothing written");

    // A request that arrives while the writer already holds the member row
    // waits in session resolution (the session's own member fence).
    let writer = hold(&fixture, &revoke_member(REVIEWER)).await;
    let reply = spawn_post(&app, &s05, &reviewer_cookie, &k42);
    lock_wait(&fixture, 1).await;
    let reply = reply.await.unwrap();
    finish(writer, "ROLLBACK").await;
    let session_wait = (
        reply.status,
        reply.json()["code"].as_str().map(str::to_owned),
    );
    assert_ne!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(tally(&fixture, &r5).await, [0, 0, 0]);
    group.check(&format!("a route request arriving while a revocation already holds the member row waits in session resolution past lock_timeout and answers {} {} (recorded as found; not 503), nothing written", session_wait.0.as_u16(), session_wait.1.clone().unwrap_or_default()));

    // Command first, through the operator command: the command holds the
    // member FOR SHARE (paused at its record lock); revoke-membership waits.
    for permission in ["discover", "run"] {
        grant(&fixture, PRINCIPAL, permission, &["--app", SPEND_APP]);
    }
    grant(&fixture, PRINCIPAL, "source_read", &["--source", EXPENSES]);
    grant(
        &fixture,
        PRINCIPAL,
        "record_write",
        &["--app", SPEND_APP, "--work", &work],
    );
    let principal_epoch = epoch_of(&fixture, PRINCIPAL).await;
    let principal_cookie = mint(&stores, &auth, PRINCIPAL).await;
    let k44 = command(
        &s05,
        &r5,
        &key(44),
        0,
        work_now,
        values("needs_context", "command first — synthetic"),
    );
    hooks.arm("record_lock");
    let first = spawn_command(&stores, PRINCIPAL, principal_epoch, &k44);
    paused(&hooks).await;
    let revocation = spawn_operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            PRINCIPAL,
        ],
    );
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let o44 = committed(first.await.unwrap());
    assert_eq!(o44.record_version, 1);
    let out = timeout(Duration::from_secs(45), revocation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (out["changed"].as_bool(), out["membership_epoch"].as_i64()),
        (Some(true), Some(principal_epoch + 1))
    );
    // The earlier authorized commit remains recorded ...
    assert_eq!(tally(&fixture, &r5).await, [1, 1, 1]);
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.command_outcomes WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' AND idempotency_key='{}' AND audit_event_id='{}'", key(44), o44.audit_event_id)).await, 1);
    let detail = stores
        .data
        .read_record_detail(
            &request(&stores, ORG, AUTHOR, author_epoch),
            SPEND_APP,
            &work,
            &s05.collection,
            &r5,
        )
        .await
        .unwrap();
    assert_eq!(detail.version, 1);
    // ... and current visibility governs its delivery.
    for epoch in [principal_epoch, principal_epoch + 1] {
        assert_eq!(
            commit(&stores, PRINCIPAL, epoch, &k44).await,
            Err(Error::Denied)
        );
        assert_eq!(
            lookup(&stores, PRINCIPAL, epoch, &s05, &key(44)).await,
            Err(Error::Denied)
        );
    }
    problem(
        &post(&app, &s05, &principal_cookie, &k44).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    problem(
        &get_outcome(&app, &s05, &principal_cookie, &key(44)).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(tally(&fixture, &r5).await, [1, 1, 1]);
    group.check("command first: while the command holds its fences (paused at its record lock) the operator's revoke-membership waits on the member row (observed); the command commits version 1, then the revocation commits; the committed revision, outcome and event remain recorded (the author reads version 1), but the revoked principal's identical retry and lookup are Denied at both epochs and the uniform 404 on the route");

    // The claim is the earliest fence: a command that has only claimed its
    // key already holds the member's key share, so the revocation waits.
    let k45 = command(
        &s05,
        &r5,
        &key(45),
        1,
        work_now,
        values("reviewed", "claim first — synthetic"),
    );
    hooks.arm("outcome");
    let first = spawn_command(&stores, REVIEWER, reviewer_epoch, &k45);
    paused(&hooks).await;
    let revocation = spawn_operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            REVIEWER,
        ],
    );
    lock_wait(&fixture, 1).await;
    hooks.release.notify_one();
    let o45 = committed(first.await.unwrap());
    assert_eq!(o45.record_version, 2);
    let out = timeout(Duration::from_secs(45), revocation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(out["membership_epoch"].as_i64(), Some(reviewer_epoch + 1));
    assert_eq!(
        commit(&stores, REVIEWER, reviewer_epoch, &k45).await,
        Err(Error::Denied)
    );
    problem(
        &get_outcome(&app, &s05, &reviewer_cookie, &key(45)).await,
        StatusCode::NOT_FOUND,
        "not_found",
    );
    assert_eq!(tally(&fixture, &r5).await, [2, 2, 2]);
    group.check("a command paused right after its claim already holds the member row's key share: revoke-membership's FOR UPDATE waits on it (observed), the command's member fence takes FOR SHARE ahead of the queued revocation without deadlock and commits version 2, the revocation then commits, and the revoked principal's retry and lookup are refused");

    // Delivery after the fence: a replay whose key is taken inserts no claim
    // and so holds no member lock before its fence; a revocation committing
    // between its precheck and its fence refuses the stored outcome.
    let reviewer_epoch = restore_member(&fixture, REVIEWER).await;
    assert_eq!(
        commit(&stores, REVIEWER, reviewer_epoch, &k45).await,
        Ok(CommandResult::Replayed(o45.clone()))
    );
    hooks.arm("outcome");
    let replay = spawn_command(&stores, REVIEWER, reviewer_epoch, &k45);
    paused(&hooks).await;
    let out = timeout(
        Duration::from_secs(45),
        spawn_operator(
            &fixture,
            &[
                "revoke-membership",
                "--organization",
                ORG,
                "--principal",
                REVIEWER,
            ],
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        (out["changed"].as_bool(), out["membership_epoch"].as_i64()),
        (Some(true), Some(reviewer_epoch + 1))
    );
    hooks.release.notify_one();
    assert_eq!(replay.await.unwrap(), Err(Error::Denied));
    assert_eq!(tally(&fixture, &r5).await, [2, 2, 2]);
    group.check("a replay of a committed key (the restored reviewer's identical retry replays it) paused after its precheck holds no member lock, so revoke-membership commits without waiting; the resumed replay's fence then refuses delivery of the stored outcome (Denied): current visibility, not the stored outcome, governs the response");
    group.close(&mut groups);

    // ===================================================================
    // SR10: every commit of the campaign is on the data chain, once.
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    let commits = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.command_outcomes WHERE organization_id='{ORG}'"),
    )
    .await;
    assert_eq!(out["command_outcomes"].as_i64(), Some(commits));
    assert_eq!(out["chains"]["data"]["events"].as_i64(), Some(commits));
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.command_outcomes o WHERE o.organization_id='{ORG}' AND NOT EXISTS (SELECT 1 FROM genui.audit_events e WHERE e.organization_id=o.organization_id AND e.event_id=o.audit_event_id AND e.writer_role='data' AND e.kind='command.committed' AND e.record_id=o.record_id AND e.outcome_digest=o.payload_digest)")).await,
        0
    );
    assert_eq!(claims(&fixture).await, commits);
    let mut closing = Vec::new();
    check(&mut closing, &format!("the operator recomputes the data chain to its head: {commits} command.committed events for {commits} outcomes and {commits} claims, every outcome naming its own event; no refused or raced command left a claim, outcome or event"));

    drop(app);
    drop(auth);
    stores.close().await.unwrap();
    drop(stores);
    super::qualification::docker(
        &["rm", "--force", "--volumes", &fixture.name],
        b"",
        "remove isolated database",
    );
    fixture.removed = true;
    std::fs::remove_dir_all(&fixture.directory.0).unwrap();
    check(
        &mut closing,
        "local database and private fixture files removed",
    );
    let total: usize = groups
        .iter()
        .map(|g| g["checks"].as_array().unwrap().len())
        .sum::<usize>()
        + closing.len();
    if let Some(path) = std::env::var_os("GENUI_RACE_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "scope": "APP-014 isolated native PostgreSQL TLS command retry, conflict and authorization race qualification",
            "managed_acceptance": false,
            "server_version": fixture.server_version,
            "s05": {"organization_id": ORG, "app_id": SPEND_APP, "work_id": s05.work, "collection_id": s05.collection,
                    "publication_id": s05.publication, "definition_digest": s05.definition_digest},
            "groups": groups,
            "closing": closing,
            "checks_total": total,
            "commits": commits,
            "session_wait_observation": {"status": session_wait.0.as_u16(), "code": session_wait.1},
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "passed": true,
            "limits": [
                "Synthetic local password authentication is not RDS IAM; the RDS lock-observation repeat of these orders is APP-080's (ADR-008).",
                "Identities are synthetic registry-bound fixtures; HTTP sessions are minted by a test-only constructor after the seeded membership.",
                "Writers that must hold their locks across a checkpoint are fixture-only SQL in the operator channel's order (genui_owner under explicit scope); the unheld revocations are the real operator command. The adoption writer is test-only by D7.4.",
                "Fixture-only restoration re-activates a revoked member between orders (as the APP-008 campaign), writing no operator audit event.",
                "The database itself is not restarted (tmpfs fixture); the restart is of every application pool and store.",
                "Races are serialized by the pool's test-only checkpoints and observed pg_stat_activity lock waits; lock_timeout is the pool baseline's 1000 ms."
            ]
        })).unwrap()).unwrap();
    }
}
