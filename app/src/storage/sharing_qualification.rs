//! APP-095 sharing and real-time campaign. Isolated test executable only:
//! local Docker, OpenSSL and the locked Python environments, as the APP-013
//! command campaign. The Northstar Spend Review S05 fixture is created through
//! the admitted path (`spend_review_s05`, the author as its creator); the
//! author shares it with a teammate through the storage module and through
//! the HTTP route with minted workspace sessions; the event stream is opened
//! over the route and read as a body stream. Every acceptance item is
//! asserted on the database (permission rows, the admission chain) or on the
//! stream's bytes, never on a response alone.
use super::{
    admission_qualification::operator,
    command_qualification::{json_headers, send, vault_token, TestPages},
    definition_qualification::{
        grant, request, revoke, spend_review_s05, SpendReview, AUTHOR, EXPENSES, SPEND_APP,
    },
    qualification::Fixture,
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    auth::{cookies, Auth, Pages},
    commands::{self, routes::Commands, RecordPage},
    events, http, sharing,
};
use axum::{
    body::{Body, BodyDataStream},
    extract::ConnectInfo,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::{pin::Pin, sync::Arc, time::Duration};
use tokio::time::timeout;
use tower::ServiceExt;

/// The seeded revoked member of both organizations (tenant_seed.py).
const REVOKED: &str = "00000000-0000-4000-8000-00000000006e";
/// A member of the canary organization only, created by this campaign.
const OUTSIDER: &str = "00000000-0000-4000-8000-000000000ab0";
/// Well formed, never a member anywhere.
const NOBODY: &str = "00000000-0000-4000-8000-000000000ab1";
const SHARED_RECORD: &str = "00000000-0000-4000-8000-0000000014a1";
const HTTP_RECORD: &str = "00000000-0000-4000-8000-0000000014a2";

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_sharing_campaign() {
    std::thread::Builder::new()
        .name("app095-sharing-campaign".into())
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
/// A principal's permission rows on the S05 app, work and source in
/// Northstar (what a share of that work can write), as the superuser sees
/// them; the seeded Northstar fixture rows (app 065, work 069) are outside it
/// and counted by `footprint` instead.
async fn permissions(fixture: &Fixture, s05: &SpendReview, principal: &str) -> Vec<String> {
    let operator = fixture.operator().await;
    let work = &s05.work;
    operator.client.query(&format!("SELECT * FROM (SELECT 'app:'||app_id::text||':'||permission AS p FROM genui.app_permissions WHERE organization_id='{ORG}' AND principal_id='{principal}' AND app_id='{SPEND_APP}' UNION ALL SELECT 'work:'||work_id::text||':'||permission FROM genui.work_permissions WHERE organization_id='{ORG}' AND principal_id='{principal}' AND work_id='{work}' UNION ALL SELECT 'source:'||source_id::text||':'||permission FROM genui.source_permissions WHERE organization_id='{ORG}' AND principal_id='{principal}' AND source_id='{EXPENSES}' UNION ALL SELECT 'org:'||permission FROM genui.organization_permissions WHERE organization_id='{ORG}' AND principal_id='{principal}') x ORDER BY p"), &[])
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect()
}
/// The permission rows in both organizations, the admission chain's
/// `permission.granted` events and the `work_shares` records: what a share
/// may write.
async fn footprint(fixture: &Fixture) -> [i64; 6] {
    let mut counts = [0; 6];
    for (slot, sql) in [
        "SELECT count(*) FROM genui.app_permissions",
        "SELECT count(*) FROM genui.work_permissions",
        "SELECT count(*) FROM genui.source_permissions",
        "SELECT count(*) FROM genui.organization_permissions",
        "SELECT count(*) FROM genui.audit_events WHERE writer_role='admission' AND kind='permission.granted'",
        "SELECT count(*) FROM genui.work_shares",
    ]
    .into_iter()
    .enumerate()
    {
        counts[slot] = scalar(fixture, sql).await;
    }
    counts
}
fn names(shared: &Shared) -> Vec<String> {
    shared
        .granted
        .iter()
        .map(|g| format!("{}:{}:{}", g.scope, g.resource, g.permission))
        .collect()
}
/// The `work_shares` row referenced by the admission chain's latest
/// `permission.granted` event: `(sharer, grantee, granted, digest recomputed
/// from the row == the event's outcome_digest)` (review M3: every share is
/// reconstructable from the row the event references).
async fn latest_share_record(fixture: &Fixture) -> (String, String, Vec<String>, bool) {
    let operator = fixture.operator().await;
    let row = operator.client.query_one(&format!("SELECT s.sharer_id::text,s.grantee_id::text,s.granted,e.outcome_digest='sha256:'||encode(sha256(convert_to('forma:share-grant:v1','UTF8')||'\\x00'::bytea||convert_to(s.grantee_id::text,'UTF8')||'\\x00'::bytea||convert_to(array_to_string(s.granted,','),'UTF8')),'hex') FROM genui.audit_events e JOIN genui.work_shares s ON s.organization_id=e.organization_id AND s.audit_event_id=e.event_id WHERE e.organization_id='{ORG}' AND e.writer_role='admission' AND e.kind='permission.granted' ORDER BY e.chain_sequence DESC LIMIT 1"), &[]).await.unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3))
}
fn expected_subset(s05: &SpendReview, write: bool) -> Vec<String> {
    let mut all = vec![
        format!("app:{SPEND_APP}:discover"),
        format!("app:{SPEND_APP}:run"),
        format!("work:{}:record_read", s05.work),
    ];
    if write {
        all.push(format!("work:{}:record_write", s05.work));
    }
    all.push(format!("source:{EXPENSES}:source_read"));
    all
}
fn command(s05: &SpendReview, record: &str, key: &str, note: &str) -> CommandRequest {
    CommandRequest {
        app: SPEND_APP.into(),
        work: s05.work.clone(),
        publication: s05.publication.clone(),
        command: "record_decision".into(),
        schema: "decision".into(),
        schema_version_expected: 1,
        record: record.into(),
        expected_record_version: 0,
        expected_work_version: 1,
        values: json!([
            {"field_id": "status", "value": {"type": "enum", "value": "needs_context"}},
            {"field_id": "note", "value": {"type": "text", "value": note}},
        ]),
        idempotency_key: key.into(),
    }
}

/// An open event stream: the response's body as a stream of frames.
struct Stream {
    body: BodyDataStream,
}
impl Stream {
    /// The next SSE frame as text, or `None` when the body ended.
    async fn frame(&mut self, within: Duration) -> Option<String> {
        loop {
            let next = timeout(
                within,
                std::future::poll_fn(|cx| {
                    futures_core::Stream::poll_next(Pin::new(&mut self.body), cx)
                }),
            )
            .await
            .expect("a frame or the end within the window")?;
            let text = String::from_utf8(next.unwrap().to_vec()).unwrap();
            // Heartbeat comments are not events.
            if text.starts_with(':') {
                continue;
            }
            return Some(text);
        }
    }
    /// Asserts the next frame is `kind` for `work`.
    async fn expect(&mut self, kind: &str, work: &str) -> Value {
        let frame = self
            .frame(Duration::from_secs(5))
            .await
            .expect("an event frame");
        let (event, data) =
            frame
                .lines()
                .fold((None, None), |(e, d), line| match line.split_once(": ") {
                    Some(("event", v)) => (Some(v.to_owned()), d),
                    Some(("data", v)) => (e, Some(v.to_owned())),
                    _ => (e, d),
                });
        assert_eq!(event.as_deref(), Some(kind), "{frame}");
        let data: Value = serde_json::from_str(data.as_deref().unwrap()).unwrap();
        assert_eq!(data["kind"], kind);
        assert_eq!(data["app_id"], SPEND_APP);
        assert_eq!(data["work_id"], work);
        assert_eq!(
            data.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["app_id", "at", "kind", "work_id"]
        );
        data
    }
    /// Asserts nothing arrives within `within`.
    async fn silent(&mut self, within: Duration) {
        let next = timeout(
            within,
            std::future::poll_fn(|cx| {
                futures_core::Stream::poll_next(Pin::new(&mut self.body), cx)
            }),
        )
        .await;
        match next {
            Err(_) => {}
            Ok(Some(Ok(bytes))) if bytes.starts_with(b":") => {}
            Ok(other) => panic!("unexpected frame: {other:?}"),
        }
    }
    /// Asserts the body has ended.
    async fn ended(&mut self) {
        assert!(
            self.frame(Duration::from_secs(5)).await.is_none(),
            "the stream ends"
        );
    }
}
async fn open_stream(app: &Router, cookie: &str) -> std::result::Result<Stream, StatusCode> {
    let mut request = Request::builder()
        .uri("/v1/events")
        .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.40:4000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    if response.status() != StatusCode::OK {
        return Err(response.status());
    }
    assert!(response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    Ok(Stream {
        body: response.into_body().into_data_stream(),
    })
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
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{OTHER}',true); INSERT INTO genui.principals VALUES ('{OTHER}','{OUTSIDER}','app008-fixture-only','ten_{OTHER}','usr_{OUTSIDER}'); INSERT INTO genui.memberships VALUES ('{OTHER}','{OUTSIDER}','active',1); COMMIT;"), "genui");
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    let s05 = spend_review_s05(&fixture, &stores).await;
    let work = s05.work.as_str();
    let author = request(&stores, ORG, AUTHOR, 1);
    let reviewer = request(&stores, ORG, REVIEWER, 1);
    let principal = request(&stores, ORG, PRINCIPAL, 1);
    let before = footprint(&fixture).await;
    assert_eq!(before[4], 0);
    let reviewer_before = permissions(&fixture, &s05, REVIEWER).await;
    // The seeded approver's review_publish is on the Northstar fixture app, not here.
    assert_eq!(reviewer_before, Vec::<String>::new());
    assert_eq!(scalar(&fixture, &format!("SELECT count(*) FROM genui.app_permissions WHERE organization_id='{ORG}' AND principal_id='{REVIEWER}' AND app_id='{APP}' AND permission='review_publish'")).await, 1);

    // --- a member without authority over the work cannot share it ---
    for (who, grantee) in [(&reviewer, PRINCIPAL), (&principal, REVIEWER)] {
        assert_eq!(
            stores
                .workspace
                .share_work(who, SPEND_APP, work, grantee, false)
                .await
                .err(),
            Some(Error::Denied)
        );
    }
    assert_eq!(footprint(&fixture).await, before);
    assert_eq!(
        stores
            .data
            .work_outline(&reviewer, SPEND_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    check(
        &mut checks,
        "a member who is neither the work's creator nor a share_work holder cannot share it (Denied, nothing written); before the share the teammate cannot open the work",
    );

    // --- the creator shares read access: exactly the creator's subset ---
    let shared = stores
        .workspace
        .share_work(&author, SPEND_APP, work, REVIEWER, false)
        .await
        .unwrap();
    assert_eq!(shared.grantee, REVIEWER);
    assert_eq!(names(&shared), expected_subset(&s05, false));
    assert!(shared.granted.iter().all(|g| g.inserted));
    let mut expected_rows = reviewer_before.clone();
    expected_rows.extend(expected_subset(&s05, false));
    expected_rows.sort();
    assert_eq!(permissions(&fixture, &s05, REVIEWER).await, expected_rows);
    let after = footprint(&fixture).await;
    assert_eq!(
        after,
        [before[0] + 2, before[1] + 1, before[2] + 1, before[3], 1, 1]
    );
    {
        let operator = fixture.operator().await;
        let event = operator.client.query_one(&format!("SELECT actor_principal_id::text,app_id::text,work_id::text,outcome_digest::text,record_id IS NULL AND code_id IS NULL FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='permission.granted'"), &[]).await.unwrap();
        assert_eq!(event.get::<_, String>(0), AUTHOR);
        assert_eq!(event.get::<_, String>(1), SPEND_APP);
        assert_eq!(event.get::<_, String>(2), work);
        assert!(event.get::<_, String>(3).starts_with("sha256:"));
        assert!(event.get::<_, bool>(4));
    }
    // The share's record names exactly the inserted set and the event's
    // outcome_digest is recomputable from it.
    let (sharer, grantee, granted, digest_matches) = latest_share_record(&fixture).await;
    assert_eq!((sharer.as_str(), grantee.as_str()), (AUTHOR, REVIEWER));
    assert_eq!(granted, expected_subset(&s05, false));
    assert!(digest_matches);
    check(
        &mut checks,
        "the creator shares the work read-only: the teammate receives exactly discover and run on the app, record_read on the work and source_read on its one source dependency (never edit_definition, record_write or anything the creator does not hold), with one permission.granted event on the admission chain naming the sharer, the app and the work, and one work_shares row naming exactly the inserted set, referenced by the event, from which the event's outcome_digest recomputes",
    );

    // --- the teammate opens it under its own grants; others still cannot ---
    let listed = stores.data.openable_works(&reviewer).await.unwrap();
    let item = listed
        .iter()
        .find(|w| w.work == work)
        .expect("the shared work is listed");
    assert!(!item.can_write);
    assert_eq!(
        stores
            .data
            .work_outline(&reviewer, SPEND_APP, work)
            .await
            .unwrap()
            .title,
        stores
            .data
            .work_outline(&author, SPEND_APP, work)
            .await
            .unwrap()
            .title
    );
    stores
        .data
        .can_open(&reviewer, SPEND_APP, work)
        .await
        .unwrap();
    assert_eq!(
        stores
            .data
            .commit_record(
                &reviewer,
                &command(
                    &s05,
                    SHARED_RECORD,
                    "00000000000000000000000000000a01",
                    "no write"
                )
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .work_outline(&principal, SPEND_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .can_open(&principal, SPEND_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert!(!stores
        .data
        .openable_works(&principal)
        .await
        .unwrap()
        .iter()
        .any(|w| w.work == work));
    check(
        &mut checks,
        "the teammate lists and opens the shared work read-only (a command is Denied without record_write); a member who was not shared cannot open or list it",
    );

    // --- a repeat share writes nothing; sharing record_write adds only that ---
    let again = stores
        .workspace
        .share_work(&author, SPEND_APP, work, REVIEWER, false)
        .await
        .unwrap();
    assert_eq!(names(&again), expected_subset(&s05, false));
    assert!(!again.changed());
    assert_eq!(footprint(&fixture).await, after);
    let write = stores
        .workspace
        .share_work(&author, SPEND_APP, work, REVIEWER, true)
        .await
        .unwrap();
    assert_eq!(names(&write), expected_subset(&s05, true));
    assert_eq!(
        write
            .granted
            .iter()
            .filter(|g| g.inserted)
            .map(|g| g.permission.as_str())
            .collect::<Vec<_>>(),
        ["record_write"]
    );
    let after_write = footprint(&fixture).await;
    assert_eq!(
        after_write,
        [after[0], after[1] + 1, after[2], after[3], 2, 2]
    );
    let (_, grantee, granted, digest_matches) = latest_share_record(&fixture).await;
    assert_eq!(grantee, REVIEWER);
    assert_eq!(granted, vec![format!("work:{}:record_write", s05.work)]);
    assert!(digest_matches);
    assert!(matches!(
        stores
            .data
            .commit_record(
                &reviewer,
                &command(
                    &s05,
                    SHARED_RECORD,
                    "00000000000000000000000000000a02",
                    "teammate writes"
                )
            )
            .await
            .unwrap(),
        CommandResult::Committed(_)
    ));
    check(
        &mut checks,
        "a repeat share inserts nothing and appends no event or record; sharing record_write adds exactly that row with its own event and a record naming only record_write, after which the teammate's command commits",
    );

    // --- refusals: self, revoked, outsider, nobody, another scope, no escalation ---
    for grantee in [AUTHOR, REVOKED, OUTSIDER, NOBODY, "not-an-id"] {
        assert_eq!(
            stores
                .workspace
                .share_work(&author, SPEND_APP, work, grantee, false)
                .await
                .err(),
            Some(Error::Denied),
            "{grantee}"
        );
    }
    // The author's identity in the canary organization: the work is not there.
    let canary_author = request(&stores, OTHER, AUTHOR, 1);
    assert_eq!(
        stores
            .workspace
            .share_work(&canary_author, SPEND_APP, work, REVIEWER, false)
            .await
            .err(),
        Some(Error::Denied)
    );
    // The teammate holds record_read and record_write now, but is neither the
    // creator nor a share_work holder: it cannot pass them on.
    assert_eq!(
        stores
            .workspace
            .share_work(&reviewer, SPEND_APP, work, PRINCIPAL, true)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(footprint(&fixture).await, after_write);
    assert_eq!(
        permissions(&fixture, &s05, PRINCIPAL).await,
        Vec::<String>::new()
    );
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.app_permissions WHERE principal_id IN ('{OUTSIDER}','{NOBODY}')")).await,
        0
    );
    check(
        &mut checks,
        "sharing with oneself, a revoked member, a member of another organization, an unknown principal or a malformed id, from another organization's scope, or by a grantee who is not the creator: all Denied (the uniform 404) and nothing written in either organization",
    );

    // --- an owner-level share_work holder shares exactly its own subset ---
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.work_permissions VALUES ('{ORG}','{REVIEWER}','{SPEND_APP}','{work}','share_work'); COMMIT;"), "genui");
    // An operator revocation advances the member's epoch (ADR-004); the
    // teammate's requests from here on carry the new one.
    let moved = revoke(&fixture, REVIEWER, "discover", &["--app", SPEND_APP]);
    let reviewer = request(
        &stores,
        ORG,
        REVIEWER,
        moved["membership_epoch"].as_i64().unwrap(),
    );
    let delegated = stores
        .workspace
        .share_work(&reviewer, SPEND_APP, work, PRINCIPAL, false)
        .await
        .unwrap();
    let mut without_discover = expected_subset(&s05, false);
    without_discover.remove(0);
    assert_eq!(names(&delegated), without_discover);
    let mut rows = without_discover.clone();
    rows.sort();
    assert_eq!(permissions(&fixture, &s05, PRINCIPAL).await, rows);
    // Without discover the work page opens but the listing omits it.
    stores
        .data
        .work_outline(&principal, SPEND_APP, work)
        .await
        .unwrap();
    assert!(!stores
        .data
        .openable_works(&principal)
        .await
        .unwrap()
        .iter()
        .any(|w| w.work == work));
    assert_eq!(
        stores
            .data
            .can_open(&principal, SPEND_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    // Not even the sharer's own record_write travels unless asked for.
    assert_eq!(
        stores
            .data
            .commit_record(
                &principal,
                &command(
                    &s05,
                    HTTP_RECORD,
                    "00000000000000000000000000000a03",
                    "no write"
                )
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    grant(&fixture, REVIEWER, "discover", &["--app", SPEND_APP]);
    check(
        &mut checks,
        "a share_work holder who lost discover shares exactly its own subset (run, record_read, source_read; no discover): the grantee opens the work page but the listing and the stream's delivery check omit it, and no record_write travels unasked",
    );

    // --- members list ---
    let members = stores.workspace.members(&author).await.unwrap();
    let ids: Vec<&str> = members.iter().map(|m| m.principal.as_str()).collect();
    assert!(ids.contains(&REVIEWER) && ids.contains(&PRINCIPAL));
    assert!(!ids.contains(&AUTHOR) && !ids.contains(&REVOKED) && !ids.contains(&OUTSIDER));
    assert!(members
        .iter()
        .all(|m| m.label == format!("Member {}", &m.principal[..8])));
    // The same identity in the canary organization sees that organization's
    // members only (RLS scope): the outsider, none of Northstar's listing.
    let canary = stores.workspace.members(&canary_author).await.unwrap();
    let canary_ids: Vec<&str> = canary.iter().map(|m| m.principal.as_str()).collect();
    assert!(canary_ids.contains(&OUTSIDER) && !canary_ids.contains(&AUTHOR));
    assert!(!ids.contains(&OUTSIDER));
    check(
        &mut checks,
        "the members list names the other active members of the sharer's organization with neutral labels only: not the sharer, not a revoked member, never a member of another organization",
    );

    // --- HTTP: the share route, the members route and the event stream ---
    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
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
    let author_cookie = mint(AUTHOR).await;
    let reviewer_cookie = mint(REVIEWER).await;
    let principal_cookie = mint(PRINCIPAL).await;
    let record_page: Arc<dyn RecordPage> = pages;
    let app = http::uniform(
        sharing::router(auth.clone())
            .merge(events::routes::router(auth.clone()))
            .merge(commands::routes::router(Arc::new(Commands {
                auth: auth.clone(),
                page: record_page,
            }))),
    );
    let listed = send(&app, "GET", "/v1/members", Some(&author_cookie), "", &[]).await;
    assert_eq!(listed.status, StatusCode::OK);
    let listed = listed.json();
    assert!(listed["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["principal_id"] == REVIEWER && m["label"] == "Member 00000000"));
    assert!(listed["members"].as_array().unwrap().iter().all(|m| m
        .as_object()
        .unwrap()
        .keys()
        .collect::<Vec<_>>()
        == ["label", "principal_id"]));
    assert_eq!(
        send(&app, "GET", "/v1/members", None, "", &[]).await.status,
        StatusCode::NOT_FOUND
    );
    let share_uri = format!("/v1/work/{SPEND_APP}/{work}/share");
    let same_origin = [
        ("sec-fetch-site", "same-origin"),
        ("content-type", "application/json"),
    ];
    let body = |grantee: &str, write: bool| {
        format!("{{\"grantee_principal_id\":\"{grantee}\",\"record_write\":{write}}}")
    };
    let http_before = footprint(&fixture).await;
    // Refusals before any write: cross-site, no session, a non-creator, a
    // malformed body, a nonexistent work (identical to the refused share).
    let cross = send(
        &app,
        "POST",
        &share_uri,
        Some(&author_cookie),
        &body(PRINCIPAL, false),
        &[
            ("sec-fetch-site", "cross-site"),
            ("content-type", "application/json"),
        ],
    )
    .await;
    assert_eq!(
        (cross.status, cross.json()["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("cross_site"))
    );
    let anonymous = send(
        &app,
        "POST",
        &share_uri,
        None,
        &body(PRINCIPAL, false),
        &same_origin,
    )
    .await;
    assert_eq!(anonymous.status, StatusCode::NOT_FOUND);
    let no_authority = send(
        &app,
        "POST",
        &share_uri,
        Some(&principal_cookie),
        &body(REVIEWER, false),
        &same_origin,
    )
    .await;
    assert_eq!(no_authority.status, StatusCode::NOT_FOUND);
    let missing = send(
        &app,
        "POST",
        &format!("/v1/work/{SPEND_APP}/{NOBODY}/share"),
        Some(&author_cookie),
        &body(PRINCIPAL, false),
        &same_origin,
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body, no_authority.body);
    assert_eq!(missing.body, anonymous.body);
    let malformed = send(
        &app,
        "POST",
        &share_uri,
        Some(&author_cookie),
        &format!("{{\"grantee_principal_id\":\"{PRINCIPAL}\",\"permission\":\"admin_policy\"}}"),
        &same_origin,
    )
    .await;
    assert_eq!(
        (malformed.status, malformed.json()["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("malformed_share"))
    );
    let form = send(
        &app,
        "POST",
        &share_uri,
        Some(&author_cookie),
        "grantee_principal_id=x",
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ],
    )
    .await;
    assert_eq!(form.status, StatusCode::BAD_REQUEST);
    assert_eq!(footprint(&fixture).await, http_before);
    check(
        &mut checks,
        "over HTTP: the members route answers labels only under a session and the uniform 404 without one; the share route refuses a cross-site post (403), no session, a member without authority and a nonexistent work with byte-identical uniform 404s, and a body naming a permission or not JSON with 400, writing nothing",
    );

    // Two streams: the teammate (can open the work) and the author.
    let mut reviewer_stream = open_stream(&app, &reviewer_cookie).await.unwrap();
    let mut principal_stream = open_stream(&app, &principal_cookie).await.unwrap();
    tokio::task::yield_now().await;
    assert_eq!(events::hub().open_streams(), 2);
    // A hint for the shared work reaches the teammate, not the member whose
    // delegated subset lacks discover (the listing clause), and nothing of
    // another organization reaches anyone.
    events::hub().record_committed(ORG, SPEND_APP, work);
    events::hub().record_committed(OTHER, SPEND_APP, work);
    let data = reviewer_stream.expect("record.committed", work).await;
    assert!(data["at"].as_str().unwrap().ends_with('Z'));
    principal_stream.silent(Duration::from_secs(1)).await;
    // The author shares with that member over the route: discover is the one
    // row inserted, app.shared is announced, and now both streams receive it.
    let shared = send(
        &app,
        "POST",
        &share_uri,
        Some(&author_cookie),
        &body(PRINCIPAL, false),
        &same_origin,
    )
    .await;
    assert_eq!(shared.status, StatusCode::OK, "{}", shared.body);
    let shared_body = shared.body.clone();
    let shared = shared.json();
    assert_eq!(shared["grantee_principal_id"], PRINCIPAL);
    assert_eq!(shared["app_id"], SPEND_APP);
    assert_eq!(shared["work_id"], work);
    // The answer is the subset now held, without which rows were new (review
    // m1): the same keys on every element, no `inserted`.
    let granted_names: Vec<String> = shared["granted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            assert_eq!(
                g.as_object().unwrap().keys().collect::<Vec<_>>(),
                ["permission", "resource_id", "scope"]
            );
            format!(
                "{}:{}:{}",
                g["scope"].as_str().unwrap(),
                g["resource_id"].as_str().unwrap(),
                g["permission"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(granted_names, expected_subset(&s05, false));
    // The database says what was new: exactly discover, one event, one record.
    assert_eq!(
        footprint(&fixture).await,
        [
            http_before[0] + 1,
            http_before[1],
            http_before[2],
            http_before[3],
            http_before[4] + 1,
            http_before[5] + 1
        ]
    );
    let (_, grantee, granted, digest_matches) = latest_share_record(&fixture).await;
    assert_eq!(grantee, PRINCIPAL);
    assert_eq!(granted, vec![format!("app:{SPEND_APP}:discover")]);
    assert!(digest_matches);
    reviewer_stream.expect("app.shared", work).await;
    principal_stream.expect("app.shared", work).await;
    let mut rows = expected_subset(&s05, false);
    rows.sort();
    assert_eq!(permissions(&fixture, &s05, PRINCIPAL).await, rows);
    // A replay over the route changes nothing, announces nothing and answers
    // the byte-identical body (nothing tells the sharer the rows existed).
    let replay = send(
        &app,
        "POST",
        &share_uri,
        Some(&author_cookie),
        &body(PRINCIPAL, false),
        &same_origin,
    )
    .await;
    assert_eq!(replay.status, StatusCode::OK);
    assert_eq!(replay.body, shared_body);
    reviewer_stream.silent(Duration::from_millis(500)).await;
    check(
        &mut checks,
        "the event stream delivers record.committed for a work the subscriber can open now and nothing for a work it cannot (or of another organization); the creator's share over the route inserts exactly the missing row (one event, one work_shares record), answers the granted subset without disclosing which rows were new, and announces app.shared to every subscriber who can open the work; a replay answers the byte-identical body and announces nothing",
    );

    // The teammate commits through the command route: both streams learn.
    let committed = send(
        &app,
        "POST",
        &format!("/v1/work/{SPEND_APP}/{work}/commands/record_decision"),
        Some(&reviewer_cookie),
        &serde_json::to_string(
            &command(
                &s05,
                HTTP_RECORD,
                "00000000000000000000000000000a04",
                "over http",
            )
            .document(),
        )
        .unwrap(),
        &json_headers("00000000000000000000000000000a04"),
    )
    .await;
    assert_eq!(committed.status, StatusCode::OK, "{}", committed.body);
    reviewer_stream.expect("record.committed", work).await;
    principal_stream.expect("record.committed", work).await;
    // The composer's hook.
    events::hub().app_placed(ORG, SPEND_APP, work);
    reviewer_stream.expect("app.placed", work).await;
    principal_stream.expect("app.placed", work).await;
    check(
        &mut checks,
        "a command committed through the route announces record.committed to every subscriber who can open the work, and the composer's app.placed hook reaches them the same way",
    );

    // Sign-out advances the epoch: the member's stream ends at the next event.
    stores
        .workspace
        .sign_out(&principal_cookie)
        .await
        .unwrap()
        .unwrap();
    events::hub().app_placed(ORG, SPEND_APP, work);
    reviewer_stream.expect("app.placed", work).await;
    principal_stream.ended().await;
    tokio::task::yield_now().await;
    assert_eq!(events::hub().open_streams(), 1);
    assert_eq!(
        open_stream(&app, &principal_cookie).await.err(),
        Some(StatusCode::NOT_FOUND)
    );
    // The operator revokes the teammate's record_read, which advances its
    // epoch (ADR-004): the stream, authorized at the earlier epoch, ends at
    // the next event instead of delivering it, and the next request (at the
    // new epoch) is refused. The session itself stays live (not a sign-out).
    let moved = revoke(
        &fixture,
        REVIEWER,
        "record_read",
        &["--app", SPEND_APP, "--work", work],
    );
    let reviewer = request(
        &stores,
        ORG,
        REVIEWER,
        moved["membership_epoch"].as_i64().unwrap(),
    );
    events::hub().record_committed(ORG, SPEND_APP, work);
    reviewer_stream.ended().await;
    assert_eq!(
        stores
            .data
            .work_outline(&reviewer, SPEND_APP, work)
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        send(&app, "GET", "/v1/members", Some(&principal_cookie), "", &[])
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // One stream per session: a second connection of the same session closes
    // the first; the member bound refuses the fifth.
    let mut reviewer_stream = open_stream(&app, &reviewer_cookie).await.unwrap();
    tokio::task::yield_now().await;
    let mut replacement = open_stream(&app, &reviewer_cookie).await.unwrap();
    reviewer_stream.ended().await;
    let mut more = Vec::new();
    for _ in 1..events::MAX_STREAMS_PER_MEMBER {
        more.push(open_stream(&app, &mint(REVIEWER).await).await.unwrap());
    }
    tokio::task::yield_now().await;
    assert_eq!(events::hub().open_streams(), events::MAX_STREAMS_PER_MEMBER);
    assert_eq!(
        open_stream(&app, &mint(REVIEWER).await).await.err(),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
    grant(
        &fixture,
        REVIEWER,
        "record_read",
        &["--app", SPEND_APP, "--work", work],
    );
    events::hub().app_placed(ORG, SPEND_APP, work);
    replacement.expect("app.placed", work).await;
    drop(more);
    drop(replacement);
    check(
        &mut checks,
        "sign-out closes the signed-out member's stream at the next event and refuses a new one; an operator revocation of the teammate's record_read (an epoch change) closes its stream at the next event instead of delivering and refuses the next request; a second connection of the same session replaces the first; the per-member bound answers 429",
    );

    // --- SR10: the chains verify with the admission chain's new events ---
    let out = operator(&fixture, &["verify", "--organization", ORG]);
    assert_eq!(out["verified"], true);
    let events_count = scalar(&fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='admission' AND kind='permission.granted'")).await;
    assert_eq!(
        events_count, 4,
        "read, write, delegated, discover over HTTP"
    );
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM genui.audit_events WHERE writer_role<>'admission' AND writer_role<>'operator' AND kind='permission.granted'").await,
        0
    );
    // Every share record pairs with exactly one admission event and recomputes
    // its digest; no record is orphaned and no admission event lacks one.
    assert_eq!(
        scalar(&fixture, &format!("SELECT count(*) FROM genui.work_shares s JOIN genui.audit_events e ON e.organization_id=s.organization_id AND e.event_id=s.audit_event_id AND e.writer_role='admission' AND e.kind='permission.granted' AND e.actor_principal_id=s.sharer_id AND e.app_id=s.app_id AND e.work_id=s.work_id AND e.outcome_digest='sha256:'||encode(sha256(convert_to('forma:share-grant:v1','UTF8')||'\\x00'::bytea||convert_to(s.grantee_id::text,'UTF8')||'\\x00'::bytea||convert_to(array_to_string(s.granted,','),'UTF8')),'hex') WHERE s.organization_id='{ORG}'")).await,
        4
    );
    assert_eq!(
        scalar(&fixture, "SELECT count(*) FROM genui.work_shares").await,
        4
    );
    check(
        &mut checks,
        "the admission chain verifies with its permission.granted events (one per share that wrote a row), each paired with the one work_shares record it references whose inserted set recomputes the event's outcome_digest; no other chain carries the kind",
    );

    if let Some(path) = std::env::var_os("GENUI_SHARING_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"APP-095 sharing and event stream campaign (ADR-011 pitch, pivot/realtime)","postgres_image":super::qualification::IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Sessions are minted by the cfg(test) constructor after the seeded membership; the fAuth path is the session campaign's.","The owner-level share_work grant is written by fixture SQL as the operator stand-in: no operator subcommand grants share_work in Phase 1.","The event hub is in-process (ADR-007 D7.18 single task); a multi-task deployment is recorded debt."]})).unwrap()).unwrap();
    }
}
