//! Real PostgreSQL + HTTP OAuth qualification. The issuer and broker are local
//! contract fakes; this is not evidence of hosted signup delivery or deployment.
use super::{
    cookies, shared, shared_broker,
    synthetic_issuer::{Issuer, TokenMode},
    tests::{self, Browser, TestPages, APP, ORG, OTHER},
    Auth,
};
use crate::{
    http,
    storage::{
        self, admission_qualification::operator, qualification::Fixture, Lookup, NewLiveShare,
        Stores,
    },
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
const PRINCIPAL: &str = "00000000-0000-4000-8000-000000000066";
const WORK: &str = "00000000-0000-4000-8000-000000000069";
const EMAIL: &str = "external@example.test";
const TOKEN: &str = "tinv_00000000-0000-4000-8000-00000000b001";
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let operator = fixture.operator().await;
    let mut result = Vec::new();
    for table in [
        "principals",
        "memberships",
        "entitlements",
        "organization_permissions",
        "app_permissions",
        "work_permissions",
        "source_permissions",
        "apps",
        "saved_work",
    ] {
        result.push(
            operator
                .client
                .query_one(&format!("SELECT count(*)::bigint FROM genui.{table}"), &[])
                .await
                .unwrap()
                .get(0),
        );
    }
    result
}
#[derive(Clone)]
struct Broker {
    issuer: Arc<Issuer>,
    token: String,
    calls: Arc<AtomicUsize>,
}
async fn broker(
    State(state): State<Broker>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    state.calls.fetch_add(1, Ordering::SeqCst);
    assert_eq!(
        headers.get("authorization").unwrap().to_str().unwrap(),
        format!("Bearer {}", state.token)
    );
    assert_eq!(body["role"], "user");
    assert_eq!(body["delivery_mode"], "server_only");
    assert_eq!(body["email"], EMAIL);
    assert_eq!(body["tenant_slug"], "northstar");
    state.issuer.invitation(TOKEN, "northstar", EMAIL);
    (
        StatusCode::CREATED,
        Json(json!({"invite_token":TOKEN,"expires_at":super::now()+300})),
    )
}
fn cookie_headers(browser: &Browser) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let mut pairs = Vec::new();
    for name in [cookies::SESSION, cookies::SHARED_SESSION] {
        if let Some(value) = browser.cookie(name) {
            pairs.push(format!("{name}={value}"));
        }
    }
    if !pairs.is_empty() {
        headers.insert("cookie", pairs.join("; ").parse().unwrap());
    }
    headers
}
async fn start(
    auth: &Auth,
    app: &Router,
    browser: &mut Browser,
    key: &str,
    intent: &str,
) -> tests::Reply {
    let csrf = shared::start_csrf(auth, &cookie_headers(browser), key);
    browser
        .post(
            app,
            &format!("/auth/shared/{key}/start"),
            &format!("intent={intent}&csrf={csrf}"),
        )
        .await
}
#[test]
#[ignore = "requires local Docker, OpenSSL and locked migration/contract Python environments"]
fn native_shared_identity_campaign() {
    std::thread::Builder::new()
        .name("shared-identity-campaign".into())
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
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    fixture.sql(&format!("INSERT INTO genui.work_permissions VALUES('{ORG}','{PRINCIPAL}','{APP}','{WORK}','share_work') ON CONFLICT DO NOTHING"),"genui");
    let issuer = Arc::new(Issuer::start(tests::tenants()).await);
    let (config, registry, _secret) = tests::deliver_auth_configuration(
        &fixture,
        &issuer.origin,
        "00000000-0000-4000-8000-000000000d09",
    );
    // Keep the fixture members' actual provider identities alongside the two
    // synthetic OAuth bindings. No recipient is inserted into principals.
    let mut document: Value = serde_json::from_slice(&std::fs::read(&registry).unwrap()).unwrap();
    for org in [ORG, OTHER] {
        document["bindings"].as_array_mut().unwrap().push(json!({"issuer":"app008-fixture-only","provider_tenant_id":format!("ten_{org}"),"organization_id":org}));
    }
    std::fs::write(&registry, serde_json::to_vec(&document).unwrap()).unwrap();
    let stores = Arc::new(Stores::new(fixture.settings_with_registry(registry)).unwrap());
    stores.readiness().await.unwrap();
    let broker_token = super::capability::random_capability().unwrap();
    let broker_calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let broker_app = Router::new()
        .route("/v1/shared-invitations", post(broker))
        .with_state(Broker {
            issuer: issuer.clone(),
            token: broker_token.clone(),
            calls: broker_calls.clone(),
        });
    let broker_server =
        tokio::spawn(async move { axum::serve(listener, broker_app).await.unwrap() });
    let auth = Arc::new(
        Auth::loopback(config, Some(stores.clone()), Arc::new(TestPages)).with_shared_broker(Some(
            shared_broker::Client::loopback(&origin, &broker_token).unwrap(),
        )),
    );
    let app =
        http::uniform(shared::router(auth.clone()).merge(super::routes::router(auth.clone())));
    let entry = http::uniform(crate::sharing::live::router(Arc::new(
        crate::query::Work::new(
            auth.clone(),
            None,
            Arc::new(crate::query::routes::tests::RecordingPages::default()),
        ),
    )));
    let log = Arc::default();
    let mut checks = Vec::new();
    let reference = super::capability::random_uuid().unwrap();
    let owner_cookie = stores
        .workspace
        .mint_session_for_tests(ORG, PRINCIPAL, &reference)
        .await
        .unwrap();
    let Lookup::Live(owner) = stores
        .workspace
        .resolve_session(&owner_cookie, &|_| true)
        .await
        .unwrap()
    else {
        panic!("fixture owner")
    };
    let share = stores
        .workspace
        .create_live_share(
            &owner,
            APP,
            WORK,
            &NewLiveShare {
                recipient_email: EMAIL.into(),
                allow_add_to_canvas: true,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-000000000001".into(),
            },
        )
        .await
        .unwrap();
    let before = footprint(&fixture).await;
    // A genuine signed provider token backs the test's existing home cookie.
    issuer.user("usr_home", EMAIL, true);
    let home_token =
        tests::issued_token(&issuer, &auth.provider.as_ref().unwrap().client, "usr_home").await;
    let home_ref = auth.vault.insert("northstar", &home_token).unwrap();
    let home = stores
        .workspace
        .mint_session_for_tests(OTHER, PRINCIPAL, &home_ref)
        .await
        .unwrap();
    // Actual entry HTTP + session resolution, without forging a trusted member.
    let mut current = Browser::new("198.51.100.82:1000", &log);
    current.install_cookie(cookies::SESSION, &home);
    let landing = current
        .send(
            &entry,
            "GET",
            &format!("/s/{}", share.key),
            "",
            &[("accept", "application/json")],
        )
        .await;
    assert_eq!(landing.status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&landing.body).unwrap()["can_sign_up"],
        false
    );
    for (index, case) in [
        "none",
        "malformed",
        "absent",
        "expired",
        "revoked",
        "missing_vault",
    ]
    .into_iter()
    .enumerate()
    {
        let mut visitor = Browser::new(&format!("198.51.100.{}:1000", 90 + index), &log);
        let mut raw_cookie = None;
        if case == "malformed" {
            raw_cookie = Some("malformed-session".to_owned());
        } else if case != "none" {
            let token =
                tests::issued_token(&issuer, &auth.provider.as_ref().unwrap().client, "usr_home")
                    .await;
            let reference = auth.vault.insert("northstar", &token).unwrap();
            let cookie = stores
                .workspace
                .mint_session_for_tests(OTHER, PRINCIPAL, &reference)
                .await
                .unwrap();
            match case {
                "absent" => fixture.sql(&format!("DELETE FROM genui.sessions WHERE refresh_token_ref='{reference}'"),"genui"),
                "expired" => fixture.sql(&format!("UPDATE genui.sessions SET created_at=now()-interval '13 hours',absolute_expires_at=now()-interval '1 hour',last_seen_at=now()-interval '2 hours' WHERE refresh_token_ref='{reference}'"),"genui"),
                "revoked" => fixture.sql(&format!("UPDATE genui.sessions SET revoked_at=clock_timestamp() WHERE refresh_token_ref='{reference}'"),"genui"),
                "missing_vault" => { auth.vault.take(&reference).await; String::new() },
                _ => unreachable!(),
            };
            raw_cookie = Some(cookie);
        }
        if let Some(cookie) = &raw_cookie {
            visitor.install_cookie(cookies::SESSION, cookie);
        }
        let landing = visitor
            .send(
                &entry,
                "GET",
                &format!("/s/{}", share.key),
                "",
                &[("accept", "application/json")],
            )
            .await;
        assert_eq!(landing.status, StatusCode::OK, "{case}");
        let props: Value = serde_json::from_str(&landing.body).unwrap();
        assert_eq!(props["can_sign_up"], true, "{case}");
        let calls = broker_calls.load(Ordering::SeqCst);
        if raw_cookie.is_some() {
            let refused = visitor
                .post(
                    &app,
                    &format!("/auth/shared/{}/start", share.key),
                    "intent=sign_up&csrf=wrong",
                )
                .await;
            assert_eq!(refused.status, StatusCode::FORBIDDEN, "{case}");
            assert_eq!(broker_calls.load(Ordering::SeqCst), calls);
        }
        let begun = visitor
            .post(
                &app,
                &format!("/auth/shared/{}/start", share.key),
                &format!(
                    "intent=sign_up&csrf={}",
                    props["sign_in_csrf"].as_str().unwrap()
                ),
            )
            .await;
        assert_eq!(
            begun.status,
            StatusCode::SEE_OTHER,
            "{case}: {}",
            begun.body
        );
        assert!(
            begun.location().contains("/t/northstar/")
                && begun.location().contains("client_id=northstar-shared"),
            "{case}"
        );
        assert_eq!(broker_calls.load(Ordering::SeqCst), calls + 1);
        assert_eq!(
            visitor.cookie(cookies::SESSION),
            raw_cookie,
            "{case}: home cookie unchanged"
        );
        stores
            .workspace
            .discard_shared_flow(&visitor.cookie(cookies::SHARED_PRELOGIN).unwrap())
            .await
            .unwrap();
    }
    tests::check(&mut checks,"anonymous, malformed, absent-row, expired, revoked and missing-vault home sessions offer signup through the actual JSON entry and exact target-tenant POST; raw-cookie CSRF remains required; live home session suppresses signup");
    // Hold the exact membership lock until both requests finish. Lookup Denied
    // is uncertainty, never permission to create in the recipient's tenant.
    let op = fixture.operator().await;
    let flow_count = || async {
        op.client
            .query_one(
                "SELECT count(*)::bigint FROM genui_identity.shared_auth_flows",
                &[],
            )
            .await
            .unwrap()
            .get::<_, i64>(0)
    };
    let flows = flow_count().await;
    let calls = broker_calls.load(Ordering::SeqCst);
    let pars = issuer.count(|c| &c.par);
    op.client.batch_execute("BEGIN").await.unwrap();
    op.client.query("SELECT 1 FROM genui.memberships WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid FOR UPDATE",&[&OTHER,&PRINCIPAL]).await.unwrap();
    let landing = current
        .send(
            &entry,
            "GET",
            &format!("/s/{}", share.key),
            "",
            &[("accept", "application/json")],
        )
        .await;
    assert_eq!(landing.status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&landing.body).unwrap()["can_sign_up"],
        false
    );
    let blocked = start(&auth, &app, &mut current, &share.key, "sign_up").await;
    assert!(blocked.uniform_404());
    assert_eq!(broker_calls.load(Ordering::SeqCst), calls);
    assert_eq!(issuer.count(|c| &c.par), pars);
    assert_eq!(flow_count().await, flows);
    assert!(current.cookie(cookies::SHARED_PRELOGIN).is_none());
    assert_eq!(
        current.cookie(cookies::SESSION).as_deref(),
        Some(home.as_str())
    );
    op.client.batch_execute("ROLLBACK").await.unwrap();
    tests::check(&mut checks,"real locked home membership makes GET suppress signup and POST refuse without flow/broker/PAR growth or replacing the home cookie; no fallback tenant on uncertain lookup");
    let mut browser = Browser::new("198.51.100.81:1000", &log);
    browser.install_cookie(cookies::SESSION, &home);
    let begun = start(&auth, &app, &mut browser, &share.key, "sign_in").await;
    assert_eq!(begun.status, StatusCode::SEE_OTHER, "{}", begun.body);
    assert!(begun.location().contains("/t/canary/"));
    assert!(begun.location().contains("client_id=canary-shared"));
    let query = issuer.login(&begun.location(), "usr_home").unwrap();
    let arrived = browser
        .get(&app, &format!("/auth/shared/callback?{query}"))
        .await;
    assert_eq!(arrived.status, StatusCode::SEE_OTHER, "{}", arrived.body);
    assert_eq!(arrived.location(), format!("/s/{}", share.key));
    assert_eq!(
        browser.cookie(cookies::SESSION).as_deref(),
        Some(home.as_str())
    );
    assert!(matches!(
        stores
            .workspace
            .resolve_session(&home, &|r| auth.vault.contains(r))
            .await
            .unwrap(),
        Lookup::Live(_)
    ));
    let account = shared::identity(&auth, &cookie_headers(&browser))
        .await
        .unwrap();
    stores
        .workspace
        .accept_live_share(&account, &share.key)
        .await
        .unwrap();
    assert_eq!(footprint(&fixture).await, before);
    tests::check(&mut checks,"existing other-organization session reauthenticates in its own provider tenant, receives account-only identity, accepts exact work, and retains its live home cookie with no tenant membership/grant growth");
    let replay = browser
        .get(&app, &format!("/auth/shared/callback?{query}"))
        .await;
    assert!(replay.uniform_404());
    assert_eq!(
        browser.cookie(cookies::SESSION).as_deref(),
        Some(home.as_str())
    );
    // Fill the remaining seven account seats, then replace this browser's
    // verified session through OAuth at the cap instead of stranding it.
    let provider = auth.provider.as_ref().unwrap();
    let access_token = auth
        .vault
        .access(account.refresh_ref(), &provider.client)
        .await
        .unwrap();
    let identity = provider
        .verifier
        .verify(&access_token, tests::CANARY_TENANT)
        .await
        .unwrap();
    let proof = storage::ShareIdentityProof::verified(&identity, EMAIL, true).unwrap();
    for _ in 0..7 {
        let token = tests::issued_token(&issuer, &provider.client, "usr_home").await;
        let reference = auth.vault.insert("northstar", &token).unwrap();
        stores
            .workspace
            .mint_shared_session_live(&proof, &reference, &|r| auth.vault.contains(r))
            .await
            .unwrap();
    }
    assert!(matches!(
        stores
            .workspace
            .mint_shared_session_live(&proof, account.refresh_ref(), &|r| auth.vault.contains(r))
            .await,
        Err(storage::Error::Denied)
    ));
    let begun = start(&auth, &app, &mut browser, &share.key, "sign_in").await;
    assert_eq!(begun.status, StatusCode::SEE_OTHER);
    let query = issuer.login(&begun.location(), "usr_home").unwrap();
    let arrived = browser
        .get(&app, &format!("/auth/shared/callback?{query}"))
        .await;
    assert_eq!(arrived.status, StatusCode::SEE_OTHER, "{}", arrived.body);
    let renewed = shared::identity(&auth, &cookie_headers(&browser))
        .await
        .unwrap();
    assert_eq!(renewed.account_id(), account.account_id());
    assert_ne!(renewed.session_id(), account.session_id());
    assert!(!auth.vault.contains(account.refresh_ref()));
    let csrf = shared::signout_csrf(&auth, &cookie_headers(&browser));
    let signed_out = browser
        .post(&app, "/auth/shared/signout", &format!("csrf={csrf}"))
        .await;
    assert_eq!(signed_out.status, StatusCode::SEE_OTHER);
    assert!(browser.cookie(cookies::SHARED_SESSION).is_none());
    assert_eq!(
        browser.cookie(cookies::SESSION).as_deref(),
        Some(home.as_str())
    );
    assert!(!auth.vault.contains(renewed.refresh_ref()));
    assert!(matches!(
        stores
            .workspace
            .resolve_session(&home, &|r| auth.vault.contains(r))
            .await
            .unwrap(),
        Lookup::Live(_)
    ));
    tests::check(&mut checks,"eight-session cap refuses a ninth seat but verified reauthentication replaces the current browser; shared-only signout revokes its vault/session and preserves the live home workspace");
    // Each refusal uses an unconsumed exact-share flow and real token exchange.
    for (index, mode, subject, email) in [
        (0, TokenMode::Normal, "usr_wrong", "wrong@example.test"),
        (1, TokenMode::WrongIssuer, "usr_home", EMAIL),
        (2, TokenMode::SubjectMismatch, "usr_home", EMAIL),
        (3, TokenMode::ForeignTenant, "usr_home", EMAIL),
    ] {
        issuer.user(subject, email, true);
        issuer.token_mode(mode);
        let mut bad = Browser::new(&format!("198.51.100.{}:1000", 82 + index), &log);
        bad.install_cookie(cookies::SESSION, &home);
        let begun = start(&auth, &app, &mut bad, &share.key, "sign_in").await;
        assert_eq!(begun.status, StatusCode::SEE_OTHER, "{}", begun.body);
        let query = issuer.login(&begun.location(), subject).unwrap();
        let denied = bad
            .get(&app, &format!("/auth/shared/callback?{query}"))
            .await;
        assert!(denied.uniform_404(), "{mode:?}: {}", denied.body);
        assert!(bad.cookie(cookies::SHARED_SESSION).is_none());
        assert_eq!(bad.cookie(cookies::SESSION).as_deref(), Some(home.as_str()));
    }
    issuer.token_mode(TokenMode::Normal);
    tests::check(&mut checks,"callback replay, wrong mailbox, wrong issuer, mismatched subject and foreign tenant uniformly refuse without replacing or clearing the home workspace session");
    // A separate exact invitation exercises broker -> PAR -> hosted registration
    // -> unverified callback -> fresh sign-in after verification, with no member.
    let signup = stores
        .workspace
        .create_live_share(
            &owner,
            APP,
            WORK,
            &NewLiveShare {
                recipient_email: EMAIL.into(),
                allow_add_to_canvas: false,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-000000000002".into(),
            },
        )
        .await
        .unwrap();
    // The synthetic issuer has global user storage; give this registration its
    // own mailbox while the broker remains bound to the expected address.
    issuer.user("usr_home", "changed@example.test", true);
    let mut new = Browser::new("198.51.100.90:1000", &log);
    let begun = start(&auth, &app, &mut new, &signup.key, "sign_up").await;
    assert_eq!(begun.status, StatusCode::SEE_OTHER, "{}", begun.body);
    let query = issuer
        .register(&begun.location(), "usr_new_external", EMAIL)
        .expect("hosted registration");
    let pending = new
        .get(&app, &format!("/auth/shared/callback?{query}"))
        .await;
    assert_eq!(pending.status, StatusCode::SEE_OTHER, "{}", pending.body);
    assert_eq!(
        pending.location(),
        format!("/s/{}?verification=pending", signup.key)
    );
    assert!(new.cookie(cookies::SHARED_SESSION).is_none());
    assert!(new.cookie(cookies::SESSION).is_none());
    assert_eq!(footprint(&fixture).await, before);
    issuer.set_verified("usr_new_external", true);
    let begun = start(&auth, &app, &mut new, &signup.key, "sign_in").await;
    assert_eq!(begun.status, StatusCode::SEE_OTHER);
    let query = issuer.login(&begun.location(), "usr_new_external").unwrap();
    let arrived = new
        .get(&app, &format!("/auth/shared/callback?{query}"))
        .await;
    assert_eq!(arrived.status, StatusCode::SEE_OTHER, "{}", arrived.body);
    assert_eq!(arrived.location(), format!("/s/{}", signup.key));
    let new_account = shared::identity(&auth, &cookie_headers(&new))
        .await
        .unwrap();
    assert_ne!(new_account.account_id(), account.account_id());
    assert!(new.cookie(cookies::SESSION).is_none());
    stores
        .workspace
        .accept_live_share(&new_account, &signup.key)
        .await
        .unwrap();
    assert_eq!(footprint(&fixture).await, before);
    tests::check(&mut checks,"real HTTP broker and provider contract fakes perform exact-email hosted registration; unverified callback grants nothing, fresh verified signin returns the exact share and creates no organization membership or workspace cookie");
    stores
        .workspace
        .revoke_live_share(&owner, APP, WORK, &signup.share_id, 1)
        .await
        .unwrap();
    let denied = start(&auth, &app, &mut new, &signup.key, "sign_in").await;
    assert!(denied.uniform_404());
    tests::check(
        &mut checks,
        "revoked exact share refuses before issuing provider authorization",
    );
    issuer.user("usr_home", EMAIL, true);
    let begun = start(&auth, &app, &mut browser, &share.key, "sign_in").await;
    assert_eq!(begun.status, StatusCode::SEE_OTHER);
    let query = issuer.login(&begun.location(), "usr_home").unwrap();
    assert_eq!(
        browser
            .get(&app, &format!("/auth/shared/callback?{query}"))
            .await
            .status,
        StatusCode::SEE_OTHER
    );
    let current = shared::identity(&auth, &cookie_headers(&browser))
        .await
        .unwrap();
    let csrf = auth.csrf.token(super::csrf::Form::SignOut, &home);
    let ended = browser
        .post(&app, "/auth/signout", &format!("csrf={csrf}"))
        .await;
    assert_eq!(ended.status, StatusCode::SEE_OTHER);
    assert!(browser.cookie(cookies::SESSION).is_none());
    assert!(browser.cookie(cookies::SHARED_SESSION).is_none());
    assert!(!auth.vault.contains(current.refresh_ref()));
    assert!(!matches!(
        stores
            .workspace
            .resolve_session(&home, &|r| auth.vault.contains(r))
            .await
            .unwrap(),
        Lookup::Live(_)
    ));
    tests::check(&mut checks,"ordinary workspace signout also destroys and clears the browser's account-only session and revokes its provider reference");
    if let Some(path) = std::env::var_os("GENUI_SHARED_IDENTITY_REPORT") {
        std::fs::write(path,serde_json::to_vec_pretty(&json!({"schema_version":1,"passed":true,"checks":checks,"limits":["Synthetic loopback broker/provider HTTP; no hosted delivery or live deployment claim.","Existing home workspace session is fixture-minted but backed by a real signed synthetic-provider token and resolved by production storage."]})).unwrap()).unwrap();
    }
    broker_server.abort();
}
