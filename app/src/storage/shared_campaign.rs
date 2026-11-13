//! Real PostgreSQL qualification of named external account sharing. The provider
//! proof here is synthetic test input; hosted provider exchange is a separate
//! auth campaign. Every share/session/grant/read is the production storage path.
use super::{
    admission_qualification::operator,
    definition_qualification::request,
    qualification::Fixture,
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL, PUBLICATION, WORK},
    *,
};
use genui_auth_verifier::VerifiedIdentity;
use serde_json::json;

const REFRESH: &str = "00000000-0000-4000-8000-00000000c001";
fn proof(subject: &str, email: &str) -> ShareIdentityProof {
    ShareIdentityProof::verified(
        &VerifiedIdentity {
            key_id: "synthetic-key".into(),
            issuer: "app008-fixture-only".into(),
            tenant_id: format!("ten_{OTHER}"),
            subject: subject.into(),
            issued_at: 1,
            expires_at: 2,
        },
        email,
        true,
    )
    .unwrap()
}
async fn account(stores: &Stores, subject: &str, email: &str) -> (String, ShareAccount) {
    let cookie = stores
        .workspace
        .mint_shared_session(&proof(subject, email), REFRESH)
        .await
        .unwrap();
    let resolved = stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .unwrap();
    (cookie, resolved)
}
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let op = fixture.operator().await;
    let mut counts = Vec::new();
    for table in [
        "principals",
        "memberships",
        "entitlements",
        "app_permissions",
        "work_permissions",
        "source_permissions",
        "saved_work",
        "apps",
    ] {
        counts.push(
            op.client
                .query_one(&format!("SELECT count(*)::bigint FROM genui.{table}"), &[])
                .await
                .unwrap()
                .get(0),
        );
    }
    counts
}
#[test]
#[ignore = "requires local Docker, OpenSSL and locked migration/contract Python environments"]
fn native_live_sharing_campaign() {
    std::thread::Builder::new()
        .name("named-sharing-campaign".into())
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
    // Existing operator-owned member permissions; no guest is added to them.
    fixture.sql(&format!("INSERT INTO genui.work_permissions VALUES('{ORG}','{PRINCIPAL}','{APP}','{WORK}','share_work') ON CONFLICT DO NOTHING"),"genui");
    let stores = Stores::new(fixture.settings()).unwrap();
    let owner = request(&stores, ORG, PRINCIPAL, 1);
    let foreign = request(&stores, OTHER, PRINCIPAL, 1);
    let before = footprint(&fixture).await;
    let mut checks = Vec::new();
    stores.readiness().await.unwrap();
    let (cookie, guest) = account(&stores, "external-recipient", "guest@example.test").await;
    let (_, wrong) = account(&stores, "wrong-recipient", "wrong@example.test").await;
    assert_ne!(guest.account_id(), wrong.account_id());
    assert!(stores
        .workspace
        .resolve_shared_session("forged", &|_| true)
        .await
        .unwrap()
        .is_none());
    assert!(stores
        .workspace
        .resolve_shared_session(&cookie, &|_| false)
        .await
        .unwrap()
        .is_none());
    assert_eq!(footprint(&fixture).await, before);
    check(&mut checks,"verified account sessions exist without principal, membership, entitlement or tenant grants; forged cookie and absent vault reference refuse");
    let input = NewLiveShare {
        recipient_email: "Guest@Example.Test".into(),
        allow_add_to_canvas: false,
        expires_in_days: 7,
        idempotency_key: "00000000-0000-4000-8000-00000000c100".into(),
    };

    let share = stores
        .workspace
        .create_live_share(&owner, APP, WORK, &input)
        .await
        .unwrap();
    assert_eq!(share.status, "pending");
    assert!(matches!(
        stores
            .workspace
            .revoke_live_share(&foreign, APP, WORK, &share.share_id, 1)
            .await,
        Err(Error::Denied)
    ));
    assert_eq!(
        stores
            .workspace
            .create_live_share(&owner, APP, WORK, &input)
            .await
            .unwrap(),
        share
    );
    let mut conflict = input.clone();
    conflict.allow_add_to_canvas = true;
    assert!(matches!(
        stores
            .workspace
            .create_live_share(&owner, APP, WORK, &conflict)
            .await,
        Err(Error::Denied)
    ));
    assert!(stores
        .workspace
        .pending_share(&guest, &share.key)
        .await
        .unwrap());
    assert!(matches!(
        stores.workspace.pending_share(&wrong, &share.key).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .workspace
            .resolve_guest_access(&guest, &share.key)
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.workspace.accept_live_share(&wrong, &share.key).await,
        Err(Error::Denied)
    ));
    let access = stores
        .workspace
        .accept_live_share(&guest, &share.key)
        .await
        .unwrap();
    assert_eq!(
        access,
        stores
            .workspace
            .accept_live_share(&guest, &share.key)
            .await
            .unwrap()
    );
    assert!(!stores
        .workspace
        .pending_share(&guest, &share.key)
        .await
        .unwrap());
    assert_eq!(
        stores
            .workspace
            .list_live_shares(&owner, APP, WORK)
            .await
            .unwrap()[0]
            .status,
        "active"
    );
    check(&mut checks,"named invitation is idempotent, wrong identity and foreign work deny, acceptance binds one account and grants no access before consent");
    let read = stores.data.guest_work_reads(&access).await.unwrap();
    let outline = read.outline.unwrap();
    assert!(!outline.can_write);
    let records = read.records.unwrap().unwrap();
    assert!(!records.can_write);
    assert!(records
        .collections
        .iter()
        .all(|c| c.commands.is_empty() && c.fields.iter().all(|f| !f.writable)));
    assert!(matches!(
        stores.workspace.link_shared_app(&guest, &share.key).await,
        Err(Error::Denied)
    ));
    assert!(stores
        .workspace
        .linked_apps(&guest)
        .await
        .unwrap()
        .is_empty());
    let run = stores
        .data
        .guest_run_query(&access, "total", &json!({}))
        .await
        .unwrap();
    assert!(matches!(
        run.engine_plan().actor(),
        QueryActor::Guest { .. }
    ));
    assert_eq!(run.engine_plan().principal(), guest.account_id());
    assert!(matches!(
        stores.data.release_result(&owner, run.engine_plan()).await,
        Err(Error::Denied)
    ));
    stores
        .data
        .release_guest_result(&access, &run)
        .await
        .unwrap();
    fixture.sql(&format!("INSERT INTO genui.publication_policies SELECT organization_id,policy_id,2,kind FROM genui.publication_policies WHERE organization_id='{ORG}' AND policy_version=1 ON CONFLICT DO NOTHING; UPDATE genui.publications SET policy_version=2 WHERE organization_id='{ORG}' AND app_id='{APP}' AND publication_id='{PUBLICATION}'"),"genui");
    assert!(matches!(
        stores.data.release_guest_result(&access, &run).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.data.release_guest_access(&access).await,
        Err(Error::Denied)
    ));
    fixture.sql(&format!("UPDATE genui.publications SET policy_version=1 WHERE organization_id='{ORG}' AND app_id='{APP}' AND publication_id='{PUBLICATION}'"),"genui");
    let policy_operator = fixture.operator().await;
    let refused=policy_operator.client.execute(&format!("UPDATE genui.publications SET operator_version=2 WHERE organization_id='{ORG}' AND app_id='{APP}' AND publication_id='{PUBLICATION}'"),&[]).await.unwrap_err();
    assert_eq!(
        refused.code().unwrap().code(),
        "23514",
        "the qualified schema cannot store an unsupported operator"
    );
    stores
        .data
        .release_guest_result(&access, &run)
        .await
        .unwrap();
    check(&mut checks,"policy epoch changes deny both guest query and final document release; unsupported operator version is rejected by the database constraint");

    assert!(matches!(
        stores
            .data
            .guest_run_query(&access, "record_observation", &json!({}))
            .await,
        Err(Error::Denied)
    ));
    let mut changed = access.clone();
    changed.work = "00000000-0000-4000-8000-00000000dead".into();
    assert!(matches!(
        stores.data.guest_work_reads(&changed).await,
        Err(Error::Denied)
    ));
    check(&mut checks,"guest reads exact published views/collections with no forms or command metadata; query attribution is the actual account and member release rejects it; sibling work substitution refuses");
    let updated = stores
        .workspace
        .update_live_share(&owner, APP, WORK, &share.share_id, 1, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.version, 2);
    assert!(stores
        .workspace
        .update_live_share(&owner, APP, WORK, &share.share_id, 1, false)
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        stores.data.release_guest_result(&access, &run).await,
        Err(Error::Denied)
    ));
    let access = stores
        .workspace
        .resolve_guest_access(&guest, &share.key)
        .await
        .unwrap();
    let linked = stores
        .workspace
        .link_shared_app(&guest, &share.key)
        .await
        .unwrap();
    assert_eq!(
        stores
            .workspace
            .link_shared_app(&guest, &share.key)
            .await
            .unwrap(),
        linked
    );
    assert_eq!(
        stores.workspace.linked_apps(&guest).await.unwrap(),
        vec![linked]
    );
    assert!(stores
        .workspace
        .linked_apps(&wrong)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(footprint(&fixture).await, before);
    stores
        .workspace
        .update_live_share(&owner, APP, WORK, &share.share_id, 2, false)
        .await
        .unwrap()
        .unwrap();
    assert!(stores
        .workspace
        .linked_apps(&guest)
        .await
        .unwrap()
        .is_empty());
    let view_only = stores
        .workspace
        .resolve_guest_access(&guest, &share.key)
        .await
        .unwrap();
    stores.data.guest_work_reads(&view_only).await.unwrap();
    assert!(matches!(
        stores.data.release_guest_access(&access).await,
        Err(Error::Denied)
    ));
    stores
        .workspace
        .unlink_shared_app(&guest, &share.key)
        .await
        .unwrap();
    stores
        .workspace
        .unlink_shared_app(&guest, &share.key)
        .await
        .unwrap();
    check(&mut checks,"add-to-canvas permission is independent, link is idempotent and account-private, old grant versions deny release, disabling placement hides links while preserving document view, no copied app or grants");
    // A real record commit is immediately visible through the existing share.
    let command = CommandRequest {
        app: APP.into(),
        work: WORK.into(),
        publication: PUBLICATION.into(),
        command: "record_decision".into(),
        schema: "decision".into(),
        schema_version_expected: 1,
        record: "00000000-0000-4000-8000-00000000c019".into(),
        expected_record_version: 0,
        expected_work_version: 1,
        values: json!([{"field_id":"status","value":{"type":"enum","value":"needs_context"}},{"field_id":"note","value":{"type":"text","value":"Live shared record"}}]),
        idempotency_key: "1000000000000000000000000000c019".into(),
    };
    let outcome = stores.data.commit_record(&owner, &command).await.unwrap();
    assert!(
        matches!(outcome, CommandResult::Committed(_)),
        "{outcome:?}"
    );
    let read = stores
        .data
        .guest_work_reads(&view_only)
        .await
        .unwrap()
        .records
        .unwrap()
        .unwrap();
    assert!(read.records.iter().any(|r| r.record_id == command.record));
    check(&mut checks,"real member command commit becomes visible through the linked original work without republishing or copying records");
    let run = stores
        .data
        .guest_run_query(&view_only, "total", &json!({}))
        .await
        .unwrap();
    fixture.sql(
        &format!(
            "UPDATE genui.source_revisions SET deleted_at=now() WHERE organization_id='{ORG}'"
        ),
        "genui",
    );
    assert!(matches!(
        stores.data.release_guest_result(&view_only, &run).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.data.guest_work_reads(&view_only).await,
        Err(Error::Denied)
    ));
    fixture.sql(&format!("UPDATE genui.source_revisions SET deleted_at=NULL WHERE organization_id='{ORG}'; UPDATE genui.saved_work SET version=version+1 WHERE organization_id='{ORG}' AND app_id='{APP}' AND work_id='{WORK}'"),"genui");
    assert!(matches!(
        stores.data.release_guest_access(&view_only).await,
        Err(Error::Denied)
    ));
    assert_eq!(
        stores
            .workspace
            .list_live_shares(&owner, APP, WORK)
            .await
            .unwrap()[0]
            .status,
        "paused"
    );
    fixture.sql(&format!("UPDATE genui.saved_work SET version=1 WHERE organization_id='{ORG}' AND app_id='{APP}' AND work_id='{WORK}'"),"genui");
    fixture.sql(&format!("UPDATE genui.memberships SET membership_epoch=2 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'"),"genui");
    assert!(matches!(
        stores.data.release_guest_access(&view_only).await,
        Err(Error::Denied)
    ));
    fixture.sql(&format!("UPDATE genui.memberships SET membership_epoch=1 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'"),"genui");
    check(&mut checks,"source deletion during query, configuration version change, and sponsor epoch change deny release; management reports paused rather than active");
    let flow = stores
        .workspace
        .begin_shared_flow(&share.key, OTHER, b"synthetic sealed verifier")
        .await
        .unwrap();
    stores
        .workspace
        .complete_shared_flow(&flow.cookie, &flow.state)
        .await
        .unwrap();
    assert!(matches!(
        stores
            .workspace
            .complete_shared_flow(&flow.cookie, &flow.state)
            .await,
        Err(Error::Denied)
    ));
    stores
        .workspace
        .discard_shared_flow(&flow.cookie)
        .await
        .unwrap();
    let actor = guest.account_id();
    fixture.sql(
        &format!("UPDATE genui_identity.accounts SET account_epoch=2 WHERE account_id='{actor}'"),
        "genui",
    );
    assert!(stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        stores.data.release_guest_access(&view_only).await,
        Err(Error::Denied)
    ));
    fixture.sql(&format!("UPDATE genui_identity.accounts SET account_epoch=1 WHERE account_id='{actor}'; UPDATE genui_identity.shared_sessions SET expires_at=clock_timestamp()-interval '1 second',created_at=clock_timestamp()-interval '2 seconds' WHERE account_id='{actor}'"),"genui");
    assert!(stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .is_none());
    check(&mut checks,"account epoch and session expiry deny cached access; account-only prelogin flow is single-use and bound to its server-side locator");
    let (_, renewed) = account(&stores, "external-recipient", "guest@example.test").await;
    let final_access = stores
        .workspace
        .resolve_guest_access(&renewed, &share.key)
        .await
        .unwrap();
    let run = stores
        .data
        .guest_run_query(&final_access, "total", &json!({}))
        .await
        .unwrap();
    let revoked = stores
        .workspace
        .revoke_live_share(&owner, APP, WORK, &share.share_id, 3)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revoked.status, "revoked");
    assert!(matches!(
        stores.data.release_guest_result(&final_access, &run).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.workspace.pending_share(&renewed, &share.key).await,
        Err(Error::Denied)
    ));
    assert!(stores
        .workspace
        .linked_apps(&renewed)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(footprint(&fixture).await, before);
    let op = fixture.operator().await;
    let events: i64 = op
        .client
        .query_one(
            "SELECT count(*)::bigint FROM genui_identity.share_events",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        events, 3,
        "accept, link, unlink are each appended exactly once"
    );
    check(&mut checks,"revocation between query authorization and release blocks results and links; account actions append exactly-once transactional audit and every tenant grant/member/app footprint stays unchanged");
    authority_regressions(&fixture, &stores, &owner, &mut checks).await;
    fail_closed(&fixture, &stores, &owner, &mut checks).await;
    drop(stores);
    http_campaign(&fixture, &mut checks).await;
    if let Some(path) = std::env::var_os("GENUI_LIVE_SHARING_REPORT") {
        std::fs::write(path,serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"Named account sharing native PostgreSQL campaign","checks":checks,"passed":true,"limits":["Synthetic verified identity proof; hosted provider exchange qualified separately.","No AWS deployment or IAM claim."]})).unwrap()).unwrap();
    }
}

async fn http_campaign(fixture: &Fixture, checks: &mut Vec<serde_json::Value>) {
    use crate::{
        auth::{self, cookies, Auth},
        http,
        query::{
            routes::tests::{Counting, RecordingPages},
            Executor, Work,
        },
        sharing::live,
    };
    use axum::{
        body::{to_bytes, Body},
        extract::ConnectInfo,
        http::{Request, StatusCode},
    };
    use std::{sync::Arc, time::Duration};
    use tower::ServiceExt;
    const HTTP_WORK: &str = "00000000-0000-4000-8000-00000000c101";
    let (_cache, engine) = super::query_qualification::prepare_shared_engine(fixture, HTTP_WORK);
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    let auth = Arc::new(Auth::loopback(
        auth::tests::config("http://127.0.0.1:9"),
        Some(stores.clone()),
        Arc::new(super::command_qualification::TestPages),
    ));
    let refresh = auth
        .vault
        .insert("northstar", &super::command_qualification::vault_token())
        .unwrap();
    let owner_cookie = stores
        .workspace
        .mint_session_for_tests(ORG, PRINCIPAL, &refresh)
        .await
        .unwrap();
    let guest_cookie = stores
        .workspace
        .mint_shared_session(
            &proof("http-recipient", "http-guest@example.test"),
            &refresh,
        )
        .await
        .unwrap();
    let wrong_cookie = stores
        .workspace
        .mint_shared_session(&proof("http-wrong", "wrong-http@example.test"), &refresh)
        .await
        .unwrap();
    let owner_header = format!("{}={owner_cookie}", cookies::SESSION);
    let guest_header = format!("{}={guest_cookie}", cookies::SHARED_SESSION);
    let wrong_header = format!("{}={wrong_cookie}", cookies::SHARED_SESSION);
    let counting = Arc::new(Counting {
        engine: Some(engine as Arc<dyn Executor>),
        ..Default::default()
    });
    let work = Arc::new(Work::new(
        auth.clone(),
        Some(counting.clone() as Arc<dyn Executor>),
        Arc::new(RecordingPages::default()),
    ));
    let routes = http::uniform(
        live::router(work.clone())
            .merge(crate::query::routes::router(work))
            .merge(crate::commands::routes::router(Arc::new(
                crate::commands::routes::Commands {
                    auth: auth.clone(),
                    page: Arc::new(super::command_qualification::TestPages),
                },
            ))),
    );
    async fn send(
        routes: &axum::Router,
        method: &str,
        path: &str,
        cookie: &str,
        origin: &str,
        body: &str,
    ) -> (StatusCode, String) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("cookie", cookie)
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .header("origin", origin)
            .header(
                "sec-fetch-site",
                if origin == "https://foreign.example.test" {
                    "cross-site"
                } else {
                    "same-origin"
                },
            )
            .body(Body::from(body.to_owned()))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "192.0.2.54:4000".parse::<std::net::SocketAddr>().unwrap(),
        ));
        let response = routes.clone().oneshot(request).await.unwrap();
        let status = response.status();
        (
            status,
            String::from_utf8(
                to_bytes(response.into_body(), 2 << 20)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap(),
        )
    }
    let origin = auth.origin();
    let collection = format!("/v1/work/{APP}/{HTTP_WORK}/shares");
    let create=json!({"recipient_email":"http-guest@example.test","idempotency_key":"00000000-0000-4000-8000-00000000c102"}).to_string();
    let (status, body) = send(&routes, "POST", &collection, &owner_header, origin, &create).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    let key = created["share"]["share_id"].as_str().unwrap();
    assert_eq!(created["share"]["allow_add_to_canvas"], false);
    assert_eq!(
        send(&routes, "GET", &collection, &owner_header, origin, "")
            .await
            .0,
        StatusCode::OK
    );
    let page = format!("/s/{key}");
    let accept = format!("/v1/shared/{key}/accept");
    assert_eq!(
        send(&routes, "GET", &page, &wrong_header, origin, "")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let pending = send(&routes, "GET", &page, &guest_header, origin, "").await;
    assert_eq!(pending.0, StatusCode::OK);
    assert!(pending.1.contains("can_accept\":true"));
    assert_eq!(
        send(
            &routes,
            "POST",
            &accept,
            &guest_header,
            "https://foreign.example.test",
            "{}"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &routes,
            "POST",
            &accept,
            &guest_header,
            origin,
            "{\"extra\":true}"
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &routes,
            "POST",
            &accept,
            &guest_header,
            origin,
            &"x".repeat(http::JSON_BODY_LIMIT + 1)
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        send(&routes, "POST", &accept, &guest_header, origin, "{}")
            .await
            .0,
        StatusCode::OK
    );
    let (status, body) = send(&routes, "GET", &page, &guest_header, origin, "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("shared_query_receipt"), "{body}");
    assert!(counting.calls() > 0);
    assert!(
        !body.contains("command_action") && !body.contains("idempotency_key"),
        "{body}"
    );
    let props: serde_json::Value = serde_json::from_str(&body).unwrap();
    // APP-098 DS-84: the sample check runs inside the guest's transaction
    // (a missing grant would refuse the page); this fixture work reads no
    // source of the synthetic catalog, so the guest sees no note.
    assert_eq!(props["sample_data"], json!(false), "{body}");
    // APP-098 DS-40: this organization is not a listed demo organization.
    assert_eq!(props["demo_data"], json!(false), "{body}");
    let parts = props["parts"].as_array().unwrap();
    assert_eq!(
        parts
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "total",
            "destinations",
            "timeline",
            "record_observation",
            "caution"
        ]
    );
    let note = parts.iter().find(|p| p["id"] == "caution").unwrap();
    assert_eq!(
        note["text"],
        "An unmatched destination does not establish compromise."
    );
    assert_eq!(
        note["class"],
        "document-part document-column-0 document-width-12 document-row-5"
    );
    assert!(parts.iter().all(|part| [
        "forms",
        "choices",
        "action",
        "command",
        "collection",
        "selection_name"
    ]
    .iter()
    .all(|key| part.get(key).is_none())));
    assert_eq!(
        parts[3]["message"],
        "This form is view only in a shared app."
    );
    check(checks, "APP-144 shared JSON retains every admitted part in default_layout order and literal note text; form parts are view-only labels without command identity, selectors or member routes");
    assert!(
        props["views"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["kind"] == "summary" || v["has_rows"] == true),
        "{body}"
    );
    let before = counting.calls();
    let authority = format!("/v1/shared/{key}/authority");
    assert_eq!(
        send(&routes, "GET", &authority, &guest_header, origin, "")
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        counting.calls(),
        before,
        "authority polling dispatches no query"
    );
    for path in [
        format!("/v1/work/{APP}/{HTTP_WORK}/queries/total"),
        format!("/v1/work/{APP}/{HTTP_WORK}/commands/record_decision"),
    ] {
        assert_eq!(
            send(&routes, "POST", &path, &guest_header, origin, "{}")
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        counting.calls(),
        before,
        "member routes refuse guest-only cookie before engine dispatch"
    );
    assert_eq!(
        send(
            &routes,
            "GET",
            &format!("/s/{ORG}.00000000-0000-4000-8000-00000000dead"),
            &guest_header,
            origin,
            ""
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let management = format!("{collection}/{key}");
    assert_eq!(
        send(
            &routes,
            "PATCH",
            &management,
            &owner_header,
            origin,
            "{\"expected_version\":999,\"allow_add_to_canvas\":true}"
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &routes,
            "PATCH",
            &management,
            &owner_header,
            origin,
            "{\"expected_version\":1,\"allow_add_to_canvas\":true}"
        )
        .await
        .0,
        StatusCode::OK
    );
    let canvas = format!("/v1/shared/{key}/canvas");
    assert_eq!(
        send(&routes, "POST", &canvas, &guest_header, origin, "{}")
            .await
            .0,
        StatusCode::OK
    );
    let (computed, resume) = counting.pause_next();
    let loading = send(&routes, "GET", &page, &guest_header, origin, "");
    tokio::pin!(loading);
    let mutation = async {
        tokio::time::timeout(Duration::from_secs(5), computed)
            .await
            .unwrap()
            .unwrap();
        let deleted = send(
            &routes,
            "DELETE",
            &management,
            &owner_header,
            origin,
            "{\"expected_version\":2}",
        )
        .await;
        assert_eq!(deleted.0, StatusCode::OK, "{}", deleted.1);
        resume.send(()).unwrap();
    };
    let (response, ()) = tokio::join!(&mut loading, mutation);
    assert_eq!(response.0, StatusCode::NOT_FOUND, "{}", response.1);
    assert!(!response.1.contains("shared_query_receipt") && !response.1.contains("record_cards"));
    assert_eq!(
        send(&routes, "GET", &authority, &guest_header, origin, "")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    check(checks,"real HTTP owner defaults/list/version conflict, verified named consent, cross-site/closed-body/oversize denials, confined engine numeric shared receipts, guest-only member API denials, zero-query authority polling, and revocation after engine computation prevent stale JSON release");
}

/// APP-178: organization shutdown, share expiry and overlapping grants. Each
/// denial is bracketed by the same call succeeding before and after, so a
/// refusal cannot come from anything but the condition under test.
async fn fail_closed(
    fixture: &Fixture,
    stores: &Stores,
    owner: &TrustedRequest,
    checks: &mut Vec<serde_json::Value>,
) {
    let (_, recipient) = account(stores, "fail-closed-recipient", "failclosed@example.test").await;
    let share = |key: &str| NewLiveShare {
        recipient_email: "failclosed@example.test".into(),
        allow_add_to_canvas: true,
        expires_in_days: 7,
        idempotency_key: key.into(),
    };
    // Rows the store keeps for this account: links not removed, and audited unlinks.
    let account_id = recipient.account_id().to_owned();
    let count = |sql: String| -> i64 { fixture.sql(&sql, "genui").trim().parse().unwrap() };
    let kept_links = || {
        count(format!("SELECT count(*) FROM genui_identity.linked_apps WHERE account_id='{account_id}' AND removed_at IS NULL"))
    };
    let unlinks = || {
        count(format!("SELECT count(*) FROM genui_identity.share_events WHERE account_id='{account_id}' AND kind='app.unlinked'"))
    };
    // Two independent grants of the same work to the same account, both placed.
    let first = stores
        .workspace
        .create_live_share(
            owner,
            APP,
            WORK,
            &share("00000000-0000-4000-8000-00000000c301"),
        )
        .await
        .unwrap();
    let second = stores
        .workspace
        .create_live_share(
            owner,
            APP,
            WORK,
            &share("00000000-0000-4000-8000-00000000c302"),
        )
        .await
        .unwrap();
    assert_ne!(first.share_id, second.share_id);
    let first_access = stores
        .workspace
        .accept_live_share(&recipient, &first.key)
        .await
        .unwrap();
    let second_access = stores
        .workspace
        .accept_live_share(&recipient, &second.key)
        .await
        .unwrap();
    for key in [&first.key, &second.key] {
        stores
            .workspace
            .link_shared_app(&recipient, key)
            .await
            .unwrap();
    }
    let linked_keys = |links: Vec<LinkedApp>| -> Vec<String> {
        let mut keys: Vec<String> = links.into_iter().map(|l| l.key).collect();
        keys.sort();
        keys
    };
    let mut both = vec![first.key.clone(), second.key.clone()];
    both.sort();
    assert_eq!(
        linked_keys(stores.workspace.linked_apps(&recipient).await.unwrap()),
        both
    );
    let unlinks_before = unlinks();
    for access in [&first_access, &second_access] {
        stores.data.release_guest_access(access).await.unwrap();
    }

    // Organization shutdown: query release, reads and access all refuse,
    // including a result computed before the shutdown. Connected links are
    // paused, not removed (ADR-014 addendum, 2026-11-12): hidden while the
    // organization is suspended, kept, and back when it is restored.
    let run = stores
        .data
        .guest_run_query(&second_access, "total", &json!({}))
        .await
        .unwrap();
    fixture.sql(
        &format!("UPDATE genui.organizations SET status='suspended' WHERE organization_id='{ORG}'"),
        "genui",
    );
    assert!(
        matches!(
            stores.data.release_guest_result(&second_access, &run).await,
            Err(Error::Denied)
        ),
        "a result computed before the shutdown is not released"
    );
    assert!(matches!(
        stores.data.release_guest_access(&first_access).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.data.guest_work_reads(&second_access).await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .data
            .guest_run_query(&second_access, "total", &json!({}))
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores
            .workspace
            .resolve_guest_access(&recipient, &first.key)
            .await,
        Err(Error::Denied)
    ));
    // Listing twice: hidden each time, and still kept afterwards.
    for _ in 0..2 {
        assert!(
            stores
                .workspace
                .linked_apps(&recipient)
                .await
                .unwrap()
                .is_empty(),
            "no connected link is shown while the sponsor organization is suspended"
        );
    }
    assert_eq!(kept_links(), 2, "a suspension keeps both connected links");
    assert_eq!(unlinks(), unlinks_before, "a suspension unlinks nothing");
    fixture.sql(
        &format!("UPDATE genui.organizations SET status='active' WHERE organization_id='{ORG}'"),
        "genui",
    );
    for access in [&first_access, &second_access] {
        stores.data.release_guest_access(access).await.unwrap();
    }
    assert_eq!(
        linked_keys(stores.workspace.linked_apps(&recipient).await.unwrap()),
        both,
        "restoring the organization brings both links back with no new Add"
    );
    check(checks, "APP-178: organization shutdown (status suspended) denies the guest's final release, a result computed before it, work reads, a new query and access resolution; connected links are paused: hidden while suspended, kept (no unlink, no app.unlinked event) and shown again when the organization is restored, with full access");

    // Overlapping independent grants: revoking one leaves the other exactly intact.
    let revoked = stores
        .workspace
        .revoke_live_share(owner, APP, WORK, &first.share_id, first.version)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revoked.status, "revoked");
    assert!(matches!(
        stores.data.release_guest_access(&first_access).await,
        Err(Error::Denied)
    ));
    assert_eq!(
        linked_keys(stores.workspace.linked_apps(&recipient).await.unwrap()),
        vec![second.key.clone()],
        "the revoked grant's link is removed and the other grant's link stays"
    );
    assert_eq!(
        unlinks(),
        unlinks_before + 1,
        "the revocation's unlink is audited once"
    );
    stores
        .data
        .release_guest_access(&second_access)
        .await
        .unwrap();
    stores.data.guest_work_reads(&second_access).await.unwrap();
    let again = stores
        .workspace
        .resolve_guest_access(&recipient, &second.key)
        .await
        .unwrap();
    assert_eq!(again, second_access, "the other grant is unchanged");
    check(checks, "APP-178: two independent grants of one work to one account, both placed: revoking the first denies it and removes only its link (one audited app.unlinked), while the second keeps its link and still resolves to the same access, reads and releases");

    // Share expiry. An expired invitation cannot be accepted, and neither the
    // signed-in path nor the sign-in path reveals more than for a key that
    // never existed. An accepted grant that expires stops resolving and
    // releasing, and its link is removed: expiry is not a pause.
    let (_, invitee) = account(stores, "expired-invitee", "expired@example.test").await;
    let expiring = stores
        .workspace
        .create_live_share(
            owner,
            APP,
            WORK,
            &NewLiveShare {
                recipient_email: "expired@example.test".into(),
                allow_add_to_canvas: false,
                expires_in_days: 7,
                idempotency_key: "00000000-0000-4000-8000-00000000c303".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        stores
            .workspace
            .pending_share(&invitee, &expiring.key)
            .await
            .unwrap(),
        "pending before expiry"
    );
    assert!(
        stores
            .workspace
            .shared_auth_target(&expiring.key)
            .await
            .is_ok(),
        "the sign-in path finds a live invitation"
    );
    let expire = |share_id: &str| {
        fixture.sql(&format!("UPDATE genui.live_work_shares SET created_at=clock_timestamp()-interval '2 seconds',expires_at=clock_timestamp()-interval '1 second' WHERE organization_id='{ORG}' AND share_id='{share_id}'"), "genui");
    };
    expire(&expiring.share_id);
    let expired = stores
        .workspace
        .accept_live_share(&invitee, &expiring.key)
        .await;
    assert!(
        matches!(expired, Err(Error::Denied)),
        "an expired invitation cannot be accepted: {expired:?}"
    );
    let mut unknown = expiring.key.clone();
    unknown.replace_range(unknown.len() - 4.., "0000");
    let never = stores.workspace.accept_live_share(&invitee, &unknown).await;
    assert_eq!(
        format!("{expired:?}"),
        format!("{never:?}"),
        "the refusal is the same as for a key that never existed"
    );
    assert!(
        matches!(
            stores.workspace.shared_auth_target(&expiring.key).await,
            Err(Error::Denied)
        ) && matches!(
            stores.workspace.shared_auth_target(&unknown).await,
            Err(Error::Denied)
        ),
        "the sign-in path refuses an expired invitation exactly as a key that never existed"
    );
    expire(&second.share_id);
    assert!(matches!(
        stores
            .workspace
            .resolve_guest_access(&recipient, &second.key)
            .await,
        Err(Error::Denied)
    ));
    assert!(matches!(
        stores.data.release_guest_access(&second_access).await,
        Err(Error::Denied)
    ));
    assert!(
        stores
            .workspace
            .linked_apps(&recipient)
            .await
            .unwrap()
            .is_empty(),
        "the expired grant's link is not shown"
    );
    assert_eq!(kept_links(), 0, "expiry removes the link");
    assert_eq!(
        unlinks(),
        unlinks_before + 2,
        "the expiry's unlink is audited"
    );
    check(checks, "APP-178: an invitation past its expiry cannot be accepted, and the refusal equals the one for a key that never existed, both when signed in (accept) and on the sign-in path (shared_auth_target); an accepted grant past its expiry no longer resolves or releases, and its link is removed (audited), unlike a suspension");
}

async fn authority_regressions(
    fixture: &Fixture,
    stores: &Stores,
    owner: &TrustedRequest,
    checks: &mut Vec<serde_json::Value>,
) {
    let (old_cookie, old_account) =
        account(stores, "email-rotation", "old-email@example.test").await;
    let (_, new_account) = account(stores, "email-rotation", "new-email@example.test").await;
    assert_eq!(old_account.account_id(), new_account.account_id());
    assert_eq!(new_account.account_epoch(), old_account.account_epoch() + 1);
    assert!(stores
        .workspace
        .resolve_shared_session(&old_cookie, &|_| true)
        .await
        .unwrap()
        .is_none());
    let tx = super::authorization::Transaction::begin_shared(
        &stores.workspace.inner,
        None,
        Some(old_account.account_id()),
    )
    .await
    .unwrap();
    assert!(matches!(
        super::shared_access::check_account(&tx, &old_account, false).await,
        Err(Error::Denied)
    ));
    tx.finish(false).await.unwrap();
    check(checks,"verified mailbox change keeps immutable account identity, advances account epoch, and invalidates both old cookie and cached account authority");
    // Same-organization identities use the same account grant, no membership shortcut.
    let same = ShareIdentityProof::verified(
        &VerifiedIdentity {
            key_id: "synthetic-key".into(),
            issuer: "app008-fixture-only".into(),
            tenant_id: format!("ten_{ORG}"),
            subject: format!("usr_{PRINCIPAL}"),
            issued_at: 1,
            expires_at: 2,
        },
        "same-org@example.test",
        true,
    )
    .unwrap();
    let cookie = stores
        .workspace
        .mint_shared_session(&same, REFRESH)
        .await
        .unwrap();
    let account = stores
        .workspace
        .resolve_shared_session(&cookie, &|_| true)
        .await
        .unwrap()
        .unwrap();
    let input = NewLiveShare {
        recipient_email: "same-org@example.test".into(),
        allow_add_to_canvas: true,
        expires_in_days: 7,
        idempotency_key: "00000000-0000-4000-8000-00000000c201".into(),
    };
    let share = stores
        .workspace
        .create_live_share(owner, APP, WORK, &input)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        stores.workspace.accept_live_share(&account, &share.key),
        stores.workspace.accept_live_share(&account, &share.key)
    );
    assert_eq!(first.unwrap(), second.unwrap());
    stores
        .workspace
        .link_shared_app(&account, &share.key)
        .await
        .unwrap();
    stores
        .workspace
        .update_live_share(owner, APP, WORK, &share.share_id, 1, false)
        .await
        .unwrap()
        .unwrap();
    stores
        .workspace
        .update_live_share(owner, APP, WORK, &share.share_id, 2, true)
        .await
        .unwrap()
        .unwrap();
    assert!(
        stores
            .workspace
            .linked_apps(&account)
            .await
            .unwrap()
            .is_empty(),
        "reenabling placement never restores an old reference without explicit Add"
    );
    // Operator fixture creates the full quota as expired references, never app data.
    let op = fixture.operator().await;
    op.client.batch_execute(&format!("WITH ids AS (SELECT gen_random_uuid() AS id FROM generate_series(1,100)) INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,accepted_account_id,accepted_at,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at) SELECT s.organization_id,i.id,s.app_id,s.work_id,s.sponsor_id,s.sponsor_epoch,s.recipient_email,s.accepted_account_id,s.accepted_at,s.publication_id,s.definition_digest,s.work_version,s.view_ids,s.collection_ids,s.title,true,1,i.id::text,s.request_digest,clock_timestamp()-interval '2 days',clock_timestamp()-interval '1 day' FROM genui.live_work_shares s CROSS JOIN ids i WHERE s.organization_id='{ORG}' AND s.share_id='{}'; INSERT INTO genui_identity.linked_apps(account_id,organization_id,share_id,created_at,grant_version) SELECT accepted_account_id,organization_id,share_id,clock_timestamp(),version FROM genui.live_work_shares WHERE organization_id='{ORG}' AND accepted_account_id='{}' AND expires_at<clock_timestamp();",share.share_id,account.account_id())).await.unwrap();
    assert_eq!(op.client.query_one("SELECT count(*)::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND removed_at IS NULL",&[&account.account_id()]).await.unwrap().get::<_,i64>(0),100);
    stores
        .workspace
        .link_shared_app(&account, &share.key)
        .await
        .unwrap();
    assert_eq!(
        stores.workspace.linked_apps(&account).await.unwrap().len(),
        1
    );
    let expired:i64=op.client.query_one("SELECT count(*)::bigint FROM genui_identity.linked_apps l JOIN genui.live_work_shares s USING(organization_id,share_id) WHERE l.account_id=$1::text::uuid AND l.removed_at IS NULL AND s.expires_at<clock_timestamp()",&[&account.account_id()]).await.unwrap().get(0);
    assert_eq!(expired, 0);
    let generation:i64=op.client.query_one("SELECT placement_generation::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND organization_id=$2::text::uuid AND share_id=$3::text::uuid",&[&account.account_id(),&ORG,&share.share_id]).await.unwrap().get(0);
    stores
        .workspace
        .unlink_shared_app(&account, &share.key)
        .await
        .unwrap();
    stores
        .workspace
        .link_shared_app(&account, &share.key)
        .await
        .unwrap();
    stores
        .workspace
        .unlink_shared_app_if_current(&account, &share.key, Some(generation))
        .await
        .unwrap();
    assert_eq!(
        stores.workspace.linked_apps(&account).await.unwrap().len(),
        1,
        "stale automatic cleanup cannot remove a newer explicit Add"
    );

    op.client.batch_execute(&format!("UPDATE genui.live_work_shares SET revoked_at=clock_timestamp(),created_at=clock_timestamp(),expires_at=clock_timestamp()+interval '1 day' WHERE organization_id='{ORG}' AND accepted_account_id='{}' AND share_id<>'{}'",account.account_id(),share.share_id)).await.unwrap();
    let managed = stores
        .workspace
        .list_live_shares(owner, APP, WORK)
        .await
        .unwrap();
    assert_eq!(
        managed.first().unwrap().share_id,
        share.share_id,
        "recent revoked history cannot bury the active grant"
    );

    // Session expiry hides inventory but cannot erase its references.
    op.client.batch_execute(&format!("UPDATE genui_identity.shared_sessions SET revoked_at=clock_timestamp() WHERE account_id='{}' AND session_id='{}'",account.account_id(),account.session_id())).await.unwrap();
    assert!(matches!(
        stores.workspace.linked_apps(&account).await,
        Err(Error::Denied)
    ));
    assert_eq!(op.client.query_one("SELECT count(*)::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND removed_at IS NULL",&[&account.account_id()]).await.unwrap().get::<_,i64>(0),1);
    check(checks,"same-org verified account follows identical consent, concurrent acceptance is idempotent, placement revoke cleans once, reenable requires new Add, 100 expired links cannot strand quota, and session revocation never deletes references");
    // RLS has no default account scope and cannot reveal the other account.
    op.client
        .batch_execute("BEGIN; SET LOCAL ROLE genui_data")
        .await
        .unwrap();
    assert_eq!(
        op.client
            .query_one(
                "SELECT count(account_id)::bigint FROM genui_identity.accounts",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    op.client
        .query_one(
            "SELECT set_config('genui.account_id',$1,true)",
            &[&new_account.account_id()],
        )
        .await
        .unwrap();
    let ids: Vec<String> = op
        .client
        .query("SELECT account_id::text FROM genui_identity.accounts", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(ids, vec![new_account.account_id().to_string()]);
    let denied = op
        .client
        .execute(
            "UPDATE genui_identity.accounts SET status='revoked' WHERE account_id=$1::text::uuid",
            &[&new_account.account_id()],
        )
        .await
        .unwrap_err();
    assert_eq!(denied.code().unwrap().code(), "42501");
    op.client.batch_execute("ROLLBACK").await.unwrap();
    // Cleanup owns only expired ephemeral rows, with an explicit batch bound.
    op.client.batch_execute(&format!("INSERT INTO genui_identity.shared_sessions(account_id,session_id,cookie_digest,refresh_token_ref,account_epoch,created_at,last_seen_at,expires_at) SELECT '{}',gen_random_uuid(),'sha256:'||encode(sha256(convert_to(n::text,'UTF8')),'hex'),gen_random_uuid(),{},clock_timestamp()-interval '3 hours',clock_timestamp()-interval '2 hours',clock_timestamp()+interval '1 hour' FROM generate_series(1,105)n",new_account.account_id(),new_account.account_epoch())).await.unwrap();
    let cleanup = stores.workspace.sweep_shared_identity().await.unwrap();
    assert_eq!(cleanup.len(), 100);
    let cleanup = stores.workspace.sweep_shared_identity().await.unwrap();
    assert!(!cleanup.is_empty() && cleanup.len() <= 100);
    assert!(stores
        .workspace
        .sweep_shared_identity()
        .await
        .unwrap()
        .is_empty());
    assert_eq!(op.client.query_one("SELECT count(*)::bigint FROM genui_identity.linked_apps WHERE account_id=$1::text::uuid AND removed_at IS NULL",&[&account.account_id()]).await.unwrap().get::<_,i64>(0),1);
    check(checks,"account FORCE RLS refuses blank/foreign account reads and data-role writes; bounded cross-account sweeper deletes at most100 expired sessions per call and preserves accounts, grants and linked references");
}
