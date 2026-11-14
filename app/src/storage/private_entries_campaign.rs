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
    definition_qualification::{grant, request, POLICY},
    qualification::Fixture,
    qualification_cases::{check, ORG},
    *,
};
use crate::admission::{self, digest};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const PRIVATE_APP: &str = "00000000-0000-4000-8000-000000000a10";
const OWNER: &str = "00000000-0000-4000-8000-000000000a01";
const REV: &str = "00000000-0000-4000-8000-000000000a02";
const BEN: &str = "00000000-0000-4000-8000-000000000a03";
const ANA: &str = "00000000-0000-4000-8000-000000000a04";
const NIA: &str = "00000000-0000-4000-8000-000000000a05";
const FAY: &str = "00000000-0000-4000-8000-000000000a06";
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
        {"id": "note", "value_type": {"type": "text"}, "nullable": true}
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
        app: PRIVATE_APP.into(),
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
    match stores.data.commit_record(who, request).await.unwrap() {
        CommandResult::Committed(outcome) => outcome,
        other => panic!("{} must commit: {other:?}", request.command),
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
    let stores = Stores::new(fixture.settings()).unwrap();
    stores.readiness().await.unwrap();
    let mut checks = Vec::new();
    for member in [OWNER, REV, BEN, ANA, NIA] {
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
    assert_eq!(commands_of(&r, &w.claims), ["approve", "edit", "submit"]);
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
                    .share_work(who, PRIVATE_APP, work, Audience::Member(NIA), role)
                    .await
                    .err(),
                Some(Error::Denied),
                "{label}"
            );
            assert_eq!(
                stores
                    .workspace
                    .share_work(who, PRIVATE_APP, work, Audience::Organization, role)
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
                ShareRole::Reviewer
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
                ShareRole::Reviewer
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
        format!("SELECT count(*) FROM genui_private.share_work('{ORG}','{sharer}','{grantee}','{PRIVATE_APP}','{work}','{role}')")
    };
    let everyone_sql = |sharer: &str, role: &str| {
        format!("SELECT count(*) FROM genui_private.set_work_everyone('{ORG}','{sharer}','{PRIVATE_APP}','{work}','{role}')")
    };
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
                stores.workspace.share_work(&owner, PRIVATE_APP, work, Audience::Organization, ShareRole::Reviewer).await.unwrap();
                back
            }
    });
    stores
        .workspace
        .revoke_organization_share(&owner, PRIVATE_APP, work)
        .await
        .unwrap();

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
        assert_eq!(content["commands"][2]["reviewer_only"], true);
        assert_eq!(content["commands"][4]["reviewer_only"], true);
        assert_eq!(content["commands"][0].get("reviewer_only"), None);
        assert_eq!(content["commands"][3].get("reviewer_only"), None);
    }
}
