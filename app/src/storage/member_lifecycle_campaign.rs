//! Member lifecycle campaign (CLASS-CONTINUATION-PLAN §2.2 and §2.3; APP-229,
//! APP-230; migration 0026). Isolated test executable only: local Docker,
//! OpenSSL and the locked Python environments, as the other storage campaigns.
//!
//! Two parts, every item asserted on the database or on storage's own paths:
//!
//! 1. **`extend-entitlement`.** A member sponsors a live share a guest has
//!    accepted; their entitlement lapses (active, `expires_at` in the past):
//!    the guest's access is refused and the sign-in predicate (the exact
//!    membership/entitlement clause of `sessions.rs`) answers nobody. The
//!    operator extends the whole organization in one transaction: the guest's
//!    access answers again, sign-in would admit the member again, one
//!    `entitlement.extended` event per changed member sits on the operator
//!    chain with a recomputable digest. An earlier `--until` changes nothing
//!    and appends nothing (never lowered); a past `--until` refuses before any
//!    write.
//! 2. **`erase-member`.** A member with an app, a work, an entry of their
//!    own, an entry in another member's app, a folder with an upload, a
//!    session, an agent token with an attribution and a log row, a Slack
//!    link, a card, a share and a name is removed and erased: their own app,
//!    folder, upload, session, attribution, link, card and share are gone;
//!    their token is revoked and relabelled; the other member's app and the
//!    entry they made in it still read, authored by a principal with no name;
//!    the principal's provider subject is a tombstone; one `member.erased`
//!    event; a repeat changes nothing; the last active owner refuses until
//!    another owner exists; every chain verifies.
use super::{
    admission_qualification::{operator, operator_as, refused_for},
    definition_qualification::request,
    qualification::Fixture,
    qualification_cases::{check, APP, ORG, PRINCIPAL, WORK},
    shared_campaign::account,
    *,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// tenant_seed's space and owner (the organization's owner since 0025).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
const SEEDED_OWNER: &str = "00000000-0000-4000-8000-000000000f00";
/// The member erased, the member whose app keeps the erased member's entry,
/// and the owner the last-owner rule protects.
const ERASED: &str = "00000000-0000-4000-8000-000000000d01";
const OTHER_OWNER: &str = "00000000-0000-4000-8000-000000000d02";
const SOLE_OWNER: &str = "00000000-0000-4000-8000-000000000d03";
const MY_APP: &str = "00000000-0000-4000-8000-000000000d10";
const MY_WORK: &str = "00000000-0000-4000-8000-000000000d11";
const THEIR_APP: &str = "00000000-0000-4000-8000-000000000d20";
const THEIR_WORK: &str = "00000000-0000-4000-8000-000000000d21";
const BOX: &str = "00000000-0000-4000-8000-000000000d30";
const FILE: &str = "00000000-0000-4000-8000-000000000d31";
const TOKEN: &str = "00000000-0000-4000-8000-000000000d40";

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_member_lifecycle_campaign() {
    std::thread::Builder::new()
        .name("app229-member-lifecycle-campaign".into())
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

/// Operator-channel stand-in (as tenant_seed writes fixture authority): one
/// active member with a live entitlement and a role.
fn admit_member(fixture: &Fixture, principal: &str, role: &str) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch,role) VALUES ('{ORG}','{principal}','active',1,'{role}'); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
}
async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn text(fixture: &Fixture, sql: &str) -> String {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// The exact clause sign-in applies to a member (`sessions.rs`, complete_admission): active
/// membership, active organization, active entitlement that has not expired.
async fn sign_in_admits(fixture: &Fixture, principal: &str) -> bool {
    scalar(fixture, &format!("SELECT count(*) FROM genui.principals p JOIN genui.memberships m ON m.organization_id=p.organization_id AND m.principal_id=p.principal_id JOIN genui.organizations o ON o.organization_id=p.organization_id JOIN genui.entitlements e ON e.organization_id=p.organization_id AND e.principal_id=p.principal_id WHERE p.organization_id='{ORG}' AND p.principal_id='{principal}' AND m.status='active' AND o.status='active' AND e.status='active' AND e.expires_at>clock_timestamp()")).await > 0
}
async fn operator_kind_count(fixture: &Fixture, kind: &str) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='operator' AND kind='{kind}'")).await
}
async fn last_operator_event(fixture: &Fixture) -> (String, String, String) {
    let operator = fixture.operator().await;
    let row = operator.client.query_one(&format!("SELECT kind,actor_principal_id::text,outcome_digest::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='operator' ORDER BY chain_sequence DESC LIMIT 1"), &[]).await.unwrap();
    (row.get(0), row.get(1), row.get(2))
}
fn digest(domain: &[u8], parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update([0u8]);
        hash.update(part.as_bytes());
    }
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
/// `now() + days`, RFC 3339 UTC to the second, from the database's own clock (the one the definer judges by).
async fn until(fixture: &Fixture, days: i64) -> String {
    text(fixture, &format!("SELECT to_char((now()+make_interval(days=>{days})) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')")).await
}
fn entry_for<'a>(out: &'a Value, principal: &str) -> &'a Value {
    out["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["principal_id"] == principal)
        .unwrap_or_else(|| panic!("no entry for {principal}"))
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

    // --- 1. extend-entitlement -----------------------------------------------------
    // The seeded author sponsors a live share of the seeded work; a guest accepts it.
    fixture.sql(&format!("INSERT INTO genui.work_permissions VALUES('{ORG}','{PRINCIPAL}','{APP}','{WORK}','share_work') ON CONFLICT DO NOTHING"), "genui");
    let sponsor = request(&stores, ORG, PRINCIPAL, 1);
    let share = stores
        .workspace
        .create_live_share(
            &sponsor,
            APP,
            WORK,
            &NewLiveShare {
                recipient_email: "guest@example.test".into(),
                allow_add_to_canvas: false,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-00000000d100".into(),
            },
        )
        .await
        .unwrap();
    let (_, guest) = account(&stores, "lifecycle-guest", "guest@example.test").await;
    stores
        .workspace
        .accept_live_share(&guest, &share.key)
        .await
        .unwrap();
    assert!(stores
        .workspace
        .resolve_guest_access(&guest, &share.key)
        .await
        .is_ok());
    assert!(sign_in_admits(&fixture, PRINCIPAL).await);
    // The lapse: active, with an expiry in the past (nothing in the schema sets `expired`).
    fixture.sql(&format!("UPDATE genui.entitlements SET starts_at=now()-interval '31 days',expires_at=now()-interval '1 hour' WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' AND status='active'"), "genui");
    assert_eq!(
        text(&fixture, &format!("SELECT string_agg(DISTINCT status,',') FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'")).await,
        "active"
    );
    assert!(
        stores
            .workspace
            .resolve_guest_access(&guest, &share.key)
            .await
            .is_err(),
        "a lapsed sponsor's guest share is refused"
    );
    assert!(!sign_in_admits(&fixture, PRINCIPAL).await);
    check(
        &mut checks,
        "a lapsed entitlement is an active row with a past expires_at: the sponsor's guest share is refused and the sign-in clause admits nobody",
    );

    let events_before = operator_kind_count(&fixture, "entitlement.extended").await;
    let active_members = scalar(&fixture, &format!("SELECT count(*) FROM genui.memberships WHERE organization_id='{ORG}' AND status='active'")).await;
    let target = until(&fixture, 90).await;
    let out = operator(
        &fixture,
        &[
            "extend-entitlement",
            "--organization",
            ORG,
            "--all-active",
            "--until",
            &target,
        ],
    );
    assert_eq!(out["until"], target);
    assert_eq!(out["members"].as_i64().unwrap(), active_members);
    let sponsor_entry = entry_for(&out, PRINCIPAL);
    assert_eq!(sponsor_entry["changed"], true);
    assert_eq!(sponsor_entry["expires_at"], target);
    let changed = out["changed"].as_i64().unwrap();
    assert!(changed >= 1);
    assert_eq!(
        operator_kind_count(&fixture, "entitlement.extended").await,
        events_before + changed,
        "one event per changed member"
    );
    assert_eq!(
        text(&fixture, &format!("SELECT outcome_digest::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='operator' AND kind='entitlement.extended' AND actor_principal_id='{PRINCIPAL}' ORDER BY chain_sequence DESC LIMIT 1")).await,
        digest(b"forma:entitlement-extended:v1", &[PRINCIPAL, &target])
    );
    assert_eq!(
        text(&fixture, &format!("SELECT to_char(max(expires_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' AND status='active'")).await,
        target
    );
    assert!(
        stores
            .workspace
            .resolve_guest_access(&guest, &share.key)
            .await
            .is_ok(),
        "the revived sponsor's guest share answers again"
    );
    assert!(sign_in_admits(&fixture, PRINCIPAL).await);
    assert_eq!(
        scalar(&fixture, &format!("SELECT membership_epoch FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'")).await,
        1,
        "no epoch advances: an extension revokes nothing"
    );
    check(
        &mut checks,
        "extend-entitlement --all-active extends every active member in one transaction: the lapsed sponsor comes back (the guest share answers, sign-in admits), one entitlement.extended event per changed member with a recomputable digest, no epoch moved",
    );

    // Never lowered: an earlier date changes nothing and appends nothing; a past date refuses before any write.
    let events = operator_kind_count(&fixture, "entitlement.extended").await;
    let lower = until(&fixture, 30).await;
    let out = operator(
        &fixture,
        &[
            "extend-entitlement",
            "--organization",
            ORG,
            "--principal",
            PRINCIPAL,
            "--until",
            &lower,
        ],
    );
    assert_eq!(out["changed"], 0);
    assert_eq!(entry_for(&out, PRINCIPAL)["expires_at"], target);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "extend-entitlement",
                "--organization",
                ORG,
                "--principal",
                PRINCIPAL,
                "--until",
                &until(&fixture, -1).await,
            ],
        ),
        "until_in_past",
    );
    assert_eq!(
        operator_kind_count(&fixture, "entitlement.extended").await,
        events
    );
    assert_eq!(
        text(&fixture, &format!("SELECT to_char(max(expires_at) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' AND status='active'")).await,
        target
    );
    check(
        &mut checks,
        "never lowered: an earlier --until is changed: false with no event, a past --until refuses until_in_past before any write, and the later date stands",
    );

    // --- 2. erase-member ------------------------------------------------------------
    for (principal, role) in [(ERASED, "member"), (OTHER_OWNER, "member")] {
        admit_member(&fixture, principal, role);
    }
    let digest64 = format!("sha256:{}", "e".repeat(64));
    fixture.sql(&format!(r#"BEGIN;
INSERT INTO genui.organization_sequences VALUES ('{ORG}',1000) ON CONFLICT (organization_id) DO UPDATE SET next_commit_sequence=greatest(genui.organization_sequences.next_commit_sequence,1000);
INSERT INTO genui.publication_policies VALUES ('{ORG}','00000000-0000-4000-8000-000000000d50',1,'owner_publish');
INSERT INTO genui.apps VALUES ('{ORG}','{MY_APP}','{SPACE}','{ERASED}'),('{ORG}','{THEIR_APP}','{SPACE}','{OTHER_OWNER}');
INSERT INTO genui.definition_revisions VALUES ('{ORG}','{MY_APP}','00000000-0000-4000-8000-000000000d12',1,'{digest64}','\x7b7d'),('{ORG}','{THEIR_APP}','00000000-0000-4000-8000-000000000d22',1,'{digest64}','\x7b7d');
INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES ('{ORG}','{MY_APP}','00000000-0000-4000-8000-000000000d12','entries',1,'{digest64}','\x7b7d'),('{ORG}','{THEIR_APP}','00000000-0000-4000-8000-000000000d22','entries',1,'{digest64}','\x7b7d');
INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope,publication_sequence)
 VALUES ('{ORG}','{MY_APP}','00000000-0000-4000-8000-000000000d13','00000000-0000-4000-8000-000000000d12','{digest64}','00000000-0000-4000-8000-000000000d50',1,'owner_publish','{ERASED}','{ERASED}',1,1,1,'{digest64}',now(),'\x7b7d',901),
        ('{ORG}','{THEIR_APP}','00000000-0000-4000-8000-000000000d23','00000000-0000-4000-8000-000000000d22','{digest64}','00000000-0000-4000-8000-000000000d50',1,'owner_publish','{OTHER_OWNER}','{OTHER_OWNER}',1,1,1,'{digest64}',now(),'\x7b7d',902);
INSERT INTO genui.publication_record_schemas VALUES ('{ORG}','{MY_APP}','00000000-0000-4000-8000-000000000d13','00000000-0000-4000-8000-000000000d12','entries',1,'{digest64}'),('{ORG}','{THEIR_APP}','00000000-0000-4000-8000-000000000d23','00000000-0000-4000-8000-000000000d22','entries',1,'{digest64}');
INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES ('{ORG}','{MY_APP}','{MY_WORK}','00000000-0000-4000-8000-000000000d13',1,'','{ERASED}'),('{ORG}','{THEIR_APP}','{THEIR_WORK}','00000000-0000-4000-8000-000000000d23',1,'','{OTHER_OWNER}');
INSERT INTO genui.record_collections VALUES ('{ORG}','{MY_APP}','{MY_WORK}','00000000-0000-4000-8000-000000000d13','00000000-0000-4000-8000-000000000d14','entries',1),('{ORG}','{THEIR_APP}','{THEIR_WORK}','00000000-0000-4000-8000-000000000d23','00000000-0000-4000-8000-000000000d24','entries',1);
INSERT INTO genui.internal_records VALUES ('{ORG}','{MY_APP}','{MY_WORK}','00000000-0000-4000-8000-000000000d14','00000000-0000-4000-8000-000000000d15',1),('{ORG}','{THEIR_APP}','{THEIR_WORK}','00000000-0000-4000-8000-000000000d24','00000000-0000-4000-8000-000000000d25',1);
INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values)
 VALUES ('{ORG}','{MY_APP}','{MY_WORK}','00000000-0000-4000-8000-000000000d14','00000000-0000-4000-8000-000000000d15',1,'{ERASED}','{ERASED}',901,now(),'\x7b7d'),
        ('{ORG}','{THEIR_APP}','{THEIR_WORK}','00000000-0000-4000-8000-000000000d24','00000000-0000-4000-8000-000000000d25',1,'{ERASED}','{ERASED}',902,now(),'\x7b7d');
INSERT INTO genui.app_permissions VALUES ('{ORG}','{ERASED}','{MY_APP}','edit_definition'),('{ORG}','{OTHER_OWNER}','{THEIR_APP}','edit_definition'),('{ORG}','{ERASED}','{THEIR_APP}','discover');
INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at,role) VALUES ('{ORG}','00000000-0000-4000-8000-000000000d60','{ERASED}','{OTHER_OWNER}','{MY_APP}','{MY_WORK}','{{record_read}}','00000000-0000-4000-8000-000000000d61',now(),'viewer');
INSERT INTO genui.boxes VALUES ('{ORG}','{BOX}','{ERASED}','Uploads','uploads',NULL,now());
INSERT INTO genui.box_files VALUES ('{ORG}','{BOX}','{FILE}','notes.csv','csv','text/csv',12,'sha256:{cd}','o/{ORG}/b/{BOX}/f/{FILE}','{ERASED}',now());
INSERT INTO genui.sessions(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,refresh_token_ref,created_at,last_seen_at,absolute_expires_at)
 VALUES ('{ORG}','00000000-0000-4000-8000-000000000d70','sha256:{ab}','{ERASED}',1,'0000e000-0000-4000-8000-000000000d01','{zeros}','00000000-0000-4000-8000-000000000d71',now(),now(),now()+interval '1 hour');
INSERT INTO genui.agent_tokens(organization_id,token_id,token_digest,principal_id,label,membership_epoch_at_mint,registry_sha256,created_at,expires_at) VALUES ('{ORG}','{TOKEN}','sha256:{ef}','{ERASED}','my laptop',1,'{zeros}',now(),now()+interval '30 days');
INSERT INTO genui.agent_attributions(organization_id,attribution_id,token_id,app_id,work_id,created_at) VALUES ('{ORG}','00000000-0000-4000-8000-000000000d41','{TOKEN}','{THEIR_APP}','{THEIR_WORK}',now());
INSERT INTO genui.agent_access_log(organization_id,entry_id,chain_sequence,token_id,principal_id,agent_label,tool,touched_apps,touched_works,touched_records,touched_folders,touched_files,outcome,reason,space,resource_label,duration_ms,occurred_at,previous_hash,hash)
 VALUES ('{ORG}','00000000-0000-4000-8000-000000000d42',1,'{TOKEN}','{ERASED}','my laptop','forma_list_apps','{{}}','{{}}','{{}}','{{}}','{{}}','allowed',NULL,NULL,NULL,1,now(),'sha256:{zeros}','sha256:{ab}');
INSERT INTO genui.slack_links(organization_id,link_id,principal_id,team_id,slack_user_id,membership_epoch_at_link,linked_at) VALUES ('{ORG}','00000000-0000-4000-8000-000000000d80','{ERASED}','T1','U1',1,now());
INSERT INTO genui.member_cards(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,everyone,created_at,updated_at) VALUES ('{ORG}','00000000-0000-4000-8000-000000000d90','{ERASED}','card','{THEIR_APP}','{THEIR_WORK}','cards','','A card','\x7b7d',true,now(),now());
UPDATE genui.memberships SET display_name='Erin' WHERE organization_id='{ORG}' AND principal_id='{ERASED}';
COMMIT;"#, cd = "cd".repeat(32), ab = "ab".repeat(32), ef = "ef".repeat(32), zeros = "0".repeat(64)), "genui");
    let footprint = |table: &'static str, column: &'static str, value: &'static str| {
        let fixture = &fixture;
        async move {
            scalar(fixture, &format!("SELECT count(*) FROM genui.{table} WHERE organization_id='{ORG}' AND {column}='{value}'")).await
        }
    };
    assert_eq!(footprint("apps", "owner_principal_id", ERASED).await, 1);
    assert_eq!(footprint("sessions", "principal_id", ERASED).await, 1);
    let theirs_before = footprint("internal_record_revisions", "app_id", THEIR_APP).await;
    let events_before = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'"),
    )
    .await;

    // An active member refuses; the procedure is remove, then erase.
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "erase-member",
                "--organization",
                ORG,
                "--principal",
                ERASED,
                "--confirm",
                ERASED,
            ],
        ),
        "member_active",
    );
    assert_eq!(footprint("apps", "owner_principal_id", ERASED).await, 1);
    let revoked = operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            ERASED,
        ],
    );
    assert_eq!(revoked["changed"], true);
    let out = operator(
        &fixture,
        &[
            "erase-member",
            "--organization",
            ORG,
            "--principal",
            ERASED,
            "--confirm",
            ERASED,
        ],
    );
    assert_eq!(out["status"], "erased");
    assert_eq!(out["changed"], true);
    assert_eq!(out["apps"], 1);
    assert_eq!(out["boxes"], 1);
    assert_eq!(out["provider_subject"], format!("usr_{ERASED}"));
    assert_eq!(
        out["object_keys"],
        json!([format!("o/{ORG}/b/{BOX}/f/{FILE}")])
    );
    for (table, column, value) in [
        ("apps", "owner_principal_id", ERASED),
        ("apps", "app_id", MY_APP),
        ("saved_work", "app_id", MY_APP),
        ("internal_record_revisions", "app_id", MY_APP),
        ("work_shares", "sharer_id", ERASED),
        ("boxes", "box_id", BOX),
        ("box_files", "uploaded_by", ERASED),
        ("sessions", "principal_id", ERASED),
        ("agent_attributions", "token_id", TOKEN),
        ("slack_links", "principal_id", ERASED),
        ("member_cards", "principal_id", ERASED),
        ("app_permissions", "principal_id", ERASED),
    ] {
        assert_eq!(
            footprint(table, column, value).await,
            0,
            "{table}.{column} still names the erased member's world"
        );
    }
    assert_eq!(
        text(&fixture, &format!("SELECT label||':'||(revoked_at IS NOT NULL)::text FROM genui.agent_tokens WHERE organization_id='{ORG}' AND token_id='{TOKEN}'")).await,
        "erased:true"
    );
    assert_eq!(footprint("agent_access_log", "token_id", TOKEN).await, 1);
    assert_eq!(
        text(&fixture, &format!("SELECT status||':'||coalesce(display_name,'<none>')||':'||role FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{ERASED}'")).await,
        "revoked:<none>:member"
    );
    assert_eq!(
        text(&fixture, &format!("SELECT provider_subject FROM genui.principals WHERE organization_id='{ORG}' AND principal_id='{ERASED}'")).await,
        format!("erased:{ERASED}")
    );
    assert_eq!(
        text(&fixture, &format!("SELECT string_agg(DISTINCT status,',') FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{ERASED}'")).await,
        "revoked"
    );
    // The other member's app and the entry the erased member made in it still read, nameless.
    assert_eq!(footprint("apps", "app_id", THEIR_APP).await, 1);
    assert_eq!(
        footprint("internal_record_revisions", "app_id", THEIR_APP).await,
        theirs_before
    );
    assert_eq!(
        text(&fixture, &format!("SELECT r.creator_id::text||':'||coalesce(m.display_name,'<none>') FROM genui.internal_record_revisions r JOIN genui.memberships m ON m.organization_id=r.organization_id AND m.principal_id=r.creator_id WHERE r.organization_id='{ORG}' AND r.app_id='{THEIR_APP}'")).await,
        format!("{ERASED}:<none>")
    );
    let (kind, actor, outcome) = last_operator_event(&fixture).await;
    assert_eq!(
        (kind.as_str(), actor.as_str(), outcome),
        (
            "member.erased",
            ERASED,
            digest(b"forma:member-erased:v1", &[ERASED])
        )
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'")
        )
        .await,
        events_before + 2,
        "membership.revoked and member.erased, nothing else"
    );
    check(
        &mut checks,
        "erase-member after revoke-membership: the member's own app, work, entry, share, folder, upload, session, agent attribution, Slack link, card and grants are gone; the agent token is revoked and relabelled, its log row stays; the membership is revoked, nameless, a member; the provider subject is a tombstone; the entitlement is revoked; the other member's app and the entry the erased member made in it still read, authored by a principal with no name; exactly one member.erased event with a recomputable digest",
    );

    // A repeat is harmless; an active member and a wrong confirmation refuse.
    let events = scalar(
        &fixture,
        &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'"),
    )
    .await;
    let again = operator(
        &fixture,
        &[
            "erase-member",
            "--organization",
            ORG,
            "--principal",
            ERASED,
            "--confirm",
            ERASED,
        ],
    );
    assert_eq!(again["changed"], false);
    assert_eq!(again["rows_deleted"], 0);
    assert!(again["provider_subject"].is_null());
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "erase-member",
                "--organization",
                ORG,
                "--principal",
                OTHER_OWNER,
                "--confirm",
                ERASED,
            ],
        ),
        "confirm_mismatch",
    );
    assert_eq!(
        scalar(
            &fixture,
            &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'")
        )
        .await,
        events
    );
    check(
        &mut checks,
        "a repeat finds nothing to do and appends nothing; a confirmation that is not the principal id refuses before any connection",
    );

    // The last owner: with the seeded owner demoted, the sole revoked owner refuses until another owner exists.
    admit_member(&fixture, SOLE_OWNER, "owner");
    fixture.sql(&format!("UPDATE genui.memberships SET role='admin' WHERE organization_id='{ORG}' AND principal_id='{SEEDED_OWNER}'"), "genui");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.memberships WHERE organization_id='{ORG}' AND role='owner' AND status='active'")).await,
        1
    );
    operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            SOLE_OWNER,
        ],
    );
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "erase-member",
                "--organization",
                ORG,
                "--principal",
                SOLE_OWNER,
                "--confirm",
                SOLE_OWNER,
            ],
        ),
        "last_owner",
    );
    assert_eq!(
        text(&fixture, &format!("SELECT role||':'||provider_subject FROM genui.memberships m JOIN genui.principals p USING (organization_id,principal_id) WHERE organization_id='{ORG}' AND principal_id='{SOLE_OWNER}'")).await,
        format!("owner:usr_{SOLE_OWNER}")
    );
    fixture.sql(&format!("UPDATE genui.memberships SET role='owner' WHERE organization_id='{ORG}' AND principal_id='{SEEDED_OWNER}'"), "genui");
    let erased_owner = operator(
        &fixture,
        &[
            "erase-member",
            "--organization",
            ORG,
            "--principal",
            SOLE_OWNER,
            "--confirm",
            SOLE_OWNER,
        ],
    );
    assert_eq!(erased_owner["changed"], true);
    assert_eq!(
        text(&fixture, &format!("SELECT role||':'||provider_subject FROM genui.memberships m JOIN genui.principals p USING (organization_id,principal_id) WHERE organization_id='{ORG}' AND principal_id='{SOLE_OWNER}'")).await,
        format!("member:erased:{SOLE_OWNER}")
    );
    check(
        &mut checks,
        "the last active owner refuses last_owner (nothing written) until another owner exists; then the erasure goes through",
    );

    // --- the chains verify ---------------------------------------------------------
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    check(
        &mut checks,
        "every chain of the organization, with the entitlement.extended and member.erased events in the operator chain, recomputes from genesis to its head",
    );

    stores.close().await.unwrap();
    if let Some(path) = std::env::var_os("GENUI_MEMBER_LIFECYCLE_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"Membership lifecycle (CLASS-CONTINUATION-PLAN §2.2/§2.3; APP-229, APP-230; migration 0026): the never-lowering entitlement extension that revives a lapse, and the per-member erasure that keeps the principal and the entries made in others' apps nameless","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Sign-in itself is not driven through the hosted provider: the campaign applies the exact membership/entitlement clause of sessions.rs and proves the sponsored guest share through storage's own path.","The S3 sweeps of the erased member's objects are ops/promotion/admission.py's, proven against a fake AWS in ops/promotion/test_admission.py.","The 25 -> 26 migration and the refusals below it are app/runtime/test_admission_operator.py's LifecycleMigrationTests."]})).unwrap()).unwrap();
    }
    println!("member-lifecycle campaign: {} checks passed", checks.len());
}
