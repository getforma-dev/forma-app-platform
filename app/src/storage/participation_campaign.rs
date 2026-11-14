//! APP-203 (ADR-020 C2, C3): the participation campaign. Isolated test
//! executable only: local Docker, OpenSSL and the locked Python environments,
//! as the other storage campaigns.
//!
//! Two general apps, admitted through the shipped path (create the
//! definition revision, publish, create the work):
//!
//! - `participation`: ADR-020 §10's worked example, verbatim from the
//!   contract specimen (`contracts/fixtures/participation.json`,
//!   `definition_v1`): a shared `items` list with a reviewer-only `post`
//!   (`set_now: opened_at`), `close` and `reopen` (re-stamps); an `own`
//!   `responses` list keyed `one_per_creator: [item]` whose `respond` create
//!   requires the item open and opened within the last 30 s; an `own`
//!   `expected` list.
//! - `sessions`: an `own` `sessions` list whose writable `closes_at` is a
//!   deadline, and an `own` keyed `checkins` list whose `check_in` requires
//!   the referenced session open and `closes_at > now`, stamping
//!   `checked_at`. A reference into another member's own list is the
//!   uniform 404.
//!
//! The actors are created here: Owner (the apps' owner and the works'
//! creator), Rev (a reviewer by a personal reviewer share), Ben and Ana
//! (contributors through the organization-wide share), 25 class members
//! (the same share) and a guest (refused). Every enforcement check marked
//! `fails under bypass` runs twice: as shipped, where it must hold, and with
//! `authorization::BYPASS` set (own_rows TRUE, the key match's creator rule
//! off), where it must fail.
use super::{
    admission_qualification::operator,
    authorization::BYPASS,
    commands::KEY_MATCH_MICROS,
    definition_qualification::{grant, request, POLICY},
    qualification::Fixture,
    qualification_cases::{check, ORG},
    *,
};
use crate::admission::{self, digest};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

/// The contract specimen of ADR-020 §10 (B0's `make_fixtures.participation()`).
const SPECIMEN: &str = include_str!("../../../contracts/fixtures/participation.json");
/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const APP: &str = "00000000-0000-4000-8000-000000000c10";
const APP_REVISION: &str = "00000000-0000-4000-8000-000000000c11";
const SESSIONS_APP: &str = "00000000-0000-4000-8000-000000000c20";
const SESSIONS_REVISION: &str = "00000000-0000-4000-8000-000000000c21";
const OWNER: &str = "00000000-0000-4000-8000-000000000c01";
const REV: &str = "00000000-0000-4000-8000-000000000c02";
const BEN: &str = "00000000-0000-4000-8000-000000000c03";
const ANA: &str = "00000000-0000-4000-8000-000000000c04";
/// The class: 25 members through the organization-wide share.
const CLASS: usize = 25;
/// Items at scale (ADR-020 §15: "measured in APP-203's campaign at 25 × 50").
const ITEMS: usize = 50;
/// ADR-004: the statement budget (`statement_timeout`).
const STATEMENT_BUDGET_MICROS: u64 = 2_000_000;

fn class_member(index: usize) -> String {
    format!("00000000-0000-4000-8000-000000000d{index:02}")
}

static NEXT: AtomicU32 = AtomicU32::new(0x1000);
/// A fresh canonical record id (UUID v4 shape) for this campaign.
fn fresh_id() -> String {
    format!(
        "00000000-0000-4000-8000-{:012x}",
        NEXT.fetch_add(1, Ordering::SeqCst)
    )
}
/// A fresh idempotency key (32 lowercase hex).
fn fresh_key() -> String {
    format!("{:032x}", NEXT.fetch_add(1, Ordering::SeqCst))
}

fn wrap(app: &str, revision: &str, content: Value) -> Value {
    let canonical = admission::encode(&content).unwrap();
    json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": ORG,
        "app_id": app,
        "revision_id": revision,
        "content": content,
        "digest": digest::digest(digest::APP_DEFINITION, &canonical),
    })
}

/// ADR-020 §10's example, verbatim content of the specimen's version-1
/// definition, under this campaign's organization and app.
pub(super) fn definition() -> Value {
    let specimen: Value = serde_json::from_str(SPECIMEN).unwrap();
    wrap(
        APP,
        APP_REVISION,
        specimen["definition_v1"]["content"].clone(),
    )
}

/// The second app: own sessions with a deadline, one check-in per session.
pub(super) fn sessions_definition() -> Value {
    let session_fields = json!([
        {"id": "title", "value_type": {"type": "text"}, "nullable": false},
        {"id": "state", "value_type": {"type": "enum", "values": ["open", "closed"]}, "nullable": false},
        {"id": "closes_at", "value_type": {"type": "timestamp"}, "nullable": false}
    ]);
    let checkin_fields = json!([
        {"id": "session", "value_type": {"type": "reference", "target_input": "sessions"}, "nullable": false},
        {"id": "note", "value_type": {"type": "text"}, "nullable": true},
        {"id": "checked_at", "value_type": {"type": "timestamp"}, "nullable": false}
    ]);
    wrap(
        SESSIONS_APP,
        SESSIONS_REVISION,
        json!({
            "title": "Sessions and check-ins",
            "description": "Synthetic campaign app: each member's own sessions with a deadline, and one check-in per session while it is open.",
            "inputs": [
                {"id": "sessions", "contract_id": "sessions_v1", "contract_version": 1, "kind": "records", "fields": session_fields},
                {"id": "checkins", "contract_id": "checkins_v1", "contract_version": 1, "kind": "records", "fields": checkin_fields}
            ],
            "views": [
                {"id": "open_session", "kind": "form", "renderer_version": 1, "title": "Open a session", "command_id": "open_session"},
                {"id": "check_in", "kind": "form", "renderer_version": 1, "title": "Check in", "command_id": "check_in"},
                {"id": "session_list", "kind": "records_list", "renderer_version": 1, "title": "Sessions", "records": {"version": 1, "input_id": "sessions", "schema_id": "sessions", "columns": ["title", "state", "closes_at"]}}
            ],
            "record_schemas": [
                {"id": "sessions", "version": 1, "visibility": "own", "fields": session_fields},
                {"id": "checkins", "version": 1, "visibility": "own", "one_per_creator": ["session"], "fields": checkin_fields}
            ],
            "commands": [
                {"id": "open_session", "record_schema_id": "sessions", "operation": "create", "writable_fields": ["title", "state", "closes_at"], "transitions": [], "initial": [{"field_id": "state", "value": "open"}]},
                {"id": "close_session", "record_schema_id": "sessions", "operation": "update", "writable_fields": ["state"], "transitions": []},
                {"id": "check_in", "record_schema_id": "checkins", "operation": "create", "writable_fields": ["session", "note"], "transitions": [],
                 "requires": [
                     {"via": "session", "field_id": "state", "comparison": "eq", "value": {"type": "enum", "value": "open"}},
                     {"via": "session", "field_id": "closes_at", "comparison": "gt", "value": {"type": "now"}}
                 ],
                 "set_now": ["checked_at"]}
            ],
            "links": [],
            "default_layout": [
                {"view_id": "open_session", "column": 0, "row": 0, "width": 12},
                {"view_id": "check_in", "column": 0, "row": 1, "width": 12},
                {"view_id": "session_list", "column": 0, "row": 2, "width": 12}
            ]
        }),
    )
}

fn admitted(definition: &Value) -> admission::AdmittedDefinition {
    admission::admit_definition(&serde_json::to_vec(definition).unwrap())
        .expect("the campaign's definition admits")
}

/// Operator-channel stand-in (as tenant_seed writes fixture authority): one
/// active member of the organization with a live entitlement.
fn admit_member(fixture: &Fixture, principal: &str) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{ORG}','{principal}','active',1); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    fixture
        .operator()
        .await
        .client
        .query_one(sql, &[])
        .await
        .unwrap()
        .get(0)
}
async fn text(fixture: &Fixture, sql: &str) -> String {
    fixture
        .operator()
        .await
        .client
        .query_one(sql, &[])
        .await
        .unwrap()
        .get(0)
}

/// The database's instant, whole-second UTC (the commits' `now()` clock).
async fn database_now(fixture: &Fixture) -> String {
    text(fixture, "SELECT to_char(date_trunc('second',now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')").await
}

struct Work {
    app: String,
    work: String,
    publication: String,
    /// `(schema, collection)`.
    collections: Vec<(String, String)>,
}
impl Work {
    fn collection(&self, schema: &str) -> &str {
        &self
            .collections
            .iter()
            .find(|(s, _)| s == schema)
            .expect("bound schema")
            .1
    }
}

#[allow(clippy::too_many_arguments)]
fn command(
    w: &Work,
    command: &str,
    schema: &str,
    record: &str,
    expected: i64,
    values: Value,
    key: &str,
) -> CommandRequest {
    CommandRequest {
        app: w.app.clone(),
        work: w.work.clone(),
        publication: w.publication.clone(),
        command: command.into(),
        schema: schema.into(),
        schema_version_expected: 1,
        record: record.into(),
        expected_record_version: expected,
        expected_work_version: 1,
        values,
        idempotency_key: key.into(),
    }
}

fn enum_value(field: &str, value: &str) -> Value {
    json!({"field_id": field, "value": {"type": "enum", "value": value}})
}
fn reference(field: &str, input: &str, record: &str) -> Value {
    json!({"field_id": field, "value": {"type": "reference", "input_id": input, "record_id": record}})
}
fn post_values(prompt: &str) -> Value {
    json!([
        {"field_id": "prompt", "value": {"type": "text", "value": prompt}},
        enum_value("state", "open")
    ])
}
fn answer_values(item: &str, choice: &str) -> Value {
    json!([
        reference("item", "items", item),
        enum_value("choice", choice)
    ])
}
fn state_values(state: &str) -> Value {
    json!([enum_value("state", state)])
}

/// An answer to `item`: a fresh record id and key (a fresh form).
fn answer(w: &Work, item: &str, choice: &str) -> CommandRequest {
    command(
        w,
        "respond",
        "responses",
        &fresh_id(),
        0,
        answer_values(item, choice),
        &fresh_key(),
    )
}

async fn commit_until_settled(
    stores: &Stores,
    who: &TrustedRequest,
    request: &CommandRequest,
) -> Result<CommandResult> {
    // InFlight and Busy are D7.9's retry-with-the-same-key answers.
    for _ in 0..40 {
        match stores.data.commit_record(who, request).await {
            Ok(CommandResult::InFlight) | Ok(CommandResult::Busy) => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            other => return other,
        }
    }
    panic!("{} never settled", request.command);
}

/// Admission refusals met by the concurrent sections (see [`admitted_commit`]).
static ADMISSION_RETRIES: AtomicU32 = AtomicU32::new(0);

/// A concurrent submission. The process admits at most eight data
/// transactions at once (`pool::capacity`, ADR-004) and refuses the next with
/// `Unavailable` before any statement runs, so nothing was written: like a
/// client after a 503, the same request (same key) is sent again. InFlight
/// and Busy are retried as D7.9 says. A store failure that persists exhausts
/// the retries and fails the campaign.
async fn admitted_commit(
    stores: &Stores,
    who: &TrustedRequest,
    request: &CommandRequest,
) -> Result<CommandResult> {
    for _ in 0..400 {
        match stores.data.commit_record(who, request).await {
            Err(Error::Unavailable) => {
                ADMISSION_RETRIES.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await
            }
            Ok(CommandResult::InFlight) | Ok(CommandResult::Busy) => {
                tokio::time::sleep(Duration::from_millis(20)).await
            }
            other => return other,
        }
    }
    panic!("{} never admitted", request.command);
}

async fn committed(
    stores: &Stores,
    who: &TrustedRequest,
    request: &CommandRequest,
) -> CommandOutcome {
    match commit_until_settled(stores, who, request).await {
        Ok(CommandResult::Committed(outcome)) => outcome,
        other => panic!(
            "{} by {} must commit: {other:?}",
            request.command,
            who.principal()
        ),
    }
}

/// The reviewer-only `post` of a fresh item; its id and outcome.
async fn post(
    stores: &Stores,
    owner: &TrustedRequest,
    w: &Work,
    prompt: &str,
) -> (String, CommandOutcome) {
    let id = fresh_id();
    let outcome = committed(
        stores,
        owner,
        &command(
            w,
            "post",
            "items",
            &id,
            0,
            post_values(prompt),
            &fresh_key(),
        ),
    )
    .await;
    (id, outcome)
}

/// An `update` of `record` at its current `version` (the reviewer's close or
/// reopen; a member's close of their own session).
#[allow(clippy::too_many_arguments)]
async fn update(
    stores: &Stores,
    who: &TrustedRequest,
    w: &Work,
    name: &str,
    schema: &str,
    record: &str,
    version: i64,
    state: &str,
) -> CommandOutcome {
    committed(
        stores,
        who,
        &command(
            w,
            name,
            schema,
            record,
            version,
            state_values(state),
            &fresh_key(),
        ),
    )
    .await
}

async fn record_version(fixture: &Fixture, record: &str) -> i64 {
    scalar(fixture, &format!("SELECT coalesce(max(current_version),0)::bigint FROM genui.internal_records WHERE organization_id='{ORG}' AND record_id='{record}'")).await
}
async fn revisions(fixture: &Fixture, work: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND work_id='{work}'")).await
}
async fn claims(fixture: &Fixture, work: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.command_claims WHERE organization_id='{ORG}' AND work_id='{work}'")).await
}
/// The current records of `collection` created by `creator` whose `field`
/// references `target`.
async fn entries(
    fixture: &Fixture,
    collection: &str,
    creator: &str,
    field: &str,
    target: &str,
) -> Vec<String> {
    let operator = fixture.operator().await;
    let rows = operator.client.query(&format!("SELECT r.record_id::text FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version WHERE r.organization_id='{ORG}' AND r.collection_id='{collection}' AND v.creator_id='{creator}' AND EXISTS (SELECT 1 FROM jsonb_array_elements(convert_from(v.canonical_values,'UTF8')::jsonb) e WHERE e->>'field_id'='{field}' AND e->'value'->>'record_id'='{target}') ORDER BY r.record_id"), &[]).await.unwrap();
    rows.iter().map(|row| row.get(0)).collect()
}
/// A record's current value of an enum, text or timestamp field.
async fn current_value(fixture: &Fixture, record: &str, field: &str) -> String {
    text(fixture, &format!("SELECT coalesce((SELECT e->'value'->>'value' FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version CROSS JOIN LATERAL jsonb_array_elements(convert_from(v.canonical_values,'UTF8')::jsonb) e WHERE r.organization_id='{ORG}' AND r.record_id='{record}' AND e->>'field_id'='{field}'),'')")).await
}

/// An enforcement check: holds as shipped, fails with the rule bypassed.
macro_rules! judged {
    ($checks:expr, $name:expr, $body:block) => {{
        BYPASS.store(false, Ordering::SeqCst);
        let held: bool = (async $body).await;
        BYPASS.store(true, Ordering::SeqCst);
        let bypassed: bool = (async $body).await;
        BYPASS.store(false, Ordering::SeqCst);
        assert!(held, "{} does not hold as shipped", $name);
        assert!(
            !bypassed,
            "{} still holds with the rule bypassed: the check does not depend on it",
            $name
        );
        check($checks, &format!("{} (fails under bypass)", $name));
    }};
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_participation_campaign() {
    std::thread::Builder::new()
        .name("app203-participation-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Creates, publishes and saves one campaign app for `owner`; the owner
/// shares it with Rev (reviewer) and the organization (contributors).
async fn install(
    fixture: &Fixture,
    stores: &Stores,
    owner: &TrustedRequest,
    app: &str,
    definition: &Value,
    title: &str,
    schemas: &[&str],
) -> Work {
    operator(
        fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            app,
            "--space",
            SPACE,
            "--owner",
            OWNER,
        ],
    );
    for permission in ["discover", "run"] {
        grant(fixture, OWNER, permission, &["--app", app]);
    }
    let revision = stores
        .workspace
        .create_definition_revision(owner, app, &admitted(definition))
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(owner, app, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let bindings: Vec<WorkBinding> = schemas
        .iter()
        .map(|schema| WorkBinding::Records {
            input: (*schema).into(),
            schema: (*schema).into(),
        })
        .collect();
    let saved = stores
        .workspace
        .create_work(owner, app, &publication.publication_id, title, &bindings)
        .await
        .unwrap();
    let mut collections = Vec::new();
    for schema in schemas {
        let collection = text(fixture, &format!("SELECT collection_id::text FROM genui.record_collections WHERE organization_id='{ORG}' AND work_id='{}' AND schema_id='{schema}'", saved.work)).await;
        collections.push(((*schema).to_owned(), collection));
    }
    for (audience, role) in [
        (Audience::Member(REV), ShareRole::Reviewer),
        (Audience::Organization, ShareRole::Contributor),
    ] {
        stores
            .workspace
            .share_work(owner, app, &saved.work, audience, role, ShareMode::Set)
            .await
            .unwrap();
    }
    Work {
        app: app.into(),
        work: saved.work,
        publication: publication.publication_id,
        collections,
    }
}

async fn campaign() {
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    operator(&fixture, &["backfill", "--organization", ORG]);
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let mut checks = Vec::new();
    let class: Vec<String> = (1..=CLASS).map(class_member).collect();
    for member in [OWNER, REV, BEN, ANA]
        .into_iter()
        .chain(class.iter().map(String::as_str))
    {
        admit_member(&fixture, member);
    }
    // A trusted request is not Clone by design; tasks share one by Arc.
    let owner = Arc::new(request(&stores, ORG, OWNER, 1));
    let rev = Arc::new(request(&stores, ORG, REV, 1));
    let ben = Arc::new(request(&stores, ORG, BEN, 1));
    let ana = Arc::new(request(&stores, ORG, ANA, 1));
    let members: Vec<Arc<TrustedRequest>> = class
        .iter()
        .map(|member| Arc::new(request(&stores, ORG, member, 1)))
        .collect();

    // --- the apps, through the admitted path --------------------------------
    let w = install(
        &fixture,
        &stores,
        &owner,
        APP,
        &definition(),
        "Participation (synthetic campaign)",
        &["items", "responses", "expected"],
    )
    .await;
    let s = install(
        &fixture,
        &stores,
        &owner,
        SESSIONS_APP,
        &sessions_definition(),
        "Sessions (synthetic campaign)",
        &["sessions", "checkins"],
    )
    .await;
    let (work, responses) = (w.work.as_str(), w.collection("responses").to_owned());
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{APP}' AND owner_only")).await,
        2
    );
    check(&mut checks, "the ADR-020 §10 example and the sessions app admit, publish and save through the shipped path; `responses` and `expected` are own collections; Rev is a reviewer, everyone else a contributor through the organization-wide share");

    // --- C3 set_now: the post stamps the transaction's instant --------------
    let (item, posted) = post(&stores, &owner, &w, "Item one").await;
    assert_eq!(
        current_value(&fixture, &item, "opened_at").await,
        posted.committed_at
    );
    assert_eq!(current_value(&fixture, &item, "state").await, "open");
    let stored_at = text(&fixture, &format!("SELECT to_char(committed_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND record_id='{item}' AND version=1")).await;
    assert_eq!(stored_at, posted.committed_at);
    // A contributor can neither post nor supply the stamp.
    let mut forged = post_values("forged").as_array().unwrap().clone();
    forged.push(json!({"field_id": "opened_at", "value": {"type": "timestamp", "value": "2030-01-01T00:00:00Z"}}));
    for (who, values) in [
        (&ben, post_values("by a contributor")),
        (&owner, Value::Array(forged)),
    ] {
        let attempt = command(&w, "post", "items", &fresh_id(), 0, values, &fresh_key());
        assert_eq!(
            stores.data.commit_record(who, &attempt).await,
            Err(Error::Denied)
        );
    }
    check(&mut checks, "C3 set_now: a post stamps opened_at with the transaction's instant, equal to the revision's committed_at and the outcome's; a contributor cannot post (reviewer_only) and nobody can supply the stamp");

    // --- C2: one entry per person per key -------------------------------------
    let first = answer(&w, &item, "a");
    let first_outcome = committed(&stores, &ben, &first).await;
    assert_eq!(
        (
            first_outcome.record_id.as_str(),
            first_outcome.record_version
        ),
        (first.record.as_str(), 1)
    );
    let ben_entry = first.record.clone();
    let dry = stores
        .data
        .dry_run_command(&ben, &answer(&w, &item, "c"))
        .await
        .unwrap();
    assert!(
        matches!(&dry, DryRun::Valid { replaces: Some(id), current_version: 0, .. } if *id == ben_entry),
        "{dry:?}"
    );
    let events = |record: &str| {
        format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND kind='command.committed' AND record_id='{record}'")
    };
    let events_before = scalar(&fixture, &events(&ben_entry)).await;
    let second = answer(&w, &item, "b");
    let second_outcome = committed(&stores, &ben, &second).await;
    assert_eq!(
        (
            second_outcome.record_id.as_str(),
            second_outcome.record_version
        ),
        (ben_entry.as_str(), 2),
        "the replacing create names the first entry at its next version"
    );
    assert_eq!(
        record_version(&fixture, &second.record).await,
        0,
        "the second form's fresh id is never created"
    );
    assert_eq!(
        entries(&fixture, &responses, BEN, "item", &item).await,
        std::slice::from_ref(&ben_entry)
    );
    assert_eq!(current_value(&fixture, &ben_entry, "choice").await, "b");
    assert_eq!(
        scalar(&fixture, &events(&ben_entry)).await,
        events_before + 1
    );
    assert_eq!(scalar(&fixture, &events(&second.record)).await, 0);
    assert_eq!(
        text(&fixture, &format!("SELECT record_id::text||' '||record_version FROM genui.command_outcomes WHERE organization_id='{ORG}' AND idempotency_key='{}'", second.idempotency_key)).await,
        format!("{ben_entry} 2")
    );
    assert_eq!(
        text(&fixture, &format!("SELECT creator_id::text||' '||updater_id::text FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND record_id='{ben_entry}' AND version=2")).await,
        format!("{BEN} {BEN}")
    );
    // A replay of the replacing form returns its stored outcome (naming Y).
    assert_eq!(
        stores.data.commit_record(&ben, &second).await,
        Ok(CommandResult::Replayed(second_outcome.clone()))
    );
    check(&mut checks, "C2: a contributor's second create for the same key replaces the first (version 2 of the first record, one command.committed event and one outcome naming it, creator and updater the contributor); the second form's id is never created; the dry run names the entry it replaces; a replay returns the stored outcome");

    let (before_revisions, before_claims) = (
        revisions(&fixture, work).await,
        claims(&fixture, work).await,
    );
    let same = answer(&w, &item, "b");
    assert_eq!(
        stores.data.commit_record(&ben, &same).await,
        Ok(CommandResult::Refused(Refusal::Unchanged))
    );
    assert_eq!(
        stores
            .data
            .dry_run_command(&ben, &answer(&w, &item, "b"))
            .await,
        Ok(DryRun::Refused(Refusal::Unchanged))
    );
    assert_eq!(revisions(&fixture, work).await, before_revisions);
    assert_eq!(
        claims(&fixture, work).await,
        before_claims,
        "the claim is released"
    );
    assert_eq!(record_version(&fixture, &ben_entry).await, 2);
    // The released key is judged again: a different value under it commits.
    let mut retried = same.clone();
    retried.values = answer_values(&item, "d");
    assert_eq!(
        committed(&stores, &ben, &retried).await.record_id,
        ben_entry
    );
    check(&mut checks, "C2: a create with the values the entry already holds is 422 unchanged (dry run too): nothing written, the claim released, so the same key is judged again later");

    judged!(&mut checks, "C2 x D7: two creators with the same key keep two entries; neither create (nor the dry run) touches the other's", {
        let (item, _) = post(&stores, &owner, &w, "Item two").await;
        let ben_answer = answer(&w, &item, "a");
        let ben_ok = matches!(commit_until_settled(&stores, &ben, &ben_answer).await, Ok(CommandResult::Committed(o)) if o.record_id == ben_answer.record);
        let ana_answer = answer(&w, &item, "b");
        let dry = stores.data.dry_run_command(&ana, &ana_answer).await;
        let dry_ok = matches!(dry, Ok(DryRun::Valid { replaces: None, .. }));
        let ana_ok = matches!(commit_until_settled(&stores, &ana, &ana_answer).await, Ok(CommandResult::Committed(o)) if o.record_id == ana_answer.record && o.record_version == 1);
        ben_ok
            && dry_ok
            && ana_ok
            && entries(&fixture, &responses, BEN, "item", &item).await == std::slice::from_ref(&ben_answer.record)
            && entries(&fixture, &responses, ANA, "item", &item).await == std::slice::from_ref(&ana_answer.record)
            && current_value(&fixture, &ben_answer.record, "choice").await == "a"
    });
    judged!(&mut checks, "C2 x D7: a reviewer's own create never matches a contributor's entry, although the reviewer reads every entry", {
        let (item, _) = post(&stores, &owner, &w, "Item three").await;
        let ben_answer = answer(&w, &item, "a");
        let ben_ok = matches!(commit_until_settled(&stores, &ben, &ben_answer).await, Ok(CommandResult::Committed(_)));
        let reads_it = stores.data.list_records(&rev, &w.app, work).await.is_ok_and(|l| l.records.iter().any(|r| r.record_id == ben_answer.record));
        let rev_answer = answer(&w, &item, "c");
        let rev_ok = matches!(commit_until_settled(&stores, &rev, &rev_answer).await, Ok(CommandResult::Committed(o)) if o.record_id == rev_answer.record && o.record_version == 1);
        ben_ok
            && reads_it
            && rev_ok
            && current_value(&fixture, &ben_answer.record, "choice").await == "a"
    });
    let listing = stores.data.list_records(&rev, &w.app, work).await.unwrap();
    let in_listing = |creator: &str, item_id: &str| {
        listing
            .records
            .iter()
            .filter(|r| {
                r.collection == responses
                    && r.creator == creator
                    && r.values.to_string().contains(item_id)
            })
            .count()
    };
    assert_eq!((in_listing(BEN, &item), in_listing(ANA, &item)), (1, 0));
    check(&mut checks, "C2: the reviewer's listing counts one entry per creator per key (Ben's four answers to item one are one record)");

    // --- C3 requires: open, closed, reopened, outside the window --------------
    let before = revisions(&fixture, work).await;
    let open_answer = answer(&w, &item, "a");
    assert!(matches!(
        stores.data.dry_run_command(&ana, &open_answer).await,
        Ok(DryRun::Valid { replaces: None, .. })
    ));
    committed(&stores, &ana, &open_answer).await;
    let closed = update(&stores, &owner, &w, "close", "items", &item, 1, "closed").await;
    assert_eq!(closed.record_version, 2);
    let late = answer(&w, &item, "c");
    assert_eq!(
        stores.data.dry_run_command(&ana, &late).await,
        Ok(DryRun::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        stores.data.commit_record(&ana, &late).await,
        Ok(CommandResult::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        revisions(&fixture, work).await,
        before + 2,
        "the answer and the close only"
    );
    assert_eq!(
        current_value(&fixture, &open_answer.record, "choice").await,
        "a"
    );
    // A contributor cannot reopen; the reviewer's reopen re-stamps.
    assert_eq!(
        stores
            .data
            .commit_record(
                &ben,
                &command(
                    &w,
                    "reopen",
                    "items",
                    &item,
                    2,
                    state_values("open"),
                    &fresh_key()
                )
            )
            .await,
        Err(Error::Denied)
    );
    let reopened = update(&stores, &owner, &w, "reopen", "items", &item, 2, "open").await;
    assert_eq!(
        current_value(&fixture, &item, "opened_at").await,
        reopened.committed_at
    );
    let again = committed(&stores, &ana, &answer(&w, &item, "c")).await;
    assert_eq!(
        (again.record_id.as_str(), again.record_version),
        (open_answer.record.as_str(), 2)
    );
    check(&mut checks, "C3: an answer while the item is open and inside its window commits; after the reviewer's close it is 422 requirement_not_met (dry run too) and nothing is written; a contributor cannot reopen; the reviewer's reopen re-stamps opened_at and answers are accepted again (replacing the entry)");

    // A reference to no item, to a record of another collection, to an id
    // outside the work: the uniform 404, in the dry run and the commit.
    let expected_entry = fresh_id();
    committed(
        &stores,
        &owner,
        &command(
            &w,
            "add_expected",
            "expected",
            &expected_entry,
            0,
            answer_values(&item, "c"),
            &fresh_key(),
        ),
    )
    .await;
    let nothing = fresh_id();
    let outside = s.collection("sessions").to_owned();
    for target in [&nothing, &expected_entry, &outside] {
        let request = answer(&w, target, "a");
        assert_eq!(
            stores.data.dry_run_command(&ana, &request).await,
            Err(Error::Denied),
            "{target}"
        );
        assert_eq!(
            stores.data.commit_record(&ana, &request).await,
            Err(Error::Denied),
            "{target}"
        );
    }
    check(&mut checks, "C3: a reference to a record that does not exist, to a record of another collection or to an id outside the work is the uniform 404 (commit and dry run), never requirement_not_met");

    // The window: opened_at gte now − 30 s, on the database's clock.
    let (windowed, windowed_post) = post(&stores, &owner, &w, "Item with a window").await;
    let inside = committed(&stores, &ben, &answer(&w, &windowed, "a")).await;
    let opened = rules::unix_seconds(&windowed_post.committed_at).unwrap();
    while rules::unix_seconds(&database_now(&fixture).await).unwrap() <= opened + 30 {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let expired = answer(&w, &windowed, "b");
    assert_eq!(
        stores.data.dry_run_command(&ben, &expired).await,
        Ok(DryRun::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        stores.data.commit_record(&ben, &expired).await,
        Ok(CommandResult::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        current_value(&fixture, &inside.record_id, "choice").await,
        "a"
    );
    assert_eq!(current_value(&fixture, &windowed, "state").await, "open");
    let restamped = update(&stores, &owner, &w, "reopen", "items", &windowed, 1, "open").await;
    assert_eq!(
        current_value(&fixture, &windowed, "opened_at").await,
        restamped.committed_at
    );
    let inside_again = committed(&stores, &ben, &answer(&w, &windowed, "b")).await;
    assert_eq!(inside_again.record_id, inside.record_id);
    check(&mut checks, "C3 now: 31 s after the post (the database's clock) an open item refuses answers with requirement_not_met (dry run too), its state still open; the reviewer's reopen (state unchanged) re-stamps opened_at and answers are accepted again");

    // --- C3 via across own lists, and now against closes_at -------------------
    let session = |title: &str, closes_at: &str| {
        json!([
            {"field_id": "title", "value": {"type": "text", "value": title}},
            enum_value("state", "open"),
            {"field_id": "closes_at", "value": {"type": "timestamp", "value": closes_at}}
        ])
    };
    let check_in = |target: &str, note: &str| {
        command(
            &s,
            "check_in",
            "checkins",
            &fresh_id(),
            0,
            json!([reference("session", "sessions", target), {"field_id": "note", "value": {"type": "text", "value": note}}]),
            &fresh_key(),
        )
    };
    let tomorrow =
        rules::timestamp_of(rules::unix_seconds(&database_now(&fixture).await).unwrap() + 86_400)
            .unwrap();
    let ben_session = fresh_id();
    committed(
        &stores,
        &ben,
        &command(
            &s,
            "open_session",
            "sessions",
            &ben_session,
            0,
            session("Ben's session", &tomorrow),
            &fresh_key(),
        ),
    )
    .await;
    let mine = committed(&stores, &ben, &check_in(&ben_session, "here")).await;
    assert_eq!(
        current_value(&fixture, &mine.record_id, "checked_at").await,
        mine.committed_at
    );
    check(&mut checks, "C3 set_now on a contributor's create: checked_at is the transaction's instant (the outcome's committed_at)");
    judged!(&mut checks, "C3 x D7: a via reference into another member's own list is the uniform 404 (commit and dry run); the creator and a reviewer resolve it", {
        let probe = check_in(&ben_session, "not mine");
        let dry = stores.data.dry_run_command(&ana, &probe).await;
        let result = stores.data.commit_record(&ana, &probe).await;
        let reviewer = matches!(commit_until_settled(&stores, &rev, &check_in(&ben_session, "reviewer")).await, Ok(CommandResult::Committed(_)) | Ok(CommandResult::Refused(Refusal::Unchanged)));
        let creator = matches!(stores.data.dry_run_command(&ben, &check_in(&ben_session, "again")).await, Ok(DryRun::Valid { .. }));
        let unwritten = record_version(&fixture, &probe.record).await == 0;
        dry == Err(Error::Denied) && result == Err(Error::Denied) && reviewer && creator && unwritten
    });
    // Ben closes his own session: his next check-in is refused.
    let session_closed = update(
        &stores,
        &ben,
        &s,
        "close_session",
        "sessions",
        &ben_session,
        1,
        "closed",
    )
    .await;
    assert_eq!(session_closed.record_version, 2);
    assert_eq!(
        stores
            .data
            .commit_record(&ben, &check_in(&ben_session, "after close"))
            .await,
        Ok(CommandResult::Refused(Refusal::RequirementNotMet))
    );
    // A deadline three seconds out: accepted before it, refused at it.
    let soon = rules::timestamp_of(rules::unix_seconds(&database_now(&fixture).await).unwrap() + 3)
        .unwrap();
    let deadline_session = fresh_id();
    committed(
        &stores,
        &ana,
        &command(
            &s,
            "open_session",
            "sessions",
            &deadline_session,
            0,
            session("Ana's session", &soon),
            &fresh_key(),
        ),
    )
    .await;
    let before_deadline = committed(&stores, &ana, &check_in(&deadline_session, "early")).await;
    while database_now(&fixture).await.as_str() < soon.as_str() {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let after = check_in(&deadline_session, "late");
    assert_eq!(
        stores.data.dry_run_command(&ana, &after).await,
        Ok(DryRun::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        stores.data.commit_record(&ana, &after).await,
        Ok(CommandResult::Refused(Refusal::RequirementNotMet))
    );
    assert_eq!(
        record_version(&fixture, &before_deadline.record_id).await,
        1
    );
    check(&mut checks, "C3 now: `closes_at > now` accepts a check-in before the deadline and refuses one at it (requirement_not_met, dry run too); a member's close of their own session refuses their next check-in");

    // --- APP-205: the reference picker over the shipped reads -----------------
    // A member's form: the record detail (its bindings) and the member's own
    // listing (under own_rows), the picker filled as the canvas, the record
    // page and the SSR form fill it.
    async fn picker(
        stores: &Stores,
        who: &TrustedRequest,
        w: &Work,
        schema: &str,
        command_id: &str,
    ) -> crate::commands::RecordView {
        let detail = stores
            .data
            .read_record_detail(who, &w.app, &w.work, w.collection(schema), &fresh_id())
            .await
            .unwrap();
        let listing = stores
            .data
            .list_records(who, &w.app, &w.work)
            .await
            .unwrap();
        assert_eq!(
            (listing.publication.as_str(), listing.work_version),
            (detail.publication.as_str(), detail.work_version)
        );
        let mut view = crate::commands::view_for_command(
            &w.app,
            &w.work,
            &detail,
            "csrf".into(),
            fresh_key(),
            Some(command_id),
        );
        crate::commands::fill_references(&mut view, &detail.bindings, &listing);
        view
    }
    let row = |view: &crate::commands::RecordView, id: &str| {
        view.fields
            .iter()
            .find(|row| row.id == id)
            .cloned()
            .unwrap()
    };
    let (picked_item, _) = post(&stores, &owner, &w, "Picker item").await;
    let form = picker(&stores, &ben, &w, "responses", "respond").await;
    let item_row = row(&form, "item");
    let newest_items = {
        let operator = fixture.operator().await;
        operator.client.query(&format!("SELECT r.record_id::text FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version WHERE r.organization_id='{ORG}' AND r.collection_id='{}' ORDER BY v.commit_sequence DESC, r.record_id", w.collection("items")), &[]).await.unwrap().iter().map(|row| row.get::<_, String>(0)).collect::<Vec<_>>()
    };
    assert_eq!(
        item_row
            .choices
            .iter()
            .map(|c| c.value.clone())
            .collect::<Vec<_>>(),
        newest_items,
        "every readable item, newest commit first"
    );
    assert_eq!(item_row.choices[0].value, picked_item);
    assert!(
        item_row.choices[0].selected && item_row.choices.iter().filter(|c| c.selected).count() == 1
    );
    assert_eq!(item_row.choices[0].label, "Picker item");
    assert!(!item_row.nothing_to_choose);
    // The stamped field of the reviewer's post form is read-only.
    let post_form = picker(&stores, &owner, &w, "items", "post").await;
    assert!(!row(&post_form, "opened_at").writable && row(&post_form, "prompt").writable);
    // Submitting the selection through the native form's own parse commits
    // the typed reference storage judges.
    let field = |name: &str, value: &str| (name.to_owned(), value.to_owned());
    let choice_row = row(&form, "choice");
    let fields = vec![
        field("csrf", "csrf"),
        field("idempotency_key", &form.idempotency_key),
        field("collection_id", &form.collection),
        field("publication_id", &form.publication),
        field("schema_id", &form.schema),
        field("schema_version_expected", &form.schema_version),
        field("record_id", &form.record_id),
        field("expected_record_version", &form.version),
        field("expected_work_version", &form.work_version),
        field(&item_row.input, &item_row.choices[0].value),
        field(&item_row.spec_input, &item_row.spec),
        field(&choice_row.input, "c"),
        field(&choice_row.spec_input, &choice_row.spec),
    ];
    let (submitted, collection) =
        crate::commands::routes::parse_form_for_tests(&fields, &w.app, &w.work, "respond").unwrap();
    assert_eq!(collection, responses);
    let outcome = committed(&stores, &ben, &submitted).await;
    assert_eq!(
        entries(&fixture, &responses, BEN, "item", &picked_item).await,
        std::slice::from_ref(&outcome.record_id)
    );
    check(&mut checks, "APP-205: a contributor's respond form offers every readable item newest commit first, the current (most recently touched) item preselected and labelled by its prompt; the post form's stamped opened_at is read-only; the selection posted through the native form's parse commits a typed reference storage accepts");
    judged!(&mut checks, "APP-205 x D7: a picker over an own list offers only the records the member may read (a contributor their own, a reviewer every member's); a member who can read none gets the disabled picker", {
        let ana_form = picker(&stores, &ana, &s, "checkins", "check_in").await;
        let ana_row = row(&ana_form, "session");
        let ana_ids: Vec<String> = ana_row.choices.iter().map(|c| c.value.clone()).collect();
        let rev_form = picker(&stores, &rev, &s, "checkins", "check_in").await;
        let rev_ids: Vec<String> = row(&rev_form, "session").choices.iter().map(|c| c.value.clone()).collect();
        let empty_form = picker(&stores, &members[0], &s, "checkins", "check_in").await;
        let empty_row = row(&empty_form, "session");
        ana_ids == std::slice::from_ref(&deadline_session)
            && rev_ids.contains(&ben_session)
            && rev_ids.contains(&deadline_session)
            && empty_row.nothing_to_choose
            && empty_row.choices.is_empty()
    });

    // --- C2 concurrency: 25 simultaneous creates by one creator for one key ---
    let (race_item, _) = post(&stores, &owner, &w, "Race item").await;
    ADMISSION_RETRIES.store(0, Ordering::SeqCst);
    let choices = ["a", "b", "c", "d"];
    let requests: Vec<CommandRequest> = (0..25)
        .map(|i| answer(&w, &race_item, choices[i % 4]))
        .collect();
    let mut tasks = Vec::new();
    for request in requests.clone() {
        let (stores, ben) = (stores.clone(), ben.clone());
        tasks.push(tokio::spawn(async move {
            admitted_commit(&stores, &ben, &request).await
        }));
    }
    let mut outcomes = Vec::new();
    let mut unchanged = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(CommandResult::Committed(outcome)) => outcomes.push(outcome),
            Ok(CommandResult::Refused(Refusal::Unchanged)) => unchanged += 1,
            other => panic!("a racing create must commit or be unchanged: {other:?}"),
        }
    }
    let entry = entries(&fixture, &responses, BEN, "item", &race_item).await;
    assert_eq!(entry.len(), 1, "exactly one entry");
    assert!(
        outcomes.iter().all(|o| o.record_id == entry[0]),
        "every outcome names that entry"
    );
    let mut versions: Vec<i64> = outcomes.iter().map(|o| o.record_version).collect();
    versions.sort_unstable();
    assert_eq!(versions, (1..=outcomes.len() as i64).collect::<Vec<_>>());
    assert_eq!(
        record_version(&fixture, &entry[0]).await,
        outcomes.len() as i64
    );
    assert!(choices.contains(&current_value(&fixture, &entry[0], "choice").await.as_str()));
    for request in &requests {
        if request.record != entry[0] {
            assert_eq!(record_version(&fixture, &request.record).await, 0);
        }
    }
    assert_eq!(outcomes.len() + unchanged, 25);
    let race_retries = ADMISSION_RETRIES.swap(0, Ordering::SeqCst);
    check(&mut checks, &format!("C2 race: 25 simultaneous creates by one creator for one key leave exactly one entry whose state is one of the submitted states; its {} commits have versions 1..{} and every outcome names it; the other {unchanged} were 422 unchanged; no other requested id exists ({race_retries} admission refusals, each resent with its key)", outcomes.len(), outcomes.len()));

    // The D7.9 order: paused right after the key match, a replacing create
    // holds the organization's commit sequence (taken first) and the matched
    // entry's row (taken by the match): neither can be locked by anyone else.
    {
        let (lock_item, _) = post(&stores, &owner, &w, "Lock order item").await;
        let held = committed(&stores, &ben, &answer(&w, &lock_item, "a")).await;
        let hooks = stores.hooks();
        hooks.arm("key_match");
        let (stores_b, ben_b) = (stores.clone(), ben.clone());
        let replace = answer(&w, &lock_item, "b");
        let replacing =
            tokio::spawn(async move { stores_b.data.commit_record(&ben_b, &replace).await });
        super::command_qualification::paused(&hooks).await;
        let probe = fixture.operator().await;
        let busy = |result: std::result::Result<u64, tokio_postgres::Error>| {
            result.is_err_and(|error| {
                error.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE)
            })
        };
        let sequence_held = busy(probe.client.execute(&format!("SELECT 1 FROM genui.organization_sequences WHERE organization_id='{ORG}' FOR UPDATE NOWAIT"), &[]).await);
        let entry_held = busy(probe.client.execute(&format!("SELECT 1 FROM genui.internal_records WHERE organization_id='{ORG}' AND record_id='{}' FOR UPDATE NOWAIT", held.record_id), &[]).await);
        hooks.release.notify_one();
        let replaced = replacing.await.unwrap();
        assert!(
            sequence_held,
            "the organization's sequence is held at the key match"
        );
        assert!(
            entry_held,
            "the matched entry's row is locked by the key match"
        );
        assert!(
            matches!(&replaced, Ok(CommandResult::Committed(o)) if o.record_id == held.record_id && o.record_version == 2),
            "{replaced:?}"
        );
    }
    check(&mut checks, "C2 lock order (D7.9): paused right after the key match, a replacing create holds the organization's commit sequence and the matched entry's row (both refuse FOR UPDATE NOWAIT to anyone else), then commits the entry's next version");

    // --- C3 race: a close against answers --------------------------------------
    let mut accepted_total = 0;
    let mut refused_total = 0;
    for round in 0..6 {
        let (race, _) = post(&stores, &owner, &w, &format!("Close race {round}")).await;
        let mut answers = Vec::new();
        let mut closing = None;
        for (index, member) in members.iter().take(8).enumerate() {
            if index == 4 {
                // The close is sent in the middle of the answers.
                let (stores_c, owner_c) = (stores.clone(), owner.clone());
                let close = command(
                    &w,
                    "close",
                    "items",
                    &race,
                    1,
                    state_values("closed"),
                    &fresh_key(),
                );
                closing = Some(tokio::spawn(async move {
                    admitted_commit(&stores_c, &owner_c, &close).await
                }));
            }
            let (stores_m, member) = (stores.clone(), member.clone());
            let request = answer(&w, &race, "a");
            answers.push((
                request.record.clone(),
                tokio::spawn(async move { admitted_commit(&stores_m, &member, &request).await }),
            ));
        }
        let closed = match closing.unwrap().await.unwrap() {
            Ok(CommandResult::Committed(outcome)) => outcome,
            other => panic!("the close commits: {other:?}"),
        };
        for (record, task) in answers {
            match task.await.unwrap() {
                Ok(CommandResult::Committed(outcome)) => {
                    assert!(
                        outcome.commit_sequence < closed.commit_sequence,
                        "an accepted answer committed before the close"
                    );
                    accepted_total += 1;
                }
                Ok(CommandResult::Refused(Refusal::RequirementNotMet)) => {
                    assert_eq!(record_version(&fixture, &record).await, 0);
                    refused_total += 1;
                }
                other => panic!("an answer racing a close commits or is refused: {other:?}"),
            }
        }
        let late = scalar(&fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions v WHERE v.organization_id='{ORG}' AND v.collection_id='{responses}' AND v.commit_sequence>{} AND position('{race}' in convert_from(v.canonical_values,'UTF8'))>0", closed.commit_sequence)).await;
        assert_eq!(
            late, 0,
            "no answer to a closed item commits after its close"
        );
    }
    check(&mut checks, &format!("C3 race: in 6 rounds of a reviewer's close against 8 members' answers, every accepted answer ({accepted_total}) committed before the close in the organization's sequence and every other ({refused_total}) was 422 requirement_not_met with nothing written"));

    // --- C2 at scale: 25 members × 50 items --------------------------------------
    KEY_MATCH_MICROS.lock().unwrap().clear();
    let started = std::time::Instant::now();
    let mut last_item = String::new();
    for index in 0..ITEMS {
        let (scale_item, _) = post(&stores, &owner, &w, &format!("Scale item {index}")).await;
        let mut tasks = Vec::new();
        for member in &members {
            let (stores_m, member) = (stores.clone(), member.clone());
            let request = answer(&w, &scale_item, choices[index % 4]);
            tasks.push(tokio::spawn(async move {
                admitted_commit(&stores_m, &member, &request).await
            }));
        }
        for task in tasks {
            let result = task.await.unwrap();
            assert!(
                matches!(&result, Ok(CommandResult::Committed(o)) if o.record_version == 1),
                "{result:?}"
            );
        }
        last_item = scale_item;
    }
    // Each member then replaces their answer to the last item: the match finds
    // one row among their fifty.
    for member in &members {
        let outcome = committed(&stores, member, &answer(&w, &last_item, "d")).await;
        assert_eq!(outcome.record_version, 2);
    }
    let elapsed = started.elapsed();
    let mut timings = KEY_MATCH_MICROS.lock().unwrap().clone();
    timings.sort_unstable();
    let at = |q: f64| timings[((timings.len() as f64 - 1.0) * q).round() as usize];
    let (p50, p95, max) = (at(0.5), at(0.95), *timings.last().unwrap());
    assert!(timings.len() >= CLASS * ITEMS + CLASS);
    assert!(
        max < STATEMENT_BUDGET_MICROS,
        "the key match stays inside ADR-004's 2 s budget"
    );
    let class_list = class
        .iter()
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(",");
    // The class also answered the close races (accepted answers only).
    let shape = text(&fixture, &format!("SELECT count(*)::text||' '||coalesce(max(n),0)::text FROM (SELECT v.creator_id, e->'value'->>'record_id' AS item, count(*) AS n FROM genui.internal_records r JOIN genui.internal_record_revisions v ON v.organization_id=r.organization_id AND v.app_id=r.app_id AND v.work_id=r.work_id AND v.collection_id=r.collection_id AND v.record_id=r.record_id AND v.version=r.current_version CROSS JOIN LATERAL jsonb_array_elements(convert_from(v.canonical_values,'UTF8')::jsonb) e WHERE r.organization_id='{ORG}' AND r.collection_id='{responses}' AND v.creator_id IN ({class_list}) AND e->>'field_id'='item' GROUP BY 1,2) g")).await;
    assert_eq!(
        shape,
        format!("{} 1", CLASS * ITEMS + accepted_total),
        "one entry per member per item"
    );
    let scale_retries = ADMISSION_RETRIES.swap(0, Ordering::SeqCst);
    check(&mut checks, &format!("C2 scale: {CLASS} members × {ITEMS} items commit ({} key matches in {:.1} s; the match statement p50 {p50} µs, p95 {p95} µs, max {max} µs, budget 2,000,000 µs; {scale_retries} admission refusals resent) and the collection holds exactly one entry per member per item", timings.len(), elapsed.as_secs_f64()));

    // --- guests -----------------------------------------------------------------
    let share = NewLiveShare {
        recipient_email: "guest@example.invalid".into(),
        allow_add_to_canvas: false,
        expires_in_days: 7,
        idempotency_key: "00000000-0000-4000-8000-00000000c401".into(),
    };
    assert_eq!(
        stores
            .workspace
            .create_live_share(&owner, APP, work, &share)
            .await
            .err(),
        Some(Error::Denied)
    );
    check(&mut checks, "guests: a live share of the participation work (own collections) is refused, so a guest never reaches a command");

    let report = json!({
        "campaign": "APP-203-participation",
        "passed": true,
        "checks": checks,
        "definition": definition()["content"],
        "sessions_definition": sessions_definition()["content"],
        "actors": {"owner": OWNER, "reviewer": REV, "contributors": [BEN, ANA], "class": class},
        "works": {"participation": w.work, "sessions": s.work},
        "key_match_micros": {"count": timings.len(), "p50": p50, "p95": p95, "max": max, "budget": STATEMENT_BUDGET_MICROS},
        "admission_refusals_resent": {"race": race_retries, "scale": scale_retries},
        "bypass": "authorization::BYPASS: own_rows TRUE, hidden empty, the reviewer_only gate and the key match's creator rule off; every check marked `fails under bypass` was run in both modes",
    });
    if let Some(path) = std::env::var_os("GENUI_PARTICIPATION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// Both campaign apps admit through the shipped admission: the §10
    /// example verbatim (keyed `responses`, `requires` with `via` and `now`,
    /// `set_now`) and the sessions app (a deadline guard with no offset).
    #[test]
    fn the_campaign_definitions_admit() {
        for definition in [super::definition(), super::sessions_definition()] {
            let admitted = super::admitted(&definition);
            let content = crate::admission::decode(&admitted.canonical).unwrap();
            let keyed: Vec<&Value> = content["record_schemas"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|schema| schema.get("one_per_creator").is_some())
                .collect();
            assert_eq!(keyed.len(), 1);
            assert_eq!(keyed[0]["visibility"], "own");
            let guarded = content["commands"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|command| command.get("requires").is_some())
                .count();
            assert_eq!(guarded, 1);
        }
    }
}
