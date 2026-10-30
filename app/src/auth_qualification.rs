//! Bounded APP-006 identity qualification, never application authentication.
//!
//! No public registration, app principal, membership or workspace session is
//! created here. No request/response payload, query, Location or secret is logged.
//!
//! ADR-007 D7.13 (APP-009): compiled only with the `qualification-harness`
//! feature, which release builds never enable; the product `auth` module owns
//! `/auth/callback` and the service binary refuses the harness configuration.
#![cfg(feature = "qualification-harness")]
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use genui_auth_verifier::{Config as VerifyConfig, ContractProfile, VerifiedIdentity, Verifier};
use reqwest::{redirect, Client};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    future::Future,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{timeout, Instant};
use url::Url;

const CALLBACK: &str = "https://beta.getforma.dev/auth/callback";
const RESULT: &str = "/auth/qualification/result";
const COOKIE: &str = "__Host-genui-qualification";
const FLOW_SECONDS: u64 = 300;
const PROVIDER_DEADLINE: Duration = Duration::from_secs(3);
const MAX_BODY: usize = 32768;
const MAX_REQUESTS: u32 = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    run_id: String,
    /// Declared source pins; deployment/image receipts must validate them.
    app_source_revision: String,
    provider_source_revision: String,
    /// Hard wall-clock cutoff, also fenced by a monotonic deadline at startup.
    expires_at: u64,
    max_attempts: u32,
    max_flows: usize,
    synthetic_tenant: bool,
    auth_origin: String,
    tenant_slug: String,
    tenant_id: String,
    client_id: String,
    issuer: String,
    audience: String,
    max_access_lifetime_seconds: u64,
    expected_email: String,
    expected_subject: Option<String>,
    expected_email_verified: bool,
    /// A random 32-byte base64url operator credential; never a URL parameter.
    operator_token: String,
    /// Null selects existing-user login; a value selects invited signup PAR.
    invite_token: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Configuration,
    Input,
    Closed,
    Operator,
    State,
    Provider,
    Identity,
    Cleanup,
}

type IdentityFuture<'a> =
    Pin<Box<dyn Future<Output = Result<VerifiedIdentity, Failure>> + Send + 'a>>;
// Private construction only. Production has exactly one implementation; the
// local fixture implementation exists solely under cfg(test).
trait IdentityCheck: Send + Sync {
    fn check<'a>(&'a self, token: &'a str, tenant: &'a str) -> IdentityFuture<'a>;
}
struct CanonicalVerifier(Verifier);
impl IdentityCheck for CanonicalVerifier {
    fn check<'a>(&'a self, token: &'a str, tenant: &'a str) -> IdentityFuture<'a> {
        Box::pin(async move {
            self.0
                .verify(token, tenant)
                .await
                .map_err(|_| Failure::Identity)
        })
    }
}

struct Flow {
    attempt: u32,
    state: String,
    verifier: String,
    deadline: Instant,
}
#[derive(Default, Serialize)]
struct Counters {
    requests: u32,
    starts: u32,
    callback_exchanges: u32,
    identity_checked: u32,
    unverified_mailbox: u32,
    refusals: u32,
    logout_acknowledged: u32,
    cleanup_unknown: u32,
}
#[derive(Default)]
struct Ledger {
    flows: HashMap<String, Flow>,
    counters: Counters,
    receipts: Vec<ExchangeReceipt>,
}
#[derive(Serialize)]
struct ExchangeReceipt {
    attempt: u32,
    completed_at: u64,
    verified_subject: Option<String>,
    email_verified: Option<bool>,
    identity_qualified: bool,
    logout_acknowledged: bool,
}
struct Qualification {
    config: Config,
    deadline: Instant,
    client: Client,
    verifier: Arc<dyn IdentityCheck>,
    ledger: Mutex<Ledger>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn source_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn provider_id(value: &str, prefix: &str) -> bool {
    let Some(uuid) = value.strip_prefix(prefix) else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
fn capability(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && URL_SAFE_NO_PAD.decode(value).is_ok_and(|v| v.len() == 32)
}
fn random_capability() -> Result<String, Failure> {
    let mut bytes = [0; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| Failure::Configuration)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn same_secret(left: &str, right: &str) -> bool {
    // Fixed-size digests avoid a prefix-dependent comparison on secret bytes.
    let a = Sha256::digest(left.as_bytes());
    let b = Sha256::digest(right.as_bytes());
    a.iter()
        .zip(b.iter())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

impl Config {
    fn validate(&self) -> Result<(), Failure> {
        let origin = Url::parse(&self.auth_origin).map_err(|_| Failure::Configuration)?;
        if !self.synthetic_tenant
            || !text(&self.run_id, 64)
            || !self
                .run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !source_revision(&self.app_source_revision)
            || !source_revision(&self.provider_source_revision)
            || self.expires_at <= now()
            || self.expires_at > now().saturating_add(6 * 3600)
            || !(1..=12).contains(&self.max_attempts)
            || !(1..=3).contains(&self.max_flows)
            || self.max_flows > self.max_attempts as usize
            || origin.scheme() != "https"
            || origin.origin().ascii_serialization() != self.auth_origin
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.host_str().is_none()
            || !text(&self.tenant_slug, 64)
            || !self
                .tenant_slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !provider_id(&self.tenant_id, "ten_")
            || !text(&self.client_id, 128)
            || !self
                .client_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || self.client_id.ends_with("-hosted")
            || !text(&self.issuer, 512)
            || !text(&self.audience, 512)
            || !(1..=86400).contains(&self.max_access_lifetime_seconds)
            || !text(&self.expected_email, 320)
            || !self.expected_email.contains('@')
            || self
                .expected_subject
                .as_ref()
                .is_some_and(|v| !provider_id(v, "usr_"))
            || !capability(&self.operator_token)
            || self
                .invite_token
                .as_ref()
                .is_some_and(|v| !text(v, 256) || v.trim() != v)
            || (self.invite_token.is_none() && self.expected_subject.is_none())
        {
            return Err(Failure::Configuration);
        }
        Ok(())
    }
}

fn private_config(path: &Path) -> Result<Config, Failure> {
    if !path.is_absolute() {
        return Err(Failure::Configuration);
    }
    let uid = unsafe { libc::geteuid() };
    let parent = path
        .parent()
        .ok_or(Failure::Configuration)?
        .symlink_metadata()
        .map_err(|_| Failure::Configuration)?;
    if !parent.is_dir() || parent.uid() != uid || parent.mode() & 0o077 != 0 {
        return Err(Failure::Configuration);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Failure::Configuration)?;
    let metadata = file.metadata().map_err(|_| Failure::Configuration)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || metadata.len() > 8192
    {
        return Err(Failure::Configuration);
    }
    let mut bytes = Vec::new();
    file.take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Configuration)?;
    if bytes.len() > 8192 {
        return Err(Failure::Configuration);
    }
    serde_json::from_slice(&bytes).map_err(|_| Failure::Configuration)
}

/// Absence is the only disabled setting. Invalid explicit configuration stops
/// startup; it cannot silently select an insecure/fixture implementation.
pub fn from_environment() -> Result<Router, &'static str> {
    let path = match std::env::var("GENUI_AUTH_QUALIFICATION_CONFIG_FILE") {
        Err(std::env::VarError::NotPresent) => return Ok(Router::new()),
        Ok(path) if !path.is_empty() => path,
        _ => return Err("invalid configuration"),
    };
    let config = private_config(Path::new(&path)).map_err(|_| "invalid private configuration")?;
    let state = Qualification::new(config).map_err(|_| "invalid qualification configuration")?;
    Ok(routes(Arc::new(state)))
}

fn routes(state: Arc<Qualification>) -> Router {
    Router::new()
        .route("/auth/qualification/start", post(start))
        .route("/auth/callback", get(callback))
        .route("/auth/qualification/status", get(status))
        .route(RESULT, get(result))
        .with_state(state)
}

impl Qualification {
    fn new(config: Config) -> Result<Self, Failure> {
        config.validate()?;
        let verifier = Verifier::new(VerifyConfig {
            profile: ContractProfile::Forma248AccessOnly,
            issuer: config.issuer.clone(),
            audience: config.audience.clone(),
            jwks_url: format!("{}/auth/jwks", config.auth_origin),
            max_access_lifetime: Duration::from_secs(config.max_access_lifetime_seconds),
        })
        .map_err(|_| Failure::Configuration)?;
        let client = Client::builder()
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .referer(false)
            .connect_timeout(Duration::from_secs(1))
            .timeout(PROVIDER_DEADLINE)
            .build()
            .map_err(|_| Failure::Configuration)?;
        Ok(Self {
            deadline: Instant::now() + Duration::from_secs(config.expires_at.saturating_sub(now())),
            config,
            client,
            verifier: Arc::new(CanonicalVerifier(verifier)),
            ledger: Mutex::new(Ledger::default()),
        })
    }
    fn open(&self) -> bool {
        now() < self.config.expires_at && Instant::now() < self.deadline
    }
    fn operator(&self, headers: &HeaderMap) -> bool {
        let mut values = headers.get_all(header::AUTHORIZATION).iter();
        let Some(value) = values
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        else {
            return false;
        };
        values.next().is_none()
            && capability(value)
            && same_secret(value, &self.config.operator_token)
    }
    fn take_request(&self) -> Result<(), Failure> {
        let mut ledger = self.ledger.lock().map_err(|_| Failure::Closed)?;
        if !self.open() || ledger.counters.requests >= MAX_REQUESTS {
            return Err(Failure::Closed);
        }
        ledger.counters.requests += 1;
        ledger
            .flows
            .retain(|_, flow| flow.deadline > Instant::now());
        Ok(())
    }
    fn refused(&self) {
        if let Ok(mut ledger) = self.ledger.lock() {
            ledger.counters.refusals = ledger.counters.refusals.saturating_add(1);
        }
    }
    fn endpoint(&self, operation: &str) -> String {
        format!(
            "{}/t/{}/{}",
            self.config.auth_origin, self.config.tenant_slug, operation
        )
    }
    async fn start_flow(&self) -> Result<(String, String), Failure> {
        let cookie = random_capability()?;
        let state = random_capability()?;
        let verifier = random_capability()?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        {
            let mut ledger = self.ledger.lock().map_err(|_| Failure::Closed)?;
            if !self.open()
                || ledger.counters.starts >= self.config.max_attempts
                || ledger.flows.len() >= self.config.max_flows
            {
                return Err(Failure::Closed);
            }
            ledger.counters.starts += 1;
            let attempt = ledger.counters.starts;
            ledger.flows.insert(
                cookie.clone(),
                Flow {
                    attempt,
                    state: state.clone(),
                    verifier,
                    deadline: (Instant::now() + Duration::from_secs(FLOW_SECONDS))
                        .min(self.deadline),
                },
            );
        }
        let mut parameters = vec![
            ("provider", "email"),
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", CALLBACK),
            ("response_type", "code"),
            ("state", &state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ];
        let authorization = async {
            let mut authorize =
                Url::parse(&self.endpoint("auth/authorize")).map_err(|_| Failure::Configuration)?;
            if let Some(invite) = &self.config.invite_token {
                parameters.push(("invite_token", invite));
                let body = url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(&parameters)
                    .finish();
                let response = self
                    .client
                    .post(self.endpoint("oauth/par"))
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(body)
                    .send()
                    .await
                    .map_err(|_| Failure::Provider)?;
                let par: Par = read_json(response, StatusCode::CREATED).await?;
                let suffix = par
                    .request_uri
                    .strip_prefix("urn:ietf:params:oauth:request_uri:")
                    .ok_or(Failure::Provider)?;
                if par.expires_in != 60
                    || suffix.len() != 32
                    || !suffix
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Failure::Provider);
                }
                authorize
                    .query_pairs_mut()
                    .append_pair("client_id", &self.config.client_id)
                    .append_pair("request_uri", &par.request_uri);
            } else {
                authorize.query_pairs_mut().extend_pairs(&parameters);
            }
            if !self.open() {
                return Err(Failure::Closed);
            }
            Ok(authorize.to_string())
        };
        let answer = timeout(PROVIDER_DEADLINE, authorization)
            .await
            .unwrap_or(Err(Failure::Provider));
        match answer {
            Ok(url) => Ok((cookie, url)),
            Err(error) => {
                self.ledger
                    .lock()
                    .map_err(|_| Failure::Closed)?
                    .flows
                    .remove(&cookie);
                Err(error)
            }
        }
    }
    fn consume(&self, cookie: &str, state: &str) -> Result<Flow, Failure> {
        let mut ledger = self.ledger.lock().map_err(|_| Failure::Closed)?;
        // Remove even on a mismatched state. No failed continuation can be retried.
        let flow = ledger.flows.remove(cookie).ok_or(Failure::State)?;
        if !self.open() || flow.deadline <= Instant::now() || !same_secret(&flow.state, state) {
            return Err(Failure::State);
        }
        ledger.counters.callback_exchanges += 1;
        Ok(flow)
    }
    async fn exchange(&self, code: &str, flow: Flow) -> Result<(), Failure> {
        // Crossing this boundary can mint remote refresh material. Unknown
        // outcomes require operator/provider cleanup, never an automatic retry.
        // Reserve before I/O and retain the reservation if this future is
        // canceled at any await. Acknowledged logout is the only clearing path.
        self.ledger
            .lock()
            .map_err(|_| Failure::Closed)?
            .counters
            .cleanup_unknown += 1;
        let response = timeout(PROVIDER_DEADLINE, async {
            let response = self.client.post(self.endpoint("auth/token")).json(&serde_json::json!({
                "grant_type":"authorization_code", "code":code,
                "code_verifier":flow.verifier, "client_id":self.config.client_id, "redirect_uri":CALLBACK,
            })).send().await.map_err(|_| Failure::Provider)?;
            read_json::<Token>(response, StatusCode::OK).await
        }).await.unwrap_or(Err(Failure::Provider));
        let token = response?;
        let checked = self.check_token(&token, flow.deadline).await;
        // Always request cleanup of a parseable response, including refused
        // identity. Provider cookies are never retained or forwarded.
        let logout = timeout(PROVIDER_DEADLINE, async {
            if !capability(&token.refresh_token) || !text(&token.access_token, 8192) {
                return Err(Failure::Cleanup);
            }
            let response = self
                .client
                .post(self.endpoint("auth/logout"))
                .bearer_auth(&token.access_token)
                .json(&serde_json::json!({"refresh_token":token.refresh_token}))
                .send()
                .await
                .map_err(|_| Failure::Cleanup)?;
            if response.status() != StatusCode::NO_CONTENT {
                return Err(Failure::Cleanup);
            }
            Ok(())
        })
        .await
        .unwrap_or(Err(Failure::Cleanup));
        let mut ledger = self.ledger.lock().map_err(|_| Failure::Closed)?;
        if logout.is_ok() {
            ledger.counters.logout_acknowledged += 1;
            ledger.counters.cleanup_unknown -= 1;
        }
        let qualified =
            logout.is_ok() && checked.is_ok() && self.open() && flow.deadline > Instant::now();
        ledger.receipts.push(ExchangeReceipt {
            attempt: flow.attempt,
            completed_at: now(),
            verified_subject: checked
                .as_ref()
                .ok()
                .map(|identity| identity.subject.clone()),
            email_verified: checked.as_ref().ok().map(|_| token.user.email_verified),
            identity_qualified: qualified,
            logout_acknowledged: logout.is_ok(),
        });
        logout?;
        checked?;
        // A late response cannot revive a closed rehearsal or expired flow.
        if !self.open() || flow.deadline <= Instant::now() {
            return Err(Failure::Closed);
        }
        ledger.counters.identity_checked += 1;
        if !token.user.email_verified {
            ledger.counters.unverified_mailbox += 1;
        }
        Ok(())
    }
    async fn check_token(
        &self,
        token: &Token,
        flow_deadline: Instant,
    ) -> Result<VerifiedIdentity, Failure> {
        if token.token_type != "Bearer"
            || !capability(&token.refresh_token)
            || !text(&token.access_token, 8192)
            || !provider_id(&token.user.id, "usr_")
            || token.user.email != self.config.expected_email
            || token.user.email_verified != self.config.expected_email_verified
            || token.expires_in == 0
            || token.expires_in > self.config.max_access_lifetime_seconds
            || token.expires_at <= now()
            || token.user.roles.len() > 64
            || token.user.roles.iter().any(|r| !text(r, 128))
            || token.user.name.as_ref().is_some_and(|v| !text(v, 512))
            || token.user.avatar.as_ref().is_some_and(|v| !text(v, 2048))
            || token.user.auth_method != "password"
            || token.user.mfa_verified
        {
            return Err(Failure::Identity);
        }
        let identity = self
            .verifier
            .check(&token.access_token, &self.config.tenant_id)
            .await?;
        if identity.issuer != self.config.issuer
            || identity.tenant_id != self.config.tenant_id
            || identity.subject != token.user.id
            || self
                .config
                .expected_subject
                .as_ref()
                .is_some_and(|s| *s != identity.subject)
            || identity.expires_at != token.expires_at
            || identity.expires_at.saturating_sub(identity.issued_at) != token.expires_in
            || identity.expires_at <= now()
            || !self.open()
            || flow_deadline <= Instant::now()
        {
            return Err(Failure::Identity);
        }
        Ok(identity)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Par {
    request_uri: String,
    expires_in: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Token {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: u64,
    expires_at: u64,
    user: User,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct User {
    id: String,
    email: String,
    name: Option<String>,
    avatar: Option<String>,
    email_verified: bool,
    roles: Vec<String>,
    auth_method: String,
    mfa_verified: bool,
}
async fn read_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
    expected: StatusCode,
) -> Result<T, Failure> {
    if response.status() != expected
        || response.headers().contains_key(header::CONTENT_ENCODING)
        || response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| v.split(';').next() != Some("application/json"))
        || response
            .content_length()
            .is_some_and(|v| v > MAX_BODY as u64)
    {
        return Err(Failure::Provider);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Provider)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(Failure::Provider);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| Failure::Provider)
}

fn secured(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
        ),
    );
    response
}
fn refused() -> Response {
    secured((StatusCode::FORBIDDEN, "Qualification refused.").into_response())
}
fn callback_done() -> Response {
    secured((StatusCode::SEE_OTHER, [
        (header::LOCATION, RESULT),
        (header::SET_COOKIE, "__Host-genui-qualification=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"),
    ]).into_response())
}
async fn result() -> Response {
    secured("Identity qualification request finished. No application access was granted. The operator receipt records the outcome; mailbox activation and provider cleanup require separate evidence.".into_response())
}
async fn empty_request(request: Request) -> Result<HeaderMap, Failure> {
    if request.uri().query().is_some() {
        return Err(Failure::Input);
    }
    let (parts, body) = request.into_parts();
    let bytes = timeout(Duration::from_secs(1), to_bytes(body, 1))
        .await
        .map_err(|_| Failure::Input)?
        .map_err(|_| Failure::Input)?;
    if !bytes.is_empty() {
        return Err(Failure::Input);
    }
    Ok(parts.headers)
}
async fn start(State(state): State<Arc<Qualification>>, request: Request) -> Response {
    let outcome = async {
        state.take_request()?;
        // Authenticate before reading the body or invoking any provider work.
        if !state.operator(request.headers()) {
            return Err(Failure::Operator);
        }
        empty_request(request).await?;
        state.start_flow().await
    }
    .await;
    match outcome {
        Ok((cookie, url)) => {
            let mut response =
                secured((StatusCode::SEE_OTHER, [(header::LOCATION, url)]).into_response());
            response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&format!("{COOKIE}={cookie}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={FLOW_SECONDS}")).expect("generated base64url cookie"));
            response
        }
        Err(_) => {
            state.refused();
            refused()
        }
    }
}
fn callback_input(request: &Request) -> Result<(String, String, String), Failure> {
    if request.method() != axum::http::Method::GET {
        return Err(Failure::Input);
    }
    let query = request.uri().query().ok_or(Failure::Input)?;
    if query.len() > 256 {
        return Err(Failure::Input);
    }
    // Capabilities use only canonical base64url; rejecting percent escapes and
    // '+' avoids ambiguous/duplicate encodings before any state lookup.
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').ok_or(Failure::Input)?;
        if !capability(value) {
            return Err(Failure::Input);
        }
        match name {
            "code" if code.is_none() => code = Some(value.to_owned()),
            "state" if state.is_none() => state = Some(value.to_owned()),
            _ => return Err(Failure::Input),
        }
    }
    let mut cookie = None;
    let mut bytes = 0;
    for value in request.headers().get_all(header::COOKIE) {
        bytes += value.len();
        if bytes > 2048 {
            return Err(Failure::Input);
        }
        for pair in value.to_str().map_err(|_| Failure::Input)?.split(';') {
            if let Some((name, value)) = pair.trim().split_once('=') {
                if name == COOKIE {
                    if cookie.is_some() || !capability(value) {
                        return Err(Failure::Input);
                    }
                    cookie = Some(value.to_owned());
                }
            }
        }
    }
    Ok((
        code.ok_or(Failure::Input)?,
        state.ok_or(Failure::Input)?,
        cookie.ok_or(Failure::Input)?,
    ))
}
async fn callback(State(state): State<Arc<Qualification>>, request: Request) -> Response {
    let outcome = async {
        state.take_request()?;
        let (code, returned_state, cookie) = callback_input(&request)?;
        let flow = state.consume(&cookie, &returned_state)?;
        // GET request bodies are not part of the callback contract.
        let bytes = timeout(Duration::from_secs(1), to_bytes(request.into_body(), 1))
            .await
            .map_err(|_| Failure::Input)?
            .map_err(|_| Failure::Input)?;
        if !bytes.is_empty() {
            return Err(Failure::Input);
        }
        state.exchange(&code, flow).await
    }
    .await;
    if outcome.is_err() {
        state.refused();
    }
    callback_done()
}
async fn status(State(state): State<Arc<Qualification>>, request: Request) -> Response {
    // This operator-only count receipt carries no principal, token or URL.
    if !state.operator(request.headers()) || request.uri().query().is_some() {
        return refused();
    }
    match empty_request(request).await {
        Ok(_) => match state.ledger.lock() {
            Ok(mut ledger) => {
                ledger
                    .flows
                    .retain(|_, flow| state.open() && flow.deadline > Instant::now());
                secured(
                Json(serde_json::json!({
                    "scope":"identity qualification only", "open":state.open(),
                    "run_id":state.config.run_id,
                    "configured_source_pins":{"app":state.config.app_source_revision,"provider":state.config.provider_source_revision},
                    "source_pins_require_deployment_receipts":true,
                    "issuer":state.config.issuer,"fauth_tenant_id":state.config.tenant_id,
                    "fauth_tenant_slug":state.config.tenant_slug,"client_id":state.config.client_id,
                    "counts":ledger.counters, "pending_flows":ledger.flows.len(),
                    "exchanges":ledger.receipts,
                    "workspace_access_granted":false, "mailbox_delivery_proved":false,
                    "provider_revocation_proved":false,
                }))
                .into_response(),
            )
            }
            Err(_) => refused(),
        },
        Err(_) => refused(),
    }
}

#[cfg(test)]
mod tests;
