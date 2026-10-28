//! Local callback/transport fixtures. FixedIdentity is NOT hosted or canonical
//! cryptographic verification evidence; the canonical crate tests run separately.
use super::*;
use axum::{body::Body, extract::Form};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{net::TcpListener, task::JoinHandle};
use tower::ServiceExt;

const SUBJECT: &str = "usr_00000000-0000-4000-8000-000000000001";
const TENANT: &str = "ten_00000000-0000-4000-8000-000000000001";
const EMAIL: &str = "callback@example.test";
const ACCESS: &str = "synthetic-access-token-not-a-jwt";

struct FixedIdentity {
    tenant: String,
    subject: String,
    issued_at: u64,
    expires_at: u64,
}
impl IdentityCheck for FixedIdentity {
    fn check<'a>(&'a self, token: &'a str, _tenant: &'a str) -> IdentityFuture<'a> {
        Box::pin(async move {
            if token != ACCESS {
                return Err(Failure::Identity);
            }
            Ok(VerifiedIdentity {
                issuer: "synthetic-issuer".into(),
                subject: self.subject.clone(),
                tenant_id: self.tenant.clone(),
                key_id: "synthetic-key".into(),
                issued_at: self.issued_at,
                expires_at: self.expires_at,
            })
        })
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Redirect,
    Oversized,
    InvalidJson,
    MissingVerification,
    WrongVerification,
    WrongSubject,
    WrongEmail,
    Dpop,
    Timeout,
    LogoutFailure,
    LogoutTimeout,
}
struct ProviderState {
    mode: Mode,
    body: Value,
    code: String,
    refresh: String,
    par: Mutex<HashMap<String, String>>,
    exchanges: AtomicUsize,
    logouts: AtomicUsize,
    redirects_followed: AtomicUsize,
}
struct Provider {
    origin: String,
    state: Arc<ProviderState>,
    task: JoinHandle<()>,
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Provider {
    async fn start(mode: Mode) -> Self {
        let timestamp = now();
        let refresh = random_capability().unwrap();
        let body = json!({"access_token":ACCESS,"refresh_token":refresh,"token_type":"Bearer","expires_in":900,"expires_at":timestamp+900,
            "user":{"id":SUBJECT,"email":EMAIL,"name":null,"avatar":null,"email_verified":false,"roles":["user"],"auth_method":"password","mfa_verified":false}});
        let state = Arc::new(ProviderState {
            mode,
            body,
            code: random_capability().unwrap(),
            refresh,
            par: Mutex::new(HashMap::new()),
            exchanges: AtomicUsize::new(0),
            logouts: AtomicUsize::new(0),
            redirects_followed: AtomicUsize::new(0),
        });
        let app = Router::new()
            .route("/t/synthetic/oauth/par", post(par))
            .route("/t/synthetic/auth/token", post(token))
            .route("/t/synthetic/auth/logout", post(logout))
            .route("/unexpected", get(unexpected))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            state,
            task,
        }
    }
}
async fn par(
    State(state): State<Arc<ProviderState>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    assert_eq!(form["provider"], "email");
    assert_eq!(form["client_id"], "synthetic-client");
    assert_eq!(form["redirect_uri"], CALLBACK);
    assert_eq!(form["response_type"], "code");
    assert_eq!(form["code_challenge_method"], "S256");
    assert!(capability(&form["state"]));
    assert_eq!(form["invite_token"], "synthetic-confidential-invitation");
    assert_eq!(form.len(), 8);
    *state.par.lock().unwrap() = form;
    (StatusCode::CREATED, Json(json!({"request_uri":"urn:ietf:params:oauth:request_uri:0123456789abcdef0123456789abcdef","expires_in":60}))).into_response()
}
async fn token(State(state): State<Arc<ProviderState>>, Json(body): Json<Value>) -> Response {
    state.exchanges.fetch_add(1, Ordering::SeqCst);
    assert_eq!(body["code"], state.code);
    assert_eq!(body["grant_type"], "authorization_code");
    assert_eq!(body["client_id"], "synthetic-client");
    assert_eq!(body["redirect_uri"], CALLBACK);
    let verifier = body["code_verifier"].as_str().unwrap();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    assert_eq!(challenge, state.par.lock().unwrap()["code_challenge"]);
    assert_eq!(body.as_object().unwrap().len(), 5);
    let mut body = state.body.clone();
    match state.mode {
        Mode::Redirect => {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [(header::LOCATION, "/unexpected")],
            )
                .into_response()
        }
        Mode::Oversized => {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                "x".repeat(MAX_BODY + 1),
            )
                .into_response()
        }
        Mode::InvalidJson => {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                "{truncated",
            )
                .into_response()
        }
        Mode::MissingVerification => {
            body["user"]
                .as_object_mut()
                .unwrap()
                .remove("email_verified");
        }
        Mode::WrongVerification => body["user"]["email_verified"] = json!(true),
        Mode::WrongSubject => {
            body["user"]["id"] = json!("usr_00000000-0000-4000-8000-000000000002")
        }
        Mode::WrongEmail => body["user"]["email"] = json!("another@example.test"),
        Mode::Dpop => body["token_type"] = json!("DPoP"),
        Mode::Timeout => tokio::time::sleep(PROVIDER_DEADLINE + Duration::from_millis(300)).await,
        _ => {}
    }
    let mut response = Json(body).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("gw_rt_synthetic=provider-cookie-secret; Secure; HttpOnly"),
    );
    response
}
async fn logout(
    State(state): State<Arc<ProviderState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> StatusCode {
    state.logouts.fetch_add(1, Ordering::SeqCst);
    assert_eq!(headers[header::AUTHORIZATION], format!("Bearer {ACCESS}"));
    assert_eq!(body, json!({"refresh_token":state.refresh}));
    match state.mode {
        Mode::LogoutFailure => StatusCode::SERVICE_UNAVAILABLE,
        Mode::LogoutTimeout => {
            tokio::time::sleep(PROVIDER_DEADLINE + Duration::from_millis(300)).await;
            StatusCode::NO_CONTENT
        }
        _ => StatusCode::NO_CONTENT,
    }
}
async fn unexpected(State(state): State<Arc<ProviderState>>) -> StatusCode {
    state.redirects_followed.fetch_add(1, Ordering::SeqCst);
    StatusCode::OK
}
fn config() -> Config {
    Config {
        run_id: "synthetic-callback-run".into(),
        app_source_revision: "0000000000000000000000000000000000000000".into(),
        provider_source_revision: "dfba787295b880816fd95295b507a4527c51e62f".into(),
        expires_at: now() + 600,
        max_attempts: 6,
        max_flows: 3,
        synthetic_tenant: true,
        auth_origin: "https://auth-staging.getforma.dev".into(),
        tenant_slug: "synthetic".into(),
        tenant_id: TENANT.into(),
        client_id: "synthetic-client".into(),
        issuer: "synthetic-issuer".into(),
        audience: "synthetic-audience".into(),
        max_access_lifetime_seconds: 900,
        expected_email: EMAIL.into(),
        expected_subject: None,
        expected_email_verified: false,
        operator_token: random_capability().unwrap(),
        invite_token: Some("synthetic-confidential-invitation".into()),
    }
}
fn qualification(provider: &Provider) -> Qualification {
    let mut state = Qualification::new(config()).unwrap();
    // Test-only route/transport fixture. The production constructor can neither
    // select HTTP nor supply this noncryptographic checker.
    state.config.auth_origin = provider.origin.clone();
    let exp = provider.state.body["expires_at"].as_u64().unwrap();
    state.verifier = Arc::new(FixedIdentity {
        tenant: TENANT.into(),
        subject: SUBJECT.into(),
        issued_at: exp - 900,
        expires_at: exp,
    });
    state
}
async fn begin(state: &Arc<Qualification>, provider: &Provider) -> (String, String, String) {
    let request = Request::builder()
        .method("POST")
        .uri("/auth/qualification/start")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", state.config.operator_token),
        )
        .body(Body::empty())
        .unwrap();
    let response = routes(state.clone()).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=300"));
    let cookie = cookie.split(';').next().unwrap().to_owned();
    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert!(!location.contains(&state.config.operator_token));
    assert!(!location.contains("synthetic-confidential-invitation"));
    let location = Url::parse(location).unwrap();
    assert_eq!(location.origin().ascii_serialization(), provider.origin);
    assert_eq!(location.path(), "/t/synthetic/auth/authorize");
    let mut query: HashMap<String, String> = location.query_pairs().into_owned().collect();
    if state.config.invite_token.is_some() {
        assert_eq!(query.len(), 2);
        assert!(query.contains_key("request_uri"));
        query = provider.state.par.lock().unwrap().clone();
    } else {
        *provider.state.par.lock().unwrap() = query.clone();
    }
    (cookie, query["state"].clone(), provider.state.code.clone())
}
async fn complete(
    state: &Arc<Qualification>,
    cookie: &str,
    returned_state: &str,
    code: &str,
) -> Response {
    routes(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/auth/callback?code={code}&state={returned_state}"))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn assert_clean(response: Response, provider: &Provider) {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], RESULT);
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    assert!(response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    let headers = format!("{:?}", response.headers());
    let body =
        String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec()).unwrap();
    for secret in [
        ACCESS,
        &provider.state.refresh,
        &provider.state.code,
        "provider-cookie-secret",
        "synthetic-confidential-invitation",
    ] {
        assert!(!headers.contains(secret));
        assert!(!body.contains(secret));
    }
}

#[tokio::test]
async fn real_router_and_backend_par_exchange_create_no_app_authority() {
    let provider = Provider::start(Mode::Normal).await;
    let state = Arc::new(qualification(&provider));
    let (cookie, s, c) = begin(&state, &provider).await;
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    {
        let ledger = state.ledger.lock().unwrap();
        assert_eq!(ledger.counters.identity_checked, 1);
        assert_eq!(ledger.counters.unverified_mailbox, 1);
        assert_eq!(ledger.counters.logout_acknowledged, 1);
        assert_eq!(ledger.counters.cleanup_unknown, 0);
        assert!(ledger.flows.is_empty());
    }
    for path in [
        "/workspace",
        "/api/query",
        "/api/import",
        "/api/generate",
        "/api/export",
    ] {
        assert_eq!(
            routes(state.clone())
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::COOKIE, &cookie)
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    let receipt = routes(state.clone())
        .oneshot(
            Request::builder()
                .uri("/auth/qualification/status")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", state.config.operator_token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(receipt.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["workspace_access_granted"], false);
    assert_eq!(value["provider_revocation_proved"], false);
    assert_eq!(value["mailbox_delivery_proved"], false);
    assert_eq!(value["run_id"], "synthetic-callback-run");
    assert_eq!(value["fauth_tenant_id"], TENANT);
    assert_eq!(value["exchanges"][0]["verified_subject"], SUBJECT);
    assert_eq!(value["exchanges"][0]["email_verified"], false);
    assert_eq!(value["exchanges"][0]["identity_qualified"], true);
}
#[tokio::test]
async fn existing_login_uses_server_bound_pkce_and_expected_subject() {
    let provider = Provider::start(Mode::Normal).await;
    let mut inner = qualification(&provider);
    inner.config.invite_token = None;
    inner.config.expected_subject = Some(SUBJECT.into());
    let state = Arc::new(inner);
    let (cookie, s, c) = begin(&state, &provider).await;
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    assert_eq!(state.ledger.lock().unwrap().counters.identity_checked, 1);
}
#[tokio::test]
async fn concurrent_and_sequential_replay_exchange_exactly_once() {
    let provider = Provider::start(Mode::Normal).await;
    let state = Arc::new(qualification(&provider));
    let (cookie, s, c) = begin(&state, &provider).await;
    let (a, b) = tokio::join!(
        complete(&state, &cookie, &s, &c),
        complete(&state, &cookie, &s, &c)
    );
    assert_clean(a, &provider).await;
    assert_clean(b, &provider).await;
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 1);
    assert_eq!(state.ledger.lock().unwrap().counters.identity_checked, 1);
}
#[tokio::test]
async fn operator_and_caller_selected_inputs_never_start_provider_work() {
    let provider = Provider::start(Mode::Normal).await;
    let state = Arc::new(qualification(&provider));
    for (uri, authorization, body) in [
        ("/auth/qualification/start", String::new(), ""),
        (
            "/auth/qualification/start",
            format!("Bearer {}", random_capability().unwrap()),
            "",
        ),
        (
            "/auth/qualification/start?tenant=foreign",
            format!("Bearer {}", state.config.operator_token),
            "",
        ),
        (
            "/auth/qualification/start",
            format!("Bearer {}", state.config.operator_token),
            "{\"role\":\"admin\"}",
        ),
    ] {
        let response = routes(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::AUTHORIZATION, authorization)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert!(provider.state.par.lock().unwrap().is_empty());
    assert_eq!(state.ledger.lock().unwrap().counters.starts, 0);
}
#[tokio::test]
async fn state_cookie_and_parser_refusals_never_exchange() {
    let provider = Provider::start(Mode::Normal).await;
    let state = Arc::new(qualification(&provider));
    let (cookie, s, c) = begin(&state, &provider).await;
    for query in [
        format!("code={c}"),
        format!("code={c}&state={s}&state={s}"),
        format!("code={c}&state={s}&tenant=foreign"),
        format!("code=%41{c}&state={s}"),
        format!("code={}&state={s}", "A".repeat(300)),
    ] {
        let response = routes(state.clone())
            .oneshot(
                Request::builder()
                    .uri(format!("/auth/callback?{query}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_clean(response, &provider).await;
    }
    assert_clean(complete(&state, "", &s, &c).await, &provider).await;
    assert_clean(
        complete(&state, &format!("{cookie}; {cookie}"), &s, &c).await,
        &provider,
    )
    .await;
    assert_clean(
        complete(&state, &cookie, &random_capability().unwrap(), &c).await,
        &provider,
    )
    .await;
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 0);
    assert!(state.ledger.lock().unwrap().flows.is_empty());
}
#[tokio::test]
async fn expiry_restart_and_attempt_limits_fail_closed() {
    let provider = Provider::start(Mode::Normal).await;
    let mut inner = qualification(&provider);
    inner.config.max_attempts = 1;
    let state = Arc::new(inner);
    let (cookie, s, c) = begin(&state, &provider).await;
    let second = routes(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/qualification/start")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", state.config.operator_token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::FORBIDDEN);
    let restarted = Arc::new(qualification(&provider));
    assert_clean(complete(&restarted, &cookie, &s, &c).await, &provider).await;
    for flow in state.ledger.lock().unwrap().flows.values_mut() {
        flow.deadline = Instant::now() - Duration::from_secs(1);
    }
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn foreign_verified_tenant_is_refused_and_logout_is_requested() {
    let provider = Provider::start(Mode::Normal).await;
    let mut inner = qualification(&provider);
    let exp = provider.state.body["expires_at"].as_u64().unwrap();
    inner.verifier = Arc::new(FixedIdentity {
        tenant: "foreign".into(),
        subject: SUBJECT.into(),
        issued_at: exp - 900,
        expires_at: exp,
    });
    let state = Arc::new(inner);
    let (cookie, s, c) = begin(&state, &provider).await;
    assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
    assert_eq!(state.ledger.lock().unwrap().counters.identity_checked, 0);
    assert_eq!(provider.state.logouts.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn token_shape_binding_redirect_body_and_cleanup_refusals() {
    for mode in [
        Mode::Redirect,
        Mode::Oversized,
        Mode::InvalidJson,
        Mode::MissingVerification,
        Mode::WrongVerification,
        Mode::WrongSubject,
        Mode::WrongEmail,
        Mode::Dpop,
        Mode::LogoutFailure,
    ] {
        let provider = Provider::start(mode).await;
        let state = Arc::new(qualification(&provider));
        let (cookie, s, c) = begin(&state, &provider).await;
        assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
        assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
        assert_eq!(state.ledger.lock().unwrap().counters.identity_checked, 0);
        assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 1);
        assert_eq!(provider.state.redirects_followed.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn token_and_logout_timeouts_are_bounded_and_never_retry() {
    for mode in [Mode::Timeout, Mode::LogoutTimeout] {
        let provider = Provider::start(mode).await;
        let state = Arc::new(qualification(&provider));
        let (cookie, s, c) = begin(&state, &provider).await;
        let started = Instant::now();
        assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
        assert!(started.elapsed() < PROVIDER_DEADLINE + Duration::from_secs(1));
        assert_clean(complete(&state, &cookie, &s, &c).await, &provider).await;
        let ledger = state.ledger.lock().unwrap();
        assert_eq!(ledger.counters.identity_checked, 0);
        assert_eq!(ledger.counters.cleanup_unknown, 1);
        assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 1);
    }
}

struct PendingIdentity(Arc<AtomicUsize>);
impl IdentityCheck for PendingIdentity {
    fn check<'a>(&'a self, _token: &'a str, _tenant: &'a str) -> IdentityFuture<'a> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::future::pending().await
        })
    }
}

#[tokio::test]
async fn canceled_exchange_or_verification_keeps_cleanup_unresolved() {
    for during_verification in [false, true] {
        let provider = Provider::start(if during_verification {
            Mode::Normal
        } else {
            Mode::Timeout
        })
        .await;
        let entered = Arc::new(AtomicUsize::new(0));
        let mut inner = qualification(&provider);
        if during_verification {
            inner.verifier = Arc::new(PendingIdentity(entered.clone()));
        }
        let state = Arc::new(inner);
        let (cookie, returned_state, code) = begin(&state, &provider).await;
        let (owned_state, owned_cookie, owned_returned, owned_code) = (
            state.clone(),
            cookie.clone(),
            returned_state.clone(),
            code.clone(),
        );
        let task = tokio::spawn(async move {
            complete(&owned_state, &owned_cookie, &owned_returned, &owned_code).await
        });
        timeout(Duration::from_secs(1), async {
            while if during_verification {
                entered.load(Ordering::SeqCst) == 0
            } else {
                provider.state.exchanges.load(Ordering::SeqCst) == 0
            } {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_clean(
            complete(&state, &cookie, &returned_state, &code).await,
            &provider,
        )
        .await;
        let ledger = state.ledger.lock().unwrap();
        assert_eq!(ledger.counters.cleanup_unknown, 1);
        assert_eq!(ledger.counters.identity_checked, 0);
        assert_eq!(ledger.counters.logout_acknowledged, 0);
        assert!(ledger.flows.is_empty());
        assert_eq!(provider.state.exchanges.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn production_configuration_cannot_select_test_transport_or_identity() {
    let mut cfg = config();
    cfg.auth_origin = "http://127.0.0.1:3247".into();
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.expires_at = now() + 21601;
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.synthetic_tenant = false;
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.max_attempts = 13;
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.invite_token = None;
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.auth_origin = "https://auth.example.test/untrusted".into();
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.tenant_id = "00000000-0000-4000-8000-000000000001".into();
    assert!(Qualification::new(cfg).is_err());
    let mut cfg = config();
    cfg.client_id = "reserved-hosted".into();
    assert!(Qualification::new(cfg).is_err());
}
