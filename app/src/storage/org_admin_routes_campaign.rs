//! Organization admin routes campaign (trial plan D4 and D6), over real rows:
//! `GET /admin`, `POST /admin/members/{principal}/remove`, `POST /me/name`
//! and the canvas's Admin link and name prompt, through the composed HTTP
//! routes with minted workspace sessions and the native pages from
//! `admin/dist`. Storage's own semantics (the definers, the audit chain, the
//! counts) are W1's `org_admin_campaign`; this campaign proves the routes
//! answer what the plan says on the same database.
//!
//! Members: tenant_seed's `admin_policy` holder `ADMIN` and the active
//! member `PEER` (no permissions of its own), both of the fixture
//! organization.
use super::{
    admission_qualification::operator,
    command_qualification::{send, vault_token, Reply, TestPages},
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER},
    *,
};
use crate::{
    auth::{csrf, Auth, Pages},
    http, member_name, org_admin,
    query::{
        canvas::{self, CanvasProps, SsrCanvasPages},
        routes::tests::RecordingPages,
        Work,
    },
};
use axum::{http::StatusCode, Router};
use std::sync::Arc;

#[derive(rust_embed::Embed)]
#[folder = "admin/dist/"]
struct Assets;

/// tenant_seed's `admin_policy` holder, active at epoch 1.
const ADMIN: &str = "00000000-0000-4000-8000-000000000f00";
/// A seeded active member with no permissions of its own.
const PEER: &str = "00000000-0000-4000-8000-00000000006f";
/// A member of the canary organization only, created by this campaign.
const OUTSIDER: &str = "00000000-0000-4000-8000-000000000ab0";
/// A member of the fixture organization only, created by this campaign
/// (tenant_seed's members belong to both organizations).
const INSIDER: &str = "00000000-0000-4000-8000-000000000ab2";
const FORM: [(&str, &str); 2] = [
    ("sec-fetch-site", "same-origin"),
    ("content-type", "application/x-www-form-urlencoded"),
];

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_org_admin_routes_campaign() {
    std::thread::Builder::new()
        .name("trial-org-admin-routes-campaign".into())
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
async fn text(fixture: &Fixture, sql: &str) -> Option<String> {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// The member rows the routes may change: status, epoch and display name.
async fn members(fixture: &Fixture) -> Vec<(String, String, i64, Option<String>)> {
    let operator = fixture.operator().await;
    operator
        .client
        .query(&format!("SELECT principal_id::text,status,membership_epoch,display_name FROM genui.memberships WHERE organization_id='{ORG}' ORDER BY principal_id"), &[])
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
        .collect()
}
fn location(reply: &Reply) -> &str {
    reply.headers["location"].to_str().unwrap()
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
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    // A member of the canary organization only (operator-channel stand-in,
    // as W1's org_admin_campaign seeds it).
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{OTHER}',true); INSERT INTO genui.principals VALUES ('{OTHER}','{OUTSIDER}','app008-fixture-only','ten_{OTHER}','usr_{OUTSIDER}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{OTHER}','{OUTSIDER}','active',1); INSERT INTO genui.entitlements VALUES ('{OTHER}','{OUTSIDER}','0000e000-0000-4000-8000-000000000ab0','active','member_invite',now(),now()+interval '30 days',NULL); COMMIT;"), "genui");
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{INSIDER}','app008-fixture-only','ten_{ORG}','usr_{INSIDER}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{ORG}','{INSIDER}','active',1); INSERT INTO genui.entitlements VALUES ('{ORG}','{INSIDER}','0000e000-0000-4000-8000-000000000ab2','active','member_invite',now(),now()+interval '30 days',NULL); COMMIT;"), "genui");
    let mut checks = Vec::new();
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let pages: Arc<dyn Pages> = Arc::new(TestPages);
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), pages).unwrap());
    let mint_in = |organization: &'static str, principal: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            stores
                .workspace
                .mint_session_for_tests(organization, principal, &reference)
                .await
                .unwrap()
        }
    };
    let admin = mint_in(ORG, ADMIN).await;
    let peer = mint_in(ORG, PEER).await;
    let outsider = mint_in(OTHER, OUTSIDER).await;
    // tenant_seed's admin is also the canary organization's admin.
    let other_admin = mint_in(OTHER, ADMIN).await;

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
    let work = Arc::new(Work::new(
        auth.clone(),
        None,
        Arc::new(RecordingPages::default()),
    ));
    let app = http::uniform_with_many(
        Router::new()
            .merge(org_admin::router(Arc::new(org_admin::OrgAdmin {
                auth: auth.clone(),
                pages: admin_pages,
            })))
            .merge(member_name::router(auth.clone())),
        vec![(
            canvas::PATH,
            canvas::router(Arc::new(canvas::Canvas {
                work,
                pages: canvas_pages,
                live_events: String::new(),
                demo_organizations: Default::default(),
            })),
        )],
    );
    let get = |cookie: &str, uri: &str| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = uri.to_owned();
        async move { send(&app, "GET", &uri, Some(&cookie), "", &[]).await }
    };
    let post = |cookie: &str, uri: &str, body: String| {
        let app = app.clone();
        let cookie = cookie.to_owned();
        let uri = uri.to_owned();
        async move { send(&app, "POST", &uri, Some(&cookie), &body, &FORM).await }
    };
    let remove_peer = format!("/admin/members/{PEER}/remove");
    let remove_admin = format!("/admin/members/{ADMIN}/remove");
    let before = members(&fixture).await;

    // --- The admin sees the table; anyone else gets the uniform 404 ---------
    let unknown = send(&app, "GET", "/elsewhere", None, "", &[]).await;
    let page = get(&admin, "/admin").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains(&format!("data-member=\"{PEER}\"")));
    assert!(page.body.contains(&format!("data-member=\"{ADMIN}\"")));
    assert!(page.body.contains(&format!("action=\"{remove_peer}\"")));
    assert!(
        !page.body.contains(&format!("action=\"{remove_admin}\"")),
        "no Remove on the admin's own row (review m5)"
    );
    assert!(page.body.contains("href=\"mailto:feedback@getforma.dev\""));
    assert!(!page.body.contains("<script src"));
    // --- Another organization's members (review m3) -------------------------
    let outsider_page = get(&outsider, "/admin").await;
    assert_eq!(outsider_page.status, StatusCode::NOT_FOUND);
    assert_eq!(outsider_page.body, unknown.body, "the uniform 404");
    let outsider_token = auth.csrf.token(csrf::Form::OrgAdminRemove, &outsider);
    let outsider_remove = post(
        &outsider,
        &remove_peer,
        form(&[("csrf", &outsider_token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(outsider_remove.status, StatusCode::NOT_FOUND);
    assert_eq!(outsider_remove.body, unknown.body, "the uniform 404");
    // The other organization's own admin sees only its own members, and
    // cannot remove a member of this organization.
    let other_page = get(&other_admin, "/admin").await;
    assert_eq!(other_page.status, StatusCode::OK, "{}", other_page.body);
    assert!(other_page
        .body
        .contains(&format!("data-member=\"{OUTSIDER}\"")));
    let operator_client = fixture.operator().await;
    let other_members: Vec<String> = operator_client
        .client
        .query(
            &format!(
                "SELECT principal_id::text FROM genui.memberships WHERE organization_id='{OTHER}'"
            ),
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    let listed: Vec<&str> = other_page
        .body
        .split("data-member=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();
    assert!(!listed.is_empty());
    assert!(
        listed.iter().all(|p| other_members.iter().any(|m| m == p)),
        "only the canary organization's members: {listed:?}"
    );
    assert!(
        !listed.contains(&INSIDER),
        "a member of this organization only"
    );
    drop(operator_client);
    let this_only = INSIDER;
    let other_token = auth.csrf.token(csrf::Form::OrgAdminRemove, &other_admin);
    let across = post(
        &other_admin,
        &format!("/admin/members/{this_only}/remove"),
        form(&[("csrf", &other_token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(location(&across), "/admin?notice=not_member");
    assert_eq!(
        members(&fixture).await,
        before,
        "this organization unchanged"
    );
    check(
        &mut checks,
        "another organization over real rows (review m3): a member of the canary organization only gets the uniform 404 on GET /admin and on Remove (with their own valid token), byte-identical to an unknown path; the canary organization's own admin sees only its own members and removing a member of this organization is the plain not_member notice; no membership of this organization changed",
    );

    let anonymous = send(&app, "GET", "/admin", None, "", &[]).await;
    let peer_page = get(&peer, "/admin").await;
    for refused in [&anonymous, &peer_page] {
        assert_eq!(refused.status, StatusCode::NOT_FOUND);
        assert_eq!(refused.body, unknown.body, "the uniform 404");
    }
    check(
        &mut checks,
        "GET /admin over real rows: the organization admin's page lists every member (rows keyed by principal) with a Remove form per active member other than the admin, and the feedback link, no script; a member without admin_policy and an anonymous caller get the uniform 404, byte-identical to an unknown path",
    );

    // --- Remove: token, confirmation, refusals, and the removal itself ------
    let token = auth.csrf.token(csrf::Form::OrgAdminRemove, &admin);
    let peer_token = auth.csrf.token(csrf::Form::OrgAdminRemove, &peer);
    let forged = post(
        &admin,
        &remove_peer,
        form(&[("csrf", &peer_token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    let cross = send(
        &app,
        "POST",
        &remove_peer,
        Some(&admin),
        &form(&[("csrf", &token), ("confirm", "remove")]),
        &[("sec-fetch-site", "cross-site")],
    )
    .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let by_peer = post(
        &peer,
        &remove_admin,
        form(&[("csrf", &peer_token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(by_peer.status, StatusCode::NOT_FOUND);
    assert_eq!(by_peer.body, unknown.body);
    let unconfirmed = post(&admin, &remove_peer, form(&[("csrf", &token)])).await;
    assert_eq!(location(&unconfirmed), "/admin?notice=unconfirmed");
    // The seeded admin is the organization's only admin: storage names
    // that refusal (last_admin) for its own removal; the "self" notice is
    // the unit tests'.
    let own = post(
        &admin,
        &remove_admin,
        form(&[("csrf", &token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(location(&own), "/admin?notice=last_admin");
    let shown = get(&admin, "/admin?notice=last_admin").await;
    assert!(
        shown
            .body
            .contains("remove the last admin of this organization."),
        "{}",
        shown.body
    );
    assert_eq!(members(&fixture).await, before, "nothing changed yet");
    check(
        &mut checks,
        "POST /admin/members/{principal}/remove over real rows: another session's token and a cross-site post are 403, a member without admin_policy gets the uniform 404, an unticked confirmation and the only admin's own removal answer 303 with their notices (the page then says the last admin cannot be removed), and no membership changed",
    );

    // --- The name: prompt, POST /me/name, shown to the admin ----------------
    let canvas_before = get(&peer, "/canvas").await;
    assert_eq!(
        canvas_before.status,
        StatusCode::OK,
        "{}",
        canvas_before.body
    );
    assert!(canvas_before.body.contains("What should others call you?"));
    assert!(!canvas_before.body.contains("href=\"/admin\""));
    let admin_canvas = get(&admin, "/canvas").await;
    assert!(admin_canvas.body.contains("href=\"/admin\""));
    let name_token = auth.csrf.token(csrf::Form::DisplayName, &peer);
    let wrong = post(
        &peer,
        member_name::PATH,
        form(&[("csrf", &token), ("name", "Ana María")]),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::FORBIDDEN);
    let blank = post(
        &peer,
        member_name::PATH,
        form(&[("csrf", &name_token), ("name", "   ")]),
    )
    .await;
    assert_eq!(location(&blank), "/canvas?name=invalid");
    for refused in ["Sarah\u{200B}Chen", "\u{3164}", "Ana\u{2028}Lopez"] {
        let disguised = post(
            &peer,
            member_name::PATH,
            form(&[("csrf", &name_token), ("name", refused)]),
        )
        .await;
        assert_eq!(location(&disguised), "/canvas?name=invalid", "{refused:?}");
    }
    let asked_again = get(&peer, "/canvas?name=invalid").await;
    assert_eq!(asked_again.status, StatusCode::OK);
    assert!(asked_again.body.contains("class=\"cv-name-notice\""));
    assert!(asked_again.body.contains("What should others call you?"));
    let other_query = get(&peer, "/canvas?name=valid").await;
    assert_eq!(other_query.status, StatusCode::NOT_FOUND);
    assert_eq!(members(&fixture).await, before);
    let named = post(
        &peer,
        member_name::PATH,
        form(&[("csrf", &name_token), ("name", "  Ana María ")]),
    )
    .await;
    assert_eq!(named.status, StatusCode::SEE_OTHER, "{}", named.body);
    assert_eq!(location(&named), "/canvas");
    assert_eq!(
        text(&fixture, &format!("SELECT display_name FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PEER}'")).await.as_deref(),
        Some("Ana María")
    );
    let canvas_after = get(&peer, "/canvas").await;
    assert!(!canvas_after.body.contains("What should others call you?"));
    assert!(canvas_after.body.contains("value=\"Ana María\""));
    let page = get(&admin, "/admin").await;
    assert!(page.body.contains("Ana María"));
    check(
        &mut checks,
        "D6 over real rows: a member with no name sees \"What should others call you?\" and no Admin link, the admin sees the Admin link; POST /me/name refuses another form's token (403), and a blank name, one with a zero-width space, a Hangul filler alone or a line separator with a 303 to /canvas?name=invalid where the canvas asks again in plain words (any other canvas query is the uniform 404), without a write; stores the trimmed name, the prompt disappears, the top bar's form shows it and the admin's table lists it",
    );

    // --- The removal ends the member's sessions -----------------------------
    let removed = post(
        &admin,
        &remove_peer,
        form(&[("csrf", &token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(location(&removed), "/admin?notice=removed");
    assert_eq!(
        text(&fixture, &format!("SELECT status FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PEER}'")).await.as_deref(),
        Some("revoked")
    );
    assert!(
        scalar(&fixture, &format!("SELECT membership_epoch FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PEER}'")).await
            > before
                .iter()
                .find(|m| m.0 == PEER)
                .map(|m| m.2)
                .unwrap()
    );
    for path in ["/canvas", "/admin"] {
        let after = get(&peer, path).await;
        assert_eq!(after.status, StatusCode::NOT_FOUND, "{path}");
    }
    let named_again = post(
        &peer,
        member_name::PATH,
        form(&[("csrf", &name_token), ("name", "Ana")]),
    )
    .await;
    assert_eq!(named_again.status, StatusCode::NOT_FOUND);
    let page = get(&admin, "/admin?notice=removed").await;
    assert!(page.body.contains("The member was removed."));
    let again = post(
        &admin,
        &remove_peer,
        form(&[("csrf", &token), ("confirm", "remove")]),
    )
    .await;
    assert_eq!(location(&again), "/admin?notice=not_member");
    check(
        &mut checks,
        "removal over real rows: the confirmed Remove answers 303 removed, the membership is revoked with an advanced epoch, the removed member's session gets the uniform 404 on /canvas, /admin and /me/name at its next request, the page says \"The member was removed.\", and removing them again is the plain not_member notice",
    );

    stores.close().await.unwrap();
    if let Some(path) = std::env::var_os("GENUI_ORG_ADMIN_ROUTES_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({"schema_version":1,"scope":"trial plan D4/D6: the /admin and /me/name routes and the canvas Admin link and name prompt over real rows (W2)","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Sessions are minted for tests; the hosted sign-in is not exercised.","Storage semantics (definers, audit chain, counts, last admin) are W1's org_admin_campaign."]})).unwrap()).unwrap();
    }
    println!("org-admin routes campaign: {} checks passed", checks.len());
}
