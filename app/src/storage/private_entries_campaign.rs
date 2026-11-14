//! APP-191 and APP-192 (trial plan D7, D8, D9): the private-entries campaign.
//! Isolated test executable only: local Docker, OpenSSL and the locked
//! Python environments, as the other storage campaigns.
//!
//! One general app, admitted through the shipped path: a `claims` collection
//! declared `visibility: "own"` and a shared `notes` collection; the commands
//! `submit` (a create), `edit` (an update) and `approve` (a `reviewer_only`
//! transition), plus `add_note`. The actors are created here: Owner (the
//! app's owner and the work's creator), Rev (a reviewer by a personal
//! reviewer share), Ben (a contributor by a personal share), Ana (a
//! contributor through the organization-wide share), Nia (no access until
//! shared with late in the campaign), Fay (admitted after the
//! organization-wide share) and a guest (a live share, refused).
//!
//! Every enforcement check runs twice: once as shipped, where it must hold,
//! and once with `authorization::BYPASS` set (own_rows TRUE, hidden empty,
//! the D8 gate skipped), where it must fail. The `notes` listing is
//! byte-equal in both modes: the shared collection is untouched by the rule.
//! Presentation (which commands a listing shows) and the D9 share grants are
//! asserted as shipped and labelled so; they are not what the bypass
//! disables.
use super::{
    admission_qualification::operator,
    authorization::BYPASS,
    definition_qualification::{
        grant, request, spend_review_definition, spend_review_s05, EXPENSES, EXPENSES_REVISION,
        POLICY,
    },
    qualification::Fixture,
    qualification_cases::{check, ORG},
    *,
};
use crate::{
    admission::{self, digest},
    events,
};
use genui_auth_verifier::VerifiedIdentity;
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Arc};

/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const PRIVATE_APP: &str = "00000000-0000-4000-8000-000000000a10";
const OWNER: &str = "00000000-0000-4000-8000-000000000a01";
const REV: &str = "00000000-0000-4000-8000-000000000a02";
const BEN: &str = "00000000-0000-4000-8000-000000000a03";
const ANA: &str = "00000000-0000-4000-8000-000000000a04";
const NIA: &str = "00000000-0000-4000-8000-000000000a05";
const FAY: &str = "00000000-0000-4000-8000-000000000a06";
/// A `share_work` holder who is neither the creator nor a reviewer (review m1, m2).
const KIM: &str = "00000000-0000-4000-8000-000000000a07";
/// The second app: a source view beside an own collection (review m4).
const PRIVATE_APP2: &str = "00000000-0000-4000-8000-000000000a20";
const BEN_CLAIM2: &str = "00000000-0000-4000-8000-00000000a111";
const OWNER_CLAIM2: &str = "00000000-0000-4000-8000-00000000a112";
const GUEST_SHARE: &str = "00000000-0000-4000-8000-00000000a601";
const GUEST_SHARE2: &str = "00000000-0000-4000-8000-00000000a602";
const REFRESH: &str = "00000000-0000-4000-8000-00000000a6ff";
const OWNER_CLAIM: &str = "00000000-0000-4000-8000-00000000a101";
const BEN_CLAIM: &str = "00000000-0000-4000-8000-00000000a102";
const REV_CLAIM: &str = "00000000-0000-4000-8000-00000000a103";
const ANA_CLAIM: &str = "00000000-0000-4000-8000-00000000a104";
const OWNER_NOTE: &str = "00000000-0000-4000-8000-00000000a201";
const BEN_NOTE: &str = "00000000-0000-4000-8000-00000000a202";
const ANSWER_CARD: &str = "00000000-0000-4000-8000-00000000a301";

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_private_entries_campaign() {
    std::thread::Builder::new()
        .name("app191-private-entries-campaign".into())
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

/// The campaign's app: `claims` is an own collection, `notes` is shared;
/// `approve` is reviewer-only. A general definition with no classroom words.
pub(super) fn definition() -> Value {
    let claim_fields = json!([
        {"id": "amount", "value_type": {"type": "integer", "unit": "count"}, "nullable": false},
        {"id": "status", "value_type": {"type": "enum", "values": ["filed", "approved"]}, "nullable": false},
        {"id": "note", "value_type": {"type": "text"}, "nullable": true},
        {"id": "grade", "value_type": {"type": "text"}, "nullable": true}
    ]);
    let note_fields = json!([
        {"id": "title", "value_type": {"type": "text"}, "nullable": false},
        {"id": "state", "value_type": {"type": "enum", "values": ["open", "closed"]}, "nullable": false}
    ]);
    let content = json!({
        "title": "Claims and notes",
        "description": "Synthetic campaign app: each member's own claims, reviewed by the app's reviewers; shared notes.",
        "inputs": [
            {"id": "claims", "contract_id": "claim_v1", "contract_version": 1, "kind": "records", "fields": claim_fields},
            {"id": "notes", "contract_id": "note_v1", "contract_version": 1, "kind": "records", "fields": note_fields}
        ],
        "views": [
            {"id": "count", "kind": "summary", "renderer_version": 1, "title": "Claims", "query": {"operator_version": 1, "inputs": ["claims"], "operations": [{"type": "aggregate", "group_by": [], "measures": [{"id": "n", "operation": "count", "field": {"input_id": "claims", "field_id": "amount"}}]}], "null_policy": "unknown"}},
            {"id": "claims_list", "kind": "records_list", "renderer_version": 1, "title": "Claims", "records": {"version": 1, "input_id": "claims", "schema_id": "claim", "columns": ["amount", "status", "note"]}},
            {"id": "notes_list", "kind": "records_list", "renderer_version": 1, "title": "Notes", "records": {"version": 1, "input_id": "notes", "schema_id": "note", "columns": ["title"]}},
            {"id": "submit", "kind": "form", "renderer_version": 1, "title": "Submit a claim", "command_id": "submit"}
        ],
        "record_schemas": [
            {"id": "claim", "version": 1, "visibility": "own", "fields": claim_fields},
            {"id": "note", "version": 1, "fields": note_fields}
        ],
        "commands": [
            {"id": "submit", "record_schema_id": "claim", "operation": "create", "writable_fields": ["amount", "status", "note"], "transitions": [], "initial": [{"field_id": "status", "value": "filed"}]},
            {"id": "edit", "record_schema_id": "claim", "operation": "update", "writable_fields": ["note"], "transitions": []},
            {"id": "grade", "record_schema_id": "claim", "operation": "update", "writable_fields": ["grade"], "transitions": [], "reviewer_only": true},
            {"id": "approve", "record_schema_id": "claim", "operation": "create", "writable_fields": ["status"], "transitions": [{"field_id": "status", "from": "filed", "to": "approved"}], "reviewer_only": true},
            {"id": "add_note", "record_schema_id": "note", "operation": "create", "writable_fields": ["title", "state"], "transitions": [], "initial": [{"field_id": "state", "value": "open"}]},
            {"id": "close", "record_schema_id": "note", "operation": "create", "writable_fields": ["state"], "transitions": [{"field_id": "state", "from": "open", "to": "closed"}], "reviewer_only": true}
        ],
        "links": [],
        "default_layout": [
            {"view_id": "count", "column": 0, "row": 0, "width": 12},
            {"view_id": "claims_list", "column": 0, "row": 1, "width": 12},
            {"view_id": "notes_list", "column": 0, "row": 2, "width": 12},
            {"view_id": "submit", "column": 0, "row": 3, "width": 12}
        ]
    });
    let canonical = admission::encode(&content).unwrap();
    json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": ORG,
        "app_id": PRIVATE_APP,
        "revision_id": "00000000-0000-4000-8000-000000000a11",
        "content": content,
        "digest": digest::digest(digest::APP_DEFINITION, &canonical),
    })
}

/// The second campaign app (review m4): the S05 expenses source beside an own
/// `claims` collection, so a source view's snapshot sequence is judged by the
/// rule for members and guests. The input contract is the fixture's, verbatim.
fn second_definition() -> Value {
    let expenses = spend_review_definition(ORG)["content"]["inputs"][0].clone();
    let claim_fields = json!([
        {"id": "amount", "value_type": {"type": "integer", "unit": "count"}, "nullable": false}
    ]);
    let content = json!({
        "title": "Spend and claims",
        "description": "Synthetic campaign app: a source view beside each member's own claims.",
        "inputs": [
            expenses,
            {"id": "claims", "contract_id": "claim2_v1", "contract_version": 1, "kind": "records", "fields": claim_fields}
        ],
        "views": [
            {"id": "spend", "kind": "summary", "renderer_version": 1, "title": "Spend", "query": {"operator_version": 1, "inputs": ["expenses"], "operations": [{"type": "aggregate", "group_by": [], "measures": [{"id": "n", "operation": "count", "field": {"input_id": "expenses", "field_id": "expense_id"}}]}], "null_policy": "unknown"}},
            {"id": "submit", "kind": "form", "renderer_version": 1, "title": "Submit a claim", "command_id": "submit"}
        ],
        "record_schemas": [
            {"id": "claim", "version": 1, "visibility": "own", "fields": claim_fields}
        ],
        "commands": [
            {"id": "submit", "record_schema_id": "claim", "operation": "create", "writable_fields": ["amount"], "transitions": []}
        ],
        "links": [],
        "default_layout": [
            {"view_id": "spend", "column": 0, "row": 0, "width": 12},
            {"view_id": "submit", "column": 0, "row": 1, "width": 12}
        ]
    });
    let canonical = admission::encode(&content).unwrap();
    json!({
        "kind": "app_definition",
        "schema_version": 1,
        "organization_id": ORG,
        "app_id": PRIVATE_APP2,
        "revision_id": "00000000-0000-4000-8000-000000000a21",
        "content": content,
        "digest": digest::digest(digest::APP_DEFINITION, &canonical),
    })
}

/// The admitted form of [`definition`] (admission refuses nothing in it).
fn admitted() -> admission::AdmittedDefinition {
    admission::admit_definition(&serde_json::to_vec(&definition()).unwrap())
        .expect("the campaign's definition admits")
}

/// Operator-channel stand-in (as tenant_seed writes fixture authority): one
/// active member of the organization with a live entitlement.
fn admit_member(fixture: &Fixture, principal: &str) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{ORG}','{principal}','active',1); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
}

/// Calls a genui_admit definer directly as the workspace role under
/// `principal`'s scope, in a transaction that is rolled back: how many rows it
/// answers (a refusal answers none). This judges the SQL-side rule by itself,
/// without the storage layer's own `Permit` checks in front of it.
async fn definer_rows(fixture: &Fixture, principal: &str, sql: &str) -> i64 {
    let mut operator = fixture.operator().await;
    let transaction = operator.client.transaction().await.unwrap();
    transaction
        .batch_execute("SET LOCAL ROLE genui_workspace")
        .await
        .unwrap();
    transaction
        .execute(
            "SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)",
            &[&ORG, &principal],
        )
        .await
        .unwrap();
    let rows: i64 = transaction.query_one(sql, &[]).await.unwrap().get(0);
    transaction.rollback().await.unwrap();
    rows
}

/// A guest's synthetic identity proof (as the live-sharing campaign mints it).
fn proof(subject: &str, email: &str) -> ShareIdentityProof {
    ShareIdentityProof::verified(
        &VerifiedIdentity {
            key_id: "synthetic-key".into(),
            issuer: "app008-fixture-only".into(),
            tenant_id: "ten_guests".into(),
            subject: subject.into(),
            issued_at: 1,
            expires_at: 2,
        },
        email,
        true,
    )
    .unwrap()
}
async fn guest_account(stores: &Stores, subject: &str, email: &str) -> ShareAccount {
    let cookie = stores
        .workspace
        .mint_shared_session(&proof(subject, email), REFRESH)
        .await
        .unwrap();
    stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .unwrap()
}
/// A live share the product refuses to create (its work has an own
/// collection), written by the operator stand-in so that the guest read sites
/// can be judged: the row rule, not the share's pinned collection list, must
/// be what keeps a guest out of the own rows.
#[allow(clippy::too_many_arguments)]
fn insert_guest_share(
    fixture: &Fixture,
    share: &str,
    app: &str,
    work: &str,
    publication: &str,
    definition_digest: &str,
    views: &[&str],
    collections: &[&str],
    email: &str,
) {
    let views = views
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(",");
    let collections = collections
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(",");
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at) VALUES ('{ORG}','{share}','{app}','{work}','{OWNER}',1,'{email}','{publication}','{definition_digest}',1,'{{{views}}}','{{{collections}}}','campaign guest share',false,1,'{share}','{definition_digest}',clock_timestamp(),clock_timestamp()+interval '7 days'); COMMIT;"), "genui");
}

/// Whether, within five seconds, another backend waits on a lock while
/// running a statement that names `needle` (`pg_stat_activity`): the
/// deterministic sign that a second session queued behind the first's open
/// transaction instead of reading past it.
async fn lock_wait(fixture: &Fixture, needle: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let waiting = scalar(fixture, &format!("SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock' AND state='active' AND pid<>pg_backend_pid() AND query LIKE '%{needle}%'")).await;
        if waiting > 0 {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    false
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

fn claim_values(amount: &str, status: &str, note: &str) -> Value {
    json!([
        {"field_id": "amount", "value": {"type": "integer", "value": amount, "unit": "count"}},
        {"field_id": "status", "value": {"type": "enum", "value": status}},
        {"field_id": "note", "value": {"type": "text", "value": note}}
    ])
}

struct Work {
    app: String,
    work: String,
    publication: String,
    claims: String,
    notes: String,
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

async fn committed(
    stores: &Stores,
    who: &TrustedRequest,
    request: &CommandRequest,
) -> CommandOutcome {
    match stores.data.commit_record(who, request).await {
        Ok(CommandResult::Committed(outcome)) => outcome,
        other => panic!(
            "{} on {} by {} must commit: {other:?}",
            request.command,
            request.work,
            who.principal()
        ),
    }
}

/// The record ids a listing shows for one collection, sorted.
fn ids_in(listing: &RecordsListing, collection: &str) -> Vec<String> {
    let mut ids: Vec<String> = listing
        .records
        .iter()
        .filter(|r| r.collection == collection)
        .map(|r| r.record_id.clone())
        .collect();
    ids.sort();
    ids
}
fn sorted(ids: &[&str]) -> Vec<String> {
    let mut ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    ids.sort();
    ids
}
/// The command ids a listing shows for one collection.
fn commands_of(listing: &RecordsListing, collection: &str) -> Vec<String> {
    listing
        .collections
        .iter()
        .find(|c| c.collection == collection)
        .map(|c| c.commands.iter().map(|d| d.id.clone()).collect())
        .unwrap_or_default()
}
/// How many revisions the work holds (nothing written on a refusal).
async fn revisions(fixture: &Fixture, work: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id='{ORG}' AND work_id='{work}'")).await
}
async fn chain(fixture: &Fixture, kind: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='{kind}'")).await
}

/// An enforcement check: holds as shipped, fails with the rule bypassed.
/// The live-hint acceptance case runs twice (as shipped, then bypassed); each
/// round admits its own members and commits its own rows.
static LIVE_HINT_ROUND: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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
            "{} still holds with own_rows bypassed: the check does not depend on the rule",
            $name
        );
        check($checks, &format!("{} (fails under bypass)", $name));
    }};
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
    for member in [OWNER, REV, BEN, ANA, NIA, KIM] {
        admit_member(&fixture, member);
    }
    let owner = request(&stores, ORG, OWNER, 1);
    let rev = request(&stores, ORG, REV, 1);
    let ben = request(&stores, ORG, BEN, 1);
    let ana = request(&stores, ORG, ANA, 1);
    let nia = request(&stores, ORG, NIA, 1);

    // --- the app, through the admitted path ---------------------------------
    let created = operator(
        &fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            PRIVATE_APP,
            "--space",
            SPACE,
            "--owner",
            OWNER,
        ],
    );
    assert_eq!(created["granted"], json!(["edit_definition"]));
    for permission in ["discover", "run"] {
        grant(&fixture, OWNER, permission, &["--app", PRIVATE_APP]);
    }
    let admitted = admitted();
    let revision = stores
        .workspace
        .create_definition_revision(&owner, PRIVATE_APP, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(&owner, PRIVATE_APP, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let saved = stores
        .workspace
        .create_work(
            &owner,
            PRIVATE_APP,
            &publication.publication_id,
            "Claims and notes (synthetic campaign)",
            &[
                WorkBinding::Records {
                    input: "claims".into(),
                    schema: "claim".into(),
                },
                WorkBinding::Records {
                    input: "notes".into(),
                    schema: "note".into(),
                },
            ],
        )
        .await
        .unwrap();
    let work_id = saved.work.clone();
    let collection = |schema: &str| {
        format!("SELECT collection_id::text FROM genui.record_collections WHERE organization_id='{ORG}' AND work_id='{work_id}' AND schema_id='{schema}'")
    };
    let w = Work {
        app: PRIVATE_APP.into(),
        work: work_id.clone(),
        publication: publication.publication_id.clone(),
        claims: text(&fixture, &collection("claim")).await,
        notes: text(&fixture, &collection("note")).await,
    };
    let work = w.work.as_str();
    // The database derived `owner_only` from the stored canonical schema.
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND owner_only")).await,
        1
    );
    assert_eq!(
        text(&fixture, &format!("SELECT schema_id FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND owner_only")).await,
        "claim"
    );
    check(&mut checks, "migration 0024: owner_only is derived in the database from the admitted canonical schema (`claim` own, `note` shared); no writer changed");

    // --- grants: Rev a reviewer, Ben a contributor, Nia nothing -------------
    let granted = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(granted.grantee.as_deref(), Some(REV));
    let mut names: Vec<String> = granted
        .granted
        .iter()
        .map(|g| g.permission.clone())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "discover",
            "record_read",
            "record_review",
            "record_write",
            "run"
        ]
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}' AND permission='record_review'")).await,
        1
    );
    let granted = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(BEN),
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    let mut names: Vec<String> = granted
        .granted
        .iter()
        .map(|g| g.permission.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["discover", "record_read", "record_write", "run"]);
    check(&mut checks, "D9: a reviewer share writes record_read, record_write and record_review (plus discover and run); a contributor share writes no record_review");

    // --- records: Owner, Ben and Rev file claims; Owner and Ben add notes ----
    let owner_claim = committed(
        &stores,
        &owner,
        &command(
            &w,
            "submit",
            "claim",
            OWNER_CLAIM,
            0,
            claim_values("120", "filed", "claim by the owner"),
            &"a".repeat(32),
        ),
    )
    .await;
    let ben_claim = committed(
        &stores,
        &ben,
        &command(
            &w,
            "submit",
            "claim",
            BEN_CLAIM,
            0,
            claim_values("45", "filed", "claim by ben"),
            &"b".repeat(32),
        ),
    )
    .await;
    committed(
        &stores,
        &rev,
        &command(
            &w,
            "submit",
            "claim",
            REV_CLAIM,
            0,
            claim_values("70", "filed", "claim by rev"),
            &"c".repeat(32),
        ),
    )
    .await;
    committed(
        &stores,
        &owner,
        &command(
            &w,
            "add_note",
            "note",
            OWNER_NOTE,
            0,
            json!([{"field_id": "title", "value": {"type": "text", "value": "owner note"}}, {"field_id": "state", "value": {"type": "enum", "value": "open"}}]),
            &"d".repeat(32),
        ),
    )
    .await;
    committed(
        &stores,
        &ben,
        &command(
            &w,
            "add_note",
            "note",
            BEN_NOTE,
            0,
            json!([{"field_id": "title", "value": {"type": "text", "value": "ben note"}}, {"field_id": "state", "value": {"type": "enum", "value": "open"}}]),
            &"e".repeat(32),
        ),
    )
    .await;
    let all_claims = sorted(&[OWNER_CLAIM, BEN_CLAIM, REV_CLAIM]);
    let all_notes = sorted(&[OWNER_NOTE, BEN_NOTE]);
    // --- list, search, target (canvas, MCP and Slack share these reads) ------
    judged!(&mut checks, "list: a contributor sees only their own claims and every note; a reviewer and the owner see all claims", {
        let b = stores.data.list_records(&ben, PRIVATE_APP, work).await.unwrap();
        let r = stores.data.list_records(&rev, PRIVATE_APP, work).await.unwrap();
        let o = stores.data.list_records(&owner, PRIVATE_APP, work).await.unwrap();
        ids_in(&b, &w.claims) == sorted(&[BEN_CLAIM])
            && ids_in(&b, &w.notes) == all_notes
            && ids_in(&r, &w.claims) == all_claims
            && ids_in(&o, &w.claims) == all_claims
            && !b.truncated
    });
    judged!(&mut checks, "search: a contributor's search of the own collection finds their rows only; a reviewer's finds all", {
        let b = stores.data.search_records(&ben, PRIVATE_APP, work, &w.claims, "claim by").await.unwrap();
        let r = stores.data.search_records(&rev, PRIVATE_APP, work, &w.claims, "claim by").await.unwrap();
        ids_in(&b, &w.claims) == sorted(&[BEN_CLAIM]) && ids_in(&r, &w.claims) == all_claims
    });
    judged!(&mut checks, "target: another member's record is absent (404) to a contributor; their own and a reviewer's targets answer", {
        stores.data.list_record_target(&ben, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.err() == Some(Error::Denied)
            && stores.data.list_record_target(&ben, PRIVATE_APP, work, &w.claims, BEN_CLAIM).await.is_ok()
            && stores.data.list_record_target(&rev, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.is_ok()
    });
    // The shared collection is byte-equal with and without the rule.
    BYPASS.store(false, Ordering::SeqCst);
    let notes_shipped = stores
        .data
        .list_records(&ben, PRIVATE_APP, work)
        .await
        .unwrap();
    BYPASS.store(true, Ordering::SeqCst);
    let notes_bypassed = stores
        .data
        .list_records(&ben, PRIVATE_APP, work)
        .await
        .unwrap();
    BYPASS.store(false, Ordering::SeqCst);
    let only_notes = |listing: &RecordsListing| -> Vec<RecordRow> {
        listing
            .records
            .iter()
            .filter(|r| r.collection == w.notes)
            .cloned()
            .collect()
    };
    assert_eq!(only_notes(&notes_shipped), only_notes(&notes_bypassed));
    assert_eq!(only_notes(&notes_shipped).len(), 2);
    assert_ne!(notes_shipped.records.len(), notes_bypassed.records.len());
    check(&mut checks, "the shared `notes` rows of a contributor's listing are identical with and without the rule; only the own collection differs");

    // --- the canvas batch and Ask -------------------------------------------
    judged!(
        &mut checks,
        "work_reads (the canvas page): a contributor's batch listing carries their own claims only",
        {
            let reads = stores
                .data
                .work_reads(&ben, &[(PRIVATE_APP, work, true)])
                .await
                .unwrap();
            let listing = reads[0].records.clone().unwrap().unwrap();
            ids_in(&listing, &w.claims) == sorted(&[BEN_CLAIM])
                && ids_in(&listing, &w.notes) == all_notes
        }
    );
    judged!(&mut checks, "Ask (read-only listings): a contributor's snapshot carries their own claims only; a reviewer's carries all", {
        let b = stores.data.read_only_listings(&ben, &[(PRIVATE_APP, work)]).await.unwrap();
        let r = stores.data.read_only_listings(&rev, &[(PRIVATE_APP, work)]).await.unwrap();
        ids_in(&b[0], &w.claims) == sorted(&[BEN_CLAIM]) && ids_in(&r[0], &w.claims) == all_claims
    });

    // --- query aggregates and record components -----------------------------
    judged!(&mut checks, "query snapshot (summaries, counts): a contributor's count covers their own rows only, a reviewer's covers all", {
        let b = stores.data.run_query(&ben, PRIVATE_APP, work, "count", &json!({})).await.unwrap();
        let r = stores.data.run_query(&rev, PRIVATE_APP, work, "count", &json!({})).await.unwrap();
        let rows = |run: &QueryRun| run.records.as_ref().unwrap().bytes().iter().filter(|b| **b == b'\n').count();
        rows(&b) == 1 && rows(&r) == 3
    });
    judged!(&mut checks, "record components (records lists, summaries, ranks): a contributor's dataset holds their own rows only", {
        let b = stores.data.record_components(&ben, PRIVATE_APP, work).await.unwrap();
        let r = stores.data.record_components(&rev, PRIVATE_APP, work).await.unwrap();
        let rows = |run: &RecordComponentsRun, input: &str| {
            run.payload()["datasets"].as_array().unwrap().iter()
                .find(|d| d["input_id"] == input).unwrap()["rows"].as_array().unwrap().len()
        };
        rows(&b, "claims") == 1 && rows(&r, "claims") == 3 && rows(&b, "notes") == 2 && rows(&r, "notes") == 2
    });

    // --- another member's record: detail, commit, dry run, wrong version, reused key
    judged!(&mut checks, "detail and read_record: another member's record is 404 to a contributor, open to a reviewer", {
        stores.data.read_record_detail(&ben, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.err() == Some(Error::Denied)
            && stores.data.read_record(&ben, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.err() == Some(Error::Denied)
            && stores.data.read_record(&ben, PRIVATE_APP, work, &w.claims, BEN_CLAIM).await.is_ok()
            && stores.data.read_record(&rev, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.is_ok()
            && stores.data.read_record_detail(&rev, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.is_ok()
    });
    judged!(&mut checks, "commit on another member's record: 404 with nothing written and no recovery view (right version, wrong version, dry run)", {
        let before = revisions(&fixture, work).await;
        let current = stores.data.read_record(&owner, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.unwrap().version;
        let edit = |expected: i64, key: &str| command(&w, "edit", "claim", OWNER_CLAIM, expected, json!([{"field_id": "note", "value": {"type": "text", "value": "tampered"}}]), key);
        let denied = |r: &Result<CommandResult>| matches!(r, Err(Error::Denied));
        // The dry run and the wrong-version commit write nothing in either
        // mode (under the bypass they answer a recovery view instead of 404);
        // the right-version commit is attempted only once both were refused,
        // so the bypass run never writes the owner's record.
        let dry = stores.data.dry_run_command(&ben, &edit(current, &"1".repeat(32))).await;
        let wrong = stores.data.commit_record(&ben, &edit(current + 6, &"0".repeat(32))).await;
        let right = if dry.is_err() && denied(&wrong) {
            denied(&stores.data.commit_record(&ben, &edit(current, &"f".repeat(32))).await)
        } else {
            false
        };
        dry.err() == Some(Error::Denied) && denied(&wrong) && right && revisions(&fixture, work).await == before
    });
    judged!(&mut checks, "a reused key against another member's record: 404, no recovery view and no stored outcome disclosed", {
        // Ben's own earlier `submit` key, replayed with a different payload at
        // the owner's record: the stored-outcome conflict path (nothing is
        // written on it) would otherwise answer a recovery view with the
        // record's current values and version.
        let before = revisions(&fixture, work).await;
        let reused = command(&w, "submit", "claim", OWNER_CLAIM, 0, claim_values("1", "filed", "replayed"), &"b".repeat(32));
        matches!(stores.data.commit_record(&ben, &reused).await, Err(Error::Denied))
            && revisions(&fixture, work).await == before
    });
    judged!(
        &mut checks,
        "a contributor can still change their own record (edit), and the change is theirs",
        {
            let own = command(
                &w,
                "edit",
                "claim",
                BEN_CLAIM,
                1,
                json!([{"field_id": "note", "value": {"type": "text", "value": "claim by ben, amended"}}]),
                &"2".repeat(32),
            );
            let dry = stores.data.dry_run_command(&ben, &own).await;
            // The edit is allowed in both modes; the check binds it to the rule
            // by also requiring that the owner's record stays unseen.
            matches!(dry, Ok(DryRun::Valid { .. }))
                && stores
                    .data
                    .read_record(&ben, PRIVATE_APP, work, &w.claims, OWNER_CLAIM)
                    .await
                    .is_err()
        }
    );
    let edited = committed(&stores, &ben, &command(&w, "edit", "claim", BEN_CLAIM, 1, json!([{"field_id": "note", "value": {"type": "text", "value": "claim by ben, amended"}}]), &"3".repeat(32))).await;
    assert_eq!(edited.record_version, 2);

    // --- D8: reviewer-only commands -----------------------------------------
    judged!(&mut checks, "D8: a contributor's reviewer-only command on a shared row they can see (close another member's note) is refused on the dry run and the commit, nothing written", {
        // The shared collection isolates the gate: Ben sees the owner's note
        // and did not file it, so only `reviewer_only` stands in the way.
        let close = |key: &str| command(&w, "close", "note", OWNER_NOTE, 1, json!([{"field_id": "state", "value": {"type": "enum", "value": "closed"}}]), key);
        let before = revisions(&fixture, work).await;
        let dry = stores.data.dry_run_command(&ben, &close(&"4".repeat(32))).await;
        // The commit is attempted only once the dry run was refused, so the
        // bypass run (where the gate is off) never closes the owner's note.
        let commit = if dry.is_err() {
            matches!(stores.data.commit_record(&ben, &close(&"5".repeat(32))).await, Err(Error::Denied))
        } else {
            false
        };
        dry.err() == Some(Error::Denied) && commit && revisions(&fixture, work).await == before
    });
    // On the own collection the gate and the row rule both refuse; `by_filer`
    // refuses Ben's approve of his own claim even where the gate is off.
    let approve_own = |key: &str| {
        command(
            &w,
            "approve",
            "claim",
            BEN_CLAIM,
            2,
            json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]),
            key,
        )
    };
    assert_eq!(
        stores
            .data
            .dry_run_command(&ben, &approve_own(&"a4".repeat(16)))
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(matches!(
        stores
            .data
            .commit_record(&ben, &approve_own(&"a5".repeat(16)))
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .data
            .commit_record(
                &ben,
                &command(
                    &w,
                    "approve",
                    "claim",
                    OWNER_CLAIM,
                    1,
                    json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]),
                    &"a6".repeat(16)
                )
            )
            .await,
        Err(Error::Denied)
    ));
    let closed = committed(
        &stores,
        &rev,
        &command(
            &w,
            "close",
            "note",
            OWNER_NOTE,
            1,
            json!([{"field_id": "state", "value": {"type": "enum", "value": "closed"}}]),
            &"a7".repeat(16),
        ),
    )
    .await;
    assert_eq!(closed.record_version, 2);
    check(&mut checks, "D8: a contributor's approve is refused on their own and on another member's claim; a reviewer's close of a member's note commits");
    let approve_ben = command(
        &w,
        "approve",
        "claim",
        BEN_CLAIM,
        2,
        json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]),
        &"6".repeat(32),
    );
    let approved = committed(&stores, &rev, &approve_ben).await;
    assert_eq!(approved.record_version, 3);
    check(
        &mut checks,
        "D8: a reviewer's approve of a contributor's claim commits",
    );
    let by_filer = command(
        &w,
        "approve",
        "claim",
        REV_CLAIM,
        1,
        json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]),
        &"7".repeat(32),
    );
    assert!(matches!(
        stores.data.commit_record(&rev, &by_filer).await,
        Err(Error::Denied)
    ));
    let owner_version = stores
        .data
        .read_record(&owner, PRIVATE_APP, work, &w.claims, OWNER_CLAIM)
        .await
        .unwrap()
        .version;
    assert!(matches!(
        stores
            .data
            .commit_record(
                &owner,
                &command(
                    &w,
                    "approve",
                    "claim",
                    OWNER_CLAIM,
                    owner_version,
                    json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]),
                    &"8".repeat(32),
                ),
            )
            .await,
        Err(Error::Denied)
    ));
    check(
        &mut checks,
        "D8: by_filer stays: a reviewer (and the owner) cannot approve their own claim",
    );
    // Presentation: shipped only (the bypass disables the gate, not the listing).
    let b = stores
        .data
        .list_records(&ben, PRIVATE_APP, work)
        .await
        .unwrap();
    let r = stores
        .data
        .list_records(&rev, PRIVATE_APP, work)
        .await
        .unwrap();
    assert_eq!(commands_of(&b, &w.claims), ["edit", "submit"]);
    assert_eq!(
        commands_of(&r, &w.claims),
        ["approve", "edit", "grade", "submit"]
    );
    assert_eq!(commands_of(&b, &w.notes), ["add_note"]);
    assert_eq!(commands_of(&r, &w.notes), ["add_note", "close"]);
    let detail = stores
        .data
        .read_record_detail(&ben, PRIVATE_APP, work, &w.claims, BEN_CLAIM)
        .await
        .unwrap();
    assert!(detail.commands.iter().all(|c| c.id != "approve"));
    let detail = stores
        .data
        .read_record_detail(&owner, PRIVATE_APP, work, &w.claims, BEN_CLAIM)
        .await
        .unwrap();
    assert!(detail.commands.iter().any(|c| c.id == "approve"));
    check(&mut checks, "D8 presentation (as shipped): listings and the record detail show `approve` to reviewers and the owner only");

    // --- live events ---------------------------------------------------------
    judged!(&mut checks, "events: a commit in the own collection reaches its creator and the reviewers, not another contributor", {
        let judged = stores.data.openable_principals(&owner, PRIVATE_APP, work, Some(ben_claim.commit_sequence), &[OWNER.into(), REV.into(), BEN.into(), NIA.into()]).await.unwrap();
        let shared = stores.data.openable_principals(&owner, PRIVATE_APP, work, Some(owner_claim.commit_sequence), &[OWNER.into(), REV.into(), BEN.into()]).await.unwrap();
        judged.standing(&owner) == Standing::Open
            && judged.standing(&rev) == Standing::Open
            && judged.standing(&ben) == Standing::Open
            && judged.standing(&nia) == Standing::Closed
            && shared.standing(&ben) == Standing::Closed
            && shared.standing(&owner) == Standing::Open
    });

    // --- pinned answers -------------------------------------------------------
    // A pinned answer is its maker's alone: `share_card` refuses adding one for
    // everyone and 0015's CHECK (kind = 'card' OR NOT everyone) refuses it in
    // the database, so no other member ever lists it. The `member_cards`
    // clause that would hide an answer over hidden rows from a non-maker is
    // therefore defence in depth today; what is asserted here is the
    // outcome it protects: another member never sees the answer.
    let card = NewCard {
        card: ANSWER_CARD.into(),
        kind: "answer",
        app: Some(PRIVATE_APP.into()),
        work: Some(work.into()),
        source: String::new(),
        address: String::new(),
        title: "Claims so far".into(),
        spec: admission::encode(&json!({"answer": "four claims"})).unwrap(),
        reads: vec![(PRIVATE_APP.into(), work.into())],
    };
    assert!(matches!(
        stores.workspace.make_card(&owner, &card).await.unwrap(),
        Carded::Saved(_)
    ));
    assert!(matches!(
        stores
            .workspace
            .share_card(&owner, ANSWER_CARD, true)
            .await
            .unwrap(),
        Carded::Refused("not_shareable")
    ));
    let forced = fixture.operator().await.client.execute(&format!("UPDATE genui.member_cards SET everyone=true WHERE organization_id='{ORG}' AND card_id='{ANSWER_CARD}'"), &[]).await;
    assert!(
        forced.is_err(),
        "0015's CHECK keeps a pinned answer personal"
    );
    let has = |cards: Vec<MemberCard>| cards.iter().any(|c| c.card == ANSWER_CARD);
    assert!(has(stores.workspace.member_cards(&owner).await.unwrap()));
    assert!(!has(stores.workspace.member_cards(&rev).await.unwrap()));
    assert!(!has(stores.workspace.member_cards(&ben).await.unwrap()));
    check(&mut checks, "pinned answers (as shipped): an answer derived from the work's rows lists to its maker only; neither the share route nor the database lets it reach another member, so no hidden row leaks through it");

    // --- D9: no access, the organization-wide share, a future member, roles ---
    assert_eq!(
        stores
            .data
            .list_records(&ana, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .list_records(&nia, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .work_reads(&nia, &[(PRIVATE_APP, work, true)])
            .await
            .unwrap()[0]
            .outline
            .as_ref()
            .err(),
        Some(&Error::Denied)
    );
    check(&mut checks, "D9: a member with no grant and no organization-wide share gets the uniform 404 on the work");
    // Share authority is never copied: a reviewer or contributor who is not the
    // creator and holds no share_work cannot share or grant anything.
    for (who, label) in [(&rev, "reviewer"), (&ben, "contributor")] {
        for role in [ShareRole::Viewer, ShareRole::Reviewer] {
            assert_eq!(
                stores
                    .workspace
                    .share_work(
                        who,
                        PRIVATE_APP,
                        work,
                        Audience::Member(NIA),
                        role,
                        ShareMode::Set
                    )
                    .await
                    .err(),
                Some(Error::Denied),
                "{label}"
            );
            assert_eq!(
                stores
                    .workspace
                    .share_work(
                        who,
                        PRIVATE_APP,
                        work,
                        Audience::Organization,
                        role,
                        ShareMode::Set
                    )
                    .await
                    .err(),
                Some(Error::Denied),
                "{label}"
            );
        }
        assert_eq!(
            stores
                .workspace
                .revoke_organization_share(who, PRIVATE_APP, work)
                .await
                .err(),
            Some(Error::Denied),
            "{label}"
        );
    }
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}'")).await, 2);
    check(&mut checks, "D9: a reviewer without share_work cannot share or grant at all (member or organization, any role); neither can a contributor");
    let granted_before = chain(&fixture, "permission.granted").await;
    let everyone = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(everyone.grantee, None);
    assert!(everyone.changed());
    let mut names: Vec<String> = everyone
        .granted
        .iter()
        .map(|g| g.permission.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["discover", "record_read", "record_write", "run"]);
    assert_eq!(
        chain(&fixture, "permission.granted").await,
        granted_before + 1
    );
    let digest_ok = scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events e JOIN genui.work_shares s ON s.organization_id=e.organization_id AND s.audit_event_id=e.event_id WHERE e.organization_id='{ORG}' AND s.grantee_id IS NULL AND s.role='contributor' AND s.revoked_at IS NULL AND e.outcome_digest='sha256:'||encode(sha256(convert_to('forma:share-grant:v1','UTF8')||'\\x00'::bytea||convert_to('everyone','UTF8')||'\\x00'::bytea||convert_to(array_to_string(s.granted,','),'UTF8')),'hex')")).await;
    assert_eq!(digest_ok, 1);
    let same = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert!(!same.changed());
    assert_eq!(
        chain(&fixture, "permission.granted").await,
        granted_before + 1
    );
    check(&mut checks, "D9: the owner shares with everyone as contributor: one live work_shares row with no grantee, one permission.granted whose digest recomputes from the row; the same share again writes nothing");
    let a = stores
        .data
        .list_records(&ana, PRIVATE_APP, work)
        .await
        .unwrap();
    assert_eq!(ids_in(&a, &w.claims), Vec::<String>::new());
    assert_eq!(ids_in(&a, &w.notes), all_notes);
    assert!(a.can_write);
    committed(
        &stores,
        &ana,
        &command(
            &w,
            "submit",
            "claim",
            ANA_CLAIM,
            0,
            claim_values("15", "filed", "claim by ana"),
            &"9".repeat(32),
        ),
    )
    .await;
    let reads = stores
        .data
        .work_reads(&ana, &[(PRIVATE_APP, work, true)])
        .await
        .unwrap();
    assert!(reads[0].outline.is_ok());
    assert!(stores
        .data
        .run_query(&ana, PRIVATE_APP, work, "count", &json!({}))
        .await
        .is_ok());
    check(&mut checks, "D9: a member with no personal grant opens, lists, queries and files through the organization-wide share (contributor)");
    judged!(&mut checks, "D9 x D7: through the organization-wide share a contributor sees their own claim only; the owner sees it too", {
        let a = stores.data.list_records(&ana, PRIVATE_APP, work).await.unwrap();
        let o = stores.data.list_records(&owner, PRIVATE_APP, work).await.unwrap();
        ids_in(&a, &w.claims) == sorted(&[ANA_CLAIM]) && ids_in(&o, &w.claims) == sorted(&[OWNER_CLAIM, BEN_CLAIM, REV_CLAIM, ANA_CLAIM])
    });
    admit_member(&fixture, FAY);
    let fay = request(&stores, ORG, FAY, 1);
    let f = stores
        .data
        .list_records(&fay, PRIVATE_APP, work)
        .await
        .unwrap();
    assert_eq!(ids_in(&f, &w.notes), all_notes);
    assert_eq!(ids_in(&f, &w.claims), Vec::<String>::new());
    assert!(stores.data.can_open(&fay, PRIVATE_APP, work).await.is_ok());
    check(&mut checks, "D9: a member admitted after the organization-wide share is covered by it (list, open) with no new grant");
    // Viewer re-share: the role is replaced in one unit.
    let revoked_before = chain(&fixture, "permission.revoked").await;
    let viewer = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Viewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert!(viewer.changed());
    assert_eq!(
        chain(&fixture, "permission.revoked").await,
        revoked_before + 1
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, 1);
    assert_eq!(text(&fixture, &format!("SELECT role FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, "viewer");
    let a = stores
        .data
        .list_records(&ana, PRIVATE_APP, work)
        .await
        .unwrap();
    assert!(!a.can_write);
    assert_eq!(ids_in(&a, &w.claims), sorted(&[ANA_CLAIM]));
    assert_eq!(
        stores
            .data
            .dry_run_command(
                &ana,
                &command(
                    &w,
                    "edit",
                    "claim",
                    ANA_CLAIM,
                    1,
                    json!([{"field_id": "note", "value": {"type": "text", "value": "x"}}]),
                    &"a1".repeat(16)
                )
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    check(&mut checks, "D9: re-sharing with everyone as viewer revokes the contributor share and grants the viewer share in one unit (permission.revoked then permission.granted); members read but cannot write");
    assert!(stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap());
    assert_eq!(
        chain(&fixture, "permission.revoked").await,
        revoked_before + 2
    );
    assert!(!stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap());
    assert_eq!(
        stores
            .data
            .list_records(&ana, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .list_records(&fay, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(stores.data.can_open(&fay, PRIVATE_APP, work).await.is_err());
    assert_eq!(
        stores
            .data
            .list_records(&ben, PRIVATE_APP, work)
            .await
            .map(|l| ids_in(&l, &w.claims))
            .unwrap(),
        sorted(&[BEN_CLAIM])
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, 0);
    check(&mut checks, "D9: revoking the organization-wide share ends it for every covered member at once (a second revocation changes nothing); personal grants stay");
    // Only a sharer who is a reviewer may grant reviewer: Ben with share_work
    // may share what he holds (viewer, contributor), never the reviewer role.
    // Operator-channel stand-in for a `share_work` grant (the operator's
    // closed GRANTABLE set has no share_work; tenant_seed writes fixture
    // authority the same way).
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission) VALUES ('{ORG}','{BEN}','{PRIVATE_APP}','{work}','share_work'); COMMIT;"), "genui");
    assert_eq!(
        stores
            .workspace
            .share_work(
                &ben,
                PRIVATE_APP,
                work,
                Audience::Member(NIA),
                ShareRole::Reviewer,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .share_work(
                &ben,
                PRIVATE_APP,
                work,
                Audience::Organization,
                ShareRole::Reviewer,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{NIA}'")).await, 0);
    let nia_share = stores
        .workspace
        .share_work(
            &ben,
            PRIVATE_APP,
            work,
            Audience::Member(NIA),
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    let mut names: Vec<String> = nia_share
        .granted
        .iter()
        .map(|g| g.permission.clone())
        .collect();
    names.sort();
    assert_eq!(names, ["discover", "record_read", "record_write", "run"]);
    let n = stores
        .data
        .list_records(&nia, PRIVATE_APP, work)
        .await
        .unwrap();
    assert_eq!(ids_in(&n, &w.claims), Vec::<String>::new());
    assert_eq!(ids_in(&n, &w.notes), all_notes);
    check(&mut checks, "D9: a share_work holder who is not a reviewer may share viewer or contributor (what they hold) but never reviewer, to a member or to everyone");
    // The SQL definers judge the same rules themselves (the storage layer's
    // `Permit::also(RecordReview)` stands in front of them in production):
    // direct calls under the workspace role, rolled back.
    let share = |sharer: &str, grantee: &str, role: &str| {
        format!("SELECT count(*) FROM genui_private.share_work('{ORG}','{sharer}','{grantee}','{PRIVATE_APP}','{work}','{role}','set')")
    };
    let ensure = |sharer: &str, grantee: &str, role: &str| {
        format!("SELECT count(*) FROM genui_private.share_work('{ORG}','{sharer}','{grantee}','{PRIVATE_APP}','{work}','{role}','ensure')")
    };
    let everyone_sql = |sharer: &str, role: &str| {
        format!("SELECT count(*) FROM genui_private.set_work_everyone('{ORG}','{sharer}','{PRIVATE_APP}','{work}','{role}')")
    };
    // Recheck r1: never the app's owner (nor the sharer) as grantee, so no
    // lower re-share can strip the owner's rows.
    let owner_rows_before = scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{OWNER}' AND work_id='{work}'")).await;
    assert_eq!(
        definer_rows(&fixture, BEN, &share(BEN, OWNER, "viewer")).await,
        0,
        "the app owner is never a grantee"
    );
    assert_eq!(
        definer_rows(&fixture, BEN, &share(BEN, BEN, "viewer")).await,
        0,
        "nor the sharer"
    );
    assert_eq!(
        stores
            .workspace
            .share_work(
                &ben,
                PRIVATE_APP,
                work,
                Audience::Member(OWNER),
                ShareRole::Viewer,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{OWNER}' AND work_id='{work}'")).await, owner_rows_before, "the owner's rows are untouched");
    check(&mut checks, "recheck r1: share_work refuses the app owner and the sharer as grantee (the definer and storage alike), so no lower re-share strips the owner's rows");
    assert_eq!(
        definer_rows(&fixture, BEN, &share(BEN, FAY, "reviewer")).await,
        0,
        "share_work: a share_work holder who is no reviewer grants no reviewer"
    );
    assert!(
        definer_rows(&fixture, BEN, &share(BEN, FAY, "contributor")).await > 0,
        "share_work: the same sharer grants what they hold"
    );
    assert_eq!(
        definer_rows(&fixture, REV, &share(REV, FAY, "viewer")).await,
        0,
        "share_work: a reviewer without share authority grants nothing"
    );
    assert!(
        definer_rows(&fixture, OWNER, &share(OWNER, FAY, "reviewer")).await > 0,
        "share_work: the owner grants reviewer"
    );
    assert_eq!(
        definer_rows(&fixture, BEN, &everyone_sql(BEN, "reviewer")).await,
        0,
        "set_work_everyone: no reviewer share from a non-reviewer"
    );
    assert!(
        definer_rows(&fixture, BEN, &everyone_sql(BEN, "contributor")).await > 0,
        "set_work_everyone: a share_work holder shares what they hold"
    );
    assert_eq!(
        definer_rows(&fixture, REV, &everyone_sql(REV, "viewer")).await,
        0,
        "set_work_everyone: no share authority, no share"
    );
    assert!(
        definer_rows(&fixture, OWNER, &everyone_sql(OWNER, "reviewer")).await > 0,
        "set_work_everyone: the owner shares reviewer with everyone"
    );
    check(&mut checks, "D9 in SQL: share_work and set_work_everyone themselves refuse the reviewer role from a sharer who is no reviewer and any share from a sharer without share authority (direct definer calls under the workspace role, rolled back)");
    let owner_rev = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert!(owner_rev
        .granted
        .iter()
        .any(|g| g.permission == "record_review"));
    judged!(&mut checks, "D9 x D7: with the organization-wide reviewer share live, every member sees every claim; revoked, a contributor sees their own again", {
        let n = stores.data.list_records(&nia, PRIVATE_APP, work).await.unwrap();
        ids_in(&n, &w.claims) == sorted(&[OWNER_CLAIM, BEN_CLAIM, REV_CLAIM, ANA_CLAIM])
            && stores.data.read_record(&nia, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.is_ok()
            && {
                // The negative half of the same check keeps it rule-bound.
                let b = stores.data.list_records(&ben, PRIVATE_APP, work).await.unwrap();
                ids_in(&b, &w.claims).len() == 4
            }
            && {
                stores.workspace.revoke_organization_share(&owner, PRIVATE_APP, work).await.unwrap();
                let b = stores.data.list_records(&ben, PRIVATE_APP, work).await.unwrap();
                let back = ids_in(&b, &w.claims) == sorted(&[BEN_CLAIM]);
                stores.workspace.share_work(&owner, PRIVATE_APP, work, Audience::Organization, ShareRole::Reviewer, ShareMode::Set).await.unwrap();
                back
            }
    });
    stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap();

    // --- review m1, m2: the definers judge PERSONAL standing ---------------------
    // Kim holds share_work, discover and run but no record_read: nothing to copy.
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission) VALUES ('{ORG}','{KIM}','{PRIVATE_APP}','{work}','share_work'); COMMIT;"), "genui");
    for permission in ["discover", "run"] {
        grant(&fixture, KIM, permission, &["--app", PRIVATE_APP]);
    }
    let kim = request(&stores, ORG, KIM, 1);
    assert_eq!(
        definer_rows(&fixture, KIM, &share(KIM, FAY, "viewer")).await,
        0,
        "m1: a sharer without record_read confers nothing, not even record_read"
    );
    assert_eq!(
        definer_rows(&fixture, KIM, &everyone_sql(KIM, "viewer")).await,
        0,
        "m1: nor an organization-wide share"
    );
    assert_eq!(
        stores
            .workspace
            .share_work(
                &kim,
                PRIVATE_APP,
                work,
                Audience::Member(FAY),
                ShareRole::Viewer,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    // Kim now reads personally (the owner's viewer share); a live contributor
    // share for everyone gives Kim write standing that is not Kim's to copy.
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(KIM),
            ShareRole::Viewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert!(stores.data.dry_run_command(&kim, &command(&w, "add_note", "note", "00000000-0000-4000-8000-00000000a2a1", 0, json!([{"field_id": "title", "value": {"type": "text", "value": "kim"}}, {"field_id": "state", "value": {"type": "enum", "value": "open"}}]), &"b1".repeat(16))).await.is_ok(), "Kim writes through the organization-wide share");
    assert!(
        definer_rows(&fixture, KIM, &share(KIM, FAY, "viewer")).await > 0,
        "m1: with record_read held, a viewer share copies"
    );
    assert_eq!(
        definer_rows(&fixture, KIM, &share(KIM, FAY, "contributor")).await,
        0,
        "m2: write held only through the organization-wide share is not copied"
    );
    assert_eq!(
        definer_rows(&fixture, KIM, &everyone_sql(KIM, "contributor")).await,
        0,
        "m2: nor re-shared organization-wide"
    );
    assert_eq!(
        stores
            .workspace
            .share_work(
                &kim,
                PRIVATE_APP,
                work,
                Audience::Member(FAY),
                ShareRole::Contributor,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap());
    check(&mut checks, "review m1, m2: share_work and set_work_everyone copy only what the sharer holds personally: a sharer without record_read confers nothing; write standing held only through a live organization-wide share is never copied into a personal grant (direct definer calls and the storage API)");
    // Review n7: a live reviewer share is the owner's (or a reviewer's) to lower or revoke.
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(
        definer_rows(&fixture, BEN, &everyone_sql(BEN, "viewer")).await,
        0,
        "n7: a non-reviewer cannot lower the owner's reviewer share"
    );
    assert_eq!(
        definer_rows(&fixture, BEN, &everyone_sql(BEN, "none")).await,
        0,
        "n7: nor revoke it"
    );
    assert_eq!(
        stores
            .workspace
            .revoke_organization_share(&ben, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap());
    check(&mut checks, "review n7: a share_work holder who is no reviewer can neither lower nor revoke a live organization-wide reviewer share; the owner can");

    // --- review m3: concurrent organization-wide shares both answer ---------------
    {
        let mut first = fixture.operator().await;
        let mut second = fixture.operator().await;
        let t1 = first.client.transaction().await.unwrap();
        t1.batch_execute("SET LOCAL ROLE genui_workspace")
            .await
            .unwrap();
        t1.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &OWNER]).await.unwrap();
        let rows1: i64 = t1
            .query_one(&everyone_sql(OWNER, "contributor"), &[])
            .await
            .unwrap()
            .get(0);
        assert!(rows1 > 0);
        // The second call starts while the first holds its row and lock.
        let second_sql = everyone_sql(OWNER, "viewer");
        let racing = tokio::spawn(async move {
            let t2 = second.client.transaction().await.unwrap();
            t2.batch_execute("SET LOCAL ROLE genui_workspace")
                .await
                .unwrap();
            t2.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &OWNER]).await.unwrap();
            let rows2 = t2
                .query_one(&second_sql, &[])
                .await
                .map(|r| r.get::<_, i64>(0));
            match rows2 {
                Ok(n) => {
                    t2.commit().await.unwrap();
                    Ok(n)
                }
                Err(e) => Err(e.to_string()),
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        assert!(!racing.is_finished(), "the second call waits on the first");
        t1.commit().await.unwrap();
        let rows2 = racing.await.unwrap();
        assert!(
            matches!(rows2, Ok(n) if n > 0),
            "the second call answers a result after the first commits: {rows2:?}"
        );
        assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, 1);
        assert_eq!(text(&fixture, &format!("SELECT role FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, "viewer");
        assert!(stores
            .workspace
            .revoke_organization_share(&owner, PRIVATE_APP, work)
            .await
            .unwrap());
        check(&mut checks, "review m3: two concurrent set_work_everyone calls for one work serialize on the advisory lock; the second waits, then replaces the first's role as one unit, and both answer rows (no unique violation, no 503); one live row remains");
    }

    // --- W2b review MAJOR-1: a queued call reads the committed state ---------------
    // Two sessions, deterministic: the first holds its change open, the second
    // (the storage path) starts, is seen waiting, the first commits, the
    // second's outcome is judged.
    {
        stores
            .workspace
            .share_work(
                &owner,
                PRIVATE_APP,
                work,
                Audience::Organization,
                ShareRole::Viewer,
                ShareMode::Set,
            )
            .await
            .unwrap();
        let mut first = fixture.operator().await;
        let t1 = first.client.transaction().await.unwrap();
        t1.batch_execute("SET LOCAL ROLE genui_workspace")
            .await
            .unwrap();
        t1.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &OWNER]).await.unwrap();
        let rows1: i64 = t1
            .query_one(&everyone_sql(OWNER, "contributor"), &[])
            .await
            .unwrap()
            .get(0);
        assert!(rows1 > 0, "viewer to contributor, left uncommitted");
        let waiter = stores
            .workspace
            .revoke_organization_share(&owner, PRIVATE_APP, work);
        let committer = async {
            let waited = lock_wait(&fixture, "set_work_everyone").await;
            t1.commit().await.unwrap();
            waited
        };
        let (revoked, waited) = tokio::join!(waiter, committer);
        assert!(waited, "the 'none' call waits behind the open change");
        assert_eq!(
            revoked,
            Ok(true),
            "the queued 'none' revokes the contributor share the first call committed"
        );
        assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL")).await, 0, "no organization-wide share stays live");
        check(&mut checks, "W2b review MAJOR-1: a 'none' queued behind an uncommitted viewer-to-contributor change waits on the per-work lock (taken before any read), then revokes the committed contributor share (storage answers true, no live organization-wide row) instead of answering 'nothing to revoke' over a stale read");
    }
    {
        let mut first = fixture.operator().await;
        let t1 = first.client.transaction().await.unwrap();
        t1.batch_execute("SET LOCAL ROLE genui_workspace")
            .await
            .unwrap();
        t1.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &OWNER]).await.unwrap();
        let rows1: i64 = t1
            .query_one(&share(OWNER, FAY, "reviewer"), &[])
            .await
            .unwrap()
            .get(0);
        assert!(
            rows1 > 0,
            "the owner's reviewer share to Fay, left uncommitted"
        );
        let waiter = stores
            .workspace
            .revoke_work_share(&owner, PRIVATE_APP, work, FAY);
        let committer = async {
            let waited = lock_wait(&fixture, "revoke_work_share").await;
            t1.commit().await.unwrap();
            waited
        };
        let (revoked, waited) = tokio::join!(waiter, committer);
        assert!(waited, "the revocation waits behind the open share");
        let revoked = revoked.unwrap();
        assert_eq!(
            revoked.removed,
            ["record_read", "record_review", "record_write"],
            "the queued revocation removes the rows the share committed"
        );
        assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{FAY}' AND work_id='{work}'")).await, 0);
        assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND grantee_id='{FAY}' AND work_id='{work}' AND revoked_at IS NULL")).await, 0);
        check(&mut checks, "W2b review MAJOR-1, the same ordering in share_work and revoke_work_share: an owner's revocation queued behind an uncommitted reviewer share waits on the per-work lock, then removes the three rows that share committed (Fay keeps nothing, her share record marked revoked), instead of answering 'nothing to remove' and leaving the grant live");
    }

    // --- review M1: the owner revokes a member's personal share ------------------
    assert_eq!(
        stores
            .workspace
            .revoke_work_share(&ben, PRIVATE_APP, work, REV)
            .await
            .err(),
        Some(Error::Denied),
        "only the owner revokes"
    );
    assert_eq!(
        stores
            .workspace
            .revoke_work_share(&rev, PRIVATE_APP, work, BEN)
            .await
            .err(),
        Some(Error::Denied),
        "a reviewer does not"
    );
    assert_eq!(
        stores
            .workspace
            .revoke_work_share(&owner, PRIVATE_APP, work, OWNER)
            .await
            .err(),
        Some(Error::Denied),
        "never the owner's own rows"
    );
    let nothing = stores
        .workspace
        .revoke_work_share(&owner, PRIVATE_APP, work, FAY)
        .await
        .unwrap();
    assert!(!nothing.changed());
    let revoked_before = chain(&fixture, "permission.revoked").await;
    // Recheck r3: the revocation takes the grantee's membership row FOR UPDATE
    // (the fence's lock order), so it waits for the grantee's fenced
    // transaction in flight (a session holding the row FOR SHARE, as
    // lock_member_authority does) and lands after it.
    let mut fenced = fixture.operator().await;
    let t1 = fenced.client.transaction().await.unwrap();
    t1.execute(&format!("SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{REV}' FOR SHARE"), &[]).await.unwrap();
    let waiter = stores
        .workspace
        .revoke_work_share(&owner, PRIVATE_APP, work, REV);
    let committer = async {
        let waited = lock_wait(&fixture, "revoke_work_share").await;
        t1.commit().await.unwrap();
        waited
    };
    let (revoked, waited) = tokio::join!(waiter, committer);
    assert!(
        waited,
        "the owner's revocation waits behind the grantee's fenced transaction"
    );
    let revoked = revoked.unwrap();
    assert_eq!(
        revoked.removed,
        ["record_read", "record_review", "record_write"]
    );
    assert_eq!(
        chain(&fixture, "permission.revoked").await,
        revoked_before + 1
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}'")).await, 0, "no row left to revive");
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND grantee_id='{REV}' AND work_id='{work}' AND revoked_at IS NULL")).await, 0, "the share record is marked revoked");
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.app_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND app_id='{PRIVATE_APP}'")).await, 2, "app-level discover and run stay");
    assert_eq!(
        stores
            .data
            .list_records(&rev, PRIVATE_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    let digest_ok = scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events e JOIN genui.work_shares s ON s.organization_id=e.organization_id AND (s.revoke_event_id=e.event_id OR s.audit_event_id=e.event_id) WHERE e.organization_id='{ORG}' AND e.kind='permission.revoked' AND e.work_id='{work}' AND s.grantee_id='{REV}' AND s.removed IS NOT NULL AND e.outcome_digest='sha256:'||encode(sha256(convert_to('forma:share-member-revoke:v1','UTF8')||'\\x00'::bytea||convert_to(s.grantee_id::text,'UTF8')||'\\x00'::bytea||convert_to(array_to_string(s.removed,','),'UTF8')),'hex')")).await;
    assert!(digest_ok >= 1, "the revocation's digest recomputes from stored rows (recheck r2: the grantee and the removed set on the marked record)");
    check(&mut checks, "recheck r3: the owner's revocation takes the grantee's membership row FOR UPDATE before the work lock (the fence's order) and waits behind the grantee's fenced transaction in flight; r2: its permission.revoked digest (forma:share-member-revoke:v1) recomputes from the stored grantee and removed set");
    check(&mut checks, "review M1: the app owner revokes a member's personal share in one unit (record_read, record_review and record_write removed, the share record marked revoked, one permission.revoked whose digest recomputes); non-owners and the owner's own rows are refused; nothing held is an unchanged outcome");
    // The resurrection sequence: a later organization-wide viewer share must
    // not revive the revoked reviewer.
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Organization,
            ShareRole::Viewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    judged!(&mut checks, "review M1: after the owner's revocation a later organization-wide viewer share gives the former reviewer their own rows only and no reviewer-only command", {
        let r = stores.data.list_records(&rev, PRIVATE_APP, work).await.unwrap();
        let approve = command(&w, "approve", "claim", OWNER_CLAIM, 1, json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]), &"d1".repeat(16));
        ids_in(&r, &w.claims) == sorted(&[REV_CLAIM])
            && stores.data.dry_run_command(&rev, &approve).await.err() == Some(Error::Denied)
            && stores.data.read_record(&rev, PRIVATE_APP, work, &w.claims, OWNER_CLAIM).await.is_err()
    });
    assert!(stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap());
    // Rev is a reviewer again for the checks below.
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    // Recheck r5: an automatic or approver share is additive ('ensure'): a
    // reviewer approver stays a reviewer after an automatic contributor
    // share, nothing is written and no permission.revoked is appended; the
    // owner as grantee reads as held. Only the dialog's exact 'set' lowers a
    // role (the f1 demotion below).
    let revoked_before = chain(&fixture, "permission.revoked").await;
    let ensured = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Contributor,
            ShareMode::Ensure,
        )
        .await
        .unwrap();
    assert!(
        ensured.removed.is_empty() && !ensured.changed(),
        "ensure writes nothing a reviewer already holds: {ensured:?}"
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}' AND permission='record_review'")).await, 1, "the reviewer approver stays a reviewer");
    assert_eq!(chain(&fixture, "permission.revoked").await, revoked_before);
    assert!(
        definer_rows(&fixture, OWNER, &ensure(OWNER, REV, "viewer")).await > 0,
        "the definer answers the role's rows under ensure"
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}'")).await, 3);
    assert!(
        definer_rows(&fixture, BEN, &ensure(BEN, OWNER, "viewer")).await > 0,
        "the owner as grantee reads as held under ensure"
    );
    assert_eq!(
        definer_rows(&fixture, BEN, &share(BEN, OWNER, "viewer")).await,
        0,
        "and is refused under set"
    );
    assert_eq!(definer_rows(&fixture, OWNER, &format!("SELECT count(*) FROM genui_private.share_work('{ORG}','{OWNER}','{REV}','{PRIVATE_APP}','{work}','viewer','maybe')")).await, 0, "the mode is a parameter: an unknown one is refused");
    check(&mut checks, "recheck r5: share_work's 'ensure' mode (every automatic or approver path) grants what is missing and never deletes: a reviewer approver keeps record_review after an automatic contributor share, nothing written, no permission.revoked; the owner as grantee reads as held; an unknown mode is refused; only 'set' (the dialog's exact role) lowers a role");
    // Review f1: a lower role re-share strips what the new role lacks, in the
    // same transaction, audited; the sharer must have been able to grant what
    // it strips.
    let revoked_before = chain(&fixture, "permission.revoked").await;
    let demoted = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(demoted.removed, ["record_review"]);
    assert!(demoted.changed());
    assert_eq!(
        chain(&fixture, "permission.revoked").await,
        revoked_before + 1
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}' AND permission='record_review'")).await, 0);
    // Recheck r4 and r2: the demotion (nothing new inserted) leaves exactly
    // one live share record, the new role and the remaining access, carrying
    // the removed set; its event's digest recomputes from that row.
    let live_after_demotion = text(&fixture, &format!("SELECT role||'|'||array_to_string(granted,',')||'|'||coalesce(array_to_string(removed,','),'') FROM genui.work_shares WHERE organization_id='{ORG}' AND grantee_id='{REV}' AND work_id='{work}' AND revoked_at IS NULL")).await;
    assert_eq!(
        live_after_demotion,
        format!("contributor|app:{PRIVATE_APP}:discover,app:{PRIVATE_APP}:run,work:{work}:record_read,work:{work}:record_write|work:{work}:record_review")
    );
    let demotion_digest = scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events e JOIN genui.work_shares s ON s.organization_id=e.organization_id AND s.audit_event_id=e.event_id WHERE e.organization_id='{ORG}' AND e.kind='permission.revoked' AND s.grantee_id='{REV}' AND s.revoked_at IS NULL AND e.outcome_digest='sha256:'||encode(sha256(convert_to('forma:share-member-revoke:v1','UTF8')||'\\x00'::bytea||convert_to(s.grantee_id::text,'UTF8')||'\\x00'::bytea||convert_to(array_to_string(s.removed,','),'UTF8')),'hex')")).await;
    assert_eq!(demotion_digest, 1);
    check(&mut checks, "recheck r4, r2: a demotion that inserts nothing leaves exactly one live share record (the new role, the remaining access, the removed set) and its permission.revoked digest recomputes from that stored row");
    judged!(&mut checks, "review f1: re-shared as contributor, a former reviewer has no reviewer standing left: their own rows only and no reviewer-only command", {
        let r = stores.data.list_records(&rev, PRIVATE_APP, work).await.unwrap();
        let approve = command(&w, "approve", "claim", OWNER_CLAIM, 1, json!([{"field_id": "status", "value": {"type": "enum", "value": "approved"}}]), &"d2".repeat(16));
        ids_in(&r, &w.claims) == sorted(&[REV_CLAIM])
            && stores.data.dry_run_command(&rev, &approve).await.err() == Some(Error::Denied)
            && commands_of(&r, &w.claims) == ["edit", "submit"]
    });
    // Down to viewer strips record_write too; a sharer who could not have
    // granted what a demotion strips is refused (Kim holds share_work and
    // read only).
    let viewer = stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Viewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(viewer.removed, ["record_write"]);
    assert!(
        !stores
            .data
            .list_records(&rev, PRIVATE_APP, work)
            .await
            .unwrap()
            .can_write
    );
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP,
            work,
            Audience::Member(REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(
        definer_rows(&fixture, KIM, &share(KIM, REV, "viewer")).await,
        0,
        "f1: a read-only sharer cannot strip a reviewer's standing by re-sharing lower"
    );
    check(&mut checks, "review f1: share_work with a lower role deletes the rows the new role lacks in the same transaction (reviewer to contributor drops record_review, to viewer also record_write) with one permission.revoked; a sharer who could not have granted the stripped rows is refused");

    // --- review m4: receipts, release, the reviewer-only update on one's own row --
    let owner_version = stores
        .data
        .read_record(&owner, PRIVATE_APP, work, &w.claims, OWNER_CLAIM)
        .await
        .unwrap()
        .version;
    judged!(&mut checks, "receipts: a receipt citing another member's own-collection row is unknown to a contributor, known to a reviewer", {
        let key = ReceiptKey { app: PRIVATE_APP.into(), work: work.into(), collection: w.claims.clone(), record: OWNER_CLAIM.into(), version: owner_version, commit_sequence: owner_claim.commit_sequence, publication: w.publication.clone(), definition_digest: revision.digest.clone() };
        !stores.data.receipts_known(&ben, &[(PRIVATE_APP, work)], std::slice::from_ref(&key)).await.unwrap()
            && stores.data.receipts_known(&rev, &[(PRIVATE_APP, work)], std::slice::from_ref(&key)).await.unwrap()
    });
    BYPASS.store(false, Ordering::SeqCst);
    let shipped_run = stores
        .data
        .run_query(&ben, PRIVATE_APP, work, "count", &json!({}))
        .await
        .unwrap();
    judged!(&mut checks, "query release: a contributor's snapshot (their rows only) is released against the same rule; judged over every row it would be stale", {
        stores.data.release_result(&ben, &shipped_run).await.is_ok()
    });
    let ben_version = stores
        .data
        .read_record(&owner, PRIVATE_APP, work, &w.claims, BEN_CLAIM)
        .await
        .unwrap()
        .version;
    judged!(&mut checks, "D8 on one's own entry over the command path: a contributor's reviewer-only update (grade) of their own claim is refused on the dry run and the commit, nothing written", {
        let grade = |key: &str| command(&w, "grade", "claim", BEN_CLAIM, ben_version, json!([{"field_id": "grade", "value": {"type": "text", "value": "A"}}]), key);
        let before = revisions(&fixture, work).await;
        let dry = stores.data.dry_run_command(&ben, &grade(&"e1".repeat(16))).await;
        let commit = if dry.is_err() {
            matches!(stores.data.commit_record(&ben, &grade(&"e2".repeat(16))).await, Err(Error::Denied))
        } else {
            false
        };
        dry.err() == Some(Error::Denied) && commit && revisions(&fixture, work).await == before
    });
    let graded = committed(
        &stores,
        &rev,
        &command(
            &w,
            "grade",
            "claim",
            BEN_CLAIM,
            ben_version,
            json!([{"field_id": "grade", "value": {"type": "text", "value": "A"}}]),
            &"e3".repeat(16),
        ),
    )
    .await;
    assert_eq!(graded.record_version, ben_version + 1);
    let self_graded = committed(
        &stores,
        &owner,
        &command(
            &w,
            "grade",
            "claim",
            OWNER_CLAIM,
            owner_version,
            json!([{"field_id": "grade", "value": {"type": "text", "value": "self"}}]),
            &"e4".repeat(16),
        ),
    )
    .await;
    assert_eq!(self_graded.record_version, owner_version + 1);
    check(&mut checks, "D8: a reviewer's grade of a contributor's claim commits (the same update a contributor was refused); a reviewer-only update is the owner's way to advance a row they created themselves (a declared transition would refuse its filer)");

    // --- review m4: the source-view sequence, for a member and for a guest --------
    let _s05 = spend_review_s05(&fixture, &stores).await;
    let created = operator(
        &fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            PRIVATE_APP2,
            "--space",
            SPACE,
            "--owner",
            OWNER,
        ],
    );
    assert_eq!(created["granted"], json!(["edit_definition"]));
    for permission in ["discover", "run"] {
        grant(&fixture, OWNER, permission, &["--app", PRIVATE_APP2]);
    }
    grant(&fixture, OWNER, "source_read", &["--source", EXPENSES]);
    let admitted2 = admission::admit_definition(&serde_json::to_vec(&second_definition()).unwrap())
        .expect("the second definition admits");
    let revision2 = stores
        .workspace
        .create_definition_revision(&owner, PRIVATE_APP2, &admitted2)
        .await
        .unwrap();
    let PublishResult::Published(publication2) = stores
        .control
        .publish(&owner, PRIVATE_APP2, &revision2.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale")
    };
    let saved2 = stores
        .workspace
        .create_work(
            &owner,
            PRIVATE_APP2,
            &publication2.publication_id,
            "Spend and claims (synthetic campaign)",
            &[
                WorkBinding::Source {
                    input: "expenses".into(),
                    source: EXPENSES.into(),
                    revision: EXPENSES_REVISION.into(),
                },
                WorkBinding::Records {
                    input: "claims".into(),
                    schema: "claim".into(),
                },
            ],
        )
        .await
        .unwrap();
    let work2 = saved2.work.as_str();
    let w2 = Work { app: PRIVATE_APP2.into(), work: work2.to_owned(), publication: publication2.publication_id.clone(), claims: text(&fixture, &format!("SELECT collection_id::text FROM genui.record_collections WHERE organization_id='{ORG}' AND work_id='{work2}' AND schema_id='claim'")).await, notes: String::new() };
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP2,
            work2,
            Audience::Member(REV),
            ShareRole::Reviewer,
            ShareMode::Set,
        )
        .await
        .unwrap();
    stores
        .workspace
        .share_work(
            &owner,
            PRIVATE_APP2,
            work2,
            Audience::Member(BEN),
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    let ben_claim2 = committed(&stores, &ben, &command(&w2, "submit", "claim", BEN_CLAIM2, 0, json!([{"field_id": "amount", "value": {"type": "integer", "value": "5", "unit": "count"}}]), &"c1".repeat(16))).await;
    let owner_claim2 = committed(&stores, &owner, &command(&w2, "submit", "claim", OWNER_CLAIM2, 0, json!([{"field_id": "amount", "value": {"type": "integer", "value": "9", "unit": "count"}}]), &"c2".repeat(16))).await;
    assert!(owner_claim2.commit_sequence > ben_claim2.commit_sequence);
    judged!(&mut checks, "source-view sequence: a contributor's source view carries the sequence of their own rows, a reviewer's the work's latest", {
        let b = stores.data.run_query(&ben, PRIVATE_APP2, work2, "spend", &json!({})).await.unwrap();
        let r = stores.data.run_query(&rev, PRIVATE_APP2, work2, "spend", &json!({})).await.unwrap();
        b.record_snapshot_sequence() == ben_claim2.commit_sequence && r.record_snapshot_sequence() == owner_claim2.commit_sequence
    });
    // Guests, through live shares the operator stand-in wrote (the product
    // refuses to create them for a work with an own collection).
    let guest = guest_account(&stores, "guest-191", "guest-191@example.invalid").await;
    insert_guest_share(
        &fixture,
        GUEST_SHARE,
        PRIVATE_APP,
        work,
        &w.publication,
        &revision.digest,
        &["count", "claims_list", "notes_list"],
        &[w.claims.as_str(), w.notes.as_str()],
        "guest-191@example.invalid",
    );
    insert_guest_share(
        &fixture,
        GUEST_SHARE2,
        PRIVATE_APP2,
        work2,
        &w2.publication,
        &revision2.digest,
        &["spend"],
        &[w2.claims.as_str()],
        "guest-191@example.invalid",
    );
    let access = stores
        .workspace
        .accept_live_share(&guest, &format!("{ORG}.{GUEST_SHARE}"))
        .await
        .unwrap();
    let access2 = stores
        .workspace
        .accept_live_share(&guest, &format!("{ORG}.{GUEST_SHARE2}"))
        .await
        .unwrap();
    judged!(&mut checks, "guest listing: a guest of a share that names the own collection still sees only the shared notes; the row rule, not the share's collection list, keeps every own row out", {
        let reads = stores.data.guest_work_reads(&access).await.unwrap();
        let listing = reads.records.unwrap().unwrap();
        ids_in(&listing, &w.claims).is_empty() && ids_in(&listing, &w.notes) == all_notes
    });
    judged!(
        &mut checks,
        "guest source-view sequence: a guest's source view carries no own row's sequence",
        {
            let run = stores
                .data
                .guest_run_query(&access2, "spend", &json!({}))
                .await
                .unwrap();
            run.engine_plan().record_snapshot_sequence() == 0
        }
    );
    stores
        .workspace
        .revoke_live_share(&owner, PRIVATE_APP, work, GUEST_SHARE, 1)
        .await
        .ok();
    stores
        .workspace
        .revoke_live_share(&owner, PRIVATE_APP2, work2, GUEST_SHARE2, 1)
        .await
        .ok();

    // --- review m5: owner_only is a real JSON parse of the stored bytes -----------
    {
        let op = fixture.operator().await;
        let revision_id = revision.revision.as_str();
        let digest_ok = format!("sha256:{}", "1".repeat(64));
        let own_utf8 =
            r#"{"fields":[],"id":"probe_utf8","title":"café über","version":1,"visibility":"own"}"#;
        op.client.execute(&format!("INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES ('{ORG}','{PRIVATE_APP}','{revision_id}','probe_utf8',1,$1::text,convert_to($2,'UTF8'))"), &[&digest_ok, &own_utf8]).await.unwrap();
        let derived: bool = op.client.query_one(&format!("SELECT owner_only FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND schema_id='probe_utf8'"), &[]).await.unwrap().get(0);
        assert!(derived, "non-ASCII bytes beside visibility own derive true");
        let escaped = op.client.execute(&format!("INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES ('{ORG}','{PRIVATE_APP}','{revision_id}','probe_escaped',1,$1::text,convert_to($2,'UTF8'))"), &[&digest_ok, &concat!(r#"{"fields":[],"id":"probe_escaped","version":1,"visibility":"\"#, r#"u006fwn"}"#)]).await;
        assert!(escaped.is_ok());
        // The stored bytes carry the backslash escape (recheck N1), so the
        // derivation below is a real decode, not a plain spelling.
        assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND schema_id='probe_escaped' AND position('\\u006f' in convert_from(canonical_schema,'UTF8'))>0")).await, 1);
        let derived: bool = op.client.query_one(&format!("SELECT owner_only FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND schema_id='probe_escaped'"), &[]).await.unwrap().get(0);
        assert!(
            derived,
            "an escaped spelling decodes to own (admission never stores one, the unit test shows)"
        );
        let invalid = op.client.execute(&format!("INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES ('{ORG}','{PRIVATE_APP}','{revision_id}','probe_invalid',1,$1::text,'\\xff7b7d'::bytea)"), &[&digest_ok]).await;
        assert!(invalid.is_err(), "invalid UTF-8 fails closed");
        op.client.execute(&format!("DELETE FROM genui.definition_record_schemas WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}' AND schema_id LIKE 'probe_%'"), &[]).await.unwrap();
        check(&mut checks, "review m5: owner_only is derived by a real JSON parse (genui.canonical_jsonb): non-ASCII bytes derive, an escaped spelling of own reads own, invalid UTF-8 is refused at write time");
    }

    // --- guests ---------------------------------------------------------------
    judged!(
        &mut checks,
        "guests: a live share of a work with an own collection is refused and writes nothing",
        {
            let before = scalar(
                &fixture,
                &format!(
                    "SELECT count(*) FROM genui.live_work_shares WHERE organization_id='{ORG}'"
                ),
            )
            .await;
            let share = NewLiveShare {
                recipient_email: "guest@example.invalid".into(),
                allow_add_to_canvas: false,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-00000000a401".into(),
            };
            let refused = stores
                .workspace
                .create_live_share(&owner, PRIVATE_APP, work, &share)
                .await
                .err()
                == Some(Error::Denied);
            let unwritten = scalar(
                &fixture,
                &format!(
                    "SELECT count(*) FROM genui.live_work_shares WHERE organization_id='{ORG}'"
                ),
            )
            .await
                == before;
            // Under bypass the share is created; remove it so the fixture ends clean.
            if !refused {
                fixture.sql(&format!("DELETE FROM genui.live_work_shares WHERE organization_id='{ORG}' AND app_id='{PRIVATE_APP}'"), "genui");
            }
            refused && unwritten
        }
    );

    // --- APP-191 acceptance: live hints over an own list, 26 pages ------------
    // 25 members (contributors through the owner's share) each commit one
    // claim to the `own` list; 26 pages hold a stream each (the 25 and the
    // owner's, the reviewer page) through the production gate. A non-reviewer
    // page receives the hint for its own row only; the reviewer page has all
    // 25 within 6 s of the last commit. Under bypass every page receives
    // every hint, so the check fails there.
    judged!(&mut checks, "APP-191 acceptance: 25 commits to an own list in under 60 s, 26 pages open: no non-reviewer page receives a hint for another member's row, each receives its own, and the reviewer page is current within 6 s of the last commit", {
        // One counter outside the macro's two expansions of this block: the
        // bypassed round admits and commits as new members and rows.
        let round = LIVE_HINT_ROUND.fetch_add(1, Ordering::SeqCst);
        let students: Vec<String> = (0..25)
            .map(|i| format!("00000000-0000-4000-8000-00000000b{:03x}", round * 32 + i))
            .collect();
        for student in &students {
            admit_member(&fixture, student);
            stores
                .workspace
                .share_work(&owner, PRIVATE_APP, work, Audience::Member(student), ShareRole::Contributor, ShareMode::Set)
                .await
                .unwrap();
        }
        let hub = events::hub();
        let mut pages = Vec::new();
        for (n, principal) in students.iter().chain(std::iter::once(&OWNER.to_owned())).enumerate() {
            let registration = hub
                .register(ORG, principal, &format!("page-{round}-{n}"))
                .unwrap();
            let (tx, rx) = events::sink();
            tokio::spawn(events::pump(
                events::routes::StoreGate::new(stores.clone(), request(&stores, ORG, principal, 1)),
                ORG.to_owned(),
                hub.subscribe(),
                registration,
                tx,
                events::RECHECK,
                events::STREAM_LIFETIME,
            ));
            pages.push((principal.clone(), rx));
        }
        let started = std::time::Instant::now();
        let mut sequences = std::collections::BTreeMap::new();
        for (i, student) in students.iter().enumerate() {
            let who = request(&stores, ORG, student, 1);
            let outcome = committed(
                &stores,
                &who,
                &command(
                    &w,
                    "submit",
                    "claim",
                    &format!("00000000-0000-4000-8000-0000000c{:04x}", round * 64 + i),
                    0,
                    claim_values("7", "filed", &format!("claim {i} of round {round}")),
                    &format!("{:032x}", 0xc0000 + round * 64 + i),
                ),
            )
            .await;
            // The command route's hook, after `CommandResult::Committed`.
            hub.record_committed(ORG, PRIVATE_APP, work, outcome.commit_sequence);
            sequences.insert(student.clone(), outcome.commit_sequence);
        }
        let last_commit = std::time::Instant::now();
        let commits_within = last_commit.duration_since(started);
        // The reviewer page: all 25 hints, judged current within 6 s.
        let (_, reviewer_rx) = pages.last_mut().unwrap();
        let mut reviewer_seen = std::collections::BTreeSet::new();
        let reviewer_current = tokio::time::timeout(std::time::Duration::from_secs(6), async {
            while reviewer_seen.len() < 25 {
                match reviewer_rx.recv().await {
                    Some(event) if event.kind == events::Kind::RecordCommitted => {
                        reviewer_seen.insert(event.sequence.unwrap_or(-1));
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        })
        .await
        .is_ok();
        let reviewer_within = last_commit.elapsed();
        // The other pumps judged the same announcements (one judgment per
        // event, shared); a short grace for their deliveries to land.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut own_only = true;
        for (principal, rx) in pages.iter_mut().take(25) {
            let mut seen = std::collections::BTreeSet::new();
            while let Ok(event) = rx.try_recv() {
                if event.kind == events::Kind::RecordCommitted {
                    seen.insert(event.sequence.unwrap_or(-1));
                }
            }
            let expected: std::collections::BTreeSet<i64> = [sequences[principal]].into_iter().collect();
            if seen != expected {
                own_only = false;
            }
        }
        let all_sequences: std::collections::BTreeSet<i64> = sequences.values().copied().collect();
        println!(
            "APP-191 live hints: 25 commits in {commits_within:?}; reviewer current {reviewer_current} after {reviewer_within:?}; non-reviewer pages own-only {own_only}"
        );
        drop(pages);
        commits_within < std::time::Duration::from_secs(60)
            && reviewer_current
            && reviewer_seen == all_sequences
            && reviewer_within < std::time::Duration::from_secs(6)
            && own_only
    });

    // --- the purge's exception (0024, W3 R1): a guest's share_events row
    // survives deleting its live_work_shares row; linked_apps still holds ---
    {
        let operator = fixture.operator().await;
        let c = &operator.client;
        let account = "00000000-0000-4000-8000-00000000a501";
        let share = "00000000-0000-4000-8000-00000000a502";
        let digest = format!("sha256:{}", "0".repeat(64));
        c.execute("INSERT INTO genui_identity.accounts(account_id,issuer,provider_tenant_id,provider_subject,verified_email) VALUES ($1::text::uuid,'app008-fixture-only','ten_guest','usr_guest','guest@example.invalid')", &[&account]).await.unwrap();
        c.execute(&format!("INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at) VALUES ('{ORG}',$1::text::uuid,'{PRIVATE_APP}','{work}','{OWNER}',1,'guest@example.invalid','{}',$2::text,1,'{{}}','{{}}','schema check',false,1,'00000000-0000-4000-8000-00000000a503',$2::text,now(),now()+interval '7 days')", w.publication), &[&share, &digest]).await.unwrap();
        c.execute(&format!("INSERT INTO genui_identity.linked_apps(account_id,organization_id,share_id,created_at,grant_version) VALUES ($1::text::uuid,'{ORG}',$2::text::uuid,now(),1)"), &[&account, &share]).await.unwrap();
        c.execute(&format!("INSERT INTO genui_identity.share_events(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at) VALUES ($1::text::uuid,'00000000-0000-4000-8000-00000000a504',1,'{ORG}',$2::text::uuid,'share.accepted',1,'\\x7b7d'::bytea,$3::text,$3::text,now())"), &[&account, &share, &digest]).await.unwrap();
        // linked_apps keeps its key: the share cannot go while a linked app refers to it.
        assert!(c.execute(&format!("DELETE FROM genui.live_work_shares WHERE organization_id='{ORG}' AND share_id=$1::text::uuid"), &[&share]).await.is_err());
        c.execute(&format!("DELETE FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND organization_id='{ORG}' AND share_id=$2::text::uuid"), &[&account, &share]).await.unwrap();
        assert_eq!(c.execute(&format!("DELETE FROM genui.live_work_shares WHERE organization_id='{ORG}' AND share_id=$1::text::uuid"), &[&share]).await.unwrap(), 1);
        let survived: i64 = c
            .query_one(
                "SELECT count(*) FROM genui_identity.share_events WHERE account_id=$1::text::uuid",
                &[&account],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(survived, 1);
        check(&mut checks, "0024 purge exception (W3 R1): a guest account's share_events row survives deleting its live_work_shares row (the chain stays verifiable); linked_apps still refuses the delete until the linked app is removed");
    }

    let report = json!({
        "campaign": "APP-191-192-private-entries",
        "passed": true,
        "checks": checks,
        "definition": definition()["content"],
        "actors": {"owner": OWNER, "reviewer": REV, "contributor": BEN, "organization_contributor": ANA, "late_member": FAY, "no_access_then_contributor": NIA},
        "work": work,
        "collections": {"claims": w.claims, "notes": w.notes},
        "bypass": "authorization::BYPASS: own_rows TRUE, hidden empty, own_collections 0, the reviewer_only gate skipped; every check marked `fails under bypass` was run in both modes",
    });
    if let Some(path) = std::env::var_os("GENUI_PRIVATE_ENTRIES_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_campaign_definition_admits_with_an_own_collection_and_a_reviewer_only_command() {
        let admitted = super::admitted();
        let content = crate::admission::decode(&admitted.canonical).unwrap();
        assert_eq!(content["record_schemas"][0]["visibility"], "own");
        assert_eq!(content["record_schemas"][1].get("visibility"), None);
        for index in [2, 3, 5] {
            assert_eq!(content["commands"][index]["reviewer_only"], true, "{index}");
        }
        for index in [0, 1, 4] {
            assert_eq!(
                content["commands"][index].get("reviewer_only"),
                None,
                "{index}"
            );
        }
    }
}
