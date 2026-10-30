//! APP-009 tests. The unit tests need nothing; `native_session_campaign`
//! (`#[ignore]`) drives every ADR-006 route over the real router against local
//! PostgreSQL 16.13 with verified TLS (the APP-008 Docker fixture), the real
//! operator command, the reviewed verifier and the cfg(test) synthetic
//! loopback issuer. SIMULATED provider; synthetic identifiers only.
//!
//! Since APP-074 the invited signup goes through the real `POST
//! /invite/validate` route (the labeled stand-in is gone): codes are issued
//! with a server-only provider invitation through the loopback management
//! stand-in (`fake_fauth.py`), whose capability file the route reads, and the
//! synthetic issuer's hosted registration consumes that invitation. The
//! browser, page and fixture helpers are shared with `auth::invite_tests`.
use super::{
    pkce::Pkce,
    provider::{Client, Failure, Resend},
    routes,
    synthetic_issuer::{
        Issuer, RefreshMode, ResendMode, Tenant, TokenMode, AUDIENCE, CALLBACK, ISSUER,
        ISSUER_COOKIE,
    },
    vault::AccessFailure,
    *,
};
use crate::{
    commands::{self, RecordPage, RecordView},
    http,
    query::{self, page::WorkPages, page::WorkProps},
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{header, HeaderMap, Request, StatusCode},
    response::IntoResponse,
    Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::ServiceExt;

/// The two fixture organizations (tenant_seed.py) and their synthetic fAuth
/// tenants. The live identifiers are APP-076's; these are local only.
pub(crate) const ORG: &str = "00000000-0000-4000-8000-000000000001";
pub(crate) const OTHER: &str = "00000000-0000-4000-8000-000000000002";
pub(crate) const APP: &str = "00000000-0000-4000-8000-000000000065";
pub(crate) const NORTHSTAR_TENANT: &str = "ten_00000000-0000-4000-8000-00000000a001";
pub(crate) const CANARY_TENANT: &str = "ten_00000000-0000-4000-8000-00000000a002";
/// The sign-in handles this slice issued for the two fixture organizations
/// (non-secret routing; evidence/APP-009/handles.json). The campaign installs
/// them with `admission_operator.py issue-handle --handle`.
pub(crate) const NORTHSTAR_HANDLE: &str = "se45m37vabqrfvbh3gbwn22qkm";
pub(crate) const CANARY_HANDLE: &str = "z33gibxjspwxcnopbsdxqtijey";
/// Well formed, never issued.
pub(crate) const UNKNOWN_HANDLE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaa";
pub(crate) const APP_ORIGIN: &str = "https://beta.getforma.dev";

pub(crate) struct TestPages;
fn page(text: String) -> Response {
    let mut response = (StatusCode::OK, text).into_response();
    http::secure(response.headers_mut());
    response
}
impl Pages for TestPages {
    fn invite(&self, v: &InviteView) -> Response {
        page(format!(
            "page=invite;failed={};limited={}{}",
            v.failed,
            v.limited.map_or("false", |w| w.as_str()),
            if v.unavailable { ";unavailable" } else { "" }
        ))
    }
    fn signin(&self, v: &SigninView) -> Response {
        page(format!(
            "page=signin;action={};failed={}",
            v.action.as_deref().unwrap_or("-"),
            v.failed
        ))
    }
    fn pending(&self, v: &PendingView) -> Response {
        page(format!(
            "page=pending;resend={};signout={};available={};cooldown={};hours={};result={:?}",
            v.csrf_resend,
            v.csrf_signout,
            v.resend_available,
            v.cooldown_seconds,
            v.hours_remaining,
            v.result
        ))
    }
    fn denied(&self, v: &DeniedView) -> Response {
        page(format!("page=denied;signout={}", v.csrf_signout))
    }
    fn workspace(&self, v: &WorkspaceView) -> Response {
        page(format!(
            "page=workspace;signout={};signout_all={}",
            v.csrf_signout, v.csrf_signout_all
        ))
    }
}
pub(crate) fn field(body: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    body.split(';')
        .find_map(|part| part.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("field {name} in {body}"))
        .to_owned()
}

pub(crate) fn config(origin: &str) -> AuthConfig {
    AuthConfig {
        auth_origin: origin.into(),
        app_origin: APP_ORIGIN.into(),
        callback: CALLBACK.into(),
        issuer: ISSUER.into(),
        audience: AUDIENCE.into(),
        max_access_lifetime: Duration::from_secs(900),
        routes: vec![
            Route {
                organization: ORG.into(),
                tenant_slug: "northstar".into(),
                client_id: "northstar-platform".into(),
            },
            Route {
                organization: OTHER.into(),
                tenant_slug: "canary".into(),
                client_id: "canary-platform".into(),
            },
        ],
    }
}
/// APP-078 B1: the AuthConfiguration secret value the window composer writes
/// (`window_requests.py auth-configuration`), with this fixture's identifiers:
/// the synthetic token issuer and audience, both organizations' routes and
/// their `ten_` bindings. The auth origin is a synthetic https origin: the
/// cfg(test) loopback issuer speaks plain HTTP on 127.0.0.1, so the campaign
/// substitutes its origin after loading (the one field it does not take from
/// the delivered file).
pub(crate) const SYNTHETIC_AUTH_ORIGIN: &str = "https://auth-synthetic.invalid";
pub(crate) fn auth_configuration_document(registry_version: &str) -> Value {
    json!({
        "format": "forma:genui-auth-configuration:v1",
        "auth_config": {
            "schema_version": 1, "auth_origin": SYNTHETIC_AUTH_ORIGIN, "app_origin": APP_ORIGIN,
            "issuer": ISSUER, "audience": AUDIENCE, "max_access_lifetime_seconds": 900,
            "organizations": [
                {"organization_id": ORG, "tenant_slug": "northstar", "client_id": "northstar-platform"},
                {"organization_id": OTHER, "tenant_slug": "canary", "client_id": "canary-platform"},
            ],
        },
        "identity_registry": {
            "schema_version": 1, "registry_version": registry_version,
            "bindings": [
                {"issuer": ISSUER, "provider_tenant_id": NORTHSTAR_TENANT, "organization_id": ORG},
                {"issuer": ISSUER, "provider_tenant_id": CANARY_TENANT, "organization_id": OTHER},
            ],
        },
    })
}
/// The configuration as the deployment delivers it: the operator's
/// `bind-provider-tenant` rows, the composed secret value put into the
/// loopback Secrets Manager stand-in, materialized by the supervisor's own
/// writer as the two private files, which the campaign's `Stores` (registry)
/// and `AuthConfig::load` then read. Returns the loaded configuration with
/// the loopback issuer's origin substituted, and the registry file path.
pub(crate) fn deliver_auth_configuration(
    fixture: &Fixture,
    issuer_origin: &str,
    registry_version: &str,
) -> (
    AuthConfig,
    std::path::PathBuf,
    crate::storage::admission_qualification::FakeSecret,
) {
    use crate::storage::admission_qualification::{operator, FakeSecret};
    use std::os::unix::fs::PermissionsExt;
    for (org, tenant) in [(ORG, NORTHSTAR_TENANT), (OTHER, CANARY_TENANT)] {
        let bound = operator(
            fixture,
            &[
                "bind-provider-tenant",
                "--organization",
                org,
                "--issuer",
                ISSUER,
                "--provider-tenant",
                tenant,
            ],
        );
        assert_eq!(bound["binding"], "created", "{org}");
        assert_eq!(
            bound["audit_event"],
            Value::Null,
            "D7.8 names no binding kind"
        );
        // Idempotent: the same binding again is reported, never duplicated.
        let again = operator(
            fixture,
            &[
                "bind-provider-tenant",
                "--organization",
                org,
                "--issuer",
                ISSUER,
                "--provider-tenant",
                tenant,
            ],
        );
        assert_eq!(again["binding"], "existing", "{org}");
    }
    let secret = FakeSecret::start_at(FakeSecret::AUTH_ARN);
    assert_eq!(
        secret.value(),
        "{}",
        "the stack's initial value: not configured"
    );
    secret.put(&auth_configuration_document(registry_version).to_string());
    let directory = fixture.directory.0.join("auth-configuration");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    secret.materialize_auth(&directory);
    let loaded = AuthConfig::load(&directory.join("auth-config.json"))
        .expect("the materialized file is the service's own format");
    assert_eq!(
        (
            loaded.auth_origin.as_str(),
            loaded.app_origin.as_str(),
            loaded.callback.as_str(),
            loaded.issuer.as_str(),
            loaded.audience.as_str(),
            loaded.max_access_lifetime
        ),
        (
            SYNTHETIC_AUTH_ORIGIN,
            APP_ORIGIN,
            CALLBACK,
            ISSUER,
            AUDIENCE,
            Duration::from_secs(900)
        )
    );
    let expected = config(issuer_origin);
    assert_eq!(
        loaded.routes, expected.routes,
        "both routes come from the delivered file"
    );
    let configured = AuthConfig {
        auth_origin: issuer_origin.to_owned(),
        ..loaded
    };
    (configured, directory.join("identity-registry.json"), secret)
}
pub(crate) fn tenants() -> Vec<Tenant> {
    vec![
        Tenant {
            slug: "northstar",
            tenant_id: NORTHSTAR_TENANT.into(),
            client_id: "northstar-platform",
        },
        Tenant {
            slug: "canary",
            tenant_id: CANARY_TENANT.into(),
            client_id: "canary-platform",
        },
    ]
}
impl WorkPages for TestPages {
    fn work(&self, v: &WorkProps) -> Response {
        page(format!("page=work;views={}", v.views.len()))
    }
}
impl RecordPage for TestPages {
    fn record(&self, v: &RecordView) -> Response {
        page(format!("page=record;version={}", v.version))
    }
}
/// The binary's whole product router (`main.rs`): the ADR-006 routes, the
/// APP-013 command route and record page, and the APP-012 query route and
/// work page (no engine: a run that reaches execution answers 503), then the
/// uniform 404 for unknown paths and methods and the 5 s handler deadline.
pub(crate) fn full_app(auth: Arc<Auth>) -> Router {
    let pages = Arc::new(TestPages);
    let command_routes = Arc::new(commands::routes::Commands {
        auth: auth.clone(),
        page: pages.clone(),
    });
    let work = Arc::new(query::routes::Work::new(auth.clone(), None, pages));
    http::uniform(
        routes::router(auth)
            .merge(commands::routes::router(command_routes))
            .merge(query::routes::router(work)),
    )
}
/// The ADR-006 routes alone: the uniform 404 for unknown paths and methods,
/// the 5 s handler deadline.
pub(crate) fn app(auth: Arc<Auth>) -> Router {
    http::uniform(routes::router(auth))
}

#[derive(Clone)]
pub(crate) struct Reply {
    pub(crate) status: StatusCode,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}
impl Reply {
    pub(crate) fn location(&self) -> String {
        match self.headers.get(header::LOCATION) {
            Some(value) => value.to_str().unwrap().to_owned(),
            None => panic!("no Location: {} {}", self.status, self.body),
        }
    }
    pub(crate) fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect()
    }
    pub(crate) fn sets(&self, name: &str) -> Option<String> {
        self.set_cookies()
            .into_iter()
            .find(|c| c.starts_with(&format!("{name}=")))
    }
    /// The uniform refusal, header for header (dates excluded: none are set).
    pub(crate) fn uniform_404(&self) -> bool {
        self.status == StatusCode::NOT_FOUND
            && self.body == "{\"code\":\"not_found\",\"status\":404}"
            && self.headers[header::CONTENT_TYPE] == "application/problem+json"
            && self.headers[header::CACHE_CONTROL] == "no-store"
            && self.headers[header::X_CONTENT_TYPE_OPTIONS] == "nosniff"
            && self.headers.get(header::SET_COOKIE).is_none()
            // APP-076 G8: exactly the uniform header set; in particular no
            // `Allow`, so a method mismatch on a mounted route is
            // indistinguishable from an unknown path.
            && self.headers.keys().all(|name| {
                [
                    header::CONTENT_TYPE,
                    header::CACHE_CONTROL,
                    header::X_CONTENT_TYPE_OPTIONS,
                    header::REFERRER_POLICY,
                    header::CONTENT_LENGTH,
                ]
                .contains(name)
            })
    }
}

pub(crate) type Log = Arc<Mutex<Vec<Reply>>>;
/// A browser: a cookie jar that obeys Set-Cookie (Max-Age=0 removes), a peer
/// address, and same-origin fetch metadata on every POST unless overridden.
pub(crate) struct Browser {
    pub(crate) jar: HashMap<String, String>,
    pub(crate) peer: SocketAddr,
    pub(crate) log: Log,
}
impl Browser {
    pub(crate) fn new(peer: &str, log: &Log) -> Self {
        Self {
            jar: HashMap::new(),
            peer: peer.parse().unwrap(),
            log: log.clone(),
        }
    }
    pub(crate) fn cookie(&self, name: &str) -> Option<String> {
        self.jar.get(name).cloned()
    }
    pub(crate) async fn send(
        &mut self,
        app: &Router,
        method: &str,
        uri: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(uri);
        if !self.jar.is_empty() {
            let cookies: Vec<String> = self.jar.iter().map(|(k, v)| format!("{k}={v}")).collect();
            request = request.header(header::COOKIE, cookies.join("; "));
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if method == "POST" {
            request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        }
        let mut request = request.body(Body::from(body.to_owned())).unwrap();
        request.extensions_mut().insert(ConnectInfo(self.peer));
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let reply = Reply {
            status,
            headers,
            body: String::from_utf8(body.to_vec()).unwrap(),
        };
        for cookie in reply.set_cookies() {
            let pair = cookie.split(';').next().unwrap();
            let (name, value) = pair.split_once('=').unwrap();
            if cookie.contains("Max-Age=0") {
                self.jar.remove(name);
            } else {
                self.jar.insert(name.into(), value.into());
            }
        }
        self.log.lock().unwrap().push(reply.clone());
        reply
    }
    pub(crate) async fn get(&mut self, app: &Router, uri: &str) -> Reply {
        self.send(app, "GET", uri, "", &[]).await
    }
    pub(crate) async fn post(&mut self, app: &Router, uri: &str, body: &str) -> Reply {
        self.send(app, "POST", uri, body, &[("sec-fetch-site", "same-origin")])
            .await
    }
}

// --- unit tests (no Docker) ------------------------------------------------

async fn issued_token(issuer: &Issuer, client: &Client, subject: &str) -> provider::Token {
    let pkce = Pkce::fresh().unwrap();
    let state = capability::random_capability().unwrap();
    let request_uri = client
        .par(
            "northstar",
            "northstar-platform",
            CALLBACK,
            &state,
            &pkce.challenge,
            None,
        )
        .await
        .unwrap();
    let url = client.authorize_url("northstar", "northstar-platform", &request_uri);
    let query = issuer.login(&url, subject).unwrap();
    let code = query
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .unwrap()
        .to_owned();
    client
        .exchange(
            "northstar",
            "northstar-platform",
            CALLBACK,
            &code,
            &pkce.verifier,
        )
        .await
        .unwrap()
}

/// ADR-006 resend mapping at the back channel: the five merged-contract
/// outcomes, the rehearsal image's two (dfba787) and a timeout; never
/// `requested` for anything but `202 provider_accepted`.
#[tokio::test]
async fn pending_resend_mapping_for_all_five_provider_outcomes() {
    let issuer = Issuer::start(tenants()).await;
    let client = Client::loopback(&issuer.origin).unwrap();
    issuer.user("usr_resend", "resend@example.test", false);
    let token = issued_token(&issuer, &client, "usr_resend").await;
    for (mode, expected) in [
        (ResendMode::Accepted, Resend::Requested),
        (ResendMode::AlreadyVerified, Resend::AlreadyVerified),
        (
            ResendMode::Cooldown,
            Resend::Unavailable {
                status: 429,
                detail: "resend_cooldown",
            },
        ),
        (
            ResendMode::TooManyRequests,
            Resend::Unavailable {
                status: 429,
                detail: "too_many_requests",
            },
        ),
        (
            ResendMode::EmailUnavailable,
            Resend::Unavailable {
                status: 503,
                detail: "email_unavailable",
            },
        ),
        (
            ResendMode::EmailFailed,
            Resend::Unavailable {
                status: 502,
                detail: "email_failed",
            },
        ),
        (
            ResendMode::LegacyMessage,
            Resend::Unavailable {
                status: 200,
                detail: "unmapped",
            },
        ),
        (
            ResendMode::LegacyFailed,
            Resend::Unavailable {
                status: 500,
                detail: "unmapped",
            },
        ),
        (
            ResendMode::Unauthorized,
            Resend::SignInAgain { status: 401 },
        ),
        (ResendMode::NotFound, Resend::SignInAgain { status: 404 }),
        (
            ResendMode::Timeout,
            Resend::Unavailable {
                status: 0,
                detail: "timeout_or_transport",
            },
        ),
    ] {
        issuer.resend_mode(mode);
        assert_eq!(
            client.resend("northstar", &token.access_token).await,
            expected,
            "{mode:?}"
        );
    }
}

/// The exchange's `deny_unknown_fields` shapes and the issuer refresh cookie
/// that is never read; refresh rotation and family revocation on reuse.
#[tokio::test]
async fn exchange_and_refresh_follow_the_pinned_contract() {
    let issuer = Issuer::start(tenants()).await;
    let client = Client::loopback(&issuer.origin).unwrap();
    issuer.user("usr_contract", "contract@example.test", true);
    let token = issued_token(&issuer, &client, "usr_contract").await;
    assert!(token_shaped(&token) && token.user.email_verified);
    for mode in [TokenMode::ExtraField, TokenMode::MissingEmailVerified] {
        issuer.token_mode(mode);
        let pkce = Pkce::fresh().unwrap();
        let state = capability::random_capability().unwrap();
        let uri = client
            .par(
                "northstar",
                "northstar-platform",
                CALLBACK,
                &state,
                &pkce.challenge,
                None,
            )
            .await
            .unwrap();
        let query = issuer
            .login(
                &client.authorize_url("northstar", "northstar-platform", &uri),
                "usr_contract",
            )
            .unwrap();
        let code = query
            .split('&')
            .find_map(|p| p.strip_prefix("code="))
            .unwrap();
        assert_eq!(
            client
                .exchange(
                    "northstar",
                    "northstar-platform",
                    CALLBACK,
                    code,
                    &pkce.verifier
                )
                .await
                .unwrap_err(),
            Failure::Contract,
            "{mode:?}"
        );
    }
    issuer.token_mode(TokenMode::Normal);
    let rotated = client
        .refresh("northstar", &token.refresh_token)
        .await
        .unwrap();
    assert_ne!(rotated.refresh_token, token.refresh_token);
    assert_eq!(
        issuer.try_refresh("northstar", &token.refresh_token).await,
        (401, Some("token_reuse_detected".into()))
    );
    // Reuse revoked the family: the rotated token is dead too.
    assert_eq!(
        issuer
            .try_refresh("northstar", &rotated.refresh_token)
            .await
            .0,
        401
    );
}

/// D7.11: the vault refreshes a near-expiry access token once, replaces both
/// tokens, and never presents a token again after a failed refresh.
#[tokio::test]
async fn vault_refresh_is_serialized_rotating_and_never_retried() {
    let issuer = Issuer::start(tenants()).await;
    let client = Client::loopback(&issuer.origin).unwrap();
    issuer.user("usr_vault", "vault@example.test", false);
    issuer.token_mode(TokenMode::ShortAccess);
    let token = issued_token(&issuer, &client, "usr_vault").await;
    issuer.token_mode(TokenMode::Normal);
    let vault = vault::Vault::new().unwrap();
    let reference = vault.insert("northstar", &token).unwrap();
    assert!(vault.contains(&reference));
    let access = vault.access(&reference, &client).await.unwrap();
    assert_ne!(access, token.access_token);
    assert_eq!(issuer.count(|c| &c.refresh), 1);
    // A fresh 900-second token: no second refresh.
    assert_eq!(vault.access(&reference, &client).await.unwrap(), access);
    assert_eq!(issuer.count(|c| &c.refresh), 1);
    // The original refresh token was rotated away and is now a reuse.
    assert_eq!(
        issuer
            .try_refresh("northstar", &token.refresh_token)
            .await
            .0,
        401
    );
    issuer.token_mode(TokenMode::ShortAccess);
    let short = issued_token(&issuer, &client, "usr_vault").await;
    let failing = vault.insert("northstar", &short).unwrap();
    issuer.refresh_mode(RefreshMode::Unavailable);
    assert_eq!(
        vault.access(&failing, &client).await,
        Err(AccessFailure::Failed)
    );
    issuer.refresh_mode(RefreshMode::Normal);
    // Spent: never presented again, not even for logout.
    assert_eq!(
        vault.access(&failing, &client).await,
        Err(AccessFailure::Failed)
    );
    assert_eq!(vault.take(&failing).await, None);
    assert_eq!(
        vault.access("absent", &client).await,
        Err(AccessFailure::Missing)
    );
    let (slug, refresh) = vault.take(&reference).await.unwrap();
    assert_eq!(slug, "northstar");
    assert!(capability_shaped(&refresh));
    assert!(!vault.contains(&reference));
}

/// APP-009 review m5: a vault entry referenced by no row (a callback
/// cancelled between the vault insert and its row) is ended by the expiry
/// sweep once it is older than the 12-hour absolute session; younger
/// entries stay. The sweep logs the provider session out.
#[tokio::test]
async fn sweep_ends_vault_entries_older_than_the_absolute_session() {
    let issuer = Issuer::start(tenants()).await;
    let client = Client::loopback(&issuer.origin).unwrap();
    issuer.user("usr_orphan", "orphan@example.test", false);
    let auth = Auth::loopback(config(&issuer.origin), None, Arc::new(TestPages));
    let orphan = auth
        .vault
        .insert(
            "northstar",
            &issued_token(&issuer, &client, "usr_orphan").await,
        )
        .unwrap();
    let young = auth
        .vault
        .insert(
            "northstar",
            &issued_token(&issuer, &client, "usr_orphan").await,
        )
        .unwrap();
    auth.vault
        .backdate(&orphan, crate::storage::SESSION_ABSOLUTE_SECONDS + 1);
    auth.vault.backdate(&young, 3_600);
    let logouts = issuer.count(|c| &c.logout);
    auth.sweep().await;
    assert!(!auth.vault.contains(&orphan) && auth.vault.contains(&young));
    assert_eq!(issuer.count(|c| &c.logout), logouts + 1);
}

/// The reviewed verifier against the synthetic issuer: unknown keys fail
/// closed and refetch at most once per refresh interval (bounded JWKS).
#[tokio::test]
async fn unknown_kid_fails_closed_with_bounded_jwks_refresh() {
    let issuer = Issuer::start(tenants()).await;
    let client = Client::loopback(&issuer.origin).unwrap();
    issuer.user("usr_kid", "kid@example.test", true);
    let verifier = genui_auth_verifier::Verifier::new_loopback_for_tests(super::verifier_config(
        &config(&issuer.origin),
    ))
    .unwrap();
    let good = issued_token(&issuer, &client, "usr_kid").await;
    let identity = verifier
        .verify(&good.access_token, NORTHSTAR_TENANT)
        .await
        .unwrap();
    assert_eq!(identity.subject, "usr_kid");
    assert_eq!(issuer.count(|c| &c.jwks), 1);
    issuer.token_mode(TokenMode::UnknownKid);
    for _ in 0..3 {
        let stray = issued_token(&issuer, &client, "usr_kid").await;
        assert!(verifier
            .verify(&stray.access_token, NORTHSTAR_TENANT)
            .await
            .is_err());
    }
    assert_eq!(
        issuer.count(|c| &c.jwks),
        1,
        "no refetch inside the 60 s interval"
    );
    assert!(verifier
        .verify(&good.access_token, CANARY_TENANT)
        .await
        .is_err());
}

/// Every anonymous refusal without storage or a provider call: fetch
/// metadata, the body bound, the limits, the uniform 404, the neutral
/// sign-in failure, and no operator harness route in the product router.
#[tokio::test]
async fn anonymous_routes_refusals_and_release_router_without_harness() {
    let issuer = Issuer::start(tenants()).await;
    let auth = Arc::new(Auth::loopback(
        config(&issuer.origin),
        None,
        Arc::new(TestPages),
    ));
    let app = app(auth.clone());
    let log: Log = Arc::default();
    let mut browser = Browser::new("198.51.100.20:4000", &log);
    let page = browser.get(&app, "/signin").await;
    assert_eq!(
        (page.status, page.body.as_str()),
        (StatusCode::OK, "page=signin;action=-;failed=false")
    );
    let known = browser
        .get(&app, &format!("/signin/{NORTHSTAR_HANDLE}"))
        .await;
    let unknown = browser
        .get(&app, &format!("/signin/{UNKNOWN_HANDLE}"))
        .await;
    assert_eq!(
        known.body.replace(NORTHSTAR_HANDLE, "H"),
        unknown.body.replace(UNKNOWN_HANDLE, "H")
    );
    let malformed = browser.get(&app, "/signin/NOT-A-HANDLE").await;
    assert_eq!(malformed.body, "page=signin;action=-;failed=false");
    // APP-009 review m2: an undecodable path segment is a malformed handle,
    // never axum's plain-text rejection.
    let undecodable = browser.get(&app, "/signin/%ff").await;
    assert_eq!(
        (undecodable.status, undecodable.body.as_str()),
        (StatusCode::OK, "page=signin;action=-;failed=false")
    );
    assert_eq!(undecodable.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        undecodable.headers[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    let posted = browser.post(&app, "/signin/%ff", "").await;
    assert_eq!(
        (posted.status, posted.body.as_str()),
        (StatusCode::BAD_REQUEST, "page=signin;action=-;failed=true")
    );
    browser
        .jar
        .insert(cookies::ORGANIZATION_HINT.into(), CANARY_HANDLE.into());
    let hinted = browser.get(&app, "/signin").await;
    assert_eq!(
        hinted.body,
        format!("page=signin;action=/signin/{CANARY_HANDLE};failed=false")
    );
    browser.jar.clear();
    // Cross-site: fetch metadata (or an exact Origin when absent) first.
    for headers in [
        &[][..],
        &[("sec-fetch-site", "cross-site")][..],
        &[("sec-fetch-site", "same-site")][..],
        &[("origin", "https://evil.example.test")][..],
    ] {
        for route in ["/auth/signout", "/auth/signout-all", "/auth/pending/resend"] {
            let reply = browser.send(&app, "POST", route, "", headers).await;
            assert_eq!(
                (reply.status, reply.body.as_str()),
                (
                    StatusCode::FORBIDDEN,
                    "{\"code\":\"cross_site\",\"status\":403}"
                )
            );
        }
        let reply = browser
            .send(
                &app,
                "POST",
                &format!("/signin/{NORTHSTAR_HANDLE}"),
                "",
                headers,
            )
            .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN);
    }
    assert_eq!(issuer.count(|c| &c.par), 0);
    // An exact Origin passes fetch metadata; without storage the start fails neutrally.
    let neutral = browser
        .send(
            &app,
            "POST",
            &format!("/signin/{NORTHSTAR_HANDLE}"),
            "",
            &[("origin", APP_ORIGIN)],
        )
        .await;
    assert_eq!(
        (neutral.status, neutral.body.as_str()),
        (StatusCode::BAD_REQUEST, "page=signin;action=-;failed=true")
    );
    let oversize = browser
        .post(
            &app,
            &format!("/signin/{NORTHSTAR_HANDLE}"),
            &"a".repeat(http::FORM_BODY_LIMIT + 1),
        )
        .await;
    assert_eq!(
        (oversize.status, oversize.body.as_str()),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "{\"code\":\"payload_too_large\",\"status\":413}"
        )
    );
    // 10 per minute per address class (the two above counted), then 429.
    let mut limited = None;
    for _ in 0..10 {
        let reply = browser
            .post(&app, &format!("/signin/{UNKNOWN_HANDLE}"), "")
            .await;
        if reply.status == StatusCode::TOO_MANY_REQUESTS {
            limited = Some(reply);
            break;
        }
        assert_eq!(reply.body, "page=signin;action=-;failed=true");
    }
    let limited = limited.expect("address-class limit reached");
    assert_eq!(
        limited.body,
        "{\"code\":\"too_many_requests\",\"status\":429}"
    );
    assert!(limited.headers.contains_key(header::RETRY_AFTER));
    let mut forwarded = Browser::new("10.0.0.5:443", &log);
    let other = forwarded
        .send(
            &app,
            "POST",
            &format!("/signin/{UNKNOWN_HANDLE}"),
            "",
            &[
                ("sec-fetch-site", "same-origin"),
                ("x-forwarded-for", "203.0.113.50"),
            ],
        )
        .await;
    assert_eq!(
        other.status,
        StatusCode::BAD_REQUEST,
        "another address class has its own budget"
    );
    assert_eq!(issuer.count(|c| &c.par), 0);
    // The uniform 404: anonymous, malformed cookies, unknown paths and methods.
    let anonymous = browser.get(&app, "/workspace").await;
    assert!(anonymous.uniform_404());
    browser.jar.insert(cookies::SESSION.into(), "A".repeat(64));
    assert!(browser.get(&app, "/workspace").await.uniform_404());
    browser.jar.clear();
    for (method, route) in [
        ("GET", "/auth/pending"),
        ("GET", "/v1/anything"),
        ("GET", "/auth/signout"),
        ("GET", "/auth/qualification/status"),
        ("GET", "/auth/qualification/result"),
        ("POST", "/auth/qualification/start"),
    ] {
        let reply = browser
            .send(
                &app,
                method,
                route,
                "",
                &[("sec-fetch-site", "same-origin")],
            )
            .await;
        assert!(reply.uniform_404(), "{method} {route}");
    }
    assert!(browser
        .post(&app, "/auth/pending/resend", "")
        .await
        .uniform_404());
    assert!(browser
        .post(&app, "/auth/signout-all", "")
        .await
        .uniform_404());
    let out = browser.post(&app, "/auth/signout", "").await;
    assert_eq!(
        (out.status, out.location()),
        (StatusCode::SEE_OTHER, "/".into())
    );
    for name in [cookies::PRELOGIN, cookies::PRE_ACCESS, cookies::SESSION] {
        assert!(out.sets(name).unwrap().ends_with("Max-Age=0"));
    }
    // A callback without its pre-login cookie or with extra parameters fails.
    let code = capability::random_capability().unwrap();
    let callback = browser
        .get(&app, &format!("/auth/callback?code={code}&state={code}"))
        .await;
    assert_eq!(callback.status, StatusCode::BAD_REQUEST);
    assert!(callback
        .sets(cookies::PRELOGIN)
        .unwrap()
        .ends_with("Max-Age=0"));
    browser.jar.insert(cookies::PRELOGIN.into(), "A".repeat(64));
    for query in [
        format!("code={code}&state={code}&return_to=https://evil.test"),
        format!("code={code}"),
        "state=x&code=y".into(),
    ] {
        assert_eq!(
            browser
                .get(&app, &format!("/auth/callback?{query}"))
                .await
                .status,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(issuer.count(|c| &c.exchange), 0);
}

// --- the Docker campaign -------------------------------------------------------

use crate::storage::{
    admission_qualification::{
        accepted, cli_command, lock_wait, operator, operator_with, wait, FakeProvider,
    },
    code_digest,
    qualification::Fixture,
    Lookup, SignOut, Stores,
};

pub(crate) fn check(checks: &mut Vec<Value>, name: &str) {
    checks.push(json!({"name": name, "passed": true}));
    println!("APP009 check: {name}");
}
async fn scalar(fixture: &Fixture, sql: &str) -> i64 {
    fixture
        .operator()
        .await
        .client
        .query_one(sql, &[])
        .await
        .unwrap()
        .get::<_, i64>(0)
}
async fn superuser(fixture: &Fixture, sql: &str) {
    fixture
        .operator()
        .await
        .client
        .batch_execute(sql)
        .await
        .unwrap();
}

pub(crate) struct Campaign {
    pub(crate) fixture: Fixture,
    pub(crate) stores: Arc<Stores>,
    pub(crate) issuer: Issuer,
    pub(crate) auth: Arc<Auth>,
    pub(crate) app: Router,
    pub(crate) log: Log,
    pub(crate) checks: Vec<Value>,
    /// The loopback management stand-in for the Northstar tenant; the
    /// operator writes each code's capability where the route reads it.
    pub(crate) provider: FakeProvider,
    /// APP-012: the Canary tenant's stand-in, sharing the Northstar
    /// stand-in's capability directory (as `invite_tests`), when a campaign
    /// signs Canary members in. None in the session campaign.
    pub(crate) canary: Option<FakeProvider>,
    /// digest → (raw code, invitation email), kept by the test only.
    pub(crate) codes: Mutex<HashMap<String, (String, String)>>,
}
impl Campaign {
    pub(crate) fn browser(&self, peer: &str) -> Browser {
        Browser::new(peer, &self.log)
    }
    /// Ordinary sign-in: `POST /signin/{handle}` → hosted login → callback.
    pub(crate) async fn sign_in(
        &self,
        browser: &mut Browser,
        handle: &str,
        subject: &str,
    ) -> Reply {
        let start = browser
            .post(&self.app, &format!("/signin/{handle}"), "")
            .await;
        assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
        let query = self
            .issuer
            .login(&start.location(), subject)
            .expect("hosted login");
        browser
            .get(&self.app, &format!("/auth/callback?{query}"))
            .await
    }
    /// A one-redemption Northstar code with its server-only provider
    /// invitation: issued by the real operator command against the loopback
    /// management stand-in, which writes the capability file the invitation
    /// route reads; the synthetic issuer learns the same invitation (one
    /// simulated provider). Returns (code id, digest).
    pub(crate) fn issue_code(&self) -> (String, String) {
        self.issue_code_in(ORG)
    }
    /// APP-012 reuses the harness for both fixture organizations: a Canary
    /// code goes through the Canary stand-in and tenant.
    pub(crate) fn issue_code_in(&self, organization: &str) -> (String, String) {
        let (provider, tenant, slug) = if organization == OTHER {
            (
                self.canary.as_ref().expect("a Canary stand-in"),
                CANARY_TENANT,
                "canary",
            )
        } else {
            (&self.provider, NORTHSTAR_TENANT, "northstar")
        };
        let mut codes = self.codes.lock().unwrap();
        let email = format!("code-{}@invite.example.test", codes.len());
        let out = operator_with(
            &self.fixture,
            provider,
            &[
                "issue",
                "--organization",
                organization,
                "--issued-by",
                "session-campaign",
                "--grant",
                &format!("discover:{APP}"),
                "--grant",
                &format!("run:{APP}"),
                "--provider-email",
                &email,
                "--provider-role",
                "user",
                "--provider-issuer",
                ISSUER,
                "--provider-tenant",
                tenant,
            ],
        );
        let invitation = &out["provider_invitation"];
        assert_eq!(invitation["capability_stored"], true, "{invitation}");
        let path = provider.capabilities.join(format!(
            "{organization}.{}.capability",
            invitation["invitation_id"].as_str().unwrap()
        ));
        let record: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        self.issuer
            .invitation(record["invite_token"].as_str().unwrap(), slug, &email);
        let code = out["code"].as_str().unwrap();
        let digest = code_digest(code).unwrap();
        assert_eq!(record["code_digest"], digest.as_str());
        codes.insert(digest.clone(), (code.to_owned(), email));
        (out["code_id"].as_str().unwrap().to_owned(), digest)
    }
    /// The invited signup through the real APP-074 route: `POST
    /// /invite/validate` with the raw code (the backend PAR carries the
    /// capability's `invite_token`), then the hosted registration of
    /// `subject` for the invitation's email, then the callback. The test
    /// clears the in-process limits first (many invited signups per minute).
    /// The code names its organization, so one route serves both.
    pub(crate) async fn invited_signup(
        &self,
        browser: &mut Browser,
        digest: &str,
        subject: &str,
    ) -> Reply {
        let (code, email) = self.codes.lock().unwrap()[digest].clone();
        self.auth.limits.clear();
        let entry = browser
            .post(&self.app, "/invite/validate", &format!("code={code}"))
            .await;
        assert_eq!(entry.status, StatusCode::SEE_OTHER, "{}", entry.body);
        let query = self
            .issuer
            .register(&entry.location(), subject, &email)
            .expect("hosted signup");
        browser
            .get(&self.app, &format!("/auth/callback?{query}"))
            .await
    }
    /// A fresh member through the whole product path: invited signup (pending),
    /// mailbox verification, then an ordinary sign-in that redeems and mints.
    pub(crate) async fn member(&self, label: &str) -> (String, Browser) {
        self.member_in(ORG, label).await
    }
    pub(crate) async fn member_in(&self, organization: &str, label: &str) -> (String, Browser) {
        let subject = format!("usr_00000000-0000-4000-8000-{:0>12}", label);
        self.issuer
            .user(&subject, &format!("{label}@example.test"), false);
        let (_, digest) = self.issue_code_in(organization);
        let mut browser = self.browser("192.0.2.30:5000");
        let pending = self.invited_signup(&mut browser, &digest, &subject).await;
        assert_eq!(pending.location(), "/auth/pending");
        self.issuer.set_verified(&subject, true);
        let handle = if organization == OTHER {
            CANARY_HANDLE
        } else {
            NORTHSTAR_HANDLE
        };
        let arrived = self.sign_in(&mut browser, handle, &subject).await;
        assert_eq!(arrived.location(), "/workspace", "{label}");
        (subject, browser)
    }
    pub(crate) async fn principal(&self, subject: &str) -> String {
        self.fixture
            .operator()
            .await
            .client
            .query_one(
                "SELECT principal_id::text FROM genui.principals WHERE provider_subject=$1",
                &[&subject],
            )
            .await
            .unwrap()
            .get(0)
    }
    pub(crate) async fn epoch(&self, principal: &str) -> i64 {
        scalar(
            &self.fixture,
            &format!(
                "SELECT membership_epoch FROM genui.memberships WHERE principal_id='{principal}'"
            ),
        )
        .await
    }
    pub(crate) fn record(&mut self, name: &str) {
        check(&mut self.checks, name);
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_session_campaign() {
    std::thread::Builder::new()
        .name("app009-session-campaign".into())
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
    let mut checks = Vec::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    let handles: Vec<Value> = [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)]
        .iter()
        .map(|(org, handle)| {
            operator(
                &fixture,
                &["issue-handle", "--organization", org, "--handle", handle],
            )
        })
        .collect();
    assert_eq!(handles[0]["handle"], NORTHSTAR_HANDLE);
    assert_eq!(handles[1]["handle"], CANARY_HANDLE);
    check(&mut checks, "operator issue-handle installs the two recorded fixture handles (digest only) with one handle.rotated event each");
    // APP-078 B1: the synthetic gatewasm bindings for both organizations
    // through the operator's own `bind-provider-tenant`, and the provider
    // routing and registry as the deployment delivers them (the composed
    // AuthConfiguration secret, materialized by the supervisor's writer).
    let issuer = Issuer::start(tenants()).await;
    let (auth_config, registry, _auth_secret) = deliver_auth_configuration(
        &fixture,
        &issuer.origin,
        "00000000-0000-4000-8000-000000000a09",
    );
    let settings = fixture.settings_with_registry(registry);
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    check(&mut checks, "APP-078 B1: both organizations' provider tenant bindings come from `admission_operator.py bind-provider-tenant` (created once, `existing` on repeat, no audit event: D7.8's operator kinds name none), and the provider routing and closed registry come from the composed AuthConfiguration secret value put into the loopback Secrets Manager stand-in and materialized by runtime.write_auth_configuration as the two 0600 files that AuthConfig::load and the storage registry read; only the auth origin is the cfg(test) loopback issuer's (plain HTTP on 127.0.0.1) in place of the file's https origin");
    let provider = FakeProvider::start_at(&fixture, NORTHSTAR_TENANT, "provider", None, None);
    let auth = Arc::new(
        Auth::loopback(auth_config, Some(stores.clone()), Arc::new(TestPages)).with_invitations(
            Some(invite::Invitations::open(&provider.capabilities).unwrap()),
        ),
    );
    let mut c = Campaign {
        app: app(auth.clone()),
        fixture,
        stores,
        issuer,
        auth,
        log: Arc::default(),
        checks,
        provider,
        canary: None,
        codes: Mutex::default(),
    };

    // --- 1. Neutral sign-in entry --------------------------------------------
    let mut anon = c.browser("198.51.100.1:1000");
    let unknown = anon
        .post(&c.app, &format!("/signin/{UNKNOWN_HANDLE}"), "")
        .await;
    let malformed = anon.post(&c.app, "/signin/NOTAHANDLE", "").await;
    assert_eq!(
        (unknown.status, &unknown.body),
        (StatusCode::BAD_REQUEST, &malformed.body)
    );
    assert_eq!(c.issuer.count(|x| &x.par), 0);
    let start = anon
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER);
    let location = start.location();
    let authorize = url::Url::parse(&location).unwrap();
    assert_eq!(authorize.path(), "/t/northstar/auth/authorize");
    let pairs: Vec<(String, String)> = authorize.query_pairs().into_owned().collect();
    assert_eq!(
        pairs.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
        ["client_id", "request_uri"]
    );
    assert_eq!(start.headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(
        start
            .sets(cookies::PRELOGIN)
            .unwrap()
            .split_once("; ")
            .unwrap()
            .1,
        "Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600"
    );
    assert_eq!(start.sets(cookies::ORGANIZATION_HINT).unwrap(), format!("__Host-genui_org={NORTHSTAR_HANDLE}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=2592000"));
    assert_eq!(c.issuer.count(|x| &x.par), 1);
    assert_eq!(c.issuer.count(|x| &x.par_invited), 0);
    let flow_rows = scalar(&c.fixture, "SELECT count(*) FROM genui.prelogin_flows").await;
    assert_eq!(flow_rows, 1);
    let stored = c.fixture.operator().await.client
        .query_one("SELECT cookie_digest::text,state_digest::text,kind,octet_length(code_verifier_ciphertext)::bigint,expires_at-created_at=interval '10 minutes' FROM genui.prelogin_flows", &[]).await.unwrap();
    let cookie = anon.cookie(cookies::PRELOGIN).unwrap();
    assert!(
        stored.get::<_, String>(0).starts_with("sha256:")
            && !stored.get::<_, String>(0).contains(&cookie)
    );
    assert_eq!(stored.get::<_, String>(2), "signin");
    assert!(stored.get::<_, i64>(3) > 43 && stored.get::<_, bool>(4));
    c.record("unknown and malformed handles answer the same neutral page with no provider request; a known handle starts a fresh flow (digest-only cookie and state, sealed verifier, 10 minutes) and PAR without invite_token, then 303s to authorize with client_id and request_uri only");

    // --- 2. Invited signup → pending (no slot, membership or entitlement) ----
    let (code_a, digest_a) = c.issue_code();
    let maya = "usr_00000000-0000-4000-8000-0000000000aa";
    c.issuer.user(maya, "maya@example.test", false);
    let mut browser = c.browser("192.0.2.10:2000");
    let pending = c.invited_signup(&mut browser, &digest_a, maya).await;
    assert_eq!(
        (pending.status, pending.location()),
        (StatusCode::SEE_OTHER, "/auth/pending".into())
    );
    assert_eq!(
        pending
            .sets(cookies::PRE_ACCESS)
            .unwrap()
            .split_once("; ")
            .unwrap()
            .1,
        "Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=1800"
    );
    assert!(pending
        .sets(cookies::PRELOGIN)
        .unwrap()
        .ends_with("Max-Age=0"));
    assert_eq!(c.issuer.count(|x| &x.par_invited), 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.principals WHERE provider_subject='{maya}'")
        )
        .await,
        0
    );
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.code_reservations WHERE provider_subject='{maya}' AND kind='pending' AND state='live' AND expires_at BETWEEN now()+interval '23 hours 59 minutes' AND now()+interval '24 hours'")).await, 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT redeemed_count FROM genui.access_codes WHERE code_id='{code_a}'")
        )
        .await,
        0
    );
    assert_eq!(
        scalar(&c.fixture, "SELECT count(*) FROM genui.prelogin_flows").await,
        1,
        "only the abandoned sign-in flow of step 1 remains"
    );
    let listed = operator(&c.fixture, &["list", "--organization", ORG]);
    let entry = listed["codes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["code_id"] == code_a)
        .unwrap()
        .clone();
    assert_eq!(
        (
            entry["live_pending_admissions"].as_i64(),
            entry["redeemed_count"].as_i64()
        ),
        (Some(1), Some(0))
    );
    c.record("unverified_mailbox_commits_no_slot_membership_or_entitlement: an invited signup whose exchange reports email_verified false lands on /auth/pending with a 24-hour pending admission bound to the verified identity, counted but not redeemed");

    // --- 3. Pre-access is restricted ------------------------------------------
    let page = browser.get(&c.app, "/auth/pending").await;
    assert_eq!(page.status, StatusCode::OK);
    assert_eq!(field(&page.body, "page"), "pending");
    assert_eq!(field(&page.body, "hours"), "24");
    assert_eq!(field(&page.body, "available"), "true");
    let pre_cookie = browser.cookie(cookies::PRE_ACCESS).unwrap();
    assert!(browser.get(&c.app, "/workspace").await.uniform_404());
    let mut swapped = c.browser("192.0.2.10:2001");
    swapped
        .jar
        .insert(cookies::SESSION.into(), pre_cookie.clone());
    assert!(swapped.get(&c.app, "/workspace").await.uniform_404());
    let resend_token = field(&page.body, "resend");
    let upgrade = browser
        .post(
            &c.app,
            "/auth/pending/resend",
            &format!("csrf={resend_token}&grant=admin_policy"),
        )
        .await;
    assert_eq!(upgrade.status, StatusCode::FORBIDDEN);
    assert!(browser
        .post(&c.app, "/auth/signout-all", "csrf=x")
        .await
        .uniform_404());
    assert_eq!(
        scalar(&c.fixture, "SELECT count(*) FROM genui.sessions").await,
        0
    );
    c.record("pre_access_cannot_reach_protected_routes_or_mint_session: the pre-access cookie reads no workspace (uniform 404, also when presented as a session cookie), cannot sign out everywhere, and a client-supplied grant field is refused");

    // --- 4. Resend mapping through the route ---------------------------------
    let mut outcomes = Vec::new();
    for (mode, expected) in [
        (ResendMode::Accepted, "Some(Requested)"),
        (ResendMode::Cooldown, "Some(Unavailable)"),
        (ResendMode::TooManyRequests, "Some(Unavailable)"),
        (ResendMode::EmailUnavailable, "Some(Unavailable)"),
        (ResendMode::EmailFailed, "Some(Unavailable)"),
        (ResendMode::LegacyMessage, "Some(Unavailable)"),
        (ResendMode::LegacyFailed, "Some(Unavailable)"),
        (ResendMode::Timeout, "Some(Unavailable)"),
    ] {
        c.auth.limits.clear();
        c.issuer.resend_mode(mode);
        let before = c.issuer.count(|x| &x.resend);
        let reply = browser
            .post(
                &c.app,
                "/auth/pending/resend",
                &format!("csrf={resend_token}"),
            )
            .await;
        assert_eq!(
            (reply.status, field(&reply.body, "result")),
            (StatusCode::OK, expected.into()),
            "{mode:?}"
        );
        assert_eq!(c.issuer.count(|x| &x.resend), before + 1);
        outcomes.push(json!({"provider": format!("{mode:?}"), "genui": expected}));
    }
    // GenUI's own 60-second per-account cooldown answers without a provider call.
    c.issuer.resend_mode(ResendMode::Accepted);
    let before = c.issuer.count(|x| &x.resend);
    let cooled = browser
        .post(
            &c.app,
            "/auth/pending/resend",
            &format!("csrf={resend_token}"),
        )
        .await;
    assert_eq!(field(&cooled.body, "result"), "Some(Unavailable)");
    assert_eq!(field(&cooled.body, "available"), "false");
    assert!(field(&cooled.body, "cooldown").parse::<u32>().unwrap() > 0);
    assert_eq!(c.issuer.count(|x| &x.resend), before);
    // CSRF for the session form: a missing, foreign-form or foreign-cookie token.
    c.auth.limits.clear();
    let signout_token = field(&page.body, "signout");
    for body in [
        "".to_owned(),
        format!("csrf={signout_token}"),
        format!(
            "csrf={}",
            c.auth.csrf.token(csrf::Form::Resend, &"B".repeat(64))
        ),
    ] {
        assert_eq!(
            browser
                .post(&c.app, "/auth/pending/resend", &body)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(c.issuer.count(|x| &x.resend), before);
    // The four outcomes that destroy the record and send to a fresh sign-in,
    // each from a fresh pending session of the same identity in another browser.
    let mut again_browser = c.browser("192.0.2.10:2002");
    for (mode, refresh) in [
        (ResendMode::AlreadyVerified, RefreshMode::Normal),
        (ResendMode::Unauthorized, RefreshMode::Normal),
        (ResendMode::NotFound, RefreshMode::Normal),
        (ResendMode::Accepted, RefreshMode::Unauthorized),
    ] {
        c.auth.limits.clear();
        c.issuer.resend_mode(mode);
        // A fresh pending session for the same identity (ordinary sign-in, still unverified);
        // the last case uses a 20-second access token so the vault must refresh first.
        if refresh == RefreshMode::Unauthorized {
            c.issuer.token_mode(TokenMode::ShortAccess);
        }
        let again = c.sign_in(&mut again_browser, NORTHSTAR_HANDLE, maya).await;
        c.issuer.token_mode(TokenMode::Normal);
        assert_eq!(again.location(), "/auth/pending");
        let page = again_browser.get(&c.app, "/auth/pending").await;
        c.issuer.refresh_mode(refresh);
        let reply = again_browser
            .post(
                &c.app,
                "/auth/pending/resend",
                &format!("csrf={}", field(&page.body, "resend")),
            )
            .await;
        c.issuer.refresh_mode(RefreshMode::Normal);
        assert_eq!(
            (reply.status, reply.location()),
            (StatusCode::SEE_OTHER, format!("/signin/{NORTHSTAR_HANDLE}")),
            "{mode:?} {refresh:?}"
        );
        assert!(reply
            .sets(cookies::PRE_ACCESS)
            .unwrap()
            .ends_with("Max-Age=0"));
        assert!(again_browser
            .get(&c.app, "/auth/pending")
            .await
            .uniform_404());
        outcomes.push(json!({"provider": format!("{mode:?}"), "refresh": format!("{refresh:?}"), "genui": "303 /signin/{handle}, pre-access destroyed"}));
    }
    // Pre-access sign-out: the original pending record is destroyed and revoked.
    let logouts = c.issuer.count(|x| &x.logout);
    let out = browser
        .post(&c.app, "/auth/signout", &format!("csrf={signout_token}"))
        .await;
    assert_eq!(
        (out.status, out.location()),
        (StatusCode::SEE_OTHER, "/".into())
    );
    assert_eq!(c.issuer.count(|x| &x.logout), logouts + 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!(
                "SELECT count(*) FROM genui.pre_access_sessions WHERE provider_subject='{maya}'"
            )
        )
        .await,
        0
    );
    c.record("pending_resend_mapping_for_all_five_provider_outcomes through POST /auth/pending/resend: 202 provider_accepted → requested; 429 resend_cooldown/too_many_requests, 503 email_unavailable, 502 email_failed, dfba787's 200 {message} and 500, and a timeout → unavailable; GenUI's 60 s cooldown answers unavailable with no provider call; 200 already_verified, 401, 404 and a failed refresh destroy the pre-access record and 303 to /signin/{handle}; CSRF refusals make no provider call; a pre-access sign-out destroys the record and revokes its token");

    // --- 5. Verified fresh sign-in redeems and mints once ----------------------
    c.issuer.set_verified(maya, true);
    let mut verified = c.browser("192.0.2.11:2100");
    let arrived = c.sign_in(&mut verified, NORTHSTAR_HANDLE, maya).await;
    assert_eq!(
        (arrived.status, arrived.location()),
        (StatusCode::SEE_OTHER, "/workspace".into())
    );
    assert_eq!(
        arrived
            .sets(cookies::SESSION)
            .unwrap()
            .split_once("; ")
            .unwrap()
            .1,
        "Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=43200"
    );
    let maya_principal = c.principal(maya).await;
    assert_eq!(c.epoch(&maya_principal).await, 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT redeemed_count FROM genui.access_codes WHERE code_id='{code_a}'")
        )
        .await,
        1
    );
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.entitlements WHERE principal_id='{maya_principal}' AND status='active'")).await, 1);
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{maya_principal}' AND absolute_expires_at-created_at=interval '12 hours' AND membership_epoch_at_mint=1")).await, 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!(
                "SELECT count(*) FROM genui.pre_access_sessions WHERE provider_subject='{maya}'"
            )
        )
        .await,
        0
    );
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE organization_id='{ORG}' AND writer_role='workspace' AND kind='session.signed_in' AND actor_principal_id='{maya_principal}'")).await, 1);
    let workspace = verified.get(&c.app, "/workspace").await;
    assert_eq!(
        (workspace.status, field(&workspace.body, "page")),
        (StatusCode::OK, "workspace".into())
    );
    // An existing session needs no provider call per request.
    let calls = |i: &Issuer| {
        [
            i.count(|x| &x.jwks),
            i.count(|x| &x.par),
            i.count(|x| &x.exchange),
            i.count(|x| &x.refresh),
            i.count(|x| &x.logout),
            i.count(|x| &x.resend),
        ]
    };
    let before = calls(&c.issuer);
    for _ in 0..3 {
        assert_eq!(
            verified.get(&c.app, "/workspace").await.status,
            StatusCode::OK
        );
    }
    assert_eq!(calls(&c.issuer), before);
    let verify = operator(&c.fixture, &["verify", "--organization", ORG]);
    assert_eq!(verify["verified"], true);
    c.record("a fresh sign-in whose exchange reports email_verified true redeems the live pending admission through admit_redemption (slot, membership epoch 1, entitlement, audit) and mints the workspace session once (12 h, digest-only cookie, session.signed_in on the workspace chain; chains verify); later requests make no provider call");

    // --- 6. State, PKCE and replay -----------------------------------------
    let exchanges = c.issuer.count(|x| &x.exchange);
    let mut replayer = c.browser("192.0.2.12:2200");
    let start = replayer
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let query = c.issuer.login(&start.location(), maya).unwrap();
    let first = replayer
        .get(&c.app, &format!("/auth/callback?{query}"))
        .await;
    assert_eq!(first.location(), "/workspace");
    // Replay the same callback with the same (now consumed) pre-login cookie.
    let cookie_value = start
        .sets(cookies::PRELOGIN)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned();
    replayer.jar.insert(cookies::PRELOGIN.into(), cookie_value);
    let replayed = replayer
        .get(&c.app, &format!("/auth/callback?{query}"))
        .await;
    assert_eq!(replayed.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        c.issuer.count(|x| &x.exchange),
        exchanges + 1,
        "a consumed flow never reaches the token endpoint"
    );
    // Mismatched state: refused, and the flow is consumed by the attempt.
    let mut mismatch = c.browser("192.0.2.13:2300");
    let start = mismatch
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let query = c.issuer.login(&start.location(), maya).unwrap();
    let code = query
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .unwrap()
        .to_owned();
    let state = query
        .split('&')
        .find_map(|p| p.strip_prefix("state="))
        .unwrap()
        .to_owned();
    let saved = mismatch.cookie(cookies::PRELOGIN).unwrap();
    let wrong = capability::random_capability().unwrap();
    assert_eq!(
        mismatch
            .get(&c.app, &format!("/auth/callback?code={code}&state={wrong}"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    mismatch.jar.insert(cookies::PRELOGIN.into(), saved);
    assert_eq!(
        mismatch
            .get(&c.app, &format!("/auth/callback?code={code}&state={state}"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    // A code bound to another flow's PKCE challenge, and a replayed code.
    let mut a = c.browser("192.0.2.14:2400");
    let mut b = c.browser("192.0.2.15:2500");
    let start_a = a
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let start_b = b
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let query_a = c.issuer.login(&start_a.location(), maya).unwrap();
    let query_b = c.issuer.login(&start_b.location(), maya).unwrap();
    let code_a_value = query_a
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .unwrap()
        .to_owned();
    let state_b = query_b
        .split('&')
        .find_map(|p| p.strip_prefix("state="))
        .unwrap()
        .to_owned();
    let swapped = b
        .get(
            &c.app,
            &format!("/auth/callback?code={code_a_value}&state={state_b}"),
        )
        .await;
    assert_eq!(
        swapped.status,
        StatusCode::BAD_REQUEST,
        "PKCE: A's code with B's verifier"
    );
    let replay_code = a.get(&c.app, &format!("/auth/callback?{query_a}")).await;
    assert_eq!(
        replay_code.status,
        StatusCode::BAD_REQUEST,
        "a code already presented is consumed upstream"
    );
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{maya_principal}'")
        )
        .await,
        2
    );
    c.record("replayed_callback_mismatched_state: a replayed callback finds no flow and never reaches the token endpoint; a mismatched state consumes the flow; a code bound to another flow's PKCE challenge and a consumed code are refused upstream; no session is minted by any of them");

    // --- 7. Token verification refusals ---------------------------------------
    let jwks = c.issuer.count(|x| &x.jwks);
    let mut refused_modes = Vec::new();
    for mode in [
        TokenMode::UnknownKid,
        TokenMode::ForgedSignature,
        TokenMode::WrongIssuer,
        TokenMode::WrongAudience,
        TokenMode::Expired,
        TokenMode::MissingTenant,
        TokenMode::ForeignTenant,
        TokenMode::SubjectMismatch,
        TokenMode::ExtraField,
        TokenMode::MissingEmailVerified,
    ] {
        c.issuer.token_mode(mode);
        let logouts = c.issuer.count(|x| &x.logout);
        let sessions = scalar(&c.fixture, "SELECT count(*) FROM genui.sessions").await;
        let pre = scalar(&c.fixture, "SELECT count(*) FROM genui.pre_access_sessions").await;
        let mut probe = c.browser("192.0.2.16:2600");
        let reply = c.sign_in(&mut probe, NORTHSTAR_HANDLE, maya).await;
        assert_eq!(
            (reply.status, field(&reply.body, "failed")),
            (StatusCode::BAD_REQUEST, "true".into()),
            "{mode:?}"
        );
        assert!(
            probe.cookie(cookies::SESSION).is_none() && probe.cookie(cookies::PRE_ACCESS).is_none()
        );
        assert_eq!(
            scalar(&c.fixture, "SELECT count(*) FROM genui.sessions").await,
            sessions
        );
        assert_eq!(
            scalar(&c.fixture, "SELECT count(*) FROM genui.pre_access_sessions").await,
            pre
        );
        if !matches!(
            mode,
            TokenMode::ExtraField | TokenMode::MissingEmailVerified
        ) {
            assert_eq!(
                c.issuer.count(|x| &x.logout),
                logouts + 1,
                "the refused provider session is revoked: {mode:?}"
            );
        }
        refused_modes.push(format!("{mode:?}"));
    }
    c.issuer.token_mode(TokenMode::Normal);
    assert!(
        c.issuer.count(|x| &x.jwks) <= jwks + 1,
        "unknown kid: at most one JWKS refetch per 60 s interval"
    );
    c.record("unknown key, forged signature, wrong issuer or audience, expiry, missing or foreign tenant, sub≠user.id and contract-shape drift are refused uniformly with no session; the provider session of a parseable refused exchange is logged out; JWKS refresh stays bounded");

    // --- 8. Foreign tenant and forged cookie scope ------------------------------
    let canary_user = "usr_00000000-0000-4000-8000-0000000000cc";
    c.issuer.user(canary_user, "canary@example.test", true);
    let mut canary = c.browser("192.0.2.17:2700");
    let reply = c.sign_in(&mut canary, CANARY_HANDLE, canary_user).await;
    assert_eq!(
        reply.status,
        StatusCode::FORBIDDEN,
        "verified canary identity without membership: neutral denial"
    );
    assert_eq!(field(&reply.body, "page"), "denied");
    assert!(
        canary.cookie(cookies::PRE_ACCESS).is_some() && canary.cookie(cookies::SESSION).is_none()
    );
    // A Northstar session cookie re-scoped to the canary prefix finds nothing.
    let session = verified.cookie(cookies::SESSION).unwrap();
    let raw = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &session)
        .unwrap();
    let canary_cookie = {
        let canary_raw = base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            canary.cookie(cookies::PRE_ACCESS).unwrap(),
        )
        .unwrap();
        let mut forged = canary_raw[..16].to_vec();
        forged.extend_from_slice(&raw[16..]);
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, forged)
    };
    let mut forger = c.browser("192.0.2.18:2800");
    forger.jar.insert(cookies::SESSION.into(), canary_cookie);
    let foreign = forger.get(&c.app, "/workspace").await;
    assert!(foreign.uniform_404());
    c.record("a verified canary identity with no membership gets the neutral denial page (sign-out only); a Northstar session value under the canary scope prefix resolves nothing (uniform 404)");

    // --- 9. Sign-out: epoch advance and receipts -------------------------------
    let (bea, mut bea_browser) = c.member("bea").await;
    let bea_principal = c.principal(&bea).await;
    let page = bea_browser.get(&c.app, "/workspace").await;
    let refresh_before = c.issuer.refresh_tokens(&bea);
    assert_eq!(refresh_before.len(), 1);
    let live = |r: &str| c.auth.vault.contains(r);
    let old = match c
        .stores
        .workspace
        .resolve_session(&bea_browser.cookie(cookies::SESSION).unwrap(), &live)
        .await
        .unwrap()
    {
        Lookup::Live(request) => request,
        _ => panic!("live session"),
    };
    let session_cookie = bea_browser.cookie(cookies::SESSION).unwrap();
    assert_eq!(
        bea_browser.post(&c.app, "/auth/signout", "").await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bea_browser
            .post(
                &c.app,
                "/auth/signout",
                &format!("csrf={}", field(&page.body, "signout_all"))
            )
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let cross = bea_browser
        .send(
            &c.app,
            "POST",
            "/auth/signout",
            &format!("csrf={}", field(&page.body, "signout")),
            &[("sec-fetch-site", "cross-site")],
        )
        .await;
    assert_eq!(cross.status, StatusCode::FORBIDDEN);
    let logouts = c.issuer.count(|x| &x.logout);
    let out = bea_browser
        .post(
            &c.app,
            "/auth/signout",
            &format!("csrf={}", field(&page.body, "signout")),
        )
        .await;
    assert_eq!(
        (out.status, out.location()),
        (StatusCode::SEE_OTHER, "/".into())
    );
    for name in [cookies::PRELOGIN, cookies::PRE_ACCESS, cookies::SESSION] {
        assert!(out.sets(name).unwrap().ends_with("Max-Age=0"));
    }
    assert_eq!(c.epoch(&bea_principal).await, 2);
    assert_eq!(c.issuer.count(|x| &x.logout), logouts + 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{bea_principal}'")
        )
        .await,
        0
    );
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE writer_role='admission' AND kind='session.sign_out' AND actor_principal_id='{bea_principal}'")).await, 1);
    // Receipts: the provider refuses the revoked token, GenUI's cookie admits nothing.
    let receipt = c.issuer.try_refresh("northstar", &refresh_before[0]).await;
    assert_eq!(receipt, (401, Some("invalid_grant".into())));
    let mut stale = c.browser("192.0.2.19:2900");
    stale
        .jar
        .insert(cookies::SESSION.into(), session_cookie.clone());
    assert!(stale.get(&c.app, "/workspace").await.uniform_404());
    // In-flight work of the old epoch fails its current-authority check.
    assert_eq!(
        c.stores.workspace.advance_epoch(&old, SignOut::One).await,
        Err(crate::storage::Error::Denied)
    );
    // Idempotent: the same cookie and token again still clears cookies, no epoch change.
    stale
        .jar
        .insert(cookies::SESSION.into(), session_cookie.clone());
    let again = stale
        .post(
            &c.app,
            "/auth/signout",
            &format!("csrf={}", field(&page.body, "signout")),
        )
        .await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert_eq!(c.epoch(&bea_principal).await, 2);
    c.record("sign-out advances the member epoch (advance_member_epoch, session.sign_out on the admission chain) and deletes the record in one transaction, then logs out the refresh token; receipts: the provider refuses that token with 401 invalid_grant and the old cookie admits nothing (uniform 404); in-flight old-epoch authority fails; repeat sign-out is idempotent; missing, wrong-form and cross-site tokens are refused");

    // --- 10. Sign-out everywhere ------------------------------------------------
    let (cy, mut cy_one) = c.member("cy").await;
    let cy_principal = c.principal(&cy).await;
    let mut cy_two = c.browser("192.0.2.31:5100");
    assert_eq!(
        c.sign_in(&mut cy_two, NORTHSTAR_HANDLE, &cy)
            .await
            .location(),
        "/workspace"
    );
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{cy_principal}'")
        )
        .await,
        2
    );
    let tokens = c.issuer.refresh_tokens(&cy);
    assert_eq!(tokens.len(), 2);
    let page = cy_one.get(&c.app, "/workspace").await;
    assert_eq!(
        cy_one
            .post(
                &c.app,
                "/auth/signout-all",
                &format!("csrf={}", field(&page.body, "signout"))
            )
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let logouts = c.issuer.count(|x| &x.logout);
    let all = cy_one
        .post(
            &c.app,
            "/auth/signout-all",
            &format!("csrf={}", field(&page.body, "signout_all")),
        )
        .await;
    assert_eq!(
        (all.status, all.location()),
        (StatusCode::SEE_OTHER, "/".into())
    );
    assert_eq!(
        c.epoch(&cy_principal).await,
        2,
        "one advance for every session"
    );
    assert_eq!(c.issuer.count(|x| &x.logout), logouts + 2);
    assert!(cy_two.get(&c.app, "/workspace").await.uniform_404());
    for token in &tokens {
        assert_eq!(c.issuer.try_refresh("northstar", token).await.0, 401);
    }
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.audit_events WHERE writer_role='admission' AND kind='session.sign_out_all' AND actor_principal_id='{cy_principal}'")).await, 1);
    c.record("sign-out-everywhere deletes every session of the member in the organization, advances the epoch once (session.sign_out_all) and revokes each refresh token (both refused afterwards)");

    // --- 11. Idle and absolute expiry, pre-login and pre-access expiry ----------
    let (dee, mut dee_browser) = c.member("dee").await;
    let dee_principal = c.principal(&dee).await;
    superuser(&c.fixture, &format!("UPDATE genui.sessions SET created_at=now()-interval '2 hours',last_seen_at=now()-interval '61 minutes',absolute_expires_at=now()+interval '9 hours' WHERE principal_id='{dee_principal}'")).await;
    let logouts = c.issuer.count(|x| &x.logout);
    assert!(dee_browser.get(&c.app, "/workspace").await.uniform_404());
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{dee_principal}'")
        )
        .await,
        0
    );
    assert_eq!(c.issuer.count(|x| &x.logout), logouts + 1);
    assert_eq!(
        c.sign_in(&mut dee_browser, NORTHSTAR_HANDLE, &dee)
            .await
            .location(),
        "/workspace"
    );
    superuser(&c.fixture, &format!("UPDATE genui.sessions SET created_at=now()-interval '12 hours 1 minute',last_seen_at=now()-interval '1 minute',absolute_expires_at=now()-interval '1 minute' WHERE principal_id='{dee_principal}'")).await;
    assert!(dee_browser.get(&c.app, "/workspace").await.uniform_404());
    assert_eq!(
        c.sign_in(&mut dee_browser, NORTHSTAR_HANDLE, &dee)
            .await
            .location(),
        "/workspace"
    );
    // A recent read does not refresh last_seen_at more than once a minute; an active one is kept.
    let seen = scalar(&c.fixture, &format!("SELECT extract(epoch FROM last_seen_at)::bigint FROM genui.sessions WHERE principal_id='{dee_principal}'")).await;
    assert_eq!(
        dee_browser.get(&c.app, "/workspace").await.status,
        StatusCode::OK
    );
    assert_eq!(scalar(&c.fixture, &format!("SELECT extract(epoch FROM last_seen_at)::bigint FROM genui.sessions WHERE principal_id='{dee_principal}'")).await, seen);
    // Pre-login flow past 10 minutes.
    let mut late = c.browser("192.0.2.32:5200");
    let start = late
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let query = c.issuer.login(&start.location(), &dee).unwrap();
    superuser(&c.fixture, "UPDATE genui.prelogin_flows SET created_at=now()-interval '11 minutes',expires_at=now()-interval '1 minute'").await;
    assert_eq!(
        late.get(&c.app, &format!("/auth/callback?{query}"))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    // Pre-access past 10 idle minutes.
    let ed = "usr_00000000-0000-4000-8000-0000000000ed";
    c.issuer.user(ed, "ed@example.test", false);
    let (_, digest_ed) = c.issue_code();
    let mut ed_browser = c.browser("192.0.2.33:5300");
    assert_eq!(
        c.invited_signup(&mut ed_browser, &digest_ed, ed)
            .await
            .location(),
        "/auth/pending"
    );
    superuser(&c.fixture, &format!("UPDATE genui.pre_access_sessions SET created_at=now()-interval '11 minutes',last_seen_at=now()-interval '11 minutes',expires_at=now()+interval '19 minutes' WHERE provider_subject='{ed}'")).await;
    assert!(ed_browser.get(&c.app, "/auth/pending").await.uniform_404());
    assert_eq!(
        scalar(
            &c.fixture,
            &format!(
                "SELECT count(*) FROM genui.pre_access_sessions WHERE provider_subject='{ed}'"
            )
        )
        .await,
        0
    );
    c.record("idle_and_absolute_expiry: a workspace session idle 1 hour or 12 hours old, a pre-login flow past 10 minutes and a pre-access session idle 10 minutes admit nothing; the expired record is deleted and its refresh token revoked; last_seen_at is written at most once a minute");

    // --- 12. Expired or revoked code during pending never mints ---------------
    let fay = "usr_00000000-0000-4000-8000-0000000000fa";
    c.issuer.user(fay, "fay@example.test", false);
    let (code_fay, digest_fay) = c.issue_code();
    let mut fay_browser = c.browser("192.0.2.34:5400");
    assert_eq!(
        c.invited_signup(&mut fay_browser, &digest_fay, fay)
            .await
            .location(),
        "/auth/pending"
    );
    operator(
        &c.fixture,
        &["revoke", "--organization", ORG, "--code-id", &code_fay],
    );
    c.issuer.set_verified(fay, true);
    let reply = c.sign_in(&mut fay_browser, NORTHSTAR_HANDLE, fay).await;
    assert_eq!(
        (reply.status, field(&reply.body, "page")),
        (StatusCode::FORBIDDEN, "denied".into())
    );
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.principals WHERE provider_subject='{fay}'")
        )
        .await,
        0
    );
    let gus = "usr_00000000-0000-4000-8000-00000000006a";
    c.issuer.user(gus, "gus@example.test", false);
    let (_, digest_gus) = c.issue_code();
    let mut gus_browser = c.browser("192.0.2.35:5500");
    assert_eq!(
        c.invited_signup(&mut gus_browser, &digest_gus, gus)
            .await
            .location(),
        "/auth/pending"
    );
    superuser(&c.fixture, &format!("UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE provider_subject='{gus}' AND kind='pending'")).await;
    superuser(&c.fixture, &format!("UPDATE genui.pre_access_sessions SET pending_expires_at=now()-interval '1 second' WHERE provider_subject='{gus}'")).await;
    // A resend after the pending admission lapsed: destroyed, 303 to a fresh sign-in (D7.17).
    let page = gus_browser.get(&c.app, "/auth/pending").await;
    assert_eq!(field(&page.body, "hours"), "0");
    // A routing hint naming another organization is ignored.
    gus_browser
        .jar
        .insert(cookies::ORGANIZATION_HINT.into(), CANARY_HANDLE.into());
    let lapsed = gus_browser
        .post(
            &c.app,
            "/auth/pending/resend",
            &format!("csrf={}", field(&page.body, "resend")),
        )
        .await;
    assert_eq!(
        lapsed.location(),
        "/signin",
        "no hint for this organization: the neutral entry"
    );
    c.issuer.set_verified(gus, true);
    let reply = c.sign_in(&mut gus_browser, NORTHSTAR_HANDLE, gus).await;
    assert_eq!(field(&reply.body, "page"), "denied");
    assert_eq!(scalar(&c.fixture, &format!("SELECT count(*) FROM genui.code_reservations WHERE provider_subject='{gus}' AND state='denied_for_reconciliation'")).await, 1);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.principals WHERE provider_subject='{gus}'")
        )
        .await,
        0
    );
    c.record("revoked_entitlement_during_pending_never_mints: a code revoked, or a pending admission lapsed (denied_for_reconciliation), before the verified sign-in yields the neutral denial and commits nothing; a resend after the lapse destroys the record and 303s to a fresh sign-in");

    // --- 13. Both completion orders mint at most one session -------------------
    // Order 1: a mint holding the member fence commits before a racing
    // entitlement revocation, which then invalidates the new session.
    let hal = "usr_00000000-0000-4000-8000-0000000000ab";
    c.issuer.user(hal, "hal@example.test", false);
    let (_, digest_hal) = c.issue_code();
    let mut hal_browser = c.browser("192.0.2.36:5600");
    assert_eq!(
        c.invited_signup(&mut hal_browser, &digest_hal, hal)
            .await
            .location(),
        "/auth/pending"
    );
    c.issuer.set_verified(hal, true);
    let mut racer = c.browser("192.0.2.37:5700");
    let start = racer
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let query = c.issuer.login(&start.location(), hal).unwrap();
    hooks.arm("mint");
    let (router, log, jar) = (c.app.clone(), c.log.clone(), racer.jar.clone());
    let first = tokio::spawn(async move {
        let mut b = Browser {
            jar,
            peer: "192.0.2.37:5700".parse().unwrap(),
            log,
        };
        b.get(&router, &format!("/auth/callback?{query}")).await
    });
    tokio::time::timeout(Duration::from_secs(10), hooks.reached.notified())
        .await
        .expect("mint barrier");
    let hal_principal = c.principal(hal).await;
    let revoke = cli_command(
        &c.fixture,
        "genui_migration",
        "synthetic-migration",
        &[
            "revoke-entitlement",
            "--organization",
            ORG,
            "--principal",
            &hal_principal,
        ],
    )
    .spawn()
    .unwrap();
    lock_wait(&c.fixture, 1).await;
    hooks.release.notify_one();
    let minted = first.await.unwrap();
    assert_eq!(
        minted.location(),
        "/workspace",
        "the mint holding the fence commits first"
    );
    let out = accepted(
        tokio::task::spawn_blocking(move || wait(revoke, "revoke-entitlement"))
            .await
            .unwrap(),
        "revoke-entitlement",
    );
    assert_eq!(out["changed"], true);
    let mut minted_browser = c.browser("192.0.2.37:5701");
    let value = minted
        .sets(cookies::SESSION)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned();
    minted_browser.jar.insert(cookies::SESSION.into(), value);
    assert!(
        minted_browser.get(&c.app, "/workspace").await.uniform_404(),
        "the revocation then invalidates it"
    );
    // Order 2: the revocation already committed; a later verified completion mints nothing.
    let mut later = c.browser("192.0.2.38:5800");
    let reply = c.sign_in(&mut later, NORTHSTAR_HANDLE, hal).await;
    assert_eq!(field(&reply.body, "page"), "denied");
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{hal_principal}'")
        )
        .await,
        0
    );
    // Two completions of one verified pre-access record race: exactly one session.
    // (Storage-level: the identity below is constructed by the test, not verified.)
    let (ivy, _) = c.member("ivy").await;
    let identity = |verified_subject: &str| genui_auth_verifier::VerifiedIdentity {
        issuer: ISSUER.into(),
        subject: verified_subject.into(),
        tenant_id: NORTHSTAR_TENANT.into(),
        key_id: "test".into(),
        issued_at: 0,
        expires_at: 0,
    };
    let mut flows = Vec::new();
    for _ in 0..2 {
        let mut twin = c.browser("192.0.2.40:6000");
        let start = twin
            .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
            .await;
        let query = c.issuer.login(&start.location(), &ivy).unwrap();
        let state = query
            .split('&')
            .find_map(|p| p.strip_prefix("state="))
            .unwrap()
            .to_owned();
        flows.push(
            c.stores
                .workspace
                .complete_flow(&twin.cookie(cookies::PRELOGIN).unwrap(), &state)
                .await
                .unwrap(),
        );
    }
    let unverified = c
        .stores
        .workspace
        .mint_pre_access(
            &flows[0],
            &identity(&ivy),
            false,
            &capability::random_uuid().unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        c.stores
            .workspace
            .complete_admission(&unverified.pre)
            .await
            .unwrap(),
        crate::storage::Completion::Denied
    ));
    let started = c
        .stores
        .workspace
        .mint_pre_access(
            &flows[1],
            &identity(&ivy),
            true,
            &capability::random_uuid().unwrap(),
        )
        .await
        .unwrap();
    let crate::storage::Completion::Member(member) = c
        .stores
        .workspace
        .complete_admission(&started.pre)
        .await
        .unwrap()
    else {
        panic!("a current member");
    };
    assert!(
        c.stores
            .workspace
            .mint_session(&unverified.pre, &member)
            .await
            .is_err(),
        "an unverified record never mints"
    );
    let (one, two) = tokio::join!(
        c.stores.workspace.mint_session(&started.pre, &member),
        c.stores.workspace.mint_session(&started.pre, &member)
    );
    assert_eq!(
        usize::from(one.is_ok()) + usize::from(two.is_ok()),
        1,
        "one record, one session"
    );
    c.record("both_admission_completion_orders_mint_at_most_one_session: a mint holding the member fence commits before a racing entitlement revocation, which then invalidates the session (uniform 404); with the revocation first a later verified completion is denied; two concurrent completions of one verified record mint exactly one session; an unverified record never mints");

    // --- 14. Restart invalidates refresh tokens and sessions ------------------
    let (jo, mut jo_browser) = c.member("jo").await;
    let jo_tokens = c.issuer.refresh_tokens(&jo);
    let kim = "usr_00000000-0000-4000-8000-0000000000cb";
    c.issuer.user(kim, "kim@example.test", false);
    let (_, digest_kim) = c.issue_code();
    let mut kim_browser = c.browser("192.0.2.41:6100");
    assert_eq!(
        c.invited_signup(&mut kim_browser, &digest_kim, kim)
            .await
            .location(),
        "/auth/pending"
    );
    let kim_page = kim_browser.get(&c.app, "/auth/pending").await;
    let mut pending_flow = c.browser("192.0.2.42:6200");
    let start = pending_flow
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let flow_query = c.issuer.login(&start.location(), &jo).unwrap();
    // A new process: new sealing key, vault and CSRF key over the same database.
    let restarted = Arc::new(Auth::loopback(
        config(&c.issuer.origin),
        Some(c.stores.clone()),
        Arc::new(TestPages),
    ));
    let restarted_app = app(restarted.clone());
    assert!(jo_browser
        .get(&restarted_app, "/workspace")
        .await
        .uniform_404());
    assert!(
        pending_flow
            .get(&restarted_app, &format!("/auth/callback?{flow_query}"))
            .await
            .status
            == StatusCode::BAD_REQUEST
    );
    let old_csrf = kim_browser
        .post(
            &restarted_app,
            "/auth/pending/resend",
            &format!("csrf={}", field(&kim_page.body, "resend")),
        )
        .await;
    assert_eq!(
        old_csrf.status,
        StatusCode::FORBIDDEN,
        "a synchronizer token from before the restart"
    );
    let page = kim_browser.get(&restarted_app, "/auth/pending").await;
    // The routing hint (set by an earlier visit) names this organization, so
    // the fresh sign-in goes to its handle; a foreign hint would not.
    kim_browser
        .jar
        .insert(cookies::ORGANIZATION_HINT.into(), CANARY_HANDLE.into());
    let page_again = kim_browser.get(&restarted_app, "/auth/pending").await;
    assert_eq!(page_again.status, StatusCode::OK);
    kim_browser
        .jar
        .insert(cookies::ORGANIZATION_HINT.into(), NORTHSTAR_HANDLE.into());
    let reply = kim_browser
        .post(
            &restarted_app,
            "/auth/pending/resend",
            &format!("csrf={}", field(&page.body, "resend")),
        )
        .await;
    assert_eq!(
        reply.location(),
        format!("/signin/{NORTHSTAR_HANDLE}"),
        "no stored refresh token after a restart"
    );
    assert_eq!(restarted.vault.len(), 0);
    // fAuth's own idle timeout bounds the orphaned tokens; they were never revoked here.
    assert_eq!(
        c.issuer.try_refresh("northstar", &jo_tokens[0]).await.0,
        200
    );
    c.record("restart_invalidates_refresh_tokens: a new process (new key, empty vault) admits no earlier workspace session, completes no earlier pre-login flow, accepts no earlier synchronizer token, and sends an earlier pending identity to a fresh sign-in");

    // --- 14b. A new arrival supersedes the browser's earlier session ----------
    // APP-009 review m1: a second sign-in in the same browser ends the first
    // workspace session (row, epoch advance, provider logout); a pending or
    // denied arrival also ends it and clears its cookie.
    c.auth.limits.clear();
    let (mo, mut mo_browser) = c.member("mo").await;
    let mo_principal = c.principal(&mo).await;
    let first = mo_browser.cookie(cookies::SESSION).unwrap();
    let epoch = c.epoch(&mo_principal).await;
    let logouts = c.issuer.count(|x| &x.logout);
    let again = c.sign_in(&mut mo_browser, NORTHSTAR_HANDLE, &mo).await;
    assert_eq!(again.location(), "/workspace");
    let second = mo_browser.cookie(cookies::SESSION).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{mo_principal}'")
        )
        .await,
        1
    );
    assert_eq!(c.epoch(&mo_principal).await, epoch + 1);
    assert_eq!(c.issuer.count(|x| &x.logout), logouts + 1);
    let mut superseded = c.browser("192.0.2.43:6300");
    superseded.jar.insert(cookies::SESSION.into(), first);
    assert!(superseded.get(&c.app, "/workspace").await.uniform_404());
    assert_eq!(
        field(&mo_browser.get(&c.app, "/workspace").await.body, "page"),
        "workspace"
    );
    let nia = "usr_00000000-0000-4000-8000-0000000000a1";
    c.issuer.user(nia, "nia@example.test", false);
    let (_, digest_nia) = c.issue_code();
    let pending = c.invited_signup(&mut mo_browser, &digest_nia, nia).await;
    assert_eq!(pending.location(), "/auth/pending");
    assert!(pending
        .sets(cookies::SESSION)
        .unwrap()
        .ends_with("Max-Age=0"));
    assert!(mo_browser.cookie(cookies::SESSION).is_none());
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{mo_principal}'")
        )
        .await,
        0
    );
    c.record("a new arrival supersedes the browser's earlier workspace session: a second sign-in ends the first (row deleted with an epoch advance, refresh token logged out, old cookie uniform 404), and a pending arrival of another identity ends it and clears its cookie");

    // --- 15. Expiry sweep ------------------------------------------------------
    let (lu, mut lu_browser) = c.member("lu").await;
    let lu_principal = c.principal(&lu).await;
    let _ = lu_browser.get(&c.app, "/workspace").await;
    superuser(&c.fixture, &format!("UPDATE genui.sessions SET created_at=now()-interval '2 hours',last_seen_at=now()-interval '61 minutes',absolute_expires_at=now()+interval '9 hours' WHERE principal_id='{lu_principal}'")).await;
    let logouts = c.issuer.count(|x| &x.logout);
    c.auth.sweep().await;
    assert_eq!(
        scalar(
            &c.fixture,
            &format!("SELECT count(*) FROM genui.sessions WHERE principal_id='{lu_principal}'")
        )
        .await,
        0
    );
    assert!(c.issuer.count(|x| &x.logout) > logouts);
    assert_eq!(
        scalar(
            &c.fixture,
            "SELECT count(*) FROM genui.prelogin_flows WHERE expires_at<=now()"
        )
        .await,
        0
    );
    c.record("the expiry sweep deletes idle or expired records in every registry organization and revokes their refresh tokens");

    // --- 16. Nothing from the provider reaches a browser -------------------------
    let issued = c.issuer.state.issued.lock().unwrap().clone();
    let replies = c.log.lock().unwrap().clone();
    for reply in &replies {
        let headers = format!("{:?}", reply.headers);
        assert!(!headers.contains(ISSUER_COOKIE) && !reply.body.contains(ISSUER_COOKIE));
        for value in &issued {
            if value.starts_with("urn:ietf:params:oauth:request_uri:")
                && reply.status == StatusCode::SEE_OTHER
            {
                // The single-use request_uri is the authorize hand-off itself (ADR-006).
                continue;
            }
            assert!(
                !headers.contains(value.as_str()) && !reply.body.contains(value.as_str()),
                "provider capability in a response"
            );
        }
        assert!(!reply.body.contains("access_token") && !reply.body.contains("refresh_token"));
    }
    c.record(&format!("no_provider_token_or_set_cookie_reaches_browser: {} responses carry none of {} issued codes, tokens or request URIs (outside the authorize redirect), no token JSON and no issuer Set-Cookie", replies.len(), issued.len()));

    // --- report ----------------------------------------------------------------
    let all = scalar(&c.fixture, "SELECT count(*) FROM genui.sessions").await;
    if let Some(path) = std::env::var_os("GENUI_SESSION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "scope": "APP-009 isolated native PostgreSQL TLS session campaign: ADR-006 routes over the real router, reviewed verifier, cfg(test) synthetic loopback issuer",
            "managed_acceptance": false, "hosted_acceptance": false,
            "server_version": c.fixture.server_version,
            "handles": {"northstar": NORTHSTAR_HANDLE, "canary": CANARY_HANDLE},
            "resend_outcomes": outcomes, "refused_token_modes": refused_modes,
            "sessions_at_end": all, "checks": c.checks, "passed": true,
            "limits": ["The provider is the cfg(test) synthetic loopback issuer (simulated transport and hosted page), never the deployed fAuth; hosted PAR, mail delivery and browsers are APP-074/076.",
                       "The invited signup runs through APP-074's POST /invite/validate; provider invitations come from the loopback management stand-in app/runtime/fake_fauth.py (simulated transport).",
                       "Synthetic local password authentication is not RDS IAM; identifiers are synthetic.",
                       "Rendering uses a test page renderer; the SSR pages are covered by the binary's own test and the HTTP probes."]
        })).unwrap()).unwrap();
    }
}
