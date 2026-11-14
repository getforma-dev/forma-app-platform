//! Organization admin campaign (trial plan D4, D5, D6; migration 0023).
//! Isolated test executable only: local Docker, OpenSSL and the locked
//! Python environments, as the APP-073 admission campaign. Every acceptance
//! item is asserted on the database (membership rows, consents, the
//! admission chain) or on the storage module's typed answers, never on a
//! label alone; the admission chain is recomputed by the operator's `verify`.
//!
//! Members: the seeded admin `ADMIN` (tenant_seed's `admin_policy` holder in
//! both organizations), the seeded `PRINCIPAL` (owns the fixture app), a
//! member admitted here through the storage admission path with the beta
//! notice version (`NEWCOMER`), the seeded revoked member, and `OUTSIDER`, a
//! member of the canary organization only.
use super::{
    admission::{code_digest, Admission, PendingAdmission, Reservation, VerifiedNonmember},
    admission_qualification::operator,
    definition_qualification::request,
    qualification::Fixture,
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL},
    *,
};
use crate::auth::notice::NOTICE_VERSION;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// tenant_seed's `admin_policy` holder, active at epoch 1 in both organizations.
const ADMIN: &str = "00000000-0000-4000-8000-000000000f00";
/// A seeded active member with no permissions of its own (tenant_seed).
const PEER: &str = "00000000-0000-4000-8000-00000000006f";
/// The seeded revoked member (status revoked, epoch 2).
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
/// A member of the canary organization only, created by this campaign.
const OUTSIDER: &str = "00000000-0000-4000-8000-000000000ab0";
/// Well formed, never a member anywhere.
const NOBODY: &str = "00000000-0000-4000-8000-000000000ab1";
const ISSUER: &str = "app008-fixture-only";

fn sha(label: &str) -> String {
    let hash = Sha256::digest(label.as_bytes());
    format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
fn domain_digest(domain: &str, value: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0u8]);
    hash.update(value.as_bytes());
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_org_admin_campaign() {
    std::thread::Builder::new()
        .name("trial-org-admin-campaign".into())
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

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn rows(fixture: &Fixture, sql: &str) -> Vec<tokio_postgres::Row> {
    let operator = fixture.operator().await;
    operator.client.query(sql, &[]).await.unwrap()
}
/// What the admin operations may write: membership statuses and epochs,
/// entitlement statuses, consents, and the admission chain's length.
async fn footprint(fixture: &Fixture, organization: &str) -> Value {
    let members = rows(fixture, &format!("SELECT principal_id::text,status,membership_epoch,display_name FROM genui.memberships WHERE organization_id='{organization}' ORDER BY principal_id")).await;
    json!({
        "members": members.iter().map(|r| json!([r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, i64>(2), r.get::<_, Option<String>>(3)])).collect::<Vec<_>>(),
        "active_entitlements": scalar(fixture, &format!("SELECT count(*) FROM genui.entitlements WHERE organization_id='{organization}' AND status='active'")).await,
        "consents": scalar(fixture, &format!("SELECT count(*) FROM genui.member_consents WHERE organization_id='{organization}'")).await,
        "admission_events": scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{organization}' AND writer_role='admission'")).await,
    })
}
/// The admission chain's events of one kind in an organization, newest last:
/// `(actor, outcome_digest, code_id is null)`.
async fn events(fixture: &Fixture, organization: &str, kind: &str) -> Vec<(String, Option<String>, bool)> {
    rows(fixture, &format!("SELECT actor_principal_id::text,outcome_digest::text,code_id IS NULL FROM genui.audit_events WHERE organization_id='{organization}' AND writer_role='admission' AND kind='{kind}' ORDER BY chain_sequence"))
        .await
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect()
}
/// The superuser's own counts for one member, in `MemberRow` order:
/// apps, works, records, uploads.
async fn counted(fixture: &Fixture, organization: &str, principal: &str) -> [i64; 4] {
    [
        scalar(fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{organization}' AND owner_principal_id='{principal}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.saved_work WHERE organization_id='{organization}' AND created_by='{principal}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id='{organization}' AND creator_id='{principal}' AND version=1")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.box_files WHERE organization_id='{organization}' AND uploaded_by='{principal}'")).await,
    ]
}
fn row<'a>(overview: &'a [MemberRow], principal: &str) -> &'a MemberRow {
    overview
        .iter()
        .find(|m| m.principal == principal)
        .unwrap_or_else(|| panic!("{principal} in the overview"))
}
fn rfc3339_second(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && value
            .bytes()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit())
}

/// A fresh identity admitted through the storage admission path with the
/// current notice version (what the invitation page's consent produces).
async fn admit(fixture: &Fixture, stores: &Stores, label: &str) -> (String, String) {
    let issued = operator(
        fixture,
        &[
            "issue",
            "--organization",
            ORG,
            "--issued-by",
            "org-admin-campaign",
            "--max-redemptions",
            "1",
            "--grant",
            &format!("discover:{APP}"),
            "--grant",
            &format!("run:{APP}"),
        ],
    );
    let code_id = issued["code_id"].as_str().unwrap().to_owned();
    let digest = code_digest(issued["code"].as_str().unwrap()).unwrap();
    let identity = VerifiedNonmember {
        issuer: ISSUER.into(),
        provider_tenant: format!("ten_{ORG}"),
        provider_subject: format!("usr_org_admin_{label}"),
        registry_sha256: stores.registry_identity().sha256.clone(),
        email_verified: true,
    };
    let Reservation::Reserved { reservation, .. } = stores
        .workspace
        .reserve_code(ORG, &digest, &sha(label), NOTICE_VERSION)
        .await
        .unwrap()
    else {
        panic!("reservation refused");
    };
    let PendingAdmission::Pending {
        pending_admission, ..
    } = stores
        .workspace
        .record_pending_admission(&identity, ORG, &reservation)
        .await
        .unwrap()
    else {
        panic!("pending admission refused");
    };
    let Admission::Admitted { principal, .. } = stores
        .workspace
        .admit(&identity, ORG, &pending_admission, &sha(label))
        .await
        .unwrap()
    else {
        panic!("admission refused");
    };
    (principal, code_id)
}

async fn campaign() {
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    // A member of the canary organization only (operator-channel stand-in).
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{OTHER}',true); INSERT INTO genui.principals VALUES ('{OTHER}','{OUTSIDER}','{ISSUER}','ten_{OTHER}','usr_{OUTSIDER}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{OTHER}','{OUTSIDER}','active',1); INSERT INTO genui.entitlements VALUES ('{OTHER}','{OUTSIDER}','0000e000-0000-4000-8000-000000000ab0','active','member_invite',now(),now()+interval '30 days',NULL); COMMIT;"), "genui");
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    let admin = request(&stores, ORG, ADMIN, 1);
    let principal = request(&stores, ORG, PRINCIPAL, 1);
    let other_admin = request(&stores, OTHER, ADMIN, 1);
    let outsider_in_org = request(&stores, ORG, OUTSIDER, 1);
    let before = footprint(&fixture, ORG).await;
    let before_other = footprint(&fixture, OTHER).await;

    // --- consent recorded with the admission -------------------------------
    let (newcomer, code_id) = admit(&fixture, &stores, "newcomer").await;
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.member_consents WHERE organization_id='{ORG}' AND principal_id='{newcomer}' AND notice_version={NOTICE_VERSION}")).await,
        1
    );
    let chain = rows(&fixture, &format!("SELECT kind,actor_principal_id::text,outcome_digest::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' ORDER BY chain_sequence DESC LIMIT 2")).await;
    assert_eq!(chain[0].get::<_, String>(0), "admission.consented");
    assert_eq!(chain[0].get::<_, String>(1), newcomer);
    assert_eq!(
        chain[0].get::<_, String>(2),
        domain_digest("forma:notice-consent:v1", &NOTICE_VERSION.to_string())
    );
    assert_eq!(chain[1].get::<_, String>(0), "admission.redeemed");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.member_consents c JOIN genui.audit_events e ON e.organization_id=c.organization_id AND e.event_id=c.audit_event_id AND e.kind='admission.consented' AND e.actor_principal_id=c.principal_id WHERE c.organization_id='{ORG}'")).await,
        1
    );
    // The reservation path refuses a notice version outside the bound and
    // the definer refuses a pending admission without one (malformed: no row).
    assert_eq!(
        stores
            .workspace
            .reserve_code(ORG, &sha("any"), &sha("ctx"), 0)
            .await
            .err(),
        Some(Error::Denied)
    );
    check(
        &mut checks,
        "an admission through the storage path stores the member's consent to the notice version the reservation carried and appends admission.consented right after admission.redeemed, naming the consent row's event; a reservation without a usable notice version is refused",
    );

    // --- a non-admin, a cross-organization caller and a non-member ---------
    assert!(!stores.workspace.is_org_admin(&principal).await.unwrap());
    assert_eq!(
        stores.workspace.org_admin_overview(&principal).await.err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(&principal, PEER)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(!stores.workspace.is_org_admin(&outsider_in_org).await.unwrap());
    assert_eq!(
        stores
            .workspace
            .org_admin_overview(&outsider_in_org)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(&outsider_in_org, PEER)
            .await
            .err(),
        Some(Error::Denied)
    );
    // The canary admin sees only the canary organization and cannot reach a
    // Northstar-only member: RLS and the definer's scope agree.
    assert!(stores.workspace.is_org_admin(&other_admin).await.unwrap());
    let other_overview = stores.workspace.org_admin_overview(&other_admin).await.unwrap();
    assert!(other_overview.iter().any(|m| m.principal == OUTSIDER));
    assert!(other_overview.iter().all(|m| m.principal != newcomer));
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(&other_admin, &newcomer)
            .await
            .unwrap(),
        RemoveOutcome::Refused("not_member")
    );
    let after_newcomer = footprint(&fixture, ORG).await;
    assert_eq!(after_newcomer["members"].as_array().unwrap().len(), before["members"].as_array().unwrap().len() + 1);
    assert_eq!(footprint(&fixture, OTHER).await, before_other);
    check(
        &mut checks,
        "a member without admin_policy, and a principal that is not a member of the request's organization, are refused (Denied, nothing written); the other organization's admin lists only its own members and cannot remove a member of this one (not_member)",
    );

    // --- the overview: counts, consent, code, names --------------------------
    fixture.sql(&format!("UPDATE genui.access_codes SET note='first wave' WHERE organization_id='{ORG}' AND code_id='{code_id}'"), "genui");
    assert!(stores.workspace.is_org_admin(&admin).await.unwrap());
    let overview = stores.workspace.org_admin_overview(&admin).await.unwrap();
    let listed: Vec<&str> = overview.iter().map(|m| m.principal.as_str()).collect();
    let seeded = rows(&fixture, &format!("SELECT principal_id::text FROM genui.memberships WHERE organization_id='{ORG}'")).await;
    assert_eq!(listed.len(), seeded.len(), "every membership, revoked included");
    for member in &overview {
        assert_eq!(
            [member.apps, member.works, member.records, member.uploads],
            counted(&fixture, ORG, &member.principal).await,
            "{}",
            member.principal
        );
        assert!(!member.label.is_empty());
        assert!(member.label.len() <= 80);
        assert!(member.consented_at.as_deref().is_none_or(rfc3339_second));
        assert!(member.last_active.as_deref().is_none_or(rfc3339_second));
        assert!(!member.label.contains("usr_"), "no provider subject");
    }
    let me = row(&overview, PRINCIPAL);
    assert!(me.apps >= 1 && me.records >= 1, "the seeded owner: {me:?}");
    assert_eq!(me.label, crate::directory::neutral_label(PRINCIPAL));
    assert!(!me.is_admin && me.consent_version.is_none());
    assert!(row(&overview, ADMIN).is_admin);
    assert_eq!(row(&overview, REVOKED).status, "revoked");
    let fresh = row(&overview, &newcomer);
    assert_eq!(fresh.consent_version, Some(NOTICE_VERSION));
    assert!(fresh.consented_at.as_deref().is_some_and(rfc3339_second));
    assert!(fresh.last_active.as_deref().is_some_and(rfc3339_second), "the admission events are activity");
    assert_eq!(fresh.code_status.as_deref(), Some("exhausted"));
    assert_eq!(fresh.note.as_deref(), Some("first wave"));
    assert_eq!([fresh.apps, fresh.works, fresh.records, fresh.uploads], [0, 0, 0, 0]);
    check(
        &mut checks,
        "the admin's overview lists every membership of its organization with the superuser's own counts of apps owned, works created, records created and files uploaded, status, admin flag, latest consent (version and RFC 3339 time), last activity, and the redeemed code's status and operator note; no provider subject appears",
    );

    // --- names: the member's own, bounded, audited, shown everywhere -----------
    let newcomer_request = request(&stores, ORG, &newcomer, 1);
    let newcomer_request = TrustedRequest {
        provider_subject: "usr_org_admin_newcomer".into(),
        ..newcomer_request
    };
    assert_eq!(
        stores.workspace.display_name(&newcomer_request).await.unwrap(),
        None
    );
    for bad in ["", " ", "a\u{0}b", "tab\tname", &"n".repeat(81)] {
        assert_eq!(
            stores
                .workspace
                .set_display_name(&newcomer_request, bad)
                .await
                .err(),
            Some(Error::Denied),
            "{bad:?}"
        );
    }
    assert_eq!(events(&fixture, ORG, "member.named").await.len(), 0);
    stores
        .workspace
        .set_display_name(&newcomer_request, "  Ada Lovelace ")
        .await
        .unwrap();
    assert_eq!(
        stores.workspace.display_name(&newcomer_request).await.unwrap().as_deref(),
        Some("Ada Lovelace")
    );
    assert_eq!(crate::directory::label(ORG, &newcomer), "Ada Lovelace");
    assert_eq!(
        crate::directory::label(OTHER, &newcomer),
        crate::directory::neutral_label(&newcomer),
        "a name belongs to one organization"
    );
    assert_eq!(member_label(ORG, &newcomer), "Ada Lovelace");
    assert_eq!(row(&stores.workspace.org_admin_overview(&admin).await.unwrap(), &newcomer).label, "Ada Lovelace");
    // The same name again writes nothing; a new name writes one more event.
    stores
        .workspace
        .set_display_name(&newcomer_request, "Ada Lovelace")
        .await
        .unwrap();
    stores
        .workspace
        .set_display_name(&newcomer_request, "Ada L.")
        .await
        .unwrap();
    let named = events(&fixture, ORG, "member.named").await;
    assert_eq!(
        named,
        vec![
            (newcomer.clone(), Some(domain_digest("forma:member-name:v1", "Ada Lovelace")), true),
            (newcomer.clone(), Some(domain_digest("forma:member-name:v1", "Ada L.")), true),
        ]
    );
    assert_eq!(crate::directory::label(ORG, &newcomer), "Ada L.");
    // A request at a stale epoch or for another principal's row cannot name.
    let stale = TrustedRequest {
        epoch: 7,
        ..request(&stores, ORG, &newcomer, 1)
    };
    assert_eq!(
        stores.workspace.set_display_name(&stale, "Mallory").await.err(),
        Some(Error::Denied)
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.memberships WHERE organization_id='{ORG}' AND display_name='Mallory'")).await, 0);
    // The startup fill reads the stored names back into the process cache.
    crate::directory::set_member_name(ORG, &newcomer, None);
    assert_eq!(crate::directory::label(ORG, &newcomer), crate::directory::neutral_label(&newcomer));
    assert_eq!(stores.workspace.load_display_names().await.unwrap(), 1);
    assert_eq!(crate::directory::label(ORG, &newcomer), "Ada L.");
    check(
        &mut checks,
        "a member sets only its own display name (trimmed, 1..80 printable characters; empty, control characters and 81 refuse with nothing written); each change is one member.named event carrying the name's digest, the same name again none; the admin overview, sharing's member_label and directory::label show it for that organization only, and the startup fill restores it from the database",
    );

    // --- self, last admin, non-members -----------------------------------------
    let settled = footprint(&fixture, ORG).await;
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, ADMIN).await.unwrap(),
        RemoveOutcome::Refused("last_admin"),
        "the sole admin removing itself"
    );
    fixture.sql(&format!("INSERT INTO genui.organization_permissions VALUES ('{ORG}','{PEER}','admin_policy')"), "genui");
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, ADMIN).await.unwrap(),
        RemoveOutcome::Refused("self")
    );
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, REVOKED).await.unwrap(),
        RemoveOutcome::Refused("not_member")
    );
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, NOBODY).await.unwrap(),
        RemoveOutcome::Refused("not_member")
    );
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, "not-an-id").await.err(),
        Some(Error::Denied)
    );
    assert_eq!(footprint(&fixture, ORG).await, settled, "refusals write nothing");
    check(
        &mut checks,
        "removal refuses the caller itself (last_admin while it is the only active admin, self once another admin exists), an already revoked member and a stranger (not_member), and a malformed id (Denied); every refusal writes nothing",
    );

    // --- removal: membership, entitlement, epoch, sessions, audit --------------
    let peer = request(&stores, ORG, PEER, 1);
    assert!(stores.workspace.is_org_admin(&peer).await.unwrap(), "the second admin");
    let reference = "00000000-0000-4000-8000-00000000c0de";
    let cookie = stores
        .workspace
        .mint_session_for_tests(ORG, PEER, reference)
        .await
        .unwrap();
    let live = |r: &str| r == reference;
    assert!(matches!(
        stores.workspace.resolve_session(&cookie, &live).await.unwrap(),
        Lookup::Live(_)
    ));
    let removal_before = footprint(&fixture, ORG).await;
    let outcome = stores.workspace.admin_remove_member(&admin, PEER).await.unwrap();
    assert_eq!(outcome, RemoveOutcome::Removed);
    let member = rows(&fixture, &format!("SELECT status,membership_epoch FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PEER}'")).await;
    assert_eq!(member[0].get::<_, String>(0), "revoked");
    assert_eq!(member[0].get::<_, i64>(1), 2, "the epoch advanced");
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{PEER}' AND status='active'")).await, 0);
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.entitlements WHERE organization_id='{ORG}' AND principal_id='{PEER}' AND status='revoked' AND revoked_at IS NOT NULL")).await, 1);
    assert_eq!(
        removal_before["active_entitlements"].as_i64().unwrap() - 1,
        footprint(&fixture, ORG).await["active_entitlements"].as_i64().unwrap()
    );
    // The session dies at its next request; the removed member's requests
    // are refused everywhere, the admin ones included.
    assert!(matches!(
        stores.workspace.resolve_session(&cookie, &live).await.unwrap(),
        Lookup::Ended(_)
    ));
    assert!(matches!(
        stores.workspace.resolve_session(&cookie, &live).await.unwrap(),
        Lookup::Absent
    ));
    assert!(!stores.workspace.is_org_admin(&peer).await.unwrap());
    assert_eq!(stores.workspace.org_admin_overview(&peer).await.err(), Some(Error::Denied));
    assert_eq!(stores.workspace.admin_remove_member(&peer, ADMIN).await.err(), Some(Error::Denied));
    assert_eq!(stores.workspace.display_name(&peer).await.err(), Some(Error::Denied));
    assert_eq!(stores.workspace.set_display_name(&peer, "Back").await.err(), Some(Error::Denied));
    let target_digest = domain_digest("forma:member-removal:v1", PEER);
    assert_eq!(
        events(&fixture, ORG, "entitlement.revoked").await,
        vec![(ADMIN.to_owned(), Some(target_digest.clone()), true)]
    );
    assert_eq!(
        events(&fixture, ORG, "membership.revoked").await,
        vec![(ADMIN.to_owned(), Some(target_digest), true)]
    );
    let tail = rows(&fixture, &format!("SELECT kind FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' ORDER BY chain_sequence DESC LIMIT 2")).await;
    assert_eq!(
        (tail[0].get::<_, String>(0), tail[1].get::<_, String>(0)),
        ("membership.revoked".into(), "entitlement.revoked".into())
    );
    assert_eq!(row(&stores.workspace.org_admin_overview(&admin).await.unwrap(), PEER).status, "revoked");
    // Removing again is not_member; the operator chain carries none of this.
    assert_eq!(
        stores.workspace.admin_remove_member(&admin, PEER).await.unwrap(),
        RemoveOutcome::Refused("not_member")
    );
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='operator' AND kind IN ('membership.revoked','entitlement.revoked')")).await, 0);
    check(
        &mut checks,
        "an admin's removal revokes the membership (epoch + 1) and every active entitlement in one transaction, appends entitlement.revoked then membership.revoked on the admission chain with the admin as actor and the removed member's digest as outcome; the removed member's live session ends at its next request and every later request of theirs is refused; a second removal is not_member",
    );

    // --- the chains verify -------------------------------------------------------
    for org in [ORG, OTHER] {
        let out = operator(&fixture, &["verify", "--organization", org]);
        assert_eq!(out["verified"], true, "{org}");
    }
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    // backfill's provisioning, then pending, redeemed, consented, two member.named,
    // entitlement.revoked, membership.revoked.
    assert!(out["chains"]["admission"]["events"].as_i64().unwrap() >= 7);
    check(
        &mut checks,
        "the admission and operator chains of both organizations recompute from genesis to their heads with the consent, name and removal events in them",
    );

    stores.close().await.unwrap();
    if let Some(path) = std::env::var_os("GENUI_ORG_ADMIN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"trial plan D4/D5/D6: consent, member names and organization admin storage (migration 0023)","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","The second admin is granted by fixture SQL as the operator stand-in; the operator's grant-admin command is W3's.","The HTTP admin route and page are W2's; this campaign exercises the storage module and the session resolution the route uses.","The directory cache is process-wide: one task (ADR-007 D7.18), refreshed by this process's own writes and the startup fill."]})).unwrap()).unwrap();
    }
    println!(
        "org-admin campaign: {} checks passed",
        checks.len()
    );
}
