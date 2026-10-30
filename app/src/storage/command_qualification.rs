//! APP-013 command campaign. Isolated test executable only: local Docker,
//! OpenSSL and the locked Python environments, as the APP-008, APP-073 and
//! APP-010 campaigns. The Northstar Spend Review S05 fixture is created
//! through the admitted path (`spend_review_s05`), the operator backfills the
//! D7.7 sequence row, and the data role commits the declared `record_decision`
//! command through the storage module over verified TLS and through the HTTP
//! route with a minted workspace session. Every acceptance item of APP-013 is
//! asserted on the database (row counts, chain events, the sequence), never on
//! a response alone; concurrency uses the pool's test-only checkpoints and
//! observed PostgreSQL lock waits, never sleeps.
use super::{
    admission_qualification::{lock_wait, operator},
    definition_qualification::{
        grant, request, spend_review_s05, SpendReview, AUTHOR, EXPENSES, SPEND_APP,
    },
    pool::TestHooks,
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    auth::{self, capability, cookies, provider, Auth, Pages},
    commands::{self, routes::Commands, RecordPage, RecordView},
    http,
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{header, HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;

/// The EXP-002 decision record of the demo (design §4 act 6); a client-named
/// record id, as the contract's `command_request` carries one.
const EXP_002: &str = "00000000-0000-4000-8000-0000000000f0";
/// Further decision records of the same collection. The S05 command is a
/// `create` whose one declared transition is `needs_context → reviewed`, and
/// a `create` on an existing record must fire a declared transition
/// (contract edition impl-2026-10-29.3 amendment, APP-013 review M1), so each
/// later race starts its own record at version 0.
const IN_FLIGHT_RECORD: &str = "00000000-0000-4000-8000-0000000013a1";
const BUSY_RECORD: &str = "00000000-0000-4000-8000-0000000013a2";
const LOST_RECORD: &str = "00000000-0000-4000-8000-0000000013a3";
const JSON_RECORD: &str = "00000000-0000-4000-8000-0000000013a4";
const HTTP_IN_FLIGHT_RECORD: &str = "00000000-0000-4000-8000-0000000013a5";
/// APP-013 review M2: `contracts/validate.py` `validate_problem` over every
/// problem body the route answered in this campaign; the HTTP layer's own
/// codes (outside the edition's registry) carry exactly `status` and `code`.
const PROBLEM_CHECK: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
from validate import validate_problem
HTTP_LAYER = {"not_found", "cross_site", "unavailable"}
codes = set()
for item in json.load(sys.stdin):
    body = json.loads(item["body"])
    assert body["status"] == item["status"], item
    if body["code"] in HTTP_LAYER:
        assert set(body) == {"status", "code"}, item
    else:
        validate_problem(body)
    assert ("retry_after_seconds" in body) == (item["retry_after"] is not None), item
    if "retry_after_seconds" in body:
        assert item["retry_after"] == str(body["retry_after_seconds"]), item
    codes.add(body["code"])
print(json.dumps(sorted(codes)))
"#;
const IK: [&str; 10] = [
    "00000000000000000000000000000001",
    "00000000000000000000000000000002",
    "00000000000000000000000000000003",
    "00000000000000000000000000000004",
    "00000000000000000000000000000005",
    "00000000000000000000000000000006",
    "00000000000000000000000000000007",
    "00000000000000000000000000000008",
    "00000000000000000000000000000009",
    "0000000000000000000000000000000a",
];

pub(super) fn values(status: &str, note: &str) -> Value {
    json!([
        {"field_id": "status", "value": {"type": "enum", "value": status}},
        {"field_id": "note", "value": {"type": "text", "value": note}},
    ])
}
fn command(
    s05: &SpendReview,
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
        record: EXP_002.into(),
        expected_record_version,
        expected_work_version,
        values,
        idempotency_key: key.into(),
    }
}
/// `command` on another record of the collection.
fn on(record: &str, mut command: CommandRequest) -> CommandRequest {
    command.record = record.into();
    command
}
/// Runs `PROBLEM_CHECK` over the collected replies; returns the codes seen.
fn validate_problems(replies: &[&Reply]) -> Value {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let input: Vec<Value> = replies
        .iter()
        .map(|reply| {
            assert_eq!(
                reply.headers[header::CONTENT_TYPE],
                "application/problem+json",
                "{}",
                reply.body
            );
            json!({
                "status": reply.status.as_u16(),
                "body": reply.body,
                "retry_after": reply.headers.get(header::RETRY_AFTER).map(|v| v.to_str().unwrap()),
            })
        })
        .collect();
    let mut child = std::process::Command::new(super::qualification::fixture_python())
        .current_dir(&repo)
        .args([
            "-c",
            PROBLEM_CHECK,
            repo.join("contracts").to_str().unwrap(),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("locked Python available");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&input).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "contracts/validate.py validate_problem refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
pub(super) async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// Every row a command can write, as the superuser counts them.
pub(super) async fn footprint(fixture: &Fixture) -> [i64; 6] {
    let mut counts = [0; 6];
    for (slot, sql) in [
        "SELECT count(*) FROM genui.internal_records",
        "SELECT count(*) FROM genui.internal_record_revisions",
        "SELECT count(*) FROM genui.record_source_dependencies",
        "SELECT count(*) FROM genui.command_claims",
        "SELECT count(*) FROM genui.command_outcomes",
        "SELECT count(*) FROM genui.audit_events WHERE writer_role='data'",
    ]
    .into_iter()
    .enumerate()
    {
        counts[slot] = scalar(fixture, sql).await;
    }
    counts
}
async fn next_sequence(fixture: &Fixture) -> i64 {
    scalar(
        fixture,
        &format!("SELECT next_commit_sequence FROM genui.organization_sequences WHERE organization_id='{ORG}'"),
    )
    .await
}
pub(super) async fn paused(hooks: &TestHooks) {
    timeout(Duration::from_secs(5), hooks.reached.notified())
        .await
        .expect("command barrier reached");
}
pub(super) fn committed(result: Result<CommandResult>) -> CommandOutcome {
    match result {
        Ok(CommandResult::Committed(outcome)) => outcome,
        other => panic!("expected a committed command: {other:?}"),
    }
}
pub(super) fn conflict(result: Result<CommandResult>) -> RecoveryView {
    match result {
        Ok(CommandResult::Conflict(view)) => view,
        other => panic!("expected a conflict: {other:?}"),
    }
}
/// A refusal of a raw statement as `role` under Northstar's scope, rolled back.
async fn refused(fixture: &Fixture, role: &str, sql: &str, code: &str) {
    let operator = fixture.operator().await;
    operator
        .client
        .batch_execute(&format!(
            "BEGIN; SET LOCAL ROLE {role}; SELECT set_config('genui.organization_id','{ORG}',true),set_config('genui.principal_id','{AUTHOR}',true),set_config('genui.membership_epoch','1',true);"
        ))
        .await
        .unwrap();
    let error = operator
        .client
        .batch_execute(sql)
        .await
        .expect_err("statement refused");
    assert_eq!(
        error.code().map(|c| c.code()),
        Some(code),
        "{role}: {sql}: {error}"
    );
    operator.client.batch_execute("ROLLBACK").await.unwrap();
}

pub(super) struct TestPages;
fn page(text: String) -> Response {
    let mut response = (StatusCode::OK, text).into_response();
    http::secure(response.headers_mut());
    response
}
impl Pages for TestPages {
    fn invite(&self, _: &auth::InviteView) -> Response {
        page("page=invite".into())
    }
    fn signin(&self, _: &auth::SigninView) -> Response {
        page("page=signin".into())
    }
    fn pending(&self, _: &auth::PendingView) -> Response {
        page("page=pending".into())
    }
    fn denied(&self, _: &auth::DeniedView) -> Response {
        page("page=denied".into())
    }
    fn workspace(&self, _: &auth::WorkspaceView) -> Response {
        page("page=workspace".into())
    }
}
impl RecordPage for TestPages {
    fn record(&self, v: &RecordView) -> Response {
        let fields: Vec<String> = v
            .fields
            .iter()
            .map(|f| format!("{}={}", f.id, f.value))
            .collect();
        let hidden: Vec<String> = v
            .fields
            .iter()
            .map(|f| format!("{}={}", f.spec_input, f.spec))
            .collect();
        page(format!(
            "page=record;version={};key={};csrf={};exists={};fields={};specs={};command={};action={};notice={};can_write={};publication={};work_version={};collection={}",
            v.version,
            v.idempotency_key,
            v.csrf,
            v.exists,
            fields.join("|"),
            hidden.join("|"),
            v.command,
            v.action,
            v.notice.map(|n| n.name()).unwrap_or("-"),
            v.can_write,
            v.publication,
            v.work_version,
            v.collection,
        ))
    }
}
fn field(body: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    body.split(';')
        .find_map(|part| part.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("field {name} in {body}"))
        .to_owned()
}
pub(super) struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}
impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| panic!("JSON body: {}", self.body))
    }
    fn location(&self) -> String {
        self.headers[header::LOCATION].to_str().unwrap().to_owned()
    }
}
pub(super) async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: &str,
    headers: &[(&str, &str)],
) -> Reply {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, format!("{}={cookie}", cookies::SESSION));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut request = request.body(Body::from(body.to_owned())).unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.40:4000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    Reply {
        status,
        headers,
        body: String::from_utf8(body.to_vec()).unwrap(),
    }
}
fn encode(pairs: &[(&str, &str)]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in pairs {
        out.append_pair(name, value);
    }
    out.finish()
}
pub(super) fn json_headers(key: &str) -> Vec<(&'static str, &str)> {
    vec![
        ("sec-fetch-site", "same-origin"),
        ("content-type", "application/json"),
        ("idempotency-key", key),
    ]
}
pub(super) fn vault_token() -> provider::Token {
    provider::Token {
        access_token: "synthetic-access".into(),
        refresh_token: capability::random_capability().unwrap(),
        token_type: "Bearer".into(),
        expires_in: 900,
        expires_at: auth::now() + 900,
        user: provider::User {
            id: format!("usr_{AUTHOR}"),
            email: "author@example.test".into(),
            name: None,
            avatar: None,
            email_verified: true,
            roles: Vec::new(),
            auth_method: "password".into(),
            mfa_verified: false,
        },
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_command_campaign() {
    std::thread::Builder::new()
        .name("app013-command-campaign".into())
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
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();

    // --- D7.7: the operator backfill creates the sequence row ---
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["backfill", "--organization", org]);
        assert_eq!(out["created"].as_array().unwrap().len(), 5);
        assert_eq!(out["sequence"]["organization_sequences"], "created");
        // The seeded fixture revision sits at commit_sequence 1 in each organization.
        assert_eq!(out["sequence"]["next_commit_sequence"], 2);
    }
    let again = operator(&fixture, &["backfill", "--organization", ORG]);
    assert_eq!(again["sequence"]["organization_sequences"], "existing");
    assert_eq!(next_sequence(&fixture).await, 2);
    check(
        &mut checks,
        "operator backfill creates each organization's organization_sequences row at 1 + max(internal_record_revisions.commit_sequence) once, and reports it existing afterwards (D7.7, D7.19)",
    );

    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    stores.readiness().await.unwrap();
    let author = request(&stores, ORG, AUTHOR, 1);
    let s05 = spend_review_s05(&fixture, &stores).await;
    let before = footprint(&fixture).await;
    assert_eq!(before, [3, 3, 5, 0, 0, 0]);
    check(
        &mut checks,
        "S05 Spend Review in Northstar through the admitted path over the three seeded placeholder records; no claim, outcome or data-chain event exists before the first command",
    );

    // --- ik-1 at version 0 → version 1, one transaction ---
    let first = command(
        &s05,
        IK[0],
        0,
        1,
        values("needs_context", "Review required — synthetic only"),
    );
    let outcome1 = committed(stores.data.commit_record(&author, &first).await);
    assert_eq!(
        (
            outcome1.record_version,
            outcome1.work_version,
            outcome1.commit_sequence
        ),
        (1, 1, 2)
    );
    assert_eq!(outcome1.record_id, EXP_002);
    assert_eq!(outcome1.idempotency_key, IK[0]);
    assert_eq!(footprint(&fixture).await, [4, 4, 6, 1, 1, 1]);
    assert_eq!(next_sequence(&fixture).await, 3);
    let stored: (String, i64, Vec<u8>, String) = {
        let operator = fixture.operator().await;
        let row = operator.client.query_one("SELECT r.creator_id::text,r.commit_sequence,r.canonical_values,e.kind FROM genui.internal_record_revisions r JOIN genui.audit_events e ON e.organization_id=r.organization_id AND e.record_id=r.record_id AND e.writer_role='data' WHERE r.organization_id=$1::text::uuid AND r.record_id=$2::text::uuid AND r.version=1", &[&ORG,&EXP_002]).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    };
    assert_eq!(stored.0, AUTHOR);
    assert_eq!(stored.1, 2);
    assert_eq!(
        serde_json::from_slice::<Value>(&stored.2).unwrap(),
        values("needs_context", "Review required — synthetic only")
    );
    assert_eq!(stored.3, COMMAND_COMMITTED);
    let event = {
        let operator = fixture.operator().await;
        let row = operator.client.query_one("SELECT event_id::text,outcome_digest::text,app_id::text,work_id::text,actor_principal_id::text FROM genui.audit_events WHERE organization_id=$1::text::uuid AND writer_role='data'", &[&ORG]).await.unwrap();
        (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, String>(2),
            row.get::<_, String>(3),
            row.get::<_, String>(4),
        )
    };
    assert_eq!(event.0, outcome1.audit_event_id);
    assert_eq!(event.1, outcome1.payload_digest);
    assert_eq!(
        (event.2.as_str(), event.3.as_str(), event.4.as_str()),
        (SPEND_APP, s05.work.as_str(), AUTHOR)
    );
    check(
        &mut checks,
        "ik-1 at expected version 0 commits record version 1: one revision (commit_sequence 2, creator and updater the author), one current pointer, the record's source dependencies, one scoped outcome and exactly one command.committed event naming the record, in one transaction; the sequence advances to 3",
    );

    // --- identical retry after a lost response ---
    let replayed = stores.data.commit_record(&author, &first).await;
    assert_eq!(replayed, Ok(CommandResult::Replayed(outcome1.clone())));
    assert_eq!(footprint(&fixture).await, [4, 4, 6, 1, 1, 1]);
    assert_eq!(next_sequence(&fixture).await, 3);
    assert_eq!(
        stores
            .data
            .read_outcome(&author, SPEND_APP, &s05.work, "record_decision", IK[0])
            .await,
        Ok(Some(outcome1.clone()))
    );
    check(
        &mut checks,
        "ik-1 retried with its now-stale expected version 0 returns the stored outcome unchanged, with no second revision, outcome or audit event; the outcome lookup by key returns the same",
    );

    // --- same key, different payload ---
    let mut changed = first.clone();
    changed.values = values("needs_context", "Review required — edited");
    let view = conflict(stores.data.commit_record(&author, &changed).await);
    assert_eq!(view.reason, ConflictReason::PayloadMismatch);
    assert_eq!(view.stored_outcome.as_ref(), Some(&outcome1));
    assert_eq!(view.current_record_version, 1);
    assert_eq!(
        view.current_values,
        Some(values("needs_context", "Review required — synthetic only"))
    );
    assert_eq!(footprint(&fixture).await, [4, 4, 6, 1, 1, 1]);
    check(
        &mut checks,
        "the same key with a changed canonical payload conflicts with the authorized recovery view naming the stored outcome and the current record; nothing is written",
    );

    // --- stale expected version (S05: a stale edit expecting version 0) ---
    let stale = command(&s05, IK[1], 0, 1, values("reviewed", "stale edit"));
    let view = conflict(stores.data.commit_record(&author, &stale).await);
    assert_eq!(view.reason, ConflictReason::ExpectedVersion);
    assert_eq!(
        (view.expected_record_version, view.current_record_version),
        (0, 1)
    );
    assert_eq!(
        view.current_values,
        Some(values("needs_context", "Review required — synthetic only"))
    );
    assert!(view.stored_outcome.is_none());
    // The conflicting command's claim was released with its rollback.
    assert_eq!(footprint(&fixture).await, [4, 4, 6, 1, 1, 1]);
    assert_eq!(next_sequence(&fixture).await, 3);
    check(
        &mut checks,
        "a new key expecting version 0 against version 1 conflicts (409 recovery view: expected 0, current 1 with the current values) without partially writing a record, claim or outcome",
    );

    // --- stale work premise ---
    let moved = command(&s05, IK[2], 1, 2, values("reviewed", "moved work"));
    match stores.data.commit_record(&author, &moved).await {
        Ok(CommandResult::Stale(current)) => {
            assert_eq!(
                current.publication.as_deref(),
                Some(s05.publication.as_str())
            );
            assert_eq!(current.work_version, Some(1));
            assert_eq!(
                current.definition_digest.as_deref(),
                Some(s05.definition_digest.as_str())
            );
        }
        other => panic!("expected Stale: {other:?}"),
    }
    let mut other_publication = command(&s05, IK[2], 1, 1, values("reviewed", "moved work"));
    other_publication.publication = EXP_002.into();
    assert!(matches!(
        stores.data.commit_record(&author, &other_publication).await,
        Ok(CommandResult::Stale(_))
    ));
    assert_eq!(footprint(&fixture).await, [4, 4, 6, 1, 1, 1]);
    check(
        &mut checks,
        "a work version or publication premise that is not the saved work's is Stale (412) naming the current publication, revision, digest and work version; nothing is written",
    );

    // --- ik-2: reviewed at version 1 → version 2 ---
    let second = command(
        &s05,
        IK[3],
        1,
        1,
        values("reviewed", "Approved — synthetic only"),
    );
    let outcome2 = committed(stores.data.commit_record(&author, &second).await);
    assert_eq!((outcome2.record_version, outcome2.commit_sequence), (2, 3));
    assert_eq!(footprint(&fixture).await, [4, 5, 7, 2, 2, 2]);
    assert_eq!(
        scalar(&fixture, &format!("SELECT current_version FROM genui.internal_records WHERE organization_id='{ORG}' AND record_id='{EXP_002}'")).await,
        2
    );
    check(
        &mut checks,
        "ik-2 (reviewed, the declared needs_context → reviewed transition) commits version 2 at commit_sequence 3: one more revision, outcome and audit event; the current pointer is 2",
    );

    // --- refusals that write nothing ---
    let mut undeclared = command(&s05, IK[4], 2, 1, values("needs_context", "back"));
    assert_eq!(
        stores.data.commit_record(&author, &undeclared).await,
        Err(Error::Denied)
    );
    // APP-013 review M1 (R3): through the create command, a note-only edit
    // and a resubmission of the current values under a new key fire no
    // declared transition and are refused: no no-op or note-only revision.
    for note in ["note-only edit", "Approved — synthetic only"] {
        assert_eq!(
            stores
                .data
                .commit_record(
                    &author,
                    &command(&s05, IK[4], 2, 1, values("reviewed", note))
                )
                .await,
            Err(Error::Denied),
            "{note}"
        );
    }
    undeclared.values = values("reviewed", "note only change is fine");
    undeclared.command = "approve".into();
    assert_eq!(
        stores.data.commit_record(&author, &undeclared).await,
        Err(Error::Denied)
    );
    let mut version = command(&s05, IK[4], 2, 1, values("reviewed", "x"));
    version.schema_version_expected = 2;
    assert_eq!(
        stores.data.commit_record(&author, &version).await,
        Err(Error::Denied)
    );
    let mut typed = command(
        &s05,
        IK[4],
        2,
        1,
        json!([{"field_id": "status", "value": {"type": "text", "value": "reviewed"}}]),
    );
    assert_eq!(
        stores.data.commit_record(&author, &typed).await,
        Err(Error::Denied)
    );
    typed.values = json!([{"field_id": "amount", "value": {"type": "text", "value": "x"}}]);
    assert_eq!(
        stores.data.commit_record(&author, &typed).await,
        Err(Error::Denied)
    );
    let mut key = command(&s05, "short", 2, 1, values("reviewed", "x"));
    assert_eq!(
        stores.data.commit_record(&author, &key).await,
        Err(Error::Denied)
    );
    key.idempotency_key = "0000000000000000000000000000000A".into();
    assert_eq!(
        stores.data.commit_record(&author, &key).await,
        Err(Error::Denied)
    );
    // A member without record_write on the work, and the canary's member.
    let bystander = request(&stores, ORG, PRINCIPAL, 1);
    assert_eq!(
        stores
            .data
            .commit_record(
                &bystander,
                &command(&s05, IK[4], 2, 1, values("reviewed", "x"))
            )
            .await,
        Err(Error::Denied)
    );
    let canary = request(&stores, OTHER, AUTHOR, 1);
    assert_eq!(
        stores
            .data
            .commit_record(
                &canary,
                &command(&s05, IK[4], 2, 1, values("reviewed", "x"))
            )
            .await,
        Err(Error::Denied)
    );
    assert_eq!(footprint(&fixture).await, [4, 5, 7, 2, 2, 2]);
    check(
        &mut checks,
        "an undeclared transition (reviewed → needs_context on a create command), a note-only edit and a resubmission of the current values through the create command (no declared transition fires; edition impl-2026-10-29.3 amendment), an undeclared command, another schema version, a mistyped or unknown field, a malformed key, a member without record_write and the canary organization are refused with nothing written",
    );

    // --- another principal cannot read or replay the outcome by key ---
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
    let reviewer = request(&stores, ORG, REVIEWER, 1);
    assert_eq!(
        stores
            .data
            .read_outcome(&reviewer, SPEND_APP, &s05.work, "record_decision", IK[0])
            .await,
        Ok(None)
    );
    // The same key and payload under another principal is that principal's
    // own fresh claim, judged on its own premise: version 0 is now stale.
    let view = conflict(stores.data.commit_record(&reviewer, &first).await);
    assert_eq!(
        (view.reason, view.current_record_version),
        (ConflictReason::ExpectedVersion, 2)
    );
    assert_eq!(footprint(&fixture).await, [4, 5, 7, 2, 2, 2]);
    assert_eq!(
        stores
            .data
            .read_outcome(&bystander, SPEND_APP, &s05.work, "record_decision", IK[0])
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .read_outcome(&author, SPEND_APP, &s05.work, "record_decision", IK[0])
            .await,
        Ok(Some(outcome1.clone()))
    );
    check(
        &mut checks,
        "the idempotency scope includes the principal: a reviewer with record_write reads no outcome under the author's key and gets its own conflict on replaying it; a member without authority is denied; the author reads its outcome",
    );

    // --- in flight: the same key while the first command holds its claim ---
    hooks.arm("record_lock");
    let paused_stores = stores.clone();
    let paused_command = on(
        IN_FLIGHT_RECORD,
        command(&s05, IK[5], 0, 1, values("needs_context", "A")),
    );
    let a = {
        let request_command = paused_command.clone();
        tokio::spawn(async move {
            let author = request(&paused_stores, ORG, AUTHOR, 1);
            paused_stores
                .data
                .commit_record(&author, &request_command)
                .await
        })
    };
    paused(&hooks).await;
    let b_stores = stores.clone();
    let b_command = paused_command.clone();
    let b = tokio::spawn(async move {
        let author = request(&b_stores, ORG, AUTHOR, 1);
        b_stores.data.commit_record(&author, &b_command).await
    });
    lock_wait(&fixture, 1).await;
    assert_eq!(b.await.unwrap(), Ok(CommandResult::InFlight));
    // A different key behind the paused command waits on the sequence row.
    let c_stores = stores.clone();
    let c_command = on(
        IN_FLIGHT_RECORD,
        command(&s05, IK[6], 0, 1, values("needs_context", "C")),
    );
    let c = tokio::spawn(async move {
        let author = request(&c_stores, ORG, AUTHOR, 1);
        c_stores.data.commit_record(&author, &c_command).await
    });
    lock_wait(&fixture, 1).await;
    assert_eq!(c.await.unwrap(), Ok(CommandResult::InFlight));
    assert_eq!(footprint(&fixture).await, [4, 5, 7, 2, 2, 2]);
    hooks.release.notify_one();
    let outcome3 = committed(a.await.unwrap());
    assert_eq!((outcome3.record_version, outcome3.commit_sequence), (1, 4));
    assert_eq!(
        stores.data.commit_record(&author, &paused_command).await,
        Ok(CommandResult::Replayed(outcome3.clone()))
    );
    let view = conflict(
        stores
            .data
            .commit_record(
                &author,
                &on(
                    IN_FLIGHT_RECORD,
                    command(&s05, IK[6], 0, 1, values("needs_context", "C")),
                ),
            )
            .await,
    );
    assert_eq!(
        (view.reason, view.current_record_version),
        (ConflictReason::ExpectedVersion, 1)
    );
    assert_eq!(footprint(&fixture).await, [5, 6, 8, 3, 3, 3]);
    check(
        &mut checks,
        "while a command holds its claim, sequence and record locks, the same key waits on the claim and a different key on the sequence row, each observed in pg_stat_activity and answered in_flight after lock_timeout (never 503) with nothing written; the released command commits its record's version 1 (commit_sequence 4), the same key then replays and the different key conflicts on the moved version",
    );

    // --- busy: a lock wait on the authorization fence ---
    // An operator revocation in progress updates the member row (FOR NO KEY
    // UPDATE): the claim's KEY SHARE reference passes and the fence's FOR
    // SHARE waits, so the command is busy. The holder rolls back, so no epoch
    // moves.
    let busy_command = on(
        BUSY_RECORD,
        command(&s05, IK[7], 0, 1, values("needs_context", "busy")),
    );
    let race = |statement: String, expected: CommandResult| {
        let stores = stores.clone();
        let command = busy_command.clone();
        let fixture = &fixture;
        async move {
            let holder = fixture.operator().await;
            holder
                .client
                .batch_execute(&format!(
                    "BEGIN; SELECT set_config('genui.organization_id','{ORG}',true); {statement}"
                ))
                .await
                .unwrap();
            let racer = tokio::spawn(async move {
                let author = request(&stores, ORG, AUTHOR, 1);
                stores.data.commit_record(&author, &command).await
            });
            lock_wait(fixture, 1).await;
            assert_eq!(racer.await.unwrap(), Ok(expected));
            holder.client.batch_execute("ROLLBACK").await.unwrap();
        }
    };
    race(
        format!("UPDATE genui.memberships SET membership_epoch=membership_epoch WHERE organization_id='{ORG}' AND principal_id='{AUTHOR}'"),
        CommandResult::Busy,
    )
    .await;
    // The D7.1 operator lock itself (member FOR UPDATE) already blocks the
    // claim's foreign-key reference, so that wait is classified in_flight:
    // the same 409 with Retry-After, never 503.
    race(
        format!("SELECT principal_id FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{AUTHOR}' FOR UPDATE"),
        CommandResult::InFlight,
    )
    .await;
    assert_eq!(footprint(&fixture).await, [5, 6, 8, 3, 3, 3]);
    let outcome4 = committed(stores.data.commit_record(&author, &busy_command).await);
    assert_eq!(outcome4.record_version, 1);
    check(
        &mut checks,
        "a member row being updated by an operator transaction makes the authorization fence wait past lock_timeout: the command answers busy (409); a member row held FOR UPDATE blocks the claim's reference and answers in_flight (409); neither is a 503, nothing is written, and the command commits once the operator releases",
    );

    // --- ambiguous commit: the backend ends before COMMIT answers ---
    hooks.arm("commit");
    let lost_command = on(
        LOST_RECORD,
        command(&s05, IK[8], 0, 1, values("needs_context", "lost answer")),
    );
    let lost_stores = stores.clone();
    let lost_request = lost_command.clone();
    let lost = tokio::spawn(async move {
        let author = request(&lost_stores, ORG, AUTHOR, 1);
        lost_stores.data.commit_record(&author, &lost_request).await
    });
    paused(&hooks).await;
    let terminated = scalar(
        &fixture,
        "SELECT count(pg_terminate_backend(pid)) FROM pg_stat_activity WHERE datname='genui' AND usename='genui_data' AND state='idle in transaction'",
    )
    .await;
    assert_eq!(terminated, 1);
    hooks.release.notify_one();
    assert_eq!(lost.await.unwrap(), Err(Error::UnknownOutcome));
    // The client re-reads by key under current authority: nothing committed.
    assert_eq!(
        stores
            .data
            .read_outcome(&author, SPEND_APP, &s05.work, "record_decision", IK[8])
            .await,
        Ok(None)
    );
    assert_eq!(footprint(&fixture).await, [6, 7, 9, 4, 4, 4]);
    let outcome5 = committed(stores.data.commit_record(&author, &lost_command).await);
    assert_eq!(outcome5.record_version, 1);
    assert_eq!(
        stores
            .data
            .read_outcome(&author, SPEND_APP, &s05.work, "record_decision", IK[8])
            .await,
        Ok(Some(outcome5.clone()))
    );
    check(
        &mut checks,
        "a backend ended before COMMIT answers is UnknownOutcome (503 unknown_outcome), never a claimed success: the outcome lookup by key finds nothing, the same key resubmitted commits once, and the lookup then finds it",
    );

    // --- insert-only outcomes and role separation at the row level ---
    for (role, sql, code) in [
        ("genui_data", format!("UPDATE genui.command_outcomes SET record_version=9 WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("DELETE FROM genui.command_outcomes WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("UPDATE genui.command_claims SET payload_digest=payload_digest WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("DELETE FROM genui.command_claims WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("UPDATE genui.internal_record_revisions SET canonical_values=canonical_values WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("UPDATE genui.audit_events SET kind=kind WHERE organization_id='{ORG}'"), "42501"),
        ("genui_data", format!("INSERT INTO genui.organization_sequences VALUES ('{ORG}',1)"), "42501"),
        ("genui_data", format!("INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash) VALUES ('{ORG}',gen_random_uuid(),'workspace',99,'command.committed','{AUTHOR}',NULL,NULL,NULL,NULL,NULL,now(),'sha256:{}','sha256:{}')", "0".repeat(64), "1".repeat(64)), "42501"),
        ("genui_data", format!("INSERT INTO genui.command_claims VALUES ('{ORG}','{REVIEWER}','{SPEND_APP}','{}','record_decision','{}','sha256:{}',now())", s05.work, IK[9], "2".repeat(64)), "42501"),
        ("genui_workspace", format!("SELECT count(*) FROM genui.command_outcomes WHERE organization_id='{ORG}'"), "42501"),
        ("genui_control", format!("SELECT count(*) FROM genui.command_claims WHERE organization_id='{ORG}'"), "42501"),
        ("genui_worker", format!("SELECT count(*) FROM genui.organization_sequences WHERE organization_id='{ORG}'"), "42501"),
    ] {
        refused(&fixture, role, &sql, code).await;
    }
    // Another principal's outcome rows are invisible to the data role under
    // this principal's scope (RESTRICTIVE policy), beyond the scoped lookup.
    {
        let operator = fixture.operator().await;
        operator.client.batch_execute(&format!("BEGIN; SET LOCAL ROLE genui_data; SELECT set_config('genui.organization_id','{ORG}',true),set_config('genui.principal_id','{REVIEWER}',true),set_config('genui.membership_epoch','1',true);")).await.unwrap();
        let visible: i64 = operator
            .client
            .query_one("SELECT count(*) FROM genui.command_outcomes", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(visible, 0);
        operator.client.batch_execute("ROLLBACK").await.unwrap();
    }
    check(
        &mut checks,
        "claims, outcomes, revisions and audit events are insert-only for the data role; it cannot write a sequence row, another chain or another principal's claim; workspace, control and worker cannot read the command relations; another principal's outcomes are row-invisible",
    );

    // --- the HTTP route and the record page with a minted session ---
    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
    let cookie = stores
        .workspace
        .mint_session_for_tests(ORG, AUTHOR, &reference)
        .await
        .unwrap();
    let record_page: Arc<dyn RecordPage> = pages;
    let app = commands::routes::router(Arc::new(Commands {
        auth: auth.clone(),
        page: record_page,
    }))
    .fallback(http::fallback);
    let record_uri = |record: &str| {
        format!(
            "/work/{SPEND_APP}/{}/records/{}/{record}",
            s05.work, s05.collection
        )
    };
    let page_uri = record_uri(LOST_RECORD);
    let demo_uri = record_uri(EXP_002);
    let shown = send(&app, "GET", &page_uri, Some(&cookie), "", &[]).await;
    assert_eq!(shown.status, StatusCode::OK, "{}", shown.body);
    assert_eq!(field(&shown.body, "version"), "1");
    assert_eq!(field(&shown.body, "exists"), "true");
    assert_eq!(
        field(&shown.body, "fields"),
        "status=needs_context|note=lost answer"
    );
    assert_eq!(field(&shown.body, "command"), "record_decision");
    assert_eq!(field(&shown.body, "can_write"), "true");
    assert_eq!(field(&shown.body, "notice"), "-");
    // M1: the reviewed EXP-002 has no transition left to fire, so its page
    // offers no command and no form.
    let decided = send(&app, "GET", &demo_uri, Some(&cookie), "", &[]).await;
    assert_eq!(
        (
            field(&decided.body, "version").as_str(),
            field(&decided.body, "command").as_str(),
            field(&decided.body, "can_write").as_str()
        ),
        ("2", "", "false")
    );
    let key = field(&shown.body, "key");
    let token = field(&shown.body, "csrf");
    assert!(idempotency_key_shaped(&key));
    let shown_again = send(&app, "GET", &page_uri, Some(&cookie), "", &[]).await;
    assert_ne!(field(&shown_again.body, "key"), key);
    assert_eq!(
        send(&app, "GET", &page_uri, None, "", &[]).await.status,
        StatusCode::NOT_FOUND
    );
    // A second member's session: its own vault entry (one refresh reference
    // per session row, 0006 UNIQUE).
    let foreign_reference = auth.vault.insert("northstar", &vault_token()).unwrap();
    let foreign_cookie = stores
        .workspace
        .mint_session_for_tests(ORG, PRINCIPAL, &foreign_reference)
        .await
        .unwrap();
    assert_eq!(
        send(&app, "GET", &page_uri, Some(&foreign_cookie), "", &[])
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    check(
        &mut checks,
        "the record page renders the current record (version 1, needs_context) with a fresh 32-hex key per render and the session-bound token; no session or a member without record_read is the uniform 404",
    );

    let action = field(&shown.body, "action");
    assert_eq!(
        action,
        format!("/v1/work/{SPEND_APP}/{}/commands/record_decision", s05.work)
    );
    let form = |key: &str, token: &str, expected: &str, note: &str| {
        encode(&[
            ("csrf", token),
            ("idempotency_key", key),
            ("collection_id", &s05.collection),
            ("publication_id", &s05.publication),
            ("schema_id", "decision"),
            ("schema_version_expected", "1"),
            ("record_id", LOST_RECORD),
            ("expected_record_version", expected),
            ("expected_work_version", "1"),
            ("value.status", "reviewed"),
            (
                "spec.status",
                "{\"type\":\"enum\",\"values\":[\"needs_context\",\"reviewed\"]}",
            ),
            ("value.note", note),
            ("spec.note", "{\"type\":\"text\"}"),
        ])
    };
    let form_headers = [
        ("sec-fetch-site", "same-origin"),
        ("content-type", "application/x-www-form-urlencoded"),
    ];
    let posted = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &form(&key, &token, "1", "from the form"),
        &form_headers,
    )
    .await;
    assert_eq!(posted.status, StatusCode::SEE_OTHER, "{}", posted.body);
    assert_eq!(posted.location(), format!("{page_uri}?notice=committed"));
    assert_eq!(posted.headers[header::CACHE_CONTROL], "no-store");
    let after = send(&app, "GET", &posted.location(), Some(&cookie), "", &[]).await;
    assert_eq!(field(&after.body, "version"), "2");
    assert_eq!(field(&after.body, "notice"), "committed");
    assert_eq!(
        field(&after.body, "fields"),
        "status=reviewed|note=from the form"
    );
    // The same form again (the browser resubmits): replayed, nothing written.
    let again = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &form(&key, &token, "1", "from the form"),
        &form_headers,
    )
    .await;
    assert_eq!(again.location(), format!("{page_uri}?notice=replayed"));
    let edited = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &form(&key, &token, "1", "edited"),
        &form_headers,
    )
    .await;
    assert_eq!(edited.location(), format!("{page_uri}?notice=conflict"));
    assert_eq!(footprint(&fixture).await, [7, 9, 11, 6, 6, 6]);
    // Refusal order: fetch metadata, the token, the key.
    assert_eq!(
        send(
            &app,
            "POST",
            &action,
            Some(&cookie),
            &form(&key, &token, "2", "x"),
            &[("content-type", "application/x-www-form-urlencoded")]
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &action,
            Some(&cookie),
            &form(&key, "not-the-token", "2", "x"),
            &form_headers
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let keyless = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &form("", &token, "2", "x"),
        &form_headers,
    )
    .await;
    assert_eq!(
        (keyless.status, keyless.json()["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("malformed_idempotency_key"))
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &action,
            None,
            &form(&key, &token, "2", "x"),
            &form_headers
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    // APP-013 review m2 (R12): a value storage refuses (a status outside the
    // enum, a field the command does not write) returns to the record page
    // with the refused notice, not a bare 404 document; nothing is written.
    let fresh = field(
        &send(&app, "GET", &page_uri, Some(&cookie), "", &[])
            .await
            .body,
        "key",
    );
    for body in [
        form(&fresh, &token, "2", "x").replace("value.status=reviewed", "value.status=approved"),
        format!(
            "{}&{}",
            form(&fresh, &token, "2", "x"),
            encode(&[
                ("value.amount", "1"),
                ("spec.amount", "{\"type\":\"text\"}")
            ])
        ),
    ] {
        let refused = send(&app, "POST", &action, Some(&cookie), &body, &form_headers).await;
        assert_eq!(refused.status, StatusCode::SEE_OTHER, "{}", refused.body);
        assert_eq!(refused.location(), format!("{page_uri}?notice=refused"));
        let shown = send(&app, "GET", &refused.location(), Some(&cookie), "", &[]).await;
        assert_eq!(
            (
                field(&shown.body, "notice").as_str(),
                field(&shown.body, "version").as_str()
            ),
            ("refused", "2")
        );
    }
    assert_eq!(footprint(&fixture).await, [7, 9, 11, 6, 6, 6]);
    check(
        &mut checks,
        "the record page's form commits through the route (303 to the page with notice=committed, the record's version 2), resubmits as replayed and conflicts when edited under the same key; a value storage refuses (a status outside the enum, an unwritable field) returns 303 to the page with notice=refused; a cross-site post, a wrong token, a missing key and no session are refused before any write",
    );

    // --- the JSON route: the S05 sequence as the island would drive it ---
    let document = |record: &str, expected: i64, note: &str| {
        serde_json::to_string(&json!({
            "kind": "command_request",
            "schema_version": 1,
            "app_id": SPEND_APP,
            "work_id": s05.work,
            "publication_id": s05.publication,
            "command_id": "record_decision",
            "schema_id": "decision",
            "schema_version_expected": 1,
            "record_id": record,
            "expected_record_version": expected,
            "expected_work_version": 1,
            "values": values("needs_context", note),
        }))
        .unwrap()
    };
    let json_key = "0123456789abcdef0123456789abcdef";
    let headers = json_headers(json_key);
    let first_json = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 0, "json"),
        &headers,
    )
    .await;
    assert_eq!(first_json.status, StatusCode::OK, "{}", first_json.body);
    assert_eq!(first_json.headers[header::CONTENT_TYPE], "application/json");
    let body = first_json.json();
    assert_eq!(body["kind"], "command_outcome");
    assert_eq!(body["record_version"], 1);
    assert_eq!(body["idempotency_key"], json_key);
    assert_eq!(body["principal_id"], AUTHOR);
    let replay = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 0, "json"),
        &headers,
    )
    .await;
    assert_eq!(
        (replay.status, replay.body.clone()),
        (StatusCode::OK, first_json.body.clone())
    );
    let stale = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 0, "stale json"),
        &json_headers("fedcba9876543210fedcba9876543210"),
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{}", stale.body);
    assert_eq!(
        stale.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let problem = stale.json();
    assert_eq!(
        (problem["status"].as_i64(), problem["code"].as_str()),
        (Some(409), Some("conflict"))
    );
    assert_eq!(problem["recovery_view"]["reason"], "expected_version");
    assert_eq!(problem["recovery_view"]["current_record_version"], 1);
    assert_eq!(
        problem["recovery_view"]["current_values"],
        values("needs_context", "json")
    );
    let mismatch = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 0, "other payload"),
        &headers,
    )
    .await;
    assert_eq!(mismatch.status, StatusCode::CONFLICT);
    assert_eq!(
        mismatch.json()["recovery_view"]["reason"],
        "payload_mismatch"
    );
    assert_eq!(
        mismatch.json()["recovery_view"]["stored_outcome"]["record_version"],
        1
    );
    let moved = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 1, "moved")
            .replace("\"expected_work_version\":1", "\"expected_work_version\":2"),
        &json_headers("00000000000000000000000000000111"),
    )
    .await;
    assert_eq!(
        moved.status,
        StatusCode::PRECONDITION_FAILED,
        "{}",
        moved.body
    );
    assert_eq!(moved.json()["code"], "stale");
    assert_eq!(moved.json()["current_revision"]["work_version"], 1);
    let lookup = send(
        &app,
        "GET",
        &format!("{action}/outcomes/{json_key}"),
        Some(&cookie),
        "",
        &[],
    )
    .await;
    assert_eq!(
        (lookup.status, lookup.body.clone()),
        (StatusCode::OK, first_json.body.clone())
    );
    let unknown_key = send(
        &app,
        "GET",
        &format!("{action}/outcomes/{}", "9".repeat(32)),
        Some(&cookie),
        "",
        &[],
    )
    .await;
    assert_eq!(unknown_key.status, StatusCode::NOT_FOUND);
    let foreign_lookup = send(
        &app,
        "GET",
        &format!("{action}/outcomes/{json_key}"),
        Some(&foreign_cookie),
        "",
        &[],
    )
    .await;
    assert_eq!(foreign_lookup.status, StatusCode::NOT_FOUND);
    let missing = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 1, "x"),
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/json"),
        ],
    )
    .await;
    assert_eq!(
        (missing.status, missing.json()["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("missing_idempotency_key"))
    );
    let short = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document(JSON_RECORD, 1, "x"),
        &json_headers("abc"),
    )
    .await;
    assert_eq!(
        (short.status, short.json()["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("malformed_idempotency_key"))
    );
    let malformed = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        "{\"kind\":\"command_request\"}",
        &json_headers("00000000000000000000000000000222"),
    )
    .await;
    assert_eq!(
        (malformed.status, malformed.json()["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("malformed_command"))
    );
    // APP-013 review m1 (R4b) and m4 (R10a): a duplicated member is not
    // forma-json-v1; two Idempotency-Key headers and a padded one are
    // malformed keys; a non-JSON body without a key is a malformed command.
    let document_x = document(JSON_RECORD, 1, "x");
    let duplicated = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &format!("{{\"schema_version\":1,{}", &document_x[1..]),
        &json_headers("00000000000000000000000000000777"),
    )
    .await;
    let two_headers = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document_x,
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/json"),
            ("idempotency-key", "00000000000000000000000000000777"),
            ("idempotency-key", "00000000000000000000000000000777"),
        ],
    )
    .await;
    let padded = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document_x,
        &json_headers(" 00000000000000000000000000000777 "),
    )
    .await;
    let not_json = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        "not json",
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/json"),
        ],
    )
    .await;
    for (reply, code) in [
        (&duplicated, "malformed_command"),
        (&two_headers, "malformed_idempotency_key"),
        (&padded, "malformed_idempotency_key"),
        (&not_json, "malformed_command"),
    ] {
        assert_eq!(
            (reply.status, reply.json()["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some(code)),
            "{}",
            reply.body
        );
    }
    let oversize = format!(
        "{{\"pad\":\"{}\"}}",
        "x".repeat(commands::routes::COMMAND_BODY_LIMIT)
    );
    let too_large = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &oversize,
        &json_headers("00000000000000000000000000000333"),
    )
    .await;
    assert_eq!(too_large.status, StatusCode::PAYLOAD_TOO_LARGE);
    let cross_site = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &document_x,
        &[
            ("content-type", "application/json"),
            ("idempotency-key", "00000000000000000000000000000444"),
        ],
    )
    .await;
    assert_eq!(cross_site.status, StatusCode::FORBIDDEN);
    let other_app = format!(
        "/v1/work/{}/{}/commands/record_decision",
        crate::storage::qualification_cases::APP,
        s05.work
    );
    let foreign_path = send(
        &app,
        "POST",
        &other_app,
        Some(&cookie),
        &document_x,
        &json_headers("00000000000000000000000000000555"),
    )
    .await;
    assert_eq!(foreign_path.status, StatusCode::BAD_REQUEST);
    let not_an_id = send(
        &app,
        "POST",
        "/v1/work/not-an-id/x/commands/record_decision",
        Some(&cookie),
        &document_x,
        &json_headers("00000000000000000000000000000666"),
    )
    .await;
    assert_eq!(not_an_id.status, StatusCode::NOT_FOUND);
    assert_eq!(footprint(&fixture).await, [8, 10, 12, 7, 7, 7]);
    // M2 (R10b): the same key over HTTP while its first command holds the
    // claim (paused at the record lock) is 409 in_flight with
    // retry_after_seconds and Retry-After: 2; the first then commits.
    let held_key = "00000000000000000000000000000888";
    let held_body = document(HTTP_IN_FLIGHT_RECORD, 0, "held");
    hooks.arm("record_lock");
    let held = {
        let (app, action, cookie, body) = (
            app.clone(),
            action.clone(),
            cookie.clone(),
            held_body.clone(),
        );
        tokio::spawn(async move {
            send(
                &app,
                "POST",
                &action,
                Some(&cookie),
                &body,
                &json_headers(held_key),
            )
            .await
        })
    };
    paused(&hooks).await;
    let in_flight = send(
        &app,
        "POST",
        &action,
        Some(&cookie),
        &held_body,
        &json_headers(held_key),
    )
    .await;
    assert_eq!(in_flight.status, StatusCode::CONFLICT, "{}", in_flight.body);
    assert_eq!(in_flight.headers[header::RETRY_AFTER], "2");
    assert_eq!(
        (
            in_flight.json()["code"].as_str(),
            in_flight.json()["retry_after_seconds"].as_i64()
        ),
        (Some("in_flight"), Some(2))
    );
    hooks.release.notify_one();
    let held = held.await.unwrap();
    assert_eq!(held.status, StatusCode::OK, "{}", held.body);
    assert_eq!(held.json()["record_version"], 1);
    assert_eq!(footprint(&fixture).await, [9, 11, 13, 8, 8, 8]);
    let codes = validate_problems(&[
        &keyless,
        &stale,
        &mismatch,
        &moved,
        &unknown_key,
        &foreign_lookup,
        &missing,
        &short,
        &malformed,
        &duplicated,
        &two_headers,
        &padded,
        &not_json,
        &too_large,
        &cross_site,
        &foreign_path,
        &not_an_id,
        &in_flight,
    ]);
    assert_eq!(
        codes,
        json!([
            "conflict",
            "cross_site",
            "in_flight",
            "malformed_command",
            "malformed_idempotency_key",
            "missing_idempotency_key",
            "not_found",
            "payload_too_large",
            "stale"
        ])
    );
    check(
        &mut checks,
        "the JSON route answers 200 with the outcome document, 200 again on the identical retry, 409 conflict with the recovery view on a stale version or a changed payload, 412 stale on a moved work premise, 409 in_flight with Retry-After: 2 for the same key while its first command holds the claim, 200 on the outcome lookup by key (404 for another key or another principal), 400 for a missing, short, padded or repeated key and for a malformed, duplicate-member or non-JSON body, 413 over 256 KiB, 403 cross-site and 404 for a foreign path; eight commits in all",
    );
    check(
        &mut checks,
        "every problem body the route answered (conflict, stale, in_flight, the three 400 codes, payload_too_large, cross_site, not_found) validates under contracts/validate.py validate_problem for edition impl-2026-10-29.3 (the HTTP layer's not_found and cross_site carry exactly status and code), with Retry-After equal to retry_after_seconds",
    );

    // --- restart: every pool and store, then reopen ---
    let session_cookie = cookie.clone();
    drop(app);
    drop(auth);
    stores.close().await.unwrap();
    drop(stores);
    let restarted = Arc::new(Stores::new(fixture.settings()).unwrap());
    restarted.readiness().await.unwrap();
    let author = request(&restarted, ORG, AUTHOR, 1);
    let detail = restarted
        .data
        .read_record_detail(&author, SPEND_APP, &s05.work, &s05.collection, EXP_002)
        .await
        .unwrap();
    assert_eq!(
        (detail.version, detail.work_version, detail.schema_version),
        (2, 1, 1)
    );
    assert_eq!(detail.publication, s05.publication);
    assert_eq!(
        detail
            .fields
            .iter()
            .map(|f| (f.id.as_str(), f.value.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("status", json!({"type": "enum", "value": "reviewed"})),
            (
                "note",
                json!({"type": "text", "value": "Approved — synthetic only"})
            ),
        ]
    );
    assert!(detail.can_write);
    assert_eq!(
        detail
            .commands
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["record_decision"]
    );
    assert_eq!(
        restarted
            .data
            .read_outcome(&author, SPEND_APP, &s05.work, "record_decision", IK[0])
            .await,
        Ok(Some(outcome1.clone()))
    );
    assert_eq!(
        restarted.data.commit_record(&author, &first).await,
        Ok(CommandResult::Replayed(outcome1.clone()))
    );
    // D7.11/D7.18: the restarted process holds no vault entry for the old
    // session, so the cookie is ended; a fresh session reopens the page.
    let restarted_auth = Arc::new(
        Auth::new(
            None,
            Some(restarted.clone()),
            Arc::new(TestPages) as Arc<dyn Pages>,
        )
        .unwrap(),
    );
    let restarted_app = commands::routes::router(Arc::new(Commands {
        auth: restarted_auth.clone(),
        page: Arc::new(TestPages),
    }))
    .fallback(http::fallback);
    assert_eq!(
        send(
            &restarted_app,
            "GET",
            &page_uri,
            Some(&session_cookie),
            "",
            &[]
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let reference = restarted_auth
        .vault
        .insert("northstar", &vault_token())
        .unwrap();
    let reopened = restarted
        .workspace
        .mint_session_for_tests(ORG, AUTHOR, &reference)
        .await
        .unwrap();
    let page = send(&restarted_app, "GET", &demo_uri, Some(&reopened), "", &[]).await;
    assert_eq!(
        (
            field(&page.body, "version").as_str(),
            field(&page.body, "fields").as_str()
        ),
        ("2", "status=reviewed|note=Approved — synthetic only")
    );
    assert_eq!(footprint(&fixture).await, [9, 11, 13, 8, 8, 8]);
    check(
        &mut checks,
        "after every pool and store is replaced (and the old session ended with its vault), the EXP-002 decision record reads back at version 2 (reviewed) with its values, ik-1's outcome is unchanged and replays, and a fresh session reopens the page",
    );

    // --- SR10: the chains verify; outcomes name their events ---
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    assert_eq!(out["chains"]["data"]["events"], 8);
    assert_eq!(out["command_outcomes"], 8);
    let canary = operator(&fixture, &["verify", "--organization", OTHER]);
    assert_eq!(canary["chains"]["data"]["events"], 0);
    assert_eq!(next_sequence(&fixture).await, 10);
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.command_outcomes o WHERE o.organization_id='{ORG}' AND NOT EXISTS (SELECT 1 FROM genui.audit_events e WHERE e.organization_id=o.organization_id AND e.event_id=o.audit_event_id AND e.writer_role='data' AND e.kind='command.committed' AND e.record_id=o.record_id AND e.outcome_digest=o.payload_digest)")).await,
        0
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(DISTINCT commit_sequence) FROM genui.internal_record_revisions WHERE organization_id='{ORG}'")).await,
        9
    );
    check(
        &mut checks,
        "the operator recomputes the data chain (eight command.committed events) to its head and every outcome names its own event with the record and payload digest; the canary's data chain is empty; the sequence is 10 after eight commits",
    );

    restarted.close().await.unwrap();
    super::qualification::docker(
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
    if let Some(path) = std::env::var_os("GENUI_COMMAND_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "scope": "APP-013 isolated native PostgreSQL TLS record command, outcome and audit qualification",
            "managed_acceptance": false,
            "server_version": fixture.server_version,
            "s05": {"organization_id": ORG, "app_id": SPEND_APP, "work_id": s05.work, "collection_id": s05.collection,
                    "record_id": EXP_002, "publication_id": s05.publication, "definition_digest": s05.definition_digest},
            "first_outcome": outcome1.document(),
            "checks": checks,
            "passed": true,
            "limits": [
                "Synthetic local password authentication is not RDS IAM.",
                "Identities are synthetic registry-bound fixtures; the HTTP session is minted by a test-only constructor after the seeded membership, not by the fAuth exchange (the session campaign owns that path).",
                "The database itself is not restarted (tmpfs fixture); the restart is of every application pool and store.",
                "Lock waits are provoked by the pool's test-only checkpoints and a superuser transaction; APP-014 owns the revocation and adoption races in both orders.",
                "The record page is rendered by a test page renderer; the SSR page is covered by the binary test and the HTTP smoke.",
                "The ambiguous commit is provoked by pg_terminate_backend at a test-only checkpoint before COMMIT."
            ]
        })).unwrap()).unwrap();
    }
}
