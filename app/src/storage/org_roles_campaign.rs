//! Organization roles campaign (ADR-021; APP-225, APP-226; migration 0025).
//! Isolated test executable only: local Docker, OpenSSL and the locked
//! Python environments, as the other storage campaigns.
//!
//! Three parts, every acceptance item asserted on the database (membership
//! rows, epochs, the admission chain) or on storage's typed answers:
//!
//! 1. **The ladder.** Every caller role × target role × action (set each of
//!    the five roles, remove), each allowed or refused exactly as the
//!    capability table says: the definer's verdict is compared cell by cell
//!    with the Rust ladder (`OrgRole::may_set`, `OrgRole::may_remove`) and
//!    with hand-written expectations for the cells that matter most (a
//!    manager never sets admin, an admin never touches an owner, nobody
//!    raises their own role, the last owner stays). The epoch advances on a
//!    change and an open session of the target loses its authority at its
//!    next request; one `membership.role_changed` event per real change and
//!    none for a no-op or a refusal; the refusal codes are the closed set.
//! 2. **The viewer walk.** A viewer with an organization-wide contributor
//!    share and a direct contributor share: every read path works (the
//!    canvas, opening the work, the record listing, the record detail) and
//!    every write path is refused (a command, a record, a source upload, a
//!    share, an agent token, compose, Ask, Shape, the folder build, `/v1/act`,
//!    the export route), in storage (the SQL clauses) and over the routes
//!    (`viewer_read_only`), with the database footprint unchanged. Mirrors
//!    the W4 private-entries campaign: the same general app, a member beside
//!    the viewer to prove the refusals are the role's and not the path's.
//! 3. **`/admin` over real rows.** The select offers only the roles the
//!    caller may set; a forged form for a disallowed role is refused
//!    server-side; a member or a viewer gets the uniform 404; the CSRF
//!    token is enforced.
use super::{
    admission_qualification::operator,
    command_qualification::{send, vault_token, TestPages},
    definition_qualification::{grant, request_as, POLICY},
    private_entries_campaign,
    qualification::{fixture_python, Fixture},
    qualification_cases::{check, ORG},
    *,
};
use crate::{
    admission,
    auth::{csrf, Auth, Pages},
    boxes, composer, http, org_admin,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        routes::tests::RecordingPages,
        Work,
    },
};
use axum::{http::StatusCode, Router};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
/// tenant_seed's owner (the former admin_policy holder; migration 0025 made
/// it the organization's owner).
const SEEDED_OWNER: &str = "00000000-0000-4000-8000-000000000f00";
/// tenant_seed's revoked member.
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
/// Well formed, never a member anywhere.
const NOBODY: &str = "00000000-0000-4000-8000-000000000cff";
/// The callers, one per role (the viewer and the member prove the 404).
const OWNER: &str = "00000000-0000-4000-8000-000000000c01";
const ADMIN: &str = "00000000-0000-4000-8000-000000000c02";
const MANAGER: &str = "00000000-0000-4000-8000-000000000c03";
const MEMBER: &str = "00000000-0000-4000-8000-000000000c04";
const VIEWER: &str = "00000000-0000-4000-8000-000000000c05";
/// The targets, one per role, reset between cells.
const T_OWNER: &str = "00000000-0000-4000-8000-000000000c11";
const T_ADMIN: &str = "00000000-0000-4000-8000-000000000c12";
const T_MANAGER: &str = "00000000-0000-4000-8000-000000000c13";
const T_MEMBER: &str = "00000000-0000-4000-8000-000000000c14";
const T_VIEWER: &str = "00000000-0000-4000-8000-000000000c15";
/// The viewer walk's app and its builder.
const WALK_APP: &str = "00000000-0000-4000-8000-000000000c20";
const BUILDER: &str = "00000000-0000-4000-8000-000000000c21";
const WALKER: &str = "00000000-0000-4000-8000-000000000c22";
const FORM: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/x-www-form-urlencoded"),
];
const JSON: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/json"),
];

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_org_roles_campaign() {
    std::thread::Builder::new()
        .name("app225-org-roles-campaign".into())
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
/// active member of the organization with a live entitlement and a role.
fn admit_member(fixture: &Fixture, principal: &str, role: OrgRole) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch,role) VALUES ('{ORG}','{principal}','active',1,'{}'); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;", role.as_str()), "genui");
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn text(fixture: &Fixture, sql: &str) -> String {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
async fn execute(fixture: &Fixture, sql: &str) {
    let operator = fixture.operator().await;
    operator.client.execute(sql, &[]).await.unwrap();
}
/// The membership row the ladder may change: role and epoch.
async fn membership(fixture: &Fixture, principal: &str) -> (String, i64, String) {
    let operator = fixture.operator().await;
    let row = operator
        .client
        .query_one(
            &format!("SELECT role,membership_epoch,status FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{principal}'"),
            &[],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1), row.get(2))
}
async fn role_events(fixture: &Fixture) -> i64 {
    scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='membership.role_changed'")).await
}
fn role_digest(principal: &str, from: &str, to: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"forma:member-role:v1");
    hash.update([0u8]);
    hash.update(principal.as_bytes());
    hash.update([0u8]);
    hash.update(from.as_bytes());
    hash.update([0u8]);
    hash.update(to.as_bytes());
    format!(
        "sha256:{}",
        hash.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn rank(role: OrgRole) -> u8 {
    match role {
        OrgRole::Owner => 4,
        OrgRole::Admin => 3,
        OrgRole::Manager => 2,
        OrgRole::Member => 1,
        OrgRole::Viewer => 0,
    }
}
fn target_of(role: OrgRole) -> &'static str {
    match role {
        OrgRole::Owner => T_OWNER,
        OrgRole::Admin => T_ADMIN,
        OrgRole::Manager => T_MANAGER,
        OrgRole::Member => T_MEMBER,
        OrgRole::Viewer => T_VIEWER,
    }
}
fn caller_of(role: OrgRole) -> &'static str {
    match role {
        OrgRole::Owner => OWNER,
        OrgRole::Admin => ADMIN,
        OrgRole::Manager => MANAGER,
        OrgRole::Member => MEMBER,
        OrgRole::Viewer => VIEWER,
    }
}
/// The hand-written table (ADR-021), independent of `OrgRole::may_set`.
fn expected_set(caller: OrgRole, target: OrgRole, to: OrgRole) -> RoleOutcome {
    let manages = match caller {
        OrgRole::Owner => 4,
        OrgRole::Admin => 2,
        OrgRole::Manager => 1,
        _ => return RoleOutcome::Refused("denied"),
    };
    if rank(target) > manages || rank(to) > manages {
        return RoleOutcome::Refused("not_allowed");
    }
    if target == to {
        return RoleOutcome::Unchanged;
    }
    RoleOutcome::Changed
}
/// What a viewer's write could touch: revisions, shares, grants, tokens,
/// folders, apps and the audit chains.
async fn footprint(fixture: &Fixture) -> [i64; 7] {
    [
        scalar(fixture, &format!("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.agent_tokens WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.boxes WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.apps WHERE organization_id='{ORG}'")).await,
        scalar(fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}'")).await,
    ]
}
fn form(pairs: &[(&str, &str)]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in pairs {
        out.append_pair(name, value);
    }
    out.finish()
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
    for (principal, role) in [
        (OWNER, OrgRole::Owner),
        (ADMIN, OrgRole::Admin),
        (MANAGER, OrgRole::Manager),
        (MEMBER, OrgRole::Member),
        (VIEWER, OrgRole::Viewer),
        (T_OWNER, OrgRole::Owner),
        (T_ADMIN, OrgRole::Admin),
        (T_MANAGER, OrgRole::Manager),
        (T_MEMBER, OrgRole::Member),
        (T_VIEWER, OrgRole::Viewer),
        (BUILDER, OrgRole::Member),
        (WALKER, OrgRole::Viewer),
    ] {
        admit_member(&fixture, principal, role);
    }
    // --- migration 0025 on the seed: the former admin_policy holder is the
    // owner, everyone else a member, and the governance table is gone.
    assert_eq!(membership(&fixture, SEEDED_OWNER).await.0, "owner");
    assert_eq!(membership(&fixture, REVOKED).await.0, "member");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.memberships WHERE organization_id='{ORG}' AND role NOT IN ('owner','admin','manager','member','viewer')")).await,
        0
    );
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname='organization_permissions'").await,
        0
    );
    check(
        &mut checks,
        "migration 0025 on the seed: tenant_seed's admin_policy holder is the organization's owner, every other membership (the revoked one included) is a member, every stored role is one of the five, and organization_permissions no longer exists",
    );

    // --- 1. The ladder ----------------------------------------------------------
    let request = |principal: &str, role: OrgRole| request_as(&stores, ORG, principal, 1, role);
    let callers = [
        (OrgRole::Owner, request(OWNER, OrgRole::Owner)),
        (OrgRole::Admin, request(ADMIN, OrgRole::Admin)),
        (OrgRole::Manager, request(MANAGER, OrgRole::Manager)),
        (OrgRole::Member, request(MEMBER, OrgRole::Member)),
        (OrgRole::Viewer, request(VIEWER, OrgRole::Viewer)),
    ];
    // Two owners exist (OWNER and the seeded one), so demoting T_OWNER is
    // never the last owner.
    let mut cells = 0;
    for (caller_role, caller) in &callers {
        for target_role in OrgRole::ALL {
            let target = target_of(target_role);
            for to in OrgRole::ALL {
                let events = role_events(&fixture).await;
                let (_, epoch_before, _) = membership(&fixture, target).await;
                let outcome = stores.workspace.set_member_role(caller, target, to).await;
                let expected = expected_set(*caller_role, target_role, to);
                let (role_after, epoch_after, _) = membership(&fixture, target).await;
                match expected {
                    RoleOutcome::Refused("denied") => {
                        assert_eq!(
                            outcome.err(),
                            Some(Error::Denied),
                            "{caller_role:?} on {target_role:?} -> {to:?}"
                        );
                        assert!(!caller_role.can(Capability::Admin));
                    }
                    other => {
                        let got = outcome.unwrap_or_else(|e| {
                            panic!("{caller_role:?} on {target_role:?} -> {to:?}: {e:?}")
                        });
                        assert_eq!(got, other, "{caller_role:?} on {target_role:?} -> {to:?}");
                        // The Rust ladder agrees with the definer, cell by cell.
                        assert_eq!(
                            caller_role.may_set(target_role, to, false),
                            !matches!(got, RoleOutcome::Refused(_)),
                            "OrgRole::may_set vs the definer: {caller_role:?} on {target_role:?} -> {to:?}"
                        );
                    }
                }
                match expected {
                    RoleOutcome::Changed => {
                        assert_eq!(role_after, to.as_str());
                        assert_eq!(epoch_after, epoch_before + 1, "the epoch advances");
                        assert_eq!(
                            role_events(&fixture).await,
                            events + 1,
                            "one event per change"
                        );
                        let digest = text(&fixture, &format!("SELECT outcome_digest::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='membership.role_changed' ORDER BY chain_sequence DESC LIMIT 1")).await;
                        assert_eq!(
                            digest,
                            role_digest(target, target_role.as_str(), to.as_str())
                        );
                        assert_eq!(
                            text(&fixture, &format!("SELECT actor_principal_id::text FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='membership.role_changed' ORDER BY chain_sequence DESC LIMIT 1")).await,
                            caller_of(*caller_role)
                        );
                        // Reset the target for the next cell.
                        execute(&fixture, &format!("UPDATE genui.memberships SET role='{}' WHERE organization_id='{ORG}' AND principal_id='{target}'", target_role.as_str())).await;
                    }
                    _ => {
                        assert_eq!(
                            role_after,
                            target_role.as_str(),
                            "a refusal or a no-op writes nothing"
                        );
                        assert_eq!(epoch_after, epoch_before);
                        assert_eq!(
                            role_events(&fixture).await,
                            events,
                            "no event for a no-op or a refusal"
                        );
                    }
                }
                cells += 1;
            }
            // Remove.
            let events = role_events(&fixture).await;
            let outcome = stores.workspace.admin_remove_member(caller, target).await;
            let expected_remove = match caller_role {
                OrgRole::Member | OrgRole::Viewer => None,
                _ => Some(caller_role.may_remove(target_role)),
            };
            match expected_remove {
                None => assert_eq!(
                    outcome.err(),
                    Some(Error::Denied),
                    "{caller_role:?} removes {target_role:?}"
                ),
                Some(true) => {
                    assert_eq!(
                        outcome.unwrap(),
                        RemoveOutcome::Removed,
                        "{caller_role:?} removes {target_role:?}"
                    );
                    let (_, _, status) = membership(&fixture, target).await;
                    assert_eq!(status, "revoked");
                    execute(&fixture, &format!("UPDATE genui.memberships SET status='active' WHERE organization_id='{ORG}' AND principal_id='{target}'")).await;
                    execute(&fixture, &format!("UPDATE genui.entitlements SET status='active',revoked_at=NULL WHERE organization_id='{ORG}' AND principal_id='{target}'")).await;
                }
                Some(false) => {
                    assert_eq!(
                        outcome.unwrap(),
                        RemoveOutcome::Refused("not_allowed"),
                        "{caller_role:?} removes {target_role:?}"
                    );
                    assert_eq!(membership(&fixture, target).await.2, "active");
                }
            }
            assert_eq!(
                role_events(&fixture).await,
                events,
                "a removal is not a role change"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 5 * 5 * 6);
    // The hand-written cells that matter most.
    let owner = &callers[0].1;
    let admin = &callers[1].1;
    let manager = &callers[2].1;
    assert_eq!(
        stores
            .workspace
            .set_member_role(manager, T_MEMBER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "a manager never sets admin"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(manager, T_MEMBER, OrgRole::Manager)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "a manager never sets manager"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(admin, T_OWNER, OrgRole::Member)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "an admin never touches an owner"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(admin, T_MEMBER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "an admin never grants admin"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(admin, T_ADMIN, OrgRole::Member)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "an admin never demotes an admin"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, T_MEMBER, OrgRole::Owner)
            .await
            .unwrap(),
        RoleOutcome::Changed,
        "an owner grants owner"
    );
    execute(&fixture, &format!("UPDATE genui.memberships SET role='member' WHERE organization_id='{ORG}' AND principal_id='{T_MEMBER}'")).await;
    check(
        &mut checks,
        "the ladder, 150 cells over real rows: every caller role x target role x (set each of the five roles, remove) is allowed or refused exactly as the ADR-021 table says, the definer and OrgRole::may_set/may_remove agree cell by cell, a member and a viewer are Denied before any read, a manager never sets manager or admin, an admin never touches an owner or an admin nor grants admin, an owner grants and removes anything; a change advances the target's epoch and appends one membership.role_changed event with the recomputable digest (target id, role before, role after) and the caller as actor; a refusal or a no-op writes nothing",
    );

    // --- own role, the last owner, the refusal codes ------------------------------
    assert_eq!(
        stores
            .workspace
            .set_member_role(admin, ADMIN, OrgRole::Owner)
            .await
            .unwrap(),
        RoleOutcome::Refused("self"),
        "an admin raising themselves"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(manager, MANAGER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Refused("self"),
        "a manager raising themselves"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(manager, MANAGER, OrgRole::Manager)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "a manager's own row is above what they manage"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(admin, ADMIN, OrgRole::Member)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed"),
        "an admin's own row is above what they manage"
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, OWNER, OrgRole::Owner)
            .await
            .unwrap(),
        RoleOutcome::Unchanged
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(owner, OWNER)
            .await
            .unwrap(),
        RemoveOutcome::Refused("self"),
        "an owner removing itself while another owner exists"
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(admin, ADMIN)
            .await
            .unwrap(),
        RemoveOutcome::Refused("self")
    );
    // The last owner: demote every other owner, then the rule holds for the
    // one left, on set and on remove, by themselves and by an admin's forged
    // path (not_allowed first).
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, T_OWNER, OrgRole::Member)
            .await
            .unwrap(),
        RoleOutcome::Changed
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, SEEDED_OWNER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Changed
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.memberships WHERE organization_id='{ORG}' AND role='owner' AND status='active'")).await,
        1
    );
    let events = role_events(&fixture).await;
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, OWNER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Refused("last_owner")
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, OWNER, OrgRole::Viewer)
            .await
            .unwrap(),
        RoleOutcome::Refused("last_owner")
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(owner, OWNER)
            .await
            .unwrap(),
        RemoveOutcome::Refused("last_owner")
    );
    let seeded_admin = request(SEEDED_OWNER, OrgRole::Admin);
    let seeded_admin = TrustedRequest {
        epoch: membership(&fixture, SEEDED_OWNER).await.1,
        ..seeded_admin
    };
    assert_eq!(
        stores
            .workspace
            .set_member_role(&seeded_admin, OWNER, OrgRole::Member)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_allowed")
    );
    assert_eq!(
        stores
            .workspace
            .admin_remove_member(&seeded_admin, OWNER)
            .await
            .unwrap(),
        RemoveOutcome::Refused("not_allowed")
    );
    assert_eq!(
        membership(&fixture, OWNER).await,
        ("owner".into(), 1, "active".into())
    );
    assert_eq!(role_events(&fixture).await, events);
    // With a second owner again, the first may step down.
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, T_OWNER, OrgRole::Owner)
            .await
            .unwrap(),
        RoleOutcome::Changed
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, OWNER, OrgRole::Admin)
            .await
            .unwrap(),
        RoleOutcome::Changed
    );
    assert_eq!(membership(&fixture, OWNER).await.0, "admin");
    // Restore the fixture's owners for what follows.
    execute(&fixture, &format!("UPDATE genui.memberships SET role='owner',membership_epoch=1 WHERE organization_id='{ORG}' AND principal_id IN ('{OWNER}','{SEEDED_OWNER}','{T_OWNER}')")).await;
    // The other refusal codes: a revoked member, a stranger, a malformed id,
    // an unknown role (the definer's own code, reached as the workspace role).
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, REVOKED, OrgRole::Viewer)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_member")
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, NOBODY, OrgRole::Viewer)
            .await
            .unwrap(),
        RoleOutcome::Refused("not_member")
    );
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, "not-an-id", OrgRole::Viewer)
            .await
            .err(),
        Some(Error::Denied)
    );
    {
        let mut operator = fixture.operator().await;
        let transaction = operator.client.transaction().await.unwrap();
        transaction
            .batch_execute("SET LOCAL ROLE genui_workspace")
            .await
            .unwrap();
        transaction.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &OWNER]).await.unwrap();
        let outcome: String = transaction
            .query_one("SELECT outcome FROM genui_private.set_member_role($1::text::uuid,$2::text::uuid,$3::text::uuid,'teacher')", &[&ORG, &OWNER, &T_MEMBER])
            .await
            .unwrap()
            .get(0);
        assert_eq!(outcome, "unknown_role");
        // A viewer or a member calling the definer directly: no row.
        let rows = transaction
            .query("SELECT outcome FROM genui_private.set_member_role($1::text::uuid,$2::text::uuid,$3::text::uuid,'viewer')", &[&ORG, &OWNER, &T_MEMBER])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        transaction.rollback().await.unwrap();
        let transaction = operator.client.transaction().await.unwrap();
        transaction
            .batch_execute("SET LOCAL ROLE genui_workspace")
            .await
            .unwrap();
        transaction.execute("SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)", &[&ORG, &VIEWER]).await.unwrap();
        let rows = transaction
            .query("SELECT outcome FROM genui_private.set_member_role($1::text::uuid,$2::text::uuid,$3::text::uuid,'viewer')", &[&ORG, &VIEWER, &T_MEMBER])
            .await
            .unwrap();
        assert!(rows.is_empty(), "a viewer gets no row from the definer");
        transaction.rollback().await.unwrap();
    }
    check(
        &mut checks,
        "own role and the last owner over real rows: an admin or a manager raising their own role is `self`, their own row otherwise `not_allowed`, an owner's same role again is unchanged; once one active owner is left, demoting or removing them is `last_owner` (by themselves) or `not_allowed` (by an admin), nothing written; with a second owner the first steps down; a revoked member and a stranger are `not_member`, a malformed id Denied, a role outside the five `unknown_role` at the definer, and a viewer calling the definer gets no row",
    );

    // --- the epoch: an open session loses authority at its next request ----------
    let auth = Arc::new(
        Auth::new(
            None,
            Some(stores.clone()),
            Arc::new(TestPages) as Arc<dyn Pages>,
        )
        .unwrap(),
    );
    let mint = |principal: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            stores
                .workspace
                .mint_session_for_tests(ORG, principal, &reference)
                .await
                .unwrap()
        }
    };
    let live = |_: &str| true;
    let member_cookie = mint(T_MEMBER).await;
    let Lookup::Live(before) = stores
        .workspace
        .resolve_session(&member_cookie, &live)
        .await
        .unwrap()
    else {
        panic!("the member's session is live");
    };
    assert_eq!(before.role(), OrgRole::Member);
    assert!(
        stores.workspace.display_name(&before).await.is_ok(),
        "the request holds authority before the change"
    );
    let token = stores
        .workspace
        .mint_agent_token(&before, "before-the-change")
        .await
        .unwrap();
    assert!(stores
        .workspace
        .resolve_agent_token(&token.token)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        stores
            .workspace
            .set_member_role(owner, T_MEMBER, OrgRole::Viewer)
            .await
            .unwrap(),
        RoleOutcome::Changed
    );
    // The request resolved before the change carries the old epoch: the
    // member fence refuses it everywhere.
    assert_eq!(
        stores.workspace.display_name(&before).await.err(),
        Some(Error::Denied),
        "the stale request loses its authority"
    );
    assert_eq!(
        stores
            .data
            .list_records(&before, WALK_APP, "00000000-0000-4000-8000-000000000000")
            .await
            .err(),
        Some(Error::Denied)
    );
    // The agent token minted at the old epoch no longer resolves (APP-099:
    // membership_epoch_at_mint), and would be refused as a viewer anyway.
    assert!(
        stores
            .workspace
            .resolve_agent_token(&token.token)
            .await
            .unwrap()
            .is_none(),
        "the token dies with the epoch"
    );
    // The browser session re-reads its authority at the next request: it
    // stays live and carries the new role and epoch (sessions fence on the
    // current epoch, APP-009), so the new capabilities apply at once.
    let Lookup::Live(after) = stores
        .workspace
        .resolve_session(&member_cookie, &live)
        .await
        .unwrap()
    else {
        panic!("the session re-reads its authority");
    };
    assert_eq!(
        after.role(),
        OrgRole::Viewer,
        "the next request carries the new role"
    );
    assert_eq!(after.epoch, before.epoch + 1);
    assert!(!after.can(Capability::Build) && after.can(Capability::Read));
    assert!(stores.workspace.display_name(&after).await.is_ok());
    execute(&fixture, &format!("UPDATE genui.memberships SET role='member' WHERE organization_id='{ORG}' AND principal_id='{T_MEMBER}'")).await;
    check(
        &mut checks,
        "the epoch over real rows: a role change advances the target's epoch, so a request resolved before the change is refused by the member fence everywhere and an agent token minted before it no longer resolves; the browser session re-reads its authority at its next request and carries the new role, epoch and capabilities",
    );

    // --- the overview: roles, callable by owner, admin and manager -----------------
    for (role, caller) in &callers {
        let overview = stores.workspace.org_admin_overview(caller).await;
        if role.can(Capability::Admin) {
            let rows = overview.unwrap_or_else(|e| panic!("{role:?}: {e:?}"));
            assert_eq!(
                rows.iter().find(|r| r.principal == OWNER).unwrap().role,
                OrgRole::Owner
            );
            assert_eq!(
                rows.iter().find(|r| r.principal == VIEWER).unwrap().role,
                OrgRole::Viewer
            );
            assert_eq!(
                rows.iter().find(|r| r.principal == MANAGER).unwrap().role,
                OrgRole::Manager
            );
            assert!(stores.workspace.is_org_admin(caller).await.unwrap());
        } else {
            assert_eq!(overview.err(), Some(Error::Denied), "{role:?}");
            assert!(!stores.workspace.is_org_admin(caller).await.unwrap());
        }
    }
    check(
        &mut checks,
        "org_admin_overview answers each member's role and is callable by an owner, an admin and a manager; a member and a viewer are Denied and is_org_admin is false for them",
    );

    // --- 2. The viewer walk ---------------------------------------------------------
    let builder = request(BUILDER, OrgRole::Member);
    let walker = request(WALKER, OrgRole::Viewer);
    let created = operator(
        &fixture,
        &[
            "create-app",
            "--organization",
            ORG,
            "--app",
            WALK_APP,
            "--space",
            SPACE,
            "--owner",
            BUILDER,
        ],
    );
    assert_eq!(created["granted"], json!(["edit_definition"]));
    for permission in ["discover", "run"] {
        grant(&fixture, BUILDER, permission, &["--app", WALK_APP]);
    }
    let admitted = admission::admit_definition(
        &serde_json::to_vec(&private_entries_campaign::definition()).unwrap(),
    )
    .expect("the private-entries definition admits");
    let revision = stores
        .workspace
        .create_definition_revision(&builder, WALK_APP, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(&builder, WALK_APP, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let saved = stores
        .workspace
        .create_work(
            &builder,
            WALK_APP,
            &publication.publication_id,
            "Claims and notes (roles campaign)",
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
    let work = saved.work.clone();
    // The organization-wide contributor share and the direct contributor share.
    stores
        .workspace
        .share_work(
            &builder,
            WALK_APP,
            &work,
            Audience::Organization,
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    stores
        .workspace
        .share_work(
            &builder,
            WALK_APP,
            &work,
            Audience::Member(WALKER),
            ShareRole::Contributor,
            ShareMode::Set,
        )
        .await
        .unwrap();
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{WALKER}' AND work_id='{work}' AND permission='record_write'")).await,
        1,
        "the viewer holds a contributor grant on paper"
    );
    let claim = |who: &str, key: &str| CommandRequest {
        app: WALK_APP.into(),
        work: work.clone(),
        publication: publication.publication_id.clone(),
        command: "submit".into(),
        schema: "claim".into(),
        schema_version_expected: 1,
        record: format!("00000000-0000-4000-8000-0000000{}", &who[who.len() - 5..]),
        expected_record_version: 0,
        expected_work_version: 1,
        values: json!([
            {"field_id": "amount", "value": {"type": "integer", "value": "7", "unit": "count"}},
            {"field_id": "status", "value": {"type": "enum", "value": "filed"}},
            {"field_id": "note", "value": {"type": "text", "value": "campaign"}}
        ]),
        idempotency_key: key.into(),
    };
    // The member beside the viewer writes: the path works for a member.
    let committed = stores
        .data
        .commit_record(&builder, &claim(BUILDER, &"a".repeat(32)))
        .await
        .unwrap();
    assert!(
        matches!(committed, CommandResult::Committed(_)),
        "{committed:?}"
    );
    let before = footprint(&fixture).await;
    // Reads work for the viewer.
    stores
        .data
        .can_open(&walker, WALK_APP, &work)
        .await
        .unwrap();
    let listing = stores
        .data
        .list_records(&walker, WALK_APP, &work)
        .await
        .unwrap();
    assert_eq!(listing.work, work);
    assert!(
        !listing.can_write,
        "the listing tells the viewer it cannot write"
    );
    // Writes are refused in storage: the SQL clause, not a Rust `if`.
    let refused = stores
        .data
        .commit_record(&walker, &claim(WALKER, &"b".repeat(32)))
        .await;
    assert_eq!(refused.err(), Some(Error::Denied), "a command by a viewer");
    assert_eq!(
        stores
            .data
            .dry_run_command(&walker, &claim(WALKER, &"c".repeat(32)))
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .workspace
            .share_work(
                &walker,
                WALK_APP,
                &work,
                Audience::Member(MEMBER),
                ShareRole::Viewer,
                ShareMode::Set
            )
            .await
            .err(),
        Some(Error::Denied),
        "a share by a viewer"
    );
    assert!(
        stores
            .workspace
            .mint_agent_token(&walker, "viewer-agent")
            .await
            .is_err(),
        "an agent token for a viewer"
    );
    assert!(stores
        .workspace
        .revoke_organization_share(&walker, WALK_APP, &work)
        .await
        .is_err());
    assert_eq!(
        footprint(&fixture).await,
        before,
        "the viewer's refusals write nothing"
    );
    // The same calls by the member beside them are not refused for the role.
    assert!(stores
        .workspace
        .mint_agent_token(&builder, "builder-agent")
        .await
        .is_ok());
    check(
        &mut checks,
        "the viewer walk in storage: a viewer with an organization-wide contributor share and a direct contributor grant opens the work and lists its records (the listing says they cannot write), while a command, a dry run, a share, a share revocation and an agent token are refused by the SQL clauses with the footprint unchanged; the member beside them commits and mints",
    );

    // Over the routes: every write and AI route answers 403 viewer_read_only
    // before any body is read; the reads work; the member is not refused.
    let viewer_cookie = mint(WALKER).await;
    let builder_cookie = mint(BUILDER).await;
    let composer_state = composer::Composer::new(None, "router-model", "generation-model");
    let work_state = Arc::new(Work::new(
        auth.clone(),
        None,
        Arc::new(RecordingPages::default()),
    ));
    let root = std::env::temp_dir().join(format!("org-roles-campaign-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let boxes_state = Arc::new(boxes::routes::Boxes {
        auth: auth.clone(),
        work: work_state.clone(),
        composer: composer_state.clone(),
        store: boxes::store::ObjectStore::Local {
            root: root.join("objects"),
        },
        normalizer: boxes::register::Normalizer {
            program: fixture_python(),
            args: vec!["-I".into(), "-c".into(), "pass".into()],
            cache_root: root.join("cache"),
            scratch_root: root.join("scratch"),
        },
        parser: None,
    });
    let mut manifest =
        forma_server::assets::try_load_manifest::<Assets>().expect("admin/dist built");
    manifest
        .routes
        .retain(|route, _| route == "/canvas" || route == "/admin");
    let modules = forma_server::load_ir_modules::<Assets>(&manifest).unwrap();
    let canvas_pages = Arc::new(SsrCanvasPages {
        page: modules.bind::<CanvasProps>("canvas").unwrap(),
    });
    let admin_pages = Arc::new(org_admin::SsrAdminPages {
        page: modules.bind::<org_admin::AdminProps>("orgAdmin").unwrap(),
    });
    modules.finalize().unwrap();
    let app = http::uniform_with_many(
        Router::new()
            .merge(crate::sharing::router(auth.clone()))
            .merge(crate::commands::routes::router(Arc::new(
                crate::commands::routes::Commands {
                    auth: auth.clone(),
                    page: Arc::new(TestPages),
                },
            )))
            .merge(crate::mcp::tokens::router(auth.clone()))
            .merge(crate::actions::routes::records_router(auth.clone()))
            .merge(boxes::routes::router(boxes_state.clone()))
            .merge(org_admin::router(Arc::new(org_admin::OrgAdmin {
                auth: auth.clone(),
                pages: admin_pages,
            }))),
        vec![
            (
                composer::routes::PATH,
                composer::routes::router(Arc::new(composer::routes::Compose::new(
                    auth.clone(),
                    composer_state.clone(),
                ))),
            ),
            (
                composer::talk::ASK_PATH,
                composer::talk::router(Arc::new(composer::talk::Talk::new(
                    auth.clone(),
                    composer_state.clone(),
                ))),
            ),
            // Each long path is its own service: the talk router answers Shape under its path too.
            (
                composer::talk::SHAPE_PATH,
                composer::talk::router(Arc::new(composer::talk::Talk::new(
                    auth.clone(),
                    composer_state.clone(),
                ))),
            ),
            (
                crate::actions::PATH,
                crate::actions::routes::router(Arc::new(crate::actions::routes::Act::new(
                    auth.clone(),
                    crate::actions::Parser::new(None, "none"),
                ))),
            ),
            (
                canvas::PATH,
                canvas::router(Arc::new(canvas::Canvas {
                    work: work_state.clone(),
                    pages: canvas_pages,
                    live_events: String::new(),
                    demo_organizations: Default::default(),
                })),
            ),
        ],
    );
    let get = |cookie: &str, uri: &str| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = uri.to_owned();
        async move { send(&app, "GET", &uri, Some(&cookie), "", &[]).await }
    };
    let post_json = |cookie: &str, uri: &str, body: String| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = uri.to_owned();
        async move { send(&app, "POST", &uri, Some(&cookie), &body, &JSON).await }
    };
    let viewer_canvas = get(&viewer_cookie, "/canvas").await;
    assert_eq!(
        viewer_canvas.status,
        StatusCode::OK,
        "{}",
        viewer_canvas.body
    );
    assert!(viewer_canvas
        .body
        .contains("You have view-only access in this workspace."));
    assert!(viewer_canvas.body.contains("data-can-build=\"false\""));
    assert!(
        !viewer_canvas.body.contains("data-create-app"),
        "no Create app for a viewer"
    );
    assert!(
        !viewer_canvas.body.contains("data-tool=\"ask-about\""),
        "no Ask for a viewer"
    );
    assert!(
        !viewer_canvas.body.contains("data-tool=\"shape\""),
        "no Shape for a viewer"
    );
    assert!(
        !viewer_canvas.body.contains("id=\"intent-input\""),
        "no composer bar for a viewer"
    );
    assert!(
        !viewer_canvas.body.contains("data-manage-share"),
        "no Share for a viewer"
    );
    assert!(!viewer_canvas.body.contains("href=\"/admin\""));
    let builder_canvas = get(&builder_cookie, "/canvas").await;
    assert_eq!(builder_canvas.status, StatusCode::OK);
    assert!(
        builder_canvas.body.contains("data-create-app")
            && builder_canvas.body.contains("id=\"intent-input\"")
    );
    assert!(builder_canvas.body.contains("data-can-build=\"true\""));
    assert!(!builder_canvas.body.contains("You have view-only access"));
    let records = get(
        &viewer_cookie,
        &format!("/v1/work/{WALK_APP}/{work}/records"),
    )
    .await;
    assert_eq!(
        records.status,
        StatusCode::OK,
        "a viewer reads the records: {}",
        records.body
    );
    let share_path = format!("/v1/work/{WALK_APP}/{work}/share");
    let command_path = format!("/v1/work/{WALK_APP}/{work}/commands/submit");
    let writes: Vec<(&str, String, String)> = vec![
        (
            "compose",
            composer::routes::PATH.to_owned(),
            json!({"intent": "an app"}).to_string(),
        ),
        (
            "ask",
            composer::talk::ASK_PATH.to_owned(),
            json!({"question": "what"}).to_string(),
        ),
        (
            "shape",
            composer::talk::SHAPE_PATH.to_owned(),
            json!({"text": "shape"}).to_string(),
        ),
        (
            "act",
            crate::actions::PATH.to_owned(),
            json!({"text": "do"}).to_string(),
        ),
        ("command", command_path.clone(), json!({}).to_string()),
        (
            "share",
            share_path.clone(),
            json!({"grantee_principal_id": MEMBER, "role": "viewer"}).to_string(),
        ),
        (
            "share-organization",
            format!("{share_path}/organization"),
            json!({"role": "viewer"}).to_string(),
        ),
        (
            "agent-token",
            crate::mcp::tokens::PATH.to_owned(),
            json!({"label": "x"}).to_string(),
        ),
        (
            "folder",
            "/v1/boxes".to_owned(),
            json!({"name": "f"}).to_string(),
        ),
        (
            "uploads",
            "/v1/boxes/uploads".to_owned(),
            json!({}).to_string(),
        ),
        (
            "folder-build",
            boxes::routes::APPS_PATH.replace("{box}", "00000000-0000-4000-8000-0000000000b0"),
            json!({}).to_string(),
        ),
        (
            "folder-ask",
            boxes::ask::ASK_PATH.replace("{box}", "00000000-0000-4000-8000-0000000000b0"),
            json!({}).to_string(),
        ),
    ];
    let before = footprint(&fixture).await;
    for (name, uri, body) in &writes {
        let reply = post_json(&viewer_cookie, uri, body.clone()).await;
        assert_eq!(
            reply.status,
            StatusCode::FORBIDDEN,
            "{name} {uri}: {}",
            reply.body
        );
        assert_eq!(
            reply.body, r#"{"code":"viewer_read_only","status":403}"#,
            "{name}"
        );
    }
    assert_eq!(
        footprint(&fixture).await,
        before,
        "no route wrote anything for the viewer"
    );
    // The member beside them is never refused for the role (whatever else
    // the route then answers: the composer is disabled, the bodies are the
    // smallest the routes take).
    for (name, uri, body) in &writes {
        let by_member = post_json(&builder_cookie, uri, body.clone()).await;
        assert_ne!(
            by_member.body, r#"{"code":"viewer_read_only","status":403}"#,
            "{name}: the member is not refused for the role"
        );
    }
    // The share dialog's read and the member list are the sharer's.
    let dialog = get(&viewer_cookie, &share_path).await;
    assert_eq!(dialog.status, StatusCode::FORBIDDEN);
    assert_eq!(
        get(&viewer_cookie, "/v1/members").await.status,
        StatusCode::FORBIDDEN
    );
    // A browser navigation to a refused route gets the themed page's sentence.
    let navigation = send(
        &app,
        "GET",
        "/v1/members",
        Some(&viewer_cookie),
        "",
        &[
            ("sec-fetch-mode", "navigate"),
            ("sec-fetch-dest", "document"),
            ("accept", "text/html"),
        ],
    )
    .await;
    assert_eq!(navigation.status, StatusCode::FORBIDDEN);
    check(
        &mut checks,
        "the viewer walk over the routes: /canvas renders for the viewer with the view-only sentence and without Create app, Ask, Shape, the composer bar or Share (the member's canvas has them), the record listing route answers 200, and compose, Ask, Shape, /v1/act, a command, a share, the organization share, an agent token, a folder, an upload plan, the folder build and the folder question each answer 403 viewer_read_only before any work while the member beside them is not refused for the role; the share dialog and the member list refuse too; the footprint is unchanged",
    );

    // --- 3. /admin over real rows -----------------------------------------------------
    let owner_cookie = mint(OWNER).await;
    let manager_cookie = mint(MANAGER).await;
    let member_cookie = mint(MEMBER).await;
    let post_form = |cookie: &str, uri: &str, body: String| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = uri.to_owned();
        async move { send(&app, "POST", &uri, Some(&cookie), &body, &FORM).await }
    };
    let unknown = send(&app, "GET", "/elsewhere", None, "", &[]).await;
    for (who, cookie) in [("member", &member_cookie), ("viewer", &viewer_cookie)] {
        let page = get(cookie, "/admin").await;
        assert_eq!(page.status, StatusCode::NOT_FOUND, "{who}");
        assert_eq!(page.body, unknown.body, "{who}: the uniform 404");
    }
    let options = |body: &str, principal: &str| -> Vec<String> {
        let row_start = body
            .find(&format!("data-member=\"{principal}\""))
            .unwrap_or_else(|| panic!("{principal} in the page"));
        let row = &body[row_start..];
        let row = &row[..row.find("</tr>").unwrap()];
        row.split("<option value=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(str::to_owned)
            .collect()
    };
    let owner_page = get(&owner_cookie, "/admin").await;
    assert_eq!(owner_page.status, StatusCode::OK, "{}", owner_page.body);
    assert!(owner_page.body.contains("<th scope=\"col\">Role</th>"));
    assert_eq!(
        options(&owner_page.body, T_MEMBER),
        ["owner", "admin", "manager", "member", "viewer"]
    );
    assert_eq!(
        options(&owner_page.body, T_ADMIN),
        ["owner", "admin", "manager", "member", "viewer"]
    );
    assert!(owner_page
        .body
        .contains("I understand this changes owner or admin access"));
    let manager_page = get(&manager_cookie, "/admin").await;
    assert_eq!(manager_page.status, StatusCode::OK);
    assert_eq!(options(&manager_page.body, T_MEMBER), ["member", "viewer"]);
    assert_eq!(options(&manager_page.body, T_VIEWER), ["member", "viewer"]);
    assert!(
        options(&manager_page.body, T_ADMIN).is_empty(),
        "no select on an admin's row for a manager"
    );
    assert!(options(&manager_page.body, T_OWNER).is_empty());
    assert!(!manager_page
        .body
        .contains("I understand this changes owner or admin access"));
    assert!(
        !manager_page
            .body
            .contains(&format!("action=\"/admin/members/{T_ADMIN}/remove\"")),
        "no Remove on an admin's row for a manager"
    );
    assert!(manager_page
        .body
        .contains(&format!("action=\"/admin/members/{T_MEMBER}/remove\"")));
    // The forged form: a manager posting admin for a member, and owner for
    // themselves; the token must be the role form's and the session's own.
    let manager_token = auth.csrf.token(csrf::Form::OrgAdminRole, &manager_cookie);
    let owner_token = auth.csrf.token(csrf::Form::OrgAdminRole, &owner_cookie);
    let remove_token = auth.csrf.token(csrf::Form::OrgAdminRemove, &manager_cookie);
    let role_path = |principal: &str| format!("/admin/members/{principal}/role");
    let before = footprint(&fixture).await;
    let events = role_events(&fixture).await;
    let forged = post_form(
        &manager_cookie,
        &role_path(T_MEMBER),
        form(&[
            ("csrf", &manager_token),
            ("role", "admin"),
            ("from", "member"),
            ("confirm", "role"),
        ]),
    )
    .await;
    assert_eq!(
        forged.headers["location"].to_str().unwrap(),
        "/admin?notice=role_not_allowed"
    );
    // Without the `from` hint the definer refuses it the same way.
    let forged = post_form(
        &manager_cookie,
        &role_path(T_MEMBER),
        form(&[
            ("csrf", &manager_token),
            ("role", "admin"),
            ("confirm", "role"),
        ]),
    )
    .await;
    assert_eq!(
        forged.headers["location"].to_str().unwrap(),
        "/admin?notice=role_not_allowed"
    );
    let forged = post_form(
        &manager_cookie,
        &role_path(MANAGER),
        form(&[
            ("csrf", &manager_token),
            ("role", "owner"),
            ("confirm", "role"),
        ]),
    )
    .await;
    assert_eq!(
        forged.headers["location"].to_str().unwrap(),
        "/admin?notice=role_self"
    );
    let forged = post_form(
        &manager_cookie,
        &role_path(T_ADMIN),
        form(&[
            ("csrf", &manager_token),
            ("role", "member"),
            ("confirm", "role"),
        ]),
    )
    .await;
    assert_eq!(
        forged.headers["location"].to_str().unwrap(),
        "/admin?notice=role_not_allowed"
    );
    assert_eq!(membership(&fixture, T_MEMBER).await.0, "member");
    assert_eq!(membership(&fixture, T_ADMIN).await.0, "admin");
    assert_eq!(membership(&fixture, MANAGER).await.0, "manager");
    // CSRF: another session's token, the Remove form's token and a cross-site post.
    let other_session = post_form(
        &manager_cookie,
        &role_path(T_MEMBER),
        form(&[("csrf", &owner_token), ("role", "viewer")]),
    )
    .await;
    assert_eq!(other_session.status, StatusCode::FORBIDDEN);
    let other_form = post_form(
        &manager_cookie,
        &role_path(T_MEMBER),
        form(&[("csrf", &remove_token), ("role", "viewer")]),
    )
    .await;
    assert_eq!(other_form.status, StatusCode::FORBIDDEN);
    let cross = send(
        &app,
        "POST",
        &role_path(T_MEMBER),
        Some(&manager_cookie),
        &form(&[("csrf", &manager_token), ("role", "viewer")]),
        &[("sec-fetch-site", "cross-site")],
    )
    .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let by_member = post_form(
        &member_cookie,
        &role_path(T_VIEWER),
        form(&[
            (
                "csrf",
                &auth.csrf.token(csrf::Form::OrgAdminRole, &member_cookie),
            ),
            ("role", "member"),
        ]),
    )
    .await;
    assert_eq!(by_member.status, StatusCode::NOT_FOUND);
    assert_eq!(by_member.body, unknown.body);
    assert_eq!(footprint(&fixture).await, before);
    assert_eq!(
        role_events(&fixture).await,
        events,
        "no forged form reached storage"
    );
    // The allowed change by the manager, and the owner's with the confirmation.
    let changed = post_form(
        &manager_cookie,
        &role_path(T_MEMBER),
        form(&[
            ("csrf", &manager_token),
            ("role", "viewer"),
            ("from", "member"),
        ]),
    )
    .await;
    assert_eq!(
        changed.headers["location"].to_str().unwrap(),
        "/admin?notice=role_changed"
    );
    assert_eq!(membership(&fixture, T_MEMBER).await.0, "viewer");
    let unconfirmed = post_form(
        &owner_cookie,
        &role_path(T_MEMBER),
        form(&[
            ("csrf", &owner_token),
            ("role", "admin"),
            ("from", "viewer"),
        ]),
    )
    .await;
    assert_eq!(
        unconfirmed.headers["location"].to_str().unwrap(),
        "/admin?notice=role_unconfirmed"
    );
    assert_eq!(membership(&fixture, T_MEMBER).await.0, "viewer");
    let promoted = post_form(
        &owner_cookie,
        &role_path(T_MEMBER),
        form(&[
            ("csrf", &owner_token),
            ("role", "admin"),
            ("from", "viewer"),
            ("confirm", "role"),
        ]),
    )
    .await;
    assert_eq!(
        promoted.headers["location"].to_str().unwrap(),
        "/admin?notice=role_changed"
    );
    assert_eq!(membership(&fixture, T_MEMBER).await.0, "admin");
    assert_eq!(role_events(&fixture).await, events + 2);
    let shown = get(&owner_cookie, "/admin?notice=role_changed").await;
    assert!(shown.body.contains("The role was changed."));
    let shown = get(&owner_cookie, "/admin?notice=role_last_owner").await;
    assert!(shown
        .body
        .contains("You can't change the last owner's role."));
    check(
        &mut checks,
        "/admin over real rows: a member and a viewer get the uniform 404; the owner's page offers every role on every row with the confirmation box, the manager's offers member and viewer on members and viewers only and no form or Remove on an admin's or an owner's row; a manager's forged post of admin (with or without the from hint), of owner on themselves (role_self) or of member on an admin, another session's token, the Remove form's token, a cross-site post and a member's post are refused with nothing written; the manager's member-to-viewer change and the owner's confirmed promotion to admin go through (two events), the unconfirmed one does not, and the notices read in plain words",
    );

    // --- the chain verifies ---------------------------------------------------------
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    check(
        &mut checks,
        "the admission chain, with every membership.role_changed event in it, recomputes from genesis to its head",
    );

    stores.close().await.unwrap();
    if let Some(path) = std::env::var_os("GENUI_ORG_ROLES_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"ADR-021 organization roles (APP-225, APP-226; migration 0025): the management ladder, the last owner, the epoch, the audit, the viewer walk in storage and over the routes, /admin over real rows","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Sessions are minted for tests; the hosted sign-in is not exercised.","The composer, the parser and the model are disabled: every AI route is refused for the viewer before them, and the member's answer only proves the refusal is the role's.","The data migration from admin_policy rows (zero, one and two holders) is app/runtime/test_admission_operator.py's RoleMigrationTests on a database stopped at schema 24."]})).unwrap()).unwrap();
    }
    println!("org-roles campaign: {} checks passed", checks.len());
}
