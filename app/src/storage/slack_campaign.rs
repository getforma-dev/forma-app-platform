//! APP-100 (ADR-011 pitch) Slack campaign: the two-member story through
//! Slack, end to end, over real rows. Isolated test executable only: local
//! Docker PostgreSQL, the locked Python environments (the operator seeds the
//! library), a loopback Slack Web API stub named by `GENUI_SLACK_API_BASE`
//! (it also receives every `response_url` post), a loopback Messages stub as
//! the model (one utterance answered after 10 s), signed synthetic Slack
//! payloads (a synthetic signing secret), minted workspace sessions for the
//! link page, and the canvas event streams read as bodies.
//!
//! The story: Diego and Sarah link their Slack users at the canvas link page;
//! Diego files the Nobu dinner with `/forma`, confirms it, and the claim is
//! shared to Sarah, who gets a DM with Approve and Reject; she approves by
//! button, the DM loses its buttons, Diego gets a DM, both canvases'
//! streams receive `record.committed` and the records listing carries
//! "via Slack". Around it: the signature refusals, the 3-second ack with a
//! 10-second model, link token reuse and tampering, foreign and hostile
//! button values, a non-approver's click, a double click, a revoked link and
//! a bumped epoch, Slack escaping, and a log scan for every secret and token.
use super::{
    admission_qualification::operator,
    command_qualification::{send, vault_token, TestPages},
    definition_qualification::grant,
    qualification::Fixture,
    qualification_cases::{check, ORG, OTHER, PRINCIPAL, REVIEWER},
    *,
};
use crate::{
    actions::Parser,
    auth::{cookies, Auth, Pages},
    commands::{routes::Commands, RecordPage},
    composer::model::{ENDPOINT_ENV, KEY_ENV},
    demo, directory, events, http, sharing,
    slack::{self, flows, signature, Config, Secret, Slack},
};
use axum::{
    body::{Body, BodyDataStream},
    extract::ConnectInfo,
    http::{header, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tower::ServiceExt;

const EMPLOYEE: &str = "00000000-0000-4000-8000-00000000006f";
const OUTSIDER: &str = "00000000-0000-4000-8000-000000000ab0";
const EXPENSE_APP: &str = "00000000-0000-4000-8000-000000001681";
const EXPENSE_SOURCE: &str = "00000000-0000-4000-8000-000000003000";

const TEAM: &str = "T0NORTH";
const CANARY_TEAM: &str = "T0CANARY";
const DIEGO: &str = "U0DIEGO";
const SARAH: &str = "U0SARAH";
const PRIYA: &str = "U0PRIYA";
const STRANGER: &str = "U0OUTSIDE";
const UNLINKED: &str = "U0NOBODY";

const SIGNING_SECRET: &str = "synthetic-campaign-signing-secret";
const BOT_TOKEN: &str = "xoxb-synthetic-campaign-not-a-token";

const CREATE: &str =
    "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses";
const OPEN: &str = "what's waiting for my approval";
/// Not rehearsed: the model stub answers after 10 s.
const SLOW: &str = "please log the team offsite slowly";
const HOSTILE: &str =
    "I spent $12 on lunch at <!channel> & <https://evil.example|co>, Sarah approves my expenses";

#[test]
#[ignore = "requires local Docker and the locked migration/contract Python environments"]
fn native_slack_campaign() {
    std::thread::Builder::new()
        .name("slack-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

// --- Captured log output (the I6 scan) ---

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        std::io::stderr().write_all(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// --- The loopback Slack Web API (and response_url) stub ---

#[derive(Default)]
struct SlackStub {
    /// (route, body): `dm:<user>` for chat.postMessage into a DM opened by
    /// conversations.open, `post:<channel>` otherwise, `hook:<name>` for a
    /// response_url post.
    sent: Mutex<Vec<(String, Value)>>,
    wrong_token: AtomicUsize,
    token_on_hook: AtomicUsize,
}
impl SlackStub {
    fn all(&self) -> Vec<(String, Value)> {
        self.sent.lock().unwrap().clone()
    }
    async fn wait(&self, route: &str, within: Duration) -> Value {
        self.wait_for(route, within, |_| true).await
    }
    async fn wait_for(
        &self,
        route: &str,
        within: Duration,
        accept: impl Fn(&Value) -> bool,
    ) -> Value {
        let deadline = Instant::now() + within;
        loop {
            if let Some((_, body)) = self
                .all()
                .into_iter()
                .find(|(r, b)| r == route && accept(b))
            {
                return body;
            }
            assert!(
                Instant::now() < deadline,
                "no {route} message within {within:?}: {:?}",
                self.all()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    fn count(&self, route: &str) -> usize {
        self.all().iter().filter(|(r, _)| r == route).count()
    }
}
async fn slack_stub() -> (String, Arc<SlackStub>) {
    let stub = Arc::new(SlackStub::default());
    let api = stub.clone();
    let hooks = stub.clone();
    let app = Router::new()
        .route(
            "/api/{method}",
            axum::routing::post(
                move |axum::extract::Path(method): axum::extract::Path<String>,
                      headers: axum::http::HeaderMap,
                      body: axum::body::Bytes| {
                    let stub = api.clone();
                    async move {
                        if headers
                            .get(header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            != Some(&format!("Bearer {BOT_TOKEN}"))
                        {
                            stub.wrong_token.fetch_add(1, Ordering::SeqCst);
                            return axum::Json(json!({"ok": false, "error": "invalid_auth"}));
                        }
                        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                        match method.as_str() {
                            "conversations.open" => {
                                let user = body["users"].as_str().unwrap_or("").to_owned();
                                axum::Json(
                                    json!({"ok": true, "channel": {"id": format!("D{user}")}}),
                                )
                            }
                            "chat.postMessage" => {
                                let channel = body["channel"].as_str().unwrap_or("").to_owned();
                                let route = match channel.strip_prefix('D') {
                                    Some(user) => format!("dm:{user}"),
                                    None => format!("post:{channel}"),
                                };
                                stub.sent.lock().unwrap().push((route, body));
                                axum::Json(json!({"ok": true, "ts": "1.000"}))
                            }
                            _ => axum::Json(json!({"ok": false, "error": "unknown_method"})),
                        }
                    }
                },
            ),
        )
        .route(
            "/hook/{name}",
            axum::routing::post(
                move |axum::extract::Path(name): axum::extract::Path<String>,
                      headers: axum::http::HeaderMap,
                      body: axum::body::Bytes| {
                    let stub = hooks.clone();
                    async move {
                        if headers.contains_key(header::AUTHORIZATION) {
                            stub.token_on_hook.fetch_add(1, Ordering::SeqCst);
                        }
                        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                        stub.sent
                            .lock()
                            .unwrap()
                            .push((format!("hook:{name}"), body));
                        StatusCode::OK
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, stub)
}

// --- The Messages stub: one slow, hostile-text reply; everything else `place` ---

async fn model_stub() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = Router::new().route(
        "/v1/messages",
        axum::routing::post(move |body: axum::body::Bytes| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                let body: Value = serde_json::from_slice(&body).unwrap();
                let user = body["messages"][0]["content"].as_str().unwrap_or("");
                let utterance = serde_json::from_str::<Value>(user)
                    .ok()
                    .and_then(|v| v["utterance"].as_str().map(str::to_owned))
                    .unwrap_or_default();
                let text = if utterance.contains("slowly") {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    json!({"action": "create", "app_hint": EXPENSE_APP, "values": {"amount": "240", "merchant": "Offsite <b>&co</b>", "category": "travel", "spent_on": "2026-10-28", "note": "team offsite"}, "approver_name": "Sarah", "target_record_hint": null}).to_string()
                } else {
                    json!({"action": "place", "app_hint": "", "values": {}, "approver_name": null, "target_record_hint": null}).to_string()
                };
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    json!({"id": "msg", "model": "stub-haiku", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 400, "output_tokens": 60}}).to_string(),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (endpoint, calls)
}

// --- The canvas event stream as a body (as storage::actions_campaign) ---

struct Stream {
    body: BodyDataStream,
}
impl Stream {
    /// The next `kind` event for (app, work) within 5 s. Other events may
    /// come first: the hub judges an announcement when a stream's pump reaches
    /// it, so a commit followed at once by a share can reach the approver as
    /// either or both (a hint to re-read, never data).
    async fn expect(&mut self, kind: &str, app: &str, work: &str) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let next = tokio::time::timeout_at(
                deadline,
                std::future::poll_fn(|cx| {
                    futures_core::Stream::poll_next(Pin::new(&mut self.body), cx)
                }),
            )
            .await
            .unwrap_or_else(|_| panic!("no {kind} event within 5 s"))
            .expect("an open stream");
            let text = String::from_utf8(next.unwrap().to_vec()).unwrap();
            if text.split("\n\n").any(|frame| {
                frame.contains(&format!("event: {kind}"))
                    && frame.contains(app)
                    && frame.contains(work)
            }) {
                return;
            }
        }
    }
}
async fn open_stream(app: &Router, cookie: &str) -> Stream {
    let mut request = Request::builder()
        .uri("/v1/events")
        .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.40:4000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    Stream {
        body: response.into_body().into_data_stream(),
    }
}

struct NoCanvas;
impl crate::query::canvas::CanvasPages for NoCanvas {
    fn canvas(&self, _: &crate::query::canvas::CanvasProps) -> axum::response::Response {
        use axum::response::IntoResponse;
        (StatusCode::OK, "canvas").into_response()
    }
}

fn ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    let operator = fixture.operator().await;
    operator.client.query_one(sql, &[]).await.unwrap().get(0)
}
/// Every row the story can write in Northstar.
async fn footprint(fixture: &Fixture) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in [
        "internal_records",
        "internal_record_revisions",
        "command_outcomes",
        "saved_work",
        "work_permissions",
        "audit_events",
    ] {
        counts.push(
            scalar(
                fixture,
                &format!("SELECT count(*) FROM genui.{table} WHERE organization_id='{ORG}'"),
            )
            .await,
        );
    }
    counts
}

/// A signed Slack request, as Slack sends it.
async fn signed(
    app: &Router,
    path: &str,
    content_type: &str,
    body: &str,
) -> (StatusCode, String, u64) {
    let ts = slack::now().to_string();
    let sig = signature::sign(SIGNING_SECRET.as_bytes(), &ts, body.as_bytes());
    let started = Instant::now();
    let reply = send(
        app,
        "POST",
        path,
        None,
        body,
        &[
            ("content-type", content_type),
            ("x-slack-request-timestamp", &ts),
            ("x-slack-signature", &sig),
        ],
    )
    .await;
    (reply.status, reply.body, ms(started))
}

struct Harness {
    app: Router,
    hook_base: String,
    hooks: AtomicUsize,
}
impl Harness {
    fn hook(&self) -> (String, String) {
        let n = self.hooks.fetch_add(1, Ordering::SeqCst);
        let name = format!("h{n}");
        (
            format!("{}/hook/{name}", self.hook_base),
            format!("hook:{name}"),
        )
    }
    /// `/forma <text>` from `user` of `team`: (ack status, ack body, ack ms, hook route).
    async fn command(
        &self,
        team: &str,
        user: &str,
        text: &str,
    ) -> (StatusCode, Value, u64, String) {
        let (url, route) = self.hook();
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("team_id", team)
            .append_pair("user_id", user)
            .append_pair("command", "/forma")
            // Slack escapes &, < and > in the text it sends.
            .append_pair("text", &text::escape_like_slack(text))
            .append_pair("response_url", &url)
            .finish();
        let (status, ack, took) = signed(
            &self.app,
            slack::COMMANDS_PATH,
            "application/x-www-form-urlencoded",
            &body,
        )
        .await;
        let ack = serde_json::from_str(&ack).unwrap_or(Value::Null);
        (status, ack, took, route)
    }
    /// A button click: (ack ms, hook route).
    async fn click(&self, team: &str, user: &str, action: &str, value: &str) -> (u64, String) {
        let (url, route) = self.hook();
        let payload = json!({
            "type": "block_actions",
            "team": {"id": team},
            "user": {"id": user, "team_id": team},
            "actions": [{"type": "button", "action_id": action, "value": value}],
            "response_url": url,
        });
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("payload", &payload.to_string())
            .finish();
        let (status, ack, took) = signed(
            &self.app,
            slack::INTERACTIVITY_PATH,
            "application/x-www-form-urlencoded",
            &body,
        )
        .await;
        assert_eq!((status, ack.as_str()), (StatusCode::OK, ""));
        (took, route)
    }
}

mod text {
    pub fn escape_like_slack(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
}

/// The link token in a `/forma link` acknowledgement.
fn token_in(ack: &Value) -> String {
    let text = ack["text"].as_str().unwrap();
    let start = text.find("t=").expect("a link") + 2;
    text[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect()
}

/// Every button (action_id, value) in a message.
fn buttons(message: &Value) -> Vec<(String, String)> {
    message["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "actions")
        .flat_map(|b| b["elements"].as_array().cloned().unwrap_or_default())
        .map(|e| {
            (
                e["action_id"].as_str().unwrap_or("").to_owned(),
                e["value"].as_str().unwrap_or("").to_owned(),
            )
        })
        .collect()
}

async fn campaign() {
    let captured = Captured::default();
    let writer = captured.clone();
    let logging = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .try_init()
        .is_ok();
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    let mut timings = serde_json::Map::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    operator(&fixture, &["seed-library", "--organization", ORG]);
    for principal in [EMPLOYEE, REVIEWER] {
        for permission in ["discover", "run"] {
            grant(&fixture, principal, permission, &["--app", EXPENSE_APP]);
        }
        grant(
            &fixture,
            principal,
            "source_read",
            &["--source", EXPENSE_SOURCE],
        );
    }
    fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{OTHER}',true); INSERT INTO genui.principals VALUES ('{OTHER}','{OUTSIDER}','app008-fixture-only','ten_{OTHER}','usr_{OUTSIDER}'); INSERT INTO genui.memberships VALUES ('{OTHER}','{OUTSIDER}','active',1); INSERT INTO genui.entitlements VALUES ('{OTHER}','{OUTSIDER}','0000e000-0000-4000-8000-000000000ab0','active','member_invite','2026-01-01T00:00:00Z','2027-01-01T00:00:00Z',NULL); COMMIT;"), "genui");
    directory::set_for_tests(&[
        (EMPLOYEE, "Diego Ramos"),
        (REVIEWER, "Sarah Chen"),
        (PRINCIPAL, "Priya Natarajan"),
    ]);
    demo::configure_for_tests(true, &[ORG]);

    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();

    let (slack_origin, stub) = slack_stub().await;
    let (endpoint, model_calls) = model_stub().await;
    std::env::set_var(KEY_ENV, "sk-ant-synthetic-not-a-real-key");
    std::env::set_var(ENDPOINT_ENV, &endpoint);
    let parser = Parser::from_env();

    let pages: Arc<TestPages> = Arc::new(TestPages);
    let auth_pages: Arc<dyn Pages> = pages.clone();
    let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
    let mint = |principal: &'static str, org: &'static str| {
        let auth = auth.clone();
        let stores = stores.clone();
        async move {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            stores
                .workspace
                .mint_session_for_tests(org, principal, &reference)
                .await
                .unwrap()
        }
    };
    let diego = mint(EMPLOYEE, ORG).await;
    let sarah = mint(REVIEWER, ORG).await;
    let priya = mint(PRINCIPAL, ORG).await;
    let outsider = mint(OUTSIDER, OTHER).await;
    let state = Slack::new(
        auth.clone(),
        Config {
            signing_secret: Secret::new(SIGNING_SECRET.into()).unwrap(),
            bot_token: Secret::new(BOT_TOKEN.into()).unwrap(),
            api_base: format!("{slack_origin}/api"),
        },
        parser,
    )
    .unwrap();
    slack::notify::spawn(state.clone());
    let record_page: Arc<dyn RecordPage> = pages;
    let work = Arc::new(crate::query::Work::new(
        auth.clone(),
        None,
        Arc::new(crate::query::routes::tests::RecordingPages::default()),
    ));
    let app = http::uniform(
        Router::new()
            .merge(crate::auth::routes::router(auth.clone()))
            .merge(sharing::router(auth.clone()))
            .merge(events::routes::router(auth.clone()))
            .merge(crate::commands::routes::router(Arc::new(Commands {
                auth: auth.clone(),
                page: record_page,
            })))
            .merge(crate::actions::routes::records_router(auth.clone()))
            .merge(crate::query::canvas::router(Arc::new(
                crate::query::canvas::Canvas {
                    work,
                    pages: Arc::new(NoCanvas),
                    live_events: "/v1/events".into(),
                    demo_organizations: [ORG.to_owned()].into_iter().collect(),
                },
            )))
            .merge(slack::router(Some(state.clone()))),
    );
    let harness = Harness {
        app: app.clone(),
        hook_base: slack_origin.clone(),
        hooks: AtomicUsize::new(0),
    };
    let before = footprint(&fixture).await;

    // --- Verification before parsing ---
    let refused_before = signature::refused_count();
    let unsigned = send(
        &app,
        "POST",
        slack::COMMANDS_PATH,
        None,
        "team_id=T0NORTH&user_id=U0DIEGO&text=link",
        &[("content-type", "application/x-www-form-urlencoded")],
    )
    .await;
    assert_eq!(unsigned.status, StatusCode::UNAUTHORIZED);
    let ts = slack::now().to_string();
    let challenge = r#"{"type":"url_verification","challenge":"campaign-challenge"}"#;
    let forged = signature::sign(b"not-the-secret", &ts, challenge.as_bytes());
    let bad = send(
        &app,
        "POST",
        slack::EVENTS_PATH,
        None,
        challenge,
        &[
            ("x-slack-request-timestamp", &ts),
            ("x-slack-signature", &forged),
        ],
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert_eq!(bad.body, unsigned.body, "one uniform refusal");
    let (status, echoed, _) = signed(&app, slack::EVENTS_PATH, "application/json", challenge).await;
    assert_eq!(
        (status, echoed.as_str()),
        (StatusCode::OK, r#"{"challenge":"campaign-challenge"}"#)
    );
    assert_eq!(signature::refused_count(), refused_before + 2);
    check(&mut checks, "an unsigned command and a url_verification signed with another secret get the same uniform 401 before any parsing (the refusal counter moved by two); the challenge is echoed only under a valid signature");

    // --- An unlinked Slack user can only get the link prompt ---
    let frozen = footprint(&fixture).await;
    let (status, ack, took, route) = harness.command(TEAM, DIEGO, CREATE).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ack["text"].as_str().unwrap().contains("Working"), "{ack}");
    assert!(took < 3000);
    let prompt = stub.wait(&route, Duration::from_secs(5)).await;
    assert!(
        prompt["text"].as_str().unwrap().contains("/forma link"),
        "{prompt}"
    );
    assert!(buttons(&prompt).is_empty());
    assert_eq!(footprint(&fixture).await, frozen);
    check(&mut checks, "an unlinked Slack user's utterance is acknowledged and answered only with the link prompt; nothing is drafted or written");

    // --- Linking at the canvas link page ---
    let mut minted_tokens = Vec::new();
    let link = |cookie: String, team: &'static str, user: &'static str| {
        let harness = &harness;
        let app = app.clone();
        async move {
            let (_, ack, _, _) = harness.command(team, user, "link").await;
            let token = token_in(&ack);
            let page = send(
                &app,
                "GET",
                &format!("/slack/link?t={token}"),
                Some(&cookie),
                "",
                &[],
            )
            .await;
            assert_eq!(page.status, StatusCode::OK);
            assert!(
                page.body.contains("Link my Slack account") && page.body.contains(user),
                "{}",
                page.body
            );
            let form = format!("t={token}");
            let linked = send(
                &app,
                "POST",
                "/slack/link",
                Some(&cookie),
                &form,
                &[
                    ("sec-fetch-site", "same-origin"),
                    ("content-type", "application/x-www-form-urlencoded"),
                ],
            )
            .await;
            assert_eq!(linked.status, StatusCode::OK, "{}", linked.body);
            assert!(linked.body.contains("Slack is linked"));
            (token, form)
        }
    };
    let (diego_token, diego_form) = link(diego.clone(), TEAM, DIEGO).await;
    minted_tokens.push(diego_token.clone());
    // Reuse, tampering, no session, cross-site: refused.
    let reused = send(
        &app,
        "POST",
        "/slack/link",
        Some(&diego),
        &diego_form,
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ],
    )
    .await;
    assert_eq!(reused.status, StatusCode::NOT_FOUND);
    let (_, ack, _, _) = harness.command(TEAM, SARAH, "link").await;
    let sarah_token = token_in(&ack);
    minted_tokens.push(sarah_token.clone());
    let (payload, tag) = sarah_token.split_once('.').unwrap();
    let tampered = format!(
        "{payload}.{}{}",
        if tag.starts_with('A') { "B" } else { "A" },
        &tag[1..]
    );
    let tampered_post = send(
        &app,
        "POST",
        "/slack/link",
        Some(&sarah),
        &format!("t={tampered}"),
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ],
    )
    .await;
    assert_eq!(tampered_post.status, StatusCode::NOT_FOUND);
    let anonymous = send(
        &app,
        "GET",
        &format!("/slack/link?t={sarah_token}"),
        None,
        "",
        &[],
    )
    .await;
    assert!(anonymous.body.contains("Sign in to Forma first"));
    let cross = send(
        &app,
        "POST",
        "/slack/link",
        Some(&sarah),
        &format!("t={sarah_token}"),
        &[
            ("sec-fetch-site", "cross-site"),
            ("content-type", "application/x-www-form-urlencoded"),
        ],
    )
    .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let linked = send(
        &app,
        "POST",
        "/slack/link",
        Some(&sarah),
        &format!("t={sarah_token}"),
        &[
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ],
    )
    .await;
    assert!(linked.body.contains("Slack is linked"));
    let (priya_token, _) = link(priya.clone(), TEAM, PRIYA).await;
    let (outsider_token, _) = link(outsider.clone(), CANARY_TEAM, STRANGER).await;
    minted_tokens.extend([priya_token, outsider_token]);
    let links_north = scalar(&fixture, &format!("SELECT count(*) FROM genui.slack_links WHERE organization_id='{ORG}' AND revoked_at IS NULL")).await;
    let links_canary = scalar(&fixture, &format!("SELECT count(*) FROM genui.slack_links WHERE organization_id='{OTHER}' AND revoked_at IS NULL")).await;
    assert_eq!((links_north, links_canary), (3, 1));
    let bound = scalar(&fixture, &format!("SELECT count(*) FROM genui.slack_links WHERE organization_id='{OTHER}' AND principal_id='{OUTSIDER}' AND team_id='{CANARY_TEAM}'")).await;
    assert_eq!(
        bound, 1,
        "the outsider's link is written in the outsider's own organization"
    );
    stub.wait_for(&format!("dm:{DIEGO}"), Duration::from_secs(5), |m| {
        m["text"]
            .as_str()
            .unwrap_or("")
            .contains("linked to Forma as *Diego Ramos*")
    })
    .await;
    check(&mut checks, "linking: /forma link answers a single-use 10-minute token; the signed-in member's page names the Slack user; the confirmation binds the confirming member in its own organization (3 Northstar links, the outsider's in the canary organization); a reused or tampered token is refused, an anonymous visitor is asked to sign in, a cross-site POST is 403; the linked user gets a DM");

    // --- Both canvases are live before the story ---
    let mut diego_stream = open_stream(&app, &diego).await;
    let mut sarah_stream = open_stream(&app, &sarah).await;

    // --- Diego files with /forma: a confirmation card, nothing written ---
    let frozen = footprint(&fixture).await;
    let started = Instant::now();
    let (_, ack, took, route) = harness.command(TEAM, DIEGO, CREATE).await;
    timings.insert("command_ack_ms".into(), json!(took));
    assert!(ack["text"].as_str().unwrap().contains("Working"));
    let card = stub.wait(&route, Duration::from_secs(10)).await;
    timings.insert("command_to_card_ms".into(), json!(ms(started)));
    let shown = card.to_string();
    for expected in [
        "$86.00",
        "Nobu",
        "meals",
        "submitted",
        "Sarah Chen",
        "Nothing is written until you confirm",
    ] {
        assert!(shown.contains(expected), "{expected}: {shown}");
    }
    let card_buttons = buttons(&card);
    assert_eq!(
        card_buttons
            .iter()
            .map(|(a, _)| a.as_str())
            .collect::<Vec<_>>(),
        [flows::CONFIRM, flows::CANCEL]
    );
    let draft_id = card_buttons[0].1.clone();
    let after_draft = footprint(&fixture).await;
    assert_eq!(after_draft[0], frozen[0], "no record from a draft");
    assert_eq!(
        after_draft[3],
        frozen[3] + 1,
        "his expense claims app placed the first time"
    );
    let listed = stores
        .data
        .openable_works(&super::definition_qualification::request(
            &stores, ORG, EMPLOYEE, 1,
        ))
        .await
        .unwrap();
    let placed = listed.iter().find(|w| w.app == EXPENSE_APP).unwrap();
    let (app_id, work_id) = (placed.app.clone(), placed.work.clone());
    diego_stream.expect("app.placed", &app_id, &work_id).await;
    assert_eq!(
        model_calls.load(Ordering::SeqCst),
        0,
        "rehearsed in the demo organization"
    );
    check(&mut checks, &format!("/forma file (rehearsed): acknowledged in {took} ms; the ephemeral confirmation card ($86.00, Nobu, meals, submitted, Approver Sarah Chen, Confirm/Cancel) arrives by response_url in {} ms; his app is placed (app.placed on his canvas) and no record is written", timings["command_to_card_ms"]));

    // --- Sarah cannot confirm Diego's draft; Diego confirms (twice) ---
    let (_, route) = harness.click(TEAM, SARAH, flows::CONFIRM, &draft_id).await;
    let expired = stub.wait(&route, Duration::from_secs(5)).await;
    assert!(expired["text"].as_str().unwrap().contains("expired"));
    assert_eq!(footprint(&fixture).await[0], frozen[0]);
    let started = Instant::now();
    let (took, route) = harness.click(TEAM, DIEGO, flows::CONFIRM, &draft_id).await;
    timings.insert("confirm_ack_ms".into(), json!(took));
    let filed = stub.wait(&route, Duration::from_secs(5)).await;
    timings.insert("confirm_to_filed_ms".into(), json!(ms(started)));
    assert_eq!(filed["replace_original"], true);
    let filed_text = filed["text"].as_str().unwrap();
    assert!(
        filed_text.starts_with("Filed: *$86.00")
            && filed_text.contains("sent to *Sarah Chen* for approval"),
        "{filed_text}"
    );
    diego_stream
        .expect("record.committed", &app_id, &work_id)
        .await;
    sarah_stream.expect("app.shared", &app_id, &work_id).await;
    let (_, route) = harness.click(TEAM, DIEGO, flows::CONFIRM, &draft_id).await;
    let again = stub.wait(&route, Duration::from_secs(5)).await;
    assert!(
        again["text"].as_str().unwrap().starts_with("Filed:"),
        "a double Confirm replays the stored outcome"
    );
    let after = footprint(&fixture).await;
    assert_eq!(after[0], frozen[0] + 1, "exactly one record");
    assert_eq!(after[2], frozen[2] + 1, "exactly one command outcome");
    // The approver's DM, triggered by the hub (from any origin).
    let dm = stub
        .wait_for(&format!("dm:{SARAH}"), Duration::from_secs(5), |m| {
            !buttons(m).is_empty()
        })
        .await;
    timings.insert("confirm_to_approver_dm_ms".into(), json!(ms(started)));
    let dm_text = dm.to_string();
    for expected in ["Diego Ramos", "$86.00", "Nobu", "Approve", "Reject"] {
        assert!(dm_text.contains(expected), "{expected}: {dm_text}");
    }
    let dm_buttons = buttons(&dm);
    assert_eq!(
        dm_buttons
            .iter()
            .map(|(a, _)| a.as_str())
            .collect::<Vec<_>>(),
        [flows::APPROVE, flows::REJECT]
    );
    let record_id = dm_buttons[0].1.clone();
    assert_eq!(
        stub.count(&format!("dm:{DIEGO}")),
        1,
        "the filer is never asked to approve his own claim"
    );
    let records = send(
        &app,
        "GET",
        &format!("/v1/work/{app_id}/{work_id}/records"),
        Some(&diego),
        "",
        &[],
    )
    .await;
    let row = records.json()["records"][0].clone();
    assert_eq!(
        (
            row["record_id"].as_str().unwrap(),
            row["version"].as_i64().unwrap()
        ),
        (record_id.as_str(), 1)
    );
    assert_eq!(row["via"], "Slack", "the canvas's origin chip");
    check(&mut checks, &format!("Confirm: another Slack user's click on the draft is refused; Diego's Confirm commits exactly one audited record (record.committed on his canvas, app.shared on Sarah's) in {} ms and replaces the card with \"Filed … sent to Sarah Chen\"; a second Confirm replays the stored outcome (one record, one outcome); Sarah gets a DM with the claim and Approve/Reject in {} ms; the records listing marks the version via Slack", timings["confirm_to_filed_ms"], timings["confirm_to_approver_dm_ms"]));

    // --- Hostile and foreign clicks change nothing ---
    let frozen = footprint(&fixture).await;
    let mut refusals = Vec::new();
    for (team, user, value) in [
        (TEAM, PRIYA, record_id.as_str()), // a member who may not decide
        (CANARY_TEAM, STRANGER, record_id.as_str()), // another organization's member
        (TEAM, SARAH, "00000000-0000-4000-8000-00000000dead"), // a nonexistent record
        (TEAM, SARAH, "<script>alert(1)</script>"), // markup as a value
    ] {
        let (_, route) = harness.click(team, user, flows::APPROVE, value).await;
        let answer = stub.wait(&route, Duration::from_secs(5)).await;
        assert_eq!(
            answer["replace_original"], false,
            "a refusal never replaces the message"
        );
        refusals.push(answer["text"].as_str().unwrap().to_owned());
    }
    assert!(
        refusals.iter().all(|r| r == &refusals[0]),
        "one uniform refusal: {refusals:?}"
    );
    let (_, route) = harness
        .click(TEAM, UNLINKED, flows::APPROVE, &record_id)
        .await;
    assert!(stub.wait(&route, Duration::from_secs(5)).await["text"]
        .as_str()
        .unwrap()
        .contains("/forma link"));
    let (_, route) = harness
        .click("T0EVIL", SARAH, flows::APPROVE, &record_id)
        .await;
    assert!(
        stub.wait(&route, Duration::from_secs(5)).await["text"]
            .as_str()
            .unwrap()
            .contains("/forma link"),
        "the same user id under another team is not Sarah"
    );
    let (_, route) = harness
        .click(TEAM, SARAH, flows::APPROVE, &"f".repeat(80))
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(stub.count(&route), 0, "an oversized value is ignored");
    assert_eq!(footprint(&fixture).await, frozen);
    check(&mut checks, "hostile clicks: a non-approver member, another organization's linked member, a nonexistent record id and markup as the value all get the same refusal without replacing the message; an unlinked user and Sarah's user id under another team get the link prompt; an oversized value is ignored; nothing changed");

    // --- Escaping and the 3-second ack with a 10-second model ---
    let frozen = footprint(&fixture).await;
    let (_, ack, took, route) = harness.command(TEAM, DIEGO, HOSTILE).await;
    assert!(took < 3000 && ack["text"].as_str().unwrap().contains("Working"));
    let hostile_card = stub.wait(&route, Duration::from_secs(10)).await;
    let hostile_text = hostile_card.to_string();
    assert!(
        hostile_text.contains("&lt;!channel&gt;") || hostile_text.contains("&amp;"),
        "{hostile_text}"
    );
    let started = Instant::now();
    let (_, ack, took, route) = harness.command(TEAM, DIEGO, SLOW).await;
    timings.insert("slow_model_ack_ms".into(), json!(took));
    assert!(took < 3000, "acknowledged in {took} ms with a 10 s model");
    assert!(ack["text"].as_str().unwrap().contains("Working"));
    let slow_card = stub.wait(&route, Duration::from_secs(20)).await;
    timings.insert("slow_model_card_ms".into(), json!(ms(started)));
    let slow_text = slow_card.to_string();
    assert!(
        slow_text.contains("Offsite &lt;b&gt;&amp;co&lt;/b&gt;"),
        "{slow_text}"
    );
    let (_, route) = harness
        .click(TEAM, DIEGO, flows::CANCEL, &buttons(&slow_card)[1].1)
        .await;
    assert!(stub.wait(&route, Duration::from_secs(5)).await["text"]
        .as_str()
        .unwrap()
        .starts_with("Cancelled"));
    assert_eq!(
        footprint(&fixture).await[0],
        frozen[0],
        "drafts and a cancel write no record"
    );
    for (_, body) in stub.all() {
        let body = body.to_string();
        assert!(
            !body.contains("<!channel") && !body.contains("<https://evil") && !body.contains("<b>"),
            "unescaped user text posted: {body}"
        );
    }
    check(&mut checks, &format!("a 10-second model: the command is acknowledged in {} ms and the card arrives by response_url in {} ms; user text in every posted message is Slack-escaped (no <!channel>, link or tag survives); Cancel writes nothing", timings["slow_model_ack_ms"], timings["slow_model_card_ms"]));

    // --- Sarah: what's waiting, then Approve by button ---
    let (_, _, _, route) = harness.command(TEAM, SARAH, OPEN).await;
    let waiting = stub.wait(&route, Duration::from_secs(10)).await;
    assert!(
        waiting["text"]
            .as_str()
            .unwrap()
            .starts_with("1 waiting for your approval"),
        "{waiting}"
    );
    assert_eq!(
        buttons(&waiting)[0],
        (flows::APPROVE.to_owned(), record_id.clone())
    );
    let started = Instant::now();
    let (took, route) = harness.click(TEAM, SARAH, flows::APPROVE, &record_id).await;
    timings.insert("approve_ack_ms".into(), json!(took));
    let approved = stub.wait(&route, Duration::from_secs(5)).await;
    timings.insert("approve_to_replaced_ms".into(), json!(ms(started)));
    assert_eq!(approved["replace_original"], true);
    assert!(approved["text"]
        .as_str()
        .unwrap()
        .starts_with("Approved by Sarah Chen"));
    assert!(
        buttons(&approved).is_empty(),
        "the replaced message has no buttons"
    );
    diego_stream
        .expect("record.committed", &app_id, &work_id)
        .await;
    sarah_stream
        .expect("record.committed", &app_id, &work_id)
        .await;
    let notice = stub
        .wait_for(&format!("dm:{DIEGO}"), Duration::from_secs(5), |m| {
            m["text"].as_str().unwrap_or("").starts_with("Your ")
        })
        .await;
    timings.insert("approve_to_filer_dm_ms".into(), json!(ms(started)));
    assert_eq!(
        notice["text"],
        "Your Nobu claim ($86.00) was *approved* by *Sarah Chen*."
    );
    let records = send(
        &app,
        "GET",
        &format!("/v1/work/{app_id}/{work_id}/records"),
        Some(&sarah),
        "",
        &[],
    )
    .await;
    let row = records.json()["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["record_id"] == record_id.as_str())
        .unwrap()
        .clone();
    assert_eq!(row["version"], 2);
    assert_eq!(row["via"], "Slack");
    assert_eq!(
        row["updater"]["principal_id"], REVIEWER,
        "decided as the clicker"
    );
    let status = row["values"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["field_id"] == "status")
        .unwrap()["value"]["value"]
        .clone();
    assert_eq!(status, "approved");
    let frozen = footprint(&fixture).await;
    let (_, route) = harness.click(TEAM, SARAH, flows::APPROVE, &record_id).await;
    let twice = stub.wait(&route, Duration::from_secs(5)).await;
    assert_eq!(twice["text"], "This claim was already decided.");
    let (_, route) = harness.click(TEAM, SARAH, flows::REJECT, &record_id).await;
    assert_eq!(
        stub.wait(&route, Duration::from_secs(5)).await["text"],
        "This claim was already decided."
    );
    assert_eq!(footprint(&fixture).await, frozen);
    assert_eq!(
        operator(&fixture, &["verify", "--organization", ORG])["verified"],
        true
    );
    check(&mut checks, &format!("Approve by button: decided as Sarah through the audited route (version 2, approved, updater Sarah, via Slack); the DM is replaced in {} ms with \"Approved by Sarah Chen\" and no buttons; record.committed on both canvases; Diego's DM \"Your Nobu claim ($86.00) was approved by Sarah Chen\" in {} ms; a second Approve or a Reject on the decided record is refused without a change; the audit chains verify", timings["approve_to_replaced_ms"], timings["approve_to_filer_dm_ms"]));

    // --- A bumped epoch and a revoked link end Slack authority ---
    let sarah_request = stores
        .workspace
        .resolve_slack_link(TEAM, SARAH)
        .await
        .unwrap()
        .unwrap();
    stores.workspace.sign_out_all(&sarah_request).await.unwrap();
    let (_, _, _, route) = harness.command(TEAM, SARAH, OPEN).await;
    assert!(stub.wait(&route, Duration::from_secs(5)).await["text"]
        .as_str()
        .unwrap()
        .contains("/forma link"));
    let (_, _, _, route) = harness.command(TEAM, DIEGO, "unlink").await;
    assert!(stub.wait(&route, Duration::from_secs(5)).await["text"]
        .as_str()
        .unwrap()
        .starts_with("Disconnected"));
    let (_, _, _, route) = harness.command(TEAM, DIEGO, OPEN).await;
    assert!(stub.wait(&route, Duration::from_secs(5)).await["text"]
        .as_str()
        .unwrap()
        .contains("/forma link"));
    check(&mut checks, "after Sarah's sign-out-all advanced her epoch her Slack link answers only the link prompt; after /forma unlink Diego's does too");

    // --- I6: no secret or token in the log, the token only in the header ---
    assert_eq!(stub.wrong_token.load(Ordering::SeqCst), 0);
    assert_eq!(
        stub.token_on_hook.load(Ordering::SeqCst),
        0,
        "response_url posts carry no bot token"
    );
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    if logging {
        assert!(
            log.contains("slack.decided") && log.contains("slack.confirmed"),
            "the capture saw the flows"
        );
    }
    for secret in [BOT_TOKEN, SIGNING_SECRET]
        .iter()
        .map(|s| s.to_string())
        .chain(minted_tokens.iter().cloned())
    {
        assert!(
            !log.contains(&secret),
            "a secret or link token reached the log"
        );
    }
    for (_, body) in stub.all() {
        let body = body.to_string();
        assert!(!body.contains(BOT_TOKEN) && !body.contains(SIGNING_SECRET));
        assert!(minted_tokens.iter().all(|t| !body.contains(t.as_str())));
    }
    check(&mut checks, "I6: the bot token travels only in the Authorization header of Web API calls (never to a response_url); the captured log and every posted message contain no bot token, signing secret or link token");
    assert!(footprint(&fixture).await[0] > before[0]);

    let report = json!({
        "campaign": "APP-100 Slack: link, file, approve and notify through the governed action path (synthetic identities and Slack payloads, loopback Slack and model stubs, Docker)",
        "passed": true,
        "timings_ms": timings,
        "checks": checks,
    });
    if let Some(path) = std::env::var_os("GENUI_SLACK_CAMPAIGN_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    std::env::remove_var(KEY_ENV);
    std::env::remove_var(ENDPOINT_ENV);
    demo::configure_for_tests(false, &[]);
    directory::set_for_tests(&[]);
    drop(diego_stream);
    drop(sarah_stream);
    stores.close().await.unwrap();
}
