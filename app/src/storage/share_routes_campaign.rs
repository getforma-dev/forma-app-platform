//! APP-192 (trial plan D9): the share routes over real rows. Isolated test
//! executable only: local Docker, OpenSSL and the locked Python
//! environments, as the other storage campaigns.
//!
//! `GET /v1/work/{app}/{work}/share` (the dialog's read), `POST
//! /v1/work/{app}/{work}/share` with a role, and `POST
//! /v1/work/{app}/{work}/share/organization`, through the HTTP router with
//! minted workspace sessions. Storage's own semantics (the definers, the
//! reviewer rule, own collections, the organization-wide clause in every
//! read) are W4's `private-entries` campaign; this one proves the routes
//! answer what APP-192 says on the same kind of database, and every outcome
//! is asserted on the rows (`work_shares`, the permission tables and the
//! admission chain), never on a response alone.
//!
//! The app is the private-entries campaign's general definition (an own
//! collection and a shared one, no source dependency). Members: Owner (the
//! app's owner and the work's creator), Rev (given the reviewer role over
//! the route), Hal (an operator-granted `share_work` holder who is a
//! contributor, not a reviewer), Ana (shared with by Hal), Nia (no access,
//! never a sharer), Fay (admitted after the organization-wide share), Vic (a
//! `share_work` holder who may only view) and Oz (a member of another
//! organization).
use super::{
    admission_qualification::operator,
    command_qualification::{send, vault_token, Reply, TestPages},
    definition_qualification::{grant, request, revoke, POLICY},
    private_entries_campaign,
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER},
    *,
};
use crate::{
    admission,
    auth::{Auth, Pages},
    http, sharing,
};
use axum::{http::StatusCode, Router};
use serde_json::{json, Value};
use std::sync::Arc;

/// The seeded fixture space (tenant_seed.py).
const SPACE: &str = "00000000-0000-4000-8000-000000000384";
/// tenant_seed's revoked member of the organization.
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
const OWNER: &str = "00000000-0000-4000-8000-000000000b01";
const REV: &str = "00000000-0000-4000-8000-000000000b02";
const HAL: &str = "00000000-0000-4000-8000-000000000b03";
const ANA: &str = "00000000-0000-4000-8000-000000000b04";
const NIA: &str = "00000000-0000-4000-8000-000000000b05";
const FAY: &str = "00000000-0000-4000-8000-000000000b06";
const VIC: &str = "00000000-0000-4000-8000-000000000b07";
/// A member of the other fixture organization only.
const OZ: &str = "00000000-0000-4000-8000-000000000b08";
/// Well formed, never a member anywhere.
const NOBODY: &str = "00000000-0000-4000-8000-000000000bff";
const JSON: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/json"),
];

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_share_routes_campaign() {
    std::thread::Builder::new()
        .name("app192-share-routes-campaign".into())
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
/// active member of `organization` with a live entitlement.
fn admit_member(fixture: &Fixture, organization: &str, principal: &str) {
    let tail = &principal[principal.len() - 12..];
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{organization}',true); INSERT INTO genui.principals VALUES ('{organization}','{principal}','app008-fixture-only','ten_{organization}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{organization}','{principal}','active',1); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{organization}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
}
/// The dialog's listing of people, as `(label, role)` sorted by label.
fn listed(dialog: &Value) -> Vec<(String, String)> {
    let mut people: Vec<(String, String)> = dialog["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["label"].as_str().unwrap().to_owned(),
                m["role"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    people.sort();
    people
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

/// What a share route may write: the permission rows, the share rows (and
/// the live organization-wide ones), and the admission chain's grant and
/// revoke events.
async fn footprint(fixture: &Fixture) -> [i64; 6] {
    let mut counts = [0; 6];
    for (slot, sql) in [
        "SELECT count(*) FROM genui.app_permissions".to_owned(),
        "SELECT count(*) FROM genui.work_permissions".to_owned(),
        "SELECT count(*) FROM genui.work_shares".to_owned(),
        "SELECT count(*) FROM genui.work_shares WHERE grantee_id IS NULL AND revoked_at IS NULL".to_owned(),
        format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='permission.granted'"),
        format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='permission.revoked'"),
    ]
    .into_iter()
    .enumerate()
    {
        counts[slot] = scalar(fixture, &sql).await;
    }
    counts
}

/// A member's permissions on the work, sorted.
async fn work_permissions(fixture: &Fixture, principal: &str, work: &str) -> Vec<String> {
    fixture
        .operator()
        .await
        .client
        .query(&format!("SELECT permission FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{principal}' AND work_id='{work}' ORDER BY permission"), &[])
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect()
}

/// The live organization-wide share row's role, if any.
async fn everyone_role(fixture: &Fixture, work: &str) -> Option<String> {
    fixture
        .operator()
        .await
        .client
        .query(&format!("SELECT role FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id IS NULL AND revoked_at IS NULL"), &[])
        .await
        .unwrap()
        .first()
        .map(|row| row.get(0))
}

fn share_body(grantee: &str, role: &str) -> String {
    json!({"grantee_principal_id": grantee, "role": role}).to_string()
}
fn role_body(role: &str) -> String {
    json!({"role": role}).to_string()
}
fn code(reply: &Reply) -> (StatusCode, String) {
    (
        reply.status,
        reply.json()["code"].as_str().unwrap_or_default().to_owned(),
    )
}
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
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
    for member in [OWNER, REV, HAL, ANA, NIA, VIC] {
        admit_member(&fixture, ORG, member);
    }
    admit_member(&fixture, OTHER, OZ);
    let owner = request(&stores, ORG, OWNER, 1);

    // --- the app and its work, through the admitted path --------------------
    let definition = private_entries_campaign::definition();
    let app_id = definition["app_id"].as_str().unwrap().to_owned();
    let app = app_id.as_str();
    operator(
        &fixture,
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
        grant(&fixture, OWNER, permission, &["--app", app]);
    }
    let admitted = admission::admit_definition(&serde_json::to_vec(&definition).unwrap())
        .expect("the campaign's definition admits");
    let revision = stores
        .workspace
        .create_definition_revision(&owner, app, &admitted)
        .await
        .unwrap();
    let PublishResult::Published(publication) = stores
        .control
        .publish(&owner, app, &revision.revision, POLICY)
        .await
        .unwrap()
    else {
        panic!("uncontended publication is not stale");
    };
    let saved = stores
        .workspace
        .create_work(
            &owner,
            app,
            &publication.publication_id,
            "Claims and notes (synthetic share-routes campaign)",
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
    let work = saved.work.as_str();
    // The names members gave themselves (0023), the labels the dialog shows.
    for (principal, name) in [(REV, "Rita Okafor"), (HAL, "Hal Moreno"), (ANA, "Ana Diaz")] {
        stores
            .workspace
            .set_display_name(&request(&stores, ORG, principal, 1), name)
            .await
            .unwrap();
    }

    // --- the HTTP stand -----------------------------------------------------
    let pages: Arc<dyn Pages> = Arc::new(TestPages);
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), pages).unwrap());
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
    let owner_cookie = mint(OWNER).await;
    let rev_cookie = mint(REV).await;
    let hal_cookie = mint(HAL).await;
    let nia_cookie = mint(NIA).await;
    let vic_cookie = mint(VIC).await;
    let oz_cookie = {
        let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
        stores
            .workspace
            .mint_session_for_tests(OTHER, OZ, &reference)
            .await
            .unwrap()
    };
    let router: Router = http::uniform(sharing::router(auth.clone()));
    let share_uri = format!("/v1/work/{app}/{work}/share");
    let everyone_uri = format!("{share_uri}/organization");
    let missing_uri = format!("/v1/work/{app}/{NOBODY}/share");
    let get = |cookie: Option<&str>, uri: &str| {
        let router = router.clone();
        let cookie = cookie.map(str::to_owned);
        let uri = uri.to_owned();
        async move { send(&router, "GET", &uri, cookie.as_deref(), "", &[]).await }
    };
    let post = |cookie: Option<&str>, uri: &str, body: String| {
        let router = router.clone();
        let cookie = cookie.map(str::to_owned);
        let uri = uri.to_owned();
        async move { send(&router, "POST", &uri, cookie.as_deref(), &body, &JSON).await }
    };

    // --- the dialog's read --------------------------------------------------
    let dialog = get(Some(&owner_cookie), &share_uri).await;
    assert_eq!(dialog.status, StatusCode::OK, "{}", dialog.body);
    assert_eq!(
        dialog.json(),
        json!({
            "app_id": app, "work_id": work,
            "roles": ["viewer", "contributor", "reviewer"],
            "can_revoke_members": true,
            "private_lists": true,
            "organization": {"available": true, "role": null},
            "members": [],
        })
    );
    assert_eq!(dialog.headers["cache-control"], "no-store");
    let anonymous = get(None, &share_uri).await;
    let stranger = get(Some(&nia_cookie), &share_uri).await;
    let missing = get(Some(&owner_cookie), &missing_uri).await;
    for refused in [&anonymous, &stranger, &missing] {
        assert_eq!(refused.status, StatusCode::NOT_FOUND);
        assert_eq!(refused.body, anonymous.body, "the uniform 404");
    }
    check(
        &mut checks,
        "GET /v1/work/{app}/{work}/share: the app's owner, the work's creator, reads the roles it may grant (viewer, contributor, reviewer), that it may remove a person's access, that the app has private lists, the organization-wide share (available, none live) and no members yet, no-store; an anonymous caller, a member without share authority and a nonexistent work get the byte-identical uniform 404",
    );

    // --- one encoding, closed bodies, fetch metadata ------------------------
    let before = footprint(&fixture).await;
    for (uri, body) in [
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "record_write": true}).to_string(),
        ),
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "record_write": false}).to_string(),
        ),
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "role": "viewer", "record_write": false})
                .to_string(),
        ),
        (&share_uri, json!({"grantee_principal_id": REV}).to_string()),
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "role": "owner"}).to_string(),
        ),
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "role": "Reviewer"}).to_string(),
        ),
        (
            &share_uri,
            json!({"grantee_principal_id": REV, "role": "viewer", "permission": "share_work"})
                .to_string(),
        ),
        (&share_uri, share_body(REV, "None")),
        (&everyone_uri, role_body("everyone")),
        (
            &everyone_uri,
            json!({"role": "viewer", "grantee_principal_id": REV}).to_string(),
        ),
        (&everyone_uri, json!({"record_write": true}).to_string()),
        (&everyone_uri, "{}".to_owned()),
    ] {
        let refused = post(Some(&owner_cookie), uri, body.clone()).await;
        assert_eq!(
            code(&refused),
            (StatusCode::BAD_REQUEST, "malformed_share".to_owned()),
            "{uri} {body}"
        );
    }
    let form = send(
        &router,
        "POST",
        &share_uri,
        Some(&owner_cookie),
        &share_body(REV, "viewer"),
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "text/plain"),
        ],
    )
    .await;
    assert_eq!(
        code(&form),
        (StatusCode::BAD_REQUEST, "malformed_share".to_owned())
    );
    for uri in [&share_uri, &everyone_uri] {
        let body = if uri == &share_uri {
            share_body(REV, "reviewer")
        } else {
            role_body("viewer")
        };
        for headers in [
            vec![
                ("sec-fetch-site", "cross-site"),
                ("content-type", "application/json"),
            ],
            vec![
                ("sec-fetch-site", "same-site"),
                ("content-type", "application/json"),
            ],
            vec![("content-type", "application/json")],
            vec![
                ("origin", "https://elsewhere.example"),
                ("content-type", "application/json"),
            ],
        ] {
            let refused = send(&router, "POST", uri, Some(&owner_cookie), &body, &headers).await;
            assert_eq!(
                code(&refused),
                (StatusCode::FORBIDDEN, "cross_site".to_owned()),
                "{uri} {headers:?}"
            );
        }
    }
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "one encoding: a share body with the earlier `record_write` flag (alone or beside a role), without a role, with an unknown role or spelling, an unknown member, `None` for a person, and an organization body naming anything but a role, are 400 `malformed_share`; a non-JSON body is 400; a cross-site, same-site, metadata-less or foreign-Origin POST on both routes is 403 `cross_site`; nothing is written",
    );

    // --- a personal reviewer share by the owner -----------------------------
    let shared = post(Some(&owner_cookie), &share_uri, share_body(REV, "reviewer")).await;
    assert_eq!(shared.status, StatusCode::OK, "{}", shared.body);
    assert_eq!(shared.json()["grantee_principal_id"], REV);
    assert_eq!(
        work_permissions(&fixture, REV, work).await,
        ["record_read", "record_review", "record_write"]
    );
    let after = footprint(&fixture).await;
    assert_eq!(after[2], before[2] + 1, "one share row");
    assert_eq!(after[4], before[4] + 1, "one permission.granted event");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares s JOIN genui.audit_events e ON e.organization_id=s.organization_id AND e.event_id=s.audit_event_id AND e.kind='permission.granted' WHERE s.organization_id='{ORG}' AND s.work_id='{work}' AND s.grantee_id='{REV}' AND s.sharer_id='{OWNER}' AND s.role='reviewer' AND s.revoked_at IS NULL")).await,
        1
    );
    let dialog = get(Some(&owner_cookie), &share_uri).await.json();
    assert_eq!(
        dialog["members"],
        json!([{"label": "Rita Okafor", "principal_id": REV, "role": "reviewer"}])
    );
    assert!(dialog["members"]
        .as_array()
        .unwrap()
        .iter()
        .all(|m| keys(m) == ["label", "principal_id", "role"]));
    check(
        &mut checks,
        "the owner shares with a member as reviewer over the route: record_read, record_write and record_review are copied, one `work_shares` row with role reviewer references one `permission.granted` event, and the dialog lists the member by the name they gave themselves with the role",
    );

    // --- a sharer who is not a reviewer -------------------------------------
    // An owner-level `share_work` holder (operator stand-in, as the sharing
    // campaign), shared with as a contributor: it may not grant reviewer.
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.work_permissions VALUES ('{ORG}','{HAL}','{app}','{work}','share_work'); COMMIT;"), "genui");
    let hal_shared = post(
        Some(&owner_cookie),
        &share_uri,
        share_body(HAL, "contributor"),
    )
    .await;
    assert_eq!(hal_shared.status, StatusCode::OK, "{}", hal_shared.body);
    let hal_dialog = get(Some(&hal_cookie), &share_uri).await;
    assert_eq!(hal_dialog.status, StatusCode::OK, "{}", hal_dialog.body);
    assert_eq!(hal_dialog.json()["roles"], json!(["viewer", "contributor"]));
    // Removing one person's access is the app owner's alone (W4's rule).
    assert_eq!(hal_dialog.json()["can_revoke_members"], false);
    let before = footprint(&fixture).await;
    // The refusal concerns the sharer alone: the same bytes whoever the
    // grantee is (a member, nobody, a revoked member, the sharer itself).
    let refused = post(Some(&hal_cookie), &share_uri, share_body(ANA, "reviewer")).await;
    assert_eq!(
        (refused.status, refused.body.as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "{\"code\":\"reviewer_not_allowed\",\"status\":422}"
        )
    );
    for grantee in [NOBODY, REVOKED, HAL, REV] {
        let again = post(
            Some(&hal_cookie),
            &share_uri,
            share_body(grantee, "reviewer"),
        )
        .await;
        assert_eq!(again.body, refused.body, "{grantee}");
    }
    let everyone = post(Some(&hal_cookie), &everyone_uri, role_body("reviewer")).await;
    assert_eq!(everyone.body, refused.body);
    assert_eq!(footprint(&fixture).await, before);
    assert_eq!(
        work_permissions(&fixture, ANA, work).await,
        Vec::<String>::new()
    );
    let viewer = post(
        Some(&hal_cookie),
        &share_uri,
        share_body(ANA, "contributor"),
    )
    .await;
    assert_eq!(viewer.status, StatusCode::OK, "{}", viewer.body);
    assert_eq!(
        work_permissions(&fixture, ANA, work).await,
        ["record_read", "record_write"]
    );
    check(
        &mut checks,
        "a `share_work` holder who is a contributor but not a reviewer reads roles viewer and contributor; its reviewer grant, to a person or to everyone, is 422 `reviewer_not_allowed`, byte-identical whoever the grantee is (a member, nobody, a revoked member, itself), writing nothing; its contributor share lands (record_read, record_write)",
    );

    // --- a sharer who may only view: contributor_not_allowed ---------------
    // A `share_work` holder (operator stand-in) whose own access is a viewer
    // share: it may share as a viewer only, and is told so in plain terms.
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.work_permissions VALUES ('{ORG}','{VIC}','{app}','{work}','share_work'); COMMIT;"), "genui");
    let vic_shared = post(Some(&owner_cookie), &share_uri, share_body(VIC, "viewer")).await;
    assert_eq!(vic_shared.status, StatusCode::OK, "{}", vic_shared.body);
    let vic_dialog = get(Some(&vic_cookie), &share_uri).await;
    assert_eq!(vic_dialog.status, StatusCode::OK, "{}", vic_dialog.body);
    assert_eq!(vic_dialog.json()["roles"], json!(["viewer"]));
    let before = footprint(&fixture).await;
    let refused = post(
        Some(&vic_cookie),
        &share_uri,
        share_body(NIA, "contributor"),
    )
    .await;
    assert_eq!(
        (refused.status, refused.body.as_str()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "{\"code\":\"contributor_not_allowed\",\"status\":422}"
        )
    );
    for grantee in [NOBODY, REVOKED, VIC, ANA] {
        let again = post(
            Some(&vic_cookie),
            &share_uri,
            share_body(grantee, "contributor"),
        )
        .await;
        assert_eq!(again.body, refused.body, "{grantee}");
    }
    let everyone = post(Some(&vic_cookie), &everyone_uri, role_body("contributor")).await;
    assert_eq!(everyone.body, refused.body);
    let reviewer = post(Some(&vic_cookie), &everyone_uri, role_body("reviewer")).await;
    assert_eq!(
        reviewer.body,
        "{\"code\":\"reviewer_not_allowed\",\"status\":422}"
    );
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "a `share_work` holder whose own access is a viewer share reads roles [viewer] only; its contributor grant, to a person or to everyone, is 422 `contributor_not_allowed`, byte-identical whoever the grantee is, and its reviewer grant to everyone 422 `reviewer_not_allowed`; nothing is written",
    );

    // --- no share authority: the uniform 404 --------------------------------
    let before = footprint(&fixture).await;
    let anonymous = post(None, &share_uri, share_body(ANA, "viewer")).await;
    assert_eq!(anonymous.status, StatusCode::NOT_FOUND);
    for (cookie, uri, body) in [
        (&nia_cookie, &share_uri, share_body(ANA, "viewer")),
        (&nia_cookie, &everyone_uri, role_body("viewer")),
        (&nia_cookie, &everyone_uri, role_body("none")),
        // A reviewer without share authority shares nothing, and its
        // reviewer request is the uniform 404 too, not a role refusal.
        (&rev_cookie, &share_uri, share_body(NIA, "viewer")),
        (&rev_cookie, &share_uri, share_body(NIA, "reviewer")),
        (&rev_cookie, &everyone_uri, role_body("reviewer")),
        (&rev_cookie, &everyone_uri, role_body("none")),
        (&owner_cookie, &missing_uri, share_body(ANA, "viewer")),
        (
            &owner_cookie,
            &format!("{missing_uri}/organization"),
            role_body("viewer"),
        ),
        // A member of another organization, on every route.
        (&oz_cookie, &share_uri, share_body(ANA, "viewer")),
        (&oz_cookie, &everyone_uri, role_body("viewer")),
        (&oz_cookie, &everyone_uri, role_body("none")),
        // A grantee who is nobody, revoked, of another organization, or the
        // sharer.
        (&owner_cookie, &share_uri, share_body(NOBODY, "viewer")),
        (&owner_cookie, &share_uri, share_body(OZ, "viewer")),
        (&owner_cookie, &share_uri, share_body(REVOKED, "viewer")),
        (&owner_cookie, &share_uri, share_body(OWNER, "viewer")),
    ] {
        let refused = post(Some(cookie), uri, body.clone()).await;
        assert_eq!(refused.status, StatusCode::NOT_FOUND, "{uri} {body}");
        assert_eq!(refused.body, anonymous.body, "{uri} {body}");
    }
    assert_eq!(
        get(Some(&rev_cookie), &share_uri).await.body,
        anonymous.body
    );
    assert_eq!(get(Some(&oz_cookie), &share_uri).await.body, anonymous.body);
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "without share authority (a member with no access, a reviewer who is neither the creator nor a `share_work` holder, a member of another organization), for a nonexistent work, and for a grantee who is nobody, revoked, of another organization or the sharer, both POSTs and the dialog read answer the byte-identical uniform 404 (a reviewer's reviewer request included), writing nothing",
    );

    // --- the body bound and the member write window ------------------------
    let before = footprint(&fixture).await;
    let padding = "x".repeat(http::JSON_BODY_LIMIT + 1);
    for uri in [&share_uri, &everyone_uri] {
        let large = post(
            Some(&owner_cookie),
            uri,
            json!({"role": "viewer", "padding": padding}).to_string(),
        )
        .await;
        assert_eq!(
            code(&large),
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large".to_owned()
            ),
            "{uri}"
        );
    }
    // Nia's member write window, used up the way 120 POSTs would.
    while auth.limits.admit_member_write(ORG, NIA).is_ok() {}
    for (uri, body) in [
        (&share_uri, share_body(ANA, "viewer")),
        (&everyone_uri, role_body("viewer")),
    ] {
        let limited = post(Some(&nia_cookie), uri, body).await;
        assert_eq!(
            code(&limited),
            (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests".to_owned()
            ),
            "{uri}"
        );
        assert!(limited.headers.contains_key("retry-after"));
    }
    // The dialog's read is a read: it is not counted against the window.
    assert_eq!(
        get(Some(&nia_cookie), &share_uri).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(footprint(&fixture).await, before);
    check(
        &mut checks,
        "a body over the JSON limit is 413 `payload_too_large` on both POSTs; a member whose write window is used up gets 429 `too_many_requests` with Retry-After on both POSTs before anything else is judged; the dialog's read is not counted; nothing is written",
    );

    // --- everyone in the organization: set, replace, revoke -----------------
    let nia = request(&stores, ORG, NIA, 1);
    assert_eq!(
        stores.data.can_open(&nia, app, work).await.err(),
        Some(Error::Denied)
    );
    let before = footprint(&fixture).await;
    let set = post(Some(&owner_cookie), &everyone_uri, role_body("viewer")).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    assert_eq!(
        set.json(),
        json!({"app_id": app, "work_id": work, "role": "viewer"})
    );
    assert_eq!(
        everyone_role(&fixture, work).await.as_deref(),
        Some("viewer")
    );
    let after = footprint(&fixture).await;
    assert_eq!(
        after,
        [
            before[0],
            before[1],
            before[2] + 1,
            before[3] + 1,
            before[4] + 1,
            before[5]
        ],
        "one live row and one grant event; no personal permission row"
    );
    stores.data.can_open(&nia, app, work).await.unwrap();
    // A member of another organization is not covered.
    assert_eq!(
        stores
            .data
            .can_open(&request(&stores, OTHER, OZ, 1), app, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    admit_member(&fixture, ORG, FAY);
    let fay = request(&stores, ORG, FAY, 1);
    stores.data.can_open(&fay, app, work).await.unwrap();
    let replay = post(Some(&owner_cookie), &everyone_uri, role_body("viewer")).await;
    assert_eq!(replay.body, set.body);
    assert_eq!(footprint(&fixture).await, after, "a replay writes nothing");
    let dialog = get(Some(&owner_cookie), &share_uri).await.json();
    assert_eq!(
        dialog["organization"],
        json!({"available": true, "role": "viewer"})
    );
    check(
        &mut checks,
        "everyone in the organization as viewer over the route: one live organization-wide row and one `permission.granted` event, no personal permission row; a member with no grant and a member admitted afterwards can open the work, a member of another organization cannot; the same request again writes nothing; the dialog shows the role",
    );

    for (replaced_so_far, role) in [(1, "contributor"), (2, "reviewer")] {
        let before = footprint(&fixture).await;
        let replaced = post(Some(&owner_cookie), &everyone_uri, role_body(role)).await;
        assert_eq!(replaced.status, StatusCode::OK, "{}", replaced.body);
        assert_eq!(replaced.json()["role"], role);
        assert_eq!(everyone_role(&fixture, work).await.as_deref(), Some(role));
        assert_eq!(
            footprint(&fixture).await,
            [
                before[0],
                before[1],
                before[2] + 1,
                before[3],
                before[4] + 1,
                before[5] + 1
            ],
            "one revocation and one grant, still one live row"
        );
        // The replaced row is revoked by the owner, with its own event.
        assert_eq!(
            scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares s JOIN genui.audit_events e ON e.organization_id=s.organization_id AND e.event_id=s.revoke_event_id AND e.kind='permission.revoked' WHERE s.organization_id='{ORG}' AND s.work_id='{work}' AND s.grantee_id IS NULL AND s.revoked_by='{OWNER}'")).await,
            replaced_so_far
        );
    }
    let dialog = get(Some(&owner_cookie), &share_uri).await.json();
    assert_eq!(dialog["organization"]["role"], "reviewer");
    check(
        &mut checks,
        "replacing the organization-wide role (viewer → contributor → reviewer) over the route revokes the live row and grants the new one in one request: one `permission.revoked` and one `permission.granted` event each time, the revoked row naming the owner and its event, one live row throughout",
    );

    let before = footprint(&fixture).await;
    let stopped = post(Some(&owner_cookie), &everyone_uri, role_body("none")).await;
    assert_eq!(stopped.status, StatusCode::OK, "{}", stopped.body);
    assert_eq!(
        stopped.json(),
        json!({"app_id": app, "work_id": work, "role": "none"})
    );
    assert_eq!(everyone_role(&fixture, work).await, None);
    assert_eq!(
        footprint(&fixture).await,
        [
            before[0],
            before[1],
            before[2],
            before[3] - 1,
            before[4],
            before[5] + 1
        ]
    );
    for member in [&nia, &fay] {
        assert_eq!(
            stores.data.can_open(member, app, work).await.err(),
            Some(Error::Denied)
        );
    }
    // Personal shares are untouched by the organization-wide revocation.
    assert_eq!(
        work_permissions(&fixture, ANA, work).await,
        ["record_read", "record_write"]
    );
    let again = post(Some(&owner_cookie), &everyone_uri, role_body("none")).await;
    assert_eq!(again.body, stopped.body);
    assert_eq!(
        footprint(&fixture).await[5],
        before[5] + 1,
        "nothing to revoke"
    );
    let dialog = get(Some(&owner_cookie), &share_uri).await.json();
    assert_eq!(
        dialog["organization"],
        json!({"available": true, "role": null})
    );
    let mut labels: Vec<&str> = dialog["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["label"].as_str().unwrap())
        .collect();
    labels.sort();
    assert_eq!(
        labels,
        ["Ana Diaz", "Hal Moreno", "Member 00000000", "Rita Okafor"]
    );
    assert!(!dialog.to_string().contains("usr_"));
    check(
        &mut checks,
        "`none` ends the organization-wide share for all at once: the live row is revoked with one `permission.revoked` event, the member with no grant and the late member can no longer open the work, personal shares stay; `none` again writes nothing; the dialog lists the four personal shares by name (Vic has no name), never by provider identity",
    );

    // --- the listed role is the grantee's live permissions -----------------
    let people = || async { listed(&get(Some(&owner_cookie), &share_uri).await.json()) };
    let pair = |label: &str, role: &str| (label.to_owned(), role.to_owned());
    assert_eq!(
        people().await,
        [
            pair("Ana Diaz", "contributor"),
            pair("Hal Moreno", "contributor"),
            pair("Member 00000000", "viewer"),
            pair("Rita Okafor", "reviewer"),
        ]
    );
    // An operator revocation of Ana's record_write: she shows as a viewer.
    revoke(
        &fixture,
        ANA,
        "record_write",
        &["--app", app, "--work", work],
    );
    // An operator demotion of Rev (record_review removed; operator stand-in,
    // as revoke-permission has no record_review on this branch).
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); DELETE FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{REV}' AND work_id='{work}' AND permission='record_review'; COMMIT;"), "genui");
    assert_eq!(
        people().await,
        [
            pair("Ana Diaz", "viewer"),
            pair("Hal Moreno", "contributor"),
            pair("Member 00000000", "viewer"),
            pair("Rita Okafor", "contributor"),
        ]
    );
    // With record_read gone too, Ana's share row lists nothing.
    revoke(
        &fixture,
        ANA,
        "record_read",
        &["--app", app, "--work", work],
    );
    let after = people().await;
    assert!(
        !after.iter().any(|(label, _)| label == "Ana Diaz"),
        "{after:?}"
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id='{ANA}' AND revoked_at IS NULL")).await,
        1,
        "the share row is still live; the listing follows the permissions"
    );
    check(
        &mut checks,
        "the dialog lists each person with the role of their live permissions on the work, not of their share rows: after an operator revoke-permission of record_write Ana shows as a viewer, after a demotion Rev shows as a contributor, and once record_read is gone Ana is not listed although her share row is still live",
    );

    // --- a demotion over the route keeps the person listed -----------------
    // W4 (f1): a lower re-share takes the surplus away and revokes the
    // grantee's earlier share rows; when nothing new is granted no live row
    // remains, and the person must still be listed with the lower role.
    let before = footprint(&fixture).await;
    let demoted = post(Some(&owner_cookie), &share_uri, share_body(HAL, "viewer")).await;
    assert_eq!(demoted.status, StatusCode::OK, "{}", demoted.body);
    assert_eq!(
        work_permissions(&fixture, HAL, work).await,
        ["record_read", "share_work"]
    );
    let after = footprint(&fixture).await;
    assert_eq!(after[1], before[1] - 1, "record_write removed");
    assert_eq!(after[5], before[5] + 1, "one permission.revoked");
    assert_eq!(after[4], before[4], "nothing granted");
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id='{HAL}' AND revoked_at IS NULL")).await,
        0,
        "the demotion left Hal no live share row"
    );
    assert_eq!(
        people().await,
        [
            pair("Hal Moreno", "viewer"),
            pair("Member 00000000", "viewer"),
            pair("Rita Okafor", "contributor"),
        ]
    );
    check(
        &mut checks,
        "a demotion over the route (Hal re-shared as viewer) removes record_write with one permission.revoked and nothing granted, leaves Hal no live share row, and the dialog still lists him, now as a viewer",
    );

    // --- removing one person's access: the owner alone ---------------------
    let removal = share_body(REV, "none");
    let before = footprint(&fixture).await;
    for (cookie, body) in [
        // A share_work holder who is not the app's owner.
        (&hal_cookie, share_body(VIC, "none")),
        (&hal_cookie, removal.clone()),
        // A member without any share authority, another organization.
        (&rev_cookie, share_body(VIC, "none")),
        (&oz_cookie, removal.clone()),
        // The owner naming themselves.
        (&owner_cookie, share_body(OWNER, "none")),
    ] {
        let refused = post(Some(cookie), &share_uri, body.clone()).await;
        assert_eq!(refused.status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(refused.body, anonymous.body, "{body}");
    }
    let missing_removal = post(Some(&owner_cookie), &missing_uri, removal.clone()).await;
    assert_eq!(missing_removal.body, anonymous.body);
    assert_eq!(
        footprint(&fixture).await,
        before,
        "no refusal wrote anything"
    );
    let removed = post(Some(&owner_cookie), &share_uri, removal.clone()).await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    assert_eq!(
        removed.json(),
        json!({"app_id": app, "work_id": work, "grantee_principal_id": REV, "role": "none"})
    );
    assert_eq!(
        work_permissions(&fixture, REV, work).await,
        Vec::<String>::new()
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares WHERE organization_id='{ORG}' AND work_id='{work}' AND grantee_id='{REV}' AND revoked_at IS NULL")).await,
        0
    );
    let after = footprint(&fixture).await;
    assert_eq!(after[5], before[5] + 1, "one permission.revoked");
    assert_eq!(
        stores
            .data
            .can_open(&request(&stores, ORG, REV, 1), app, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(!people()
        .await
        .iter()
        .any(|(label, _)| label == "Rita Okafor"));
    // Again, and for someone who holds nothing: the same answer, nothing
    // written (nothing tells the owner what the person held).
    let replay = post(Some(&owner_cookie), &share_uri, removal.clone()).await;
    assert_eq!(replay.body, removed.body);
    let nobody = post(Some(&owner_cookie), &share_uri, share_body(NOBODY, "none")).await;
    assert_eq!(nobody.status, StatusCode::OK);
    assert_eq!(footprint(&fixture).await, after);
    check(
        &mut checks,
        "removing one person's access ({\"grantee_principal_id\",\"role\":\"none\"}): the app's owner removes Rev's record_read, record_write and record_review in one unit with one permission.revoked, his share rows are revoked, he can no longer open the work and is no longer listed; a share_work holder who is not the owner, a member without share authority, a member of another organization, the owner naming themselves and a nonexistent work get the byte-identical uniform 404 and write nothing; a repeat, or a person who holds nothing, answers 200 and writes nothing",
    );

    // --- MAJOR-1 at the route: a stop queued behind a role change ----------
    let set = post(Some(&owner_cookie), &everyone_uri, role_body("viewer")).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    // The owner's change to contributor, held uncommitted in a session of
    // the workspace role (it holds the per-work sharing lock).
    let mut held = fixture.operator().await;
    let change = held.client.transaction().await.unwrap();
    change
        .batch_execute("SET LOCAL ROLE genui_workspace")
        .await
        .unwrap();
    change
        .execute(
            "SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)",
            &[&ORG, &OWNER],
        )
        .await
        .unwrap();
    let rows: i64 = change
        .query_one(&format!("SELECT count(*) FROM genui_private.set_work_everyone('{ORG}','{OWNER}','{app}','{work}','contributor')"), &[])
        .await
        .unwrap()
        .get(0);
    assert!(rows > 0, "the held change is a grant");
    // Hal's "Stop sharing with everyone" over the route, in flight.
    let stop = tokio::spawn(post(Some(&hal_cookie), &everyone_uri, role_body("none")));
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    assert!(!stop.is_finished(), "the stop waits on the per-work lock");
    change.commit().await.unwrap();
    drop(held);
    let stopped = stop.await.unwrap();
    assert_eq!(stopped.status, StatusCode::OK, "{}", stopped.body);
    assert_eq!(stopped.json()["role"], "none");
    assert_eq!(
        everyone_role(&fixture, work).await,
        None,
        "the final state is the final answer: nothing is shared with everyone"
    );
    assert_eq!(
        stores.data.can_open(&nia, app, work).await.err(),
        Some(Error::Denied)
    );
    // The other order: a stop held, a share with everyone in flight.
    let set = post(Some(&owner_cookie), &everyone_uri, role_body("viewer")).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    let mut held = fixture.operator().await;
    let stop = held.client.transaction().await.unwrap();
    stop.batch_execute("SET LOCAL ROLE genui_workspace")
        .await
        .unwrap();
    stop.execute(
        "SELECT set_config('genui.organization_id',$1,true),set_config('genui.principal_id',$2,true),set_config('genui.membership_epoch','1',true)",
        &[&ORG, &OWNER],
    )
    .await
    .unwrap();
    stop.query_one(&format!("SELECT count(*) FROM genui_private.set_work_everyone('{ORG}','{OWNER}','{app}','{work}','none')"), &[])
        .await
        .unwrap();
    let share = tokio::spawn(post(Some(&hal_cookie), &everyone_uri, role_body("viewer")));
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    assert!(!share.is_finished(), "the share waits on the per-work lock");
    stop.commit().await.unwrap();
    drop(held);
    let shared = share.await.unwrap();
    assert_eq!(shared.status, StatusCode::OK, "{}", shared.body);
    assert_eq!(shared.json()["role"], "viewer");
    assert_eq!(
        everyone_role(&fixture, work).await.as_deref(),
        Some("viewer"),
        "the final state is the final answer: shared with everyone as viewer"
    );
    stores.data.can_open(&nia, app, work).await.unwrap();
    check(
        &mut checks,
        "MAJOR-1 at the route: with the owner's change to contributor held uncommitted, Hal's \"Stop sharing with everyone\" waits on the per-work lock and, once the change commits, stops the new share: it answers none and nothing is shared with everyone (Nia cannot open the work); in the other order, with a stop held, Hal's share as viewer waits, then answers viewer and the share with everyone is live as viewer",
    );

    stores.close().await.unwrap();
    if let Some(path) = std::env::var_os("GENUI_SHARE_ROUTES_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"APP-192 (trial plan D9): the share routes with roles and the organization-wide share, over real rows (W2b)","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Sessions are minted for tests; the hosted sign-in is not exercised.","Storage semantics (definers, reviewer rule, own collections, the organization-wide clause in every read) are W4's private-entries campaign.","A personal share is not revoked over a route here: the owner-side revoke is W4's pending API."]})).unwrap()).unwrap();
    }
    println!("share-routes campaign: {} checks passed", checks.len());
}
