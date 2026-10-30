//! cfg(test) loopback synthetic fAuth issuer implementing the pinned contract
//! (evidence/APP-006/upstream-auth-source: GENUI-IDENTITY-CONTRACT-2026-10-28
//! and merged-contract-2026-10-29/1bf5d3b): access-only `/auth/jwks`
//! (OKP/Ed25519, `Cache-Control: public,max-age=3600`); form-encoded PAR
//! answering `201 {request_uri, expires_in: 60}` with a single-use 60-second
//! `request_uri`; the authorize hand-off (driven by `login`, standing in for
//! the hosted page); the JSON code exchange with the exact token-response
//! shape including `user.email_verified` and an issuer `Set-Cookie`; refresh
//! rotation with family revocation on reuse; logout `204`; and the resend
//! outcomes of 1bf5d3b plus the rehearsal image's (dfba787). Literal issuer
//! `gatewasm`, audience `gatewasm-app`, 900-second access tokens. SIMULATED:
//! never the deployed fAuth; synthetic identifiers only.
use super::capability::random_capability;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
    digest,
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{net::TcpListener, task::JoinHandle};

pub const ISSUER: &str = "gatewasm";
pub const AUDIENCE: &str = "gatewasm-app";
pub const CALLBACK: &str = "https://beta.getforma.dev/auth/callback";
pub const ISSUER_COOKIE: &str = "gw_rt";

pub struct Key(Ed25519KeyPair);
impl Key {
    pub fn fresh() -> Self {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        Self(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap())
    }
    pub fn kid(&self) -> String {
        let hash = digest::digest(&digest::SHA256, self.0.public_key().as_ref());
        URL_SAFE_NO_PAD.encode(&hash.as_ref()[..8])
    }
    fn jwk(&self) -> Value {
        json!({"kty":"OKP","use":"sig","kid":self.kid(),"alg":"EdDSA","crv":"Ed25519",
               "x":URL_SAFE_NO_PAD.encode(self.0.public_key().as_ref())})
    }
    pub fn sign(&self, claims: &Value) -> String {
        let head = json!({"alg":"EdDSA","typ":"JWT","kid":self.kid()});
        let signed = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(head.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        format!(
            "{signed}.{}",
            URL_SAFE_NO_PAD.encode(self.0.sign(signed.as_bytes()).as_ref())
        )
    }
}

/// How the next code exchanges answer (each a refusal case of ADR-006).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenMode {
    Normal,
    /// Signed by a key the JWKS does not publish.
    UnknownKid,
    /// A valid signature over other claims.
    ForgedSignature,
    WrongIssuer,
    WrongAudience,
    Expired,
    /// No `tenant_id` claim.
    MissingTenant,
    /// Another tenant's `tenant_id`.
    ForeignTenant,
    /// JWT `sub` differs from `user.id`.
    SubjectMismatch,
    /// An extra top-level field (deny_unknown_fields).
    ExtraField,
    /// No `user.email_verified`.
    MissingEmailVerified,
    /// A 20-second access token, so the vault refreshes before a resend.
    ShortAccess,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshMode {
    Normal,
    Unauthorized,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResendMode {
    Accepted,
    AlreadyVerified,
    Cooldown,
    TooManyRequests,
    EmailUnavailable,
    EmailFailed,
    /// dfba787: `200 {message}` without `status`.
    LegacyMessage,
    /// dfba787: `500 email_failed`.
    LegacyFailed,
    Unauthorized,
    NotFound,
    Timeout,
}

pub struct Tenant {
    pub slug: &'static str,
    pub tenant_id: String,
    pub client_id: &'static str,
}

struct Par {
    slug: String,
    state: String,
    challenge: String,
    invited: bool,
    created: Instant,
}
struct Grant {
    slug: String,
    challenge: String,
    subject: String,
    created: Instant,
}
struct Refresh {
    slug: String,
    subject: String,
    family: String,
    used: bool,
}
#[derive(Default)]
pub struct Counters {
    pub jwks: AtomicUsize,
    pub par: AtomicUsize,
    pub par_invited: AtomicUsize,
    pub exchange: AtomicUsize,
    pub refresh: AtomicUsize,
    pub logout: AtomicUsize,
    pub resend: AtomicUsize,
}

pub struct IssuerState {
    key: Key,
    stray: Key,
    tenants: Vec<Tenant>,
    pars: Mutex<HashMap<String, Par>>,
    grants: Mutex<HashMap<String, Grant>>,
    refreshes: Mutex<HashMap<String, Refresh>>,
    revoked_families: Mutex<Vec<String>>,
    users: Mutex<HashMap<String, (String, bool)>>,
    pub token_mode: Mutex<TokenMode>,
    pub refresh_mode: Mutex<RefreshMode>,
    pub resend_mode: Mutex<ResendMode>,
    pub counters: Counters,
    /// Every capability this issuer handed out (codes, tokens, request URIs),
    /// so a test can prove none reaches a GenUI response.
    pub issued: Mutex<Vec<String>>,
}

pub struct Issuer {
    pub origin: String,
    pub state: Arc<IssuerState>,
    task: JoinHandle<()>,
}
impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn now() -> u64 {
    super::now()
}

impl Issuer {
    pub async fn start(tenants: Vec<Tenant>) -> Self {
        let state = Arc::new(IssuerState {
            key: Key::fresh(),
            stray: Key::fresh(),
            tenants,
            pars: Mutex::default(),
            grants: Mutex::default(),
            refreshes: Mutex::default(),
            revoked_families: Mutex::default(),
            users: Mutex::default(),
            token_mode: Mutex::new(TokenMode::Normal),
            refresh_mode: Mutex::new(RefreshMode::Normal),
            resend_mode: Mutex::new(ResendMode::Accepted),
            counters: Counters::default(),
            issued: Mutex::default(),
        });
        let app = Router::new()
            .route("/auth/jwks", get(jwks))
            .route("/t/{slug}/oauth/par", post(par))
            .route("/t/{slug}/auth/token", post(token))
            .route("/t/{slug}/auth/logout", post(logout))
            .route("/t/{slug}/auth/resend-verification", post(resend))
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
    pub fn user(&self, subject: &str, email: &str, verified: bool) {
        self.state
            .users
            .lock()
            .unwrap()
            .insert(subject.into(), (email.into(), verified));
    }
    pub fn set_verified(&self, subject: &str, verified: bool) {
        if let Some(user) = self.state.users.lock().unwrap().get_mut(subject) {
            user.1 = verified;
        }
    }
    pub fn token_mode(&self, mode: TokenMode) {
        *self.state.token_mode.lock().unwrap() = mode;
    }
    pub fn refresh_mode(&self, mode: RefreshMode) {
        *self.state.refresh_mode.lock().unwrap() = mode;
    }
    pub fn resend_mode(&self, mode: ResendMode) {
        *self.state.resend_mode.lock().unwrap() = mode;
    }
    pub fn count(&self, counter: fn(&Counters) -> &AtomicUsize) -> usize {
        counter(&self.state.counters).load(Ordering::SeqCst)
    }
    /// The hosted page: consumes the PAR handle named by an authorize URL and,
    /// for `subject`, returns the callback query `code=…&state=…`. None when
    /// the handle is unknown, consumed, expired or for another client.
    pub fn login(&self, authorize_url: &str, subject: &str) -> Option<String> {
        let url = url::Url::parse(authorize_url).ok()?;
        let slug = url
            .path()
            .strip_prefix("/t/")?
            .strip_suffix("/auth/authorize")?
            .to_owned();
        let pairs: HashMap<String, String> = url.query_pairs().into_owned().collect();
        if pairs.len() != 2 {
            return None;
        }
        let tenant = self.state.tenants.iter().find(|t| t.slug == slug)?;
        if pairs.get("client_id")? != tenant.client_id {
            return None;
        }
        let par = self
            .state
            .pars
            .lock()
            .unwrap()
            .remove(pairs.get("request_uri")?)?;
        if par.slug != slug || par.created.elapsed() > Duration::from_secs(60) {
            return None;
        }
        let code = random_capability().unwrap();
        self.state.issued.lock().unwrap().push(code.clone());
        self.state.grants.lock().unwrap().insert(
            code.clone(),
            Grant {
                slug,
                challenge: par.challenge,
                subject: subject.into(),
                created: Instant::now(),
            },
        );
        let _ = par.invited;
        Some(format!("code={code}&state={}", par.state))
    }
    /// The live (unused, unrevoked) refresh tokens issued to a subject.
    pub fn refresh_tokens(&self, subject: &str) -> Vec<String> {
        let revoked = self.state.revoked_families.lock().unwrap().clone();
        self.state
            .refreshes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, r)| r.subject == subject && !r.used && !revoked.contains(&r.family))
            .map(|(token, _)| token.clone())
            .collect()
    }
    /// A direct back-channel refresh (the sign-out receipt): the HTTP status
    /// and the error code.
    pub async fn try_refresh(&self, slug: &str, refresh_token: &str) -> (u16, Option<String>) {
        let response = reqwest::Client::new()
            .post(format!("{}/t/{slug}/auth/token", self.origin))
            .json(&json!({"grant_type":"refresh_token","refresh_token":refresh_token}))
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        (status, body["error"].as_str().map(str::to_owned))
    }
}

async fn jwks(State(state): State<Arc<IssuerState>>) -> Response {
    state.counters.jwks.fetch_add(1, Ordering::SeqCst);
    (
        [
            (header::CACHE_CONTROL, "public,max-age=3600"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        json!({"keys":[state.key.jwk()]}).to_string(),
    )
        .into_response()
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

async fn par(
    State(state): State<Arc<IssuerState>>,
    Path(slug): Path<String>,
    body: Bytes,
) -> Response {
    state.counters.par.fetch_add(1, Ordering::SeqCst);
    let form: Vec<(String, String)> = url::form_urlencoded::parse(&body).into_owned().collect();
    let get = |name: &str| {
        let found: Vec<&String> = form
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v)
            .collect();
        (found.len() == 1).then(|| found[0].clone())
    };
    let Some(tenant) = state.tenants.iter().find(|t| t.slug == slug) else {
        return error(StatusCode::NOT_FOUND, "tenant_not_found");
    };
    let invite = get("invite_token");
    let allowed = [
        "provider",
        "client_id",
        "redirect_uri",
        "response_type",
        "state",
        "code_challenge",
        "code_challenge_method",
        "invite_token",
    ];
    if form.iter().any(|(k, _)| !allowed.contains(&k.as_str()))
        || get("provider").as_deref() != Some("email")
        || get("client_id").as_deref() != Some(tenant.client_id)
        || get("redirect_uri").as_deref() != Some(CALLBACK)
        || get("response_type").as_deref() != Some("code")
        || get("code_challenge_method").as_deref() != Some("S256")
        || get("code_challenge").is_none_or(|c| c.len() != 43)
        || get("state").is_none_or(|s| s.is_empty() || s.len() > 1024)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    if invite.is_some() {
        state.counters.par_invited.fetch_add(1, Ordering::SeqCst);
    }
    let suffix: String = (0..16)
        .map(|_| format!("{:02x}", super::capability::random_bytes::<1>().unwrap()[0]))
        .collect();
    let request_uri = format!("urn:ietf:params:oauth:request_uri:{suffix}");
    state.issued.lock().unwrap().push(request_uri.clone());
    state.pars.lock().unwrap().insert(
        request_uri.clone(),
        Par {
            slug,
            state: get("state").unwrap(),
            challenge: get("code_challenge").unwrap(),
            invited: invite.is_some(),
            created: Instant::now(),
        },
    );
    (
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({"request_uri": request_uri, "expires_in": 60})),
    )
        .into_response()
}

fn issue_tokens(
    state: &IssuerState,
    slug: &str,
    subject: &str,
    family: Option<String>,
) -> Response {
    let mode = *state.token_mode.lock().unwrap();
    let tenant = state.tenants.iter().find(|t| t.slug == slug).unwrap();
    let (email, verified) = state
        .users
        .lock()
        .unwrap()
        .get(subject)
        .cloned()
        .unwrap_or_else(|| ("synthetic@example.test".into(), false));
    let iat = now();
    let lifetime = if mode == TokenMode::ShortAccess {
        20
    } else {
        900
    };
    let mut claims = json!({
        "sub": subject, "email": email, "name": "Synthetic member",
        "tenant_id": tenant.tenant_id, "iss": ISSUER, "aud": AUDIENCE,
        "exp": iat + lifetime, "iat": iat, "roles": ["user"],
        "auth_method": "password", "mfa_verified": false,
    });
    match mode {
        TokenMode::WrongIssuer => claims["iss"] = json!("https://auth-staging.getforma.dev"),
        TokenMode::WrongAudience => claims["aud"] = json!(tenant.client_id),
        TokenMode::Expired => {
            claims["iat"] = json!(iat - 1000);
            claims["exp"] = json!(iat - 100);
        }
        TokenMode::MissingTenant => {
            claims.as_object_mut().unwrap().remove("tenant_id");
        }
        TokenMode::ForeignTenant => {
            let other = state.tenants.iter().find(|t| t.slug != slug).unwrap();
            claims["tenant_id"] = json!(other.tenant_id);
        }
        TokenMode::SubjectMismatch => claims["sub"] = json!(format!("{subject}-other")),
        _ => {}
    }
    let access = match mode {
        TokenMode::UnknownKid => state.stray.sign(&claims),
        TokenMode::ForgedSignature => {
            let genuine = state.key.sign(&claims);
            let mut forged = claims.clone();
            forged["roles"] = json!(["admin"]);
            let mut parts: Vec<String> = genuine.split('.').map(str::to_owned).collect();
            parts[1] = URL_SAFE_NO_PAD.encode(forged.to_string());
            parts.join(".")
        }
        _ => state.key.sign(&claims),
    };
    let refresh = random_capability().unwrap();
    {
        let mut issued = state.issued.lock().unwrap();
        issued.push(access.clone());
        issued.push(refresh.clone());
    }
    state.refreshes.lock().unwrap().insert(
        refresh.clone(),
        Refresh {
            slug: slug.into(),
            subject: subject.into(),
            family: family.unwrap_or_else(|| random_capability().unwrap()),
            used: false,
        },
    );
    let expires_at = iat + lifetime;
    let mut body = json!({
        "access_token": access, "refresh_token": refresh, "token_type": "Bearer",
        "expires_in": lifetime, "expires_at": expires_at,
        "user": {"id": subject, "email": email, "name": "Synthetic member", "avatar": null,
                 "email_verified": verified, "roles": ["user"], "auth_method": "password",
                 "mfa_verified": false},
    });
    match mode {
        TokenMode::ExtraField => body["id_token"] = json!("not-issued-by-this-contract"),
        TokenMode::MissingEmailVerified => {
            body["user"]
                .as_object_mut()
                .unwrap()
                .remove("email_verified");
        }
        _ => {}
    }
    let mut response = Json(body).into_response();
    // The issuer refresh cookie of the pinned contract; GenUI never forwards it.
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!("{ISSUER_COOKIE}={refresh}; Path=/; Secure; HttpOnly; SameSite=Strict")
            .parse()
            .unwrap(),
    );
    response
}

async fn token(
    State(state): State<Arc<IssuerState>>,
    Path(slug): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let fields = body.as_object().cloned().unwrap_or_default();
    match fields.get("grant_type").and_then(Value::as_str) {
        Some("authorization_code") => {
            state.counters.exchange.fetch_add(1, Ordering::SeqCst);
            let text = |name: &str| fields.get(name).and_then(Value::as_str).unwrap_or("");
            let tenant = state.tenants.iter().find(|t| t.slug == slug);
            // One-use code: removed whatever happens next.
            let grant = state.grants.lock().unwrap().remove(text("code"));
            let valid = fields.len() == 5
                && tenant.is_some_and(|t| t.client_id == text("client_id"))
                && text("redirect_uri") == CALLBACK
                && grant.as_ref().is_some_and(|g| {
                    g.slug == slug
                        && g.created.elapsed() <= Duration::from_secs(60)
                        && super::pkce::challenge(text("code_verifier")) == g.challenge
                });
            match (valid, grant) {
                (true, Some(grant)) => issue_tokens(&state, &slug, &grant.subject, None),
                _ => error(StatusCode::BAD_REQUEST, "invalid_grant"),
            }
        }
        Some("refresh_token") => {
            state.counters.refresh.fetch_add(1, Ordering::SeqCst);
            match *state.refresh_mode.lock().unwrap() {
                RefreshMode::Unauthorized => {
                    return error(StatusCode::UNAUTHORIZED, "invalid_grant")
                }
                RefreshMode::Unavailable => {
                    return error(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
                }
                RefreshMode::Normal => {}
            }
            let presented = fields
                .get("refresh_token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let (subject, family) = {
                let mut refreshes = state.refreshes.lock().unwrap();
                let Some(record) = refreshes.get_mut(&presented) else {
                    return error(StatusCode::UNAUTHORIZED, "invalid_grant");
                };
                if record.slug != slug {
                    return error(StatusCode::UNAUTHORIZED, "invalid_grant");
                }
                if state
                    .revoked_families
                    .lock()
                    .unwrap()
                    .contains(&record.family)
                {
                    return error(StatusCode::UNAUTHORIZED, "invalid_grant");
                }
                if record.used {
                    // Reuse revokes the whole family.
                    state
                        .revoked_families
                        .lock()
                        .unwrap()
                        .push(record.family.clone());
                    return error(StatusCode::UNAUTHORIZED, "token_reuse_detected");
                }
                record.used = true;
                (record.subject.clone(), record.family.clone())
            };
            issue_tokens(&state, &slug, &subject, Some(family))
        }
        _ => error(StatusCode::BAD_REQUEST, "unsupported_grant_type"),
    }
}

async fn logout(
    State(state): State<Arc<IssuerState>>,
    Path(slug): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    state.counters.logout.fetch_add(1, Ordering::SeqCst);
    let presented = body["refresh_token"].as_str().unwrap_or("").to_owned();
    if let Some(record) = state.refreshes.lock().unwrap().get(&presented) {
        if record.slug == slug {
            state
                .revoked_families
                .lock()
                .unwrap()
                .push(record.family.clone());
        }
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!("{ISSUER_COOKIE}=; Path=/; Max-Age=0")
            .parse()
            .unwrap(),
    );
    response
}

fn bearer_subject(state: &IssuerState, headers: &HeaderMap) -> Option<String> {
    let token = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    // Signature, expiry and audience as the provider would judge them.
    let mut parts = token.split('.');
    let (head, body, signature) = (parts.next()?, parts.next()?, parts.next()?);
    let public = ring::signature::UnparsedPublicKey::new(
        &ring::signature::ED25519,
        state.key.0.public_key().as_ref().to_vec(),
    );
    public
        .verify(
            format!("{head}.{body}").as_bytes(),
            &URL_SAFE_NO_PAD.decode(signature).ok()?,
        )
        .ok()?;
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(body).ok()?).ok()?;
    (claims["exp"].as_u64()? > now()).then(|| claims["sub"].as_str().map(str::to_owned))?
}

async fn resend(State(state): State<Arc<IssuerState>>, headers: HeaderMap) -> Response {
    state.counters.resend.fetch_add(1, Ordering::SeqCst);
    if bearer_subject(&state, &headers).is_none() {
        return error(StatusCode::UNAUTHORIZED, "invalid_token");
    }
    let mode = *state.resend_mode.lock().unwrap();
    match mode {
        ResendMode::Accepted => (
            StatusCode::ACCEPTED,
            Json(json!({"status":"provider_accepted"})),
        )
            .into_response(),
        ResendMode::AlreadyVerified => Json(json!({"status":"already_verified"})).into_response(),
        ResendMode::Cooldown => error(StatusCode::TOO_MANY_REQUESTS, "resend_cooldown"),
        ResendMode::TooManyRequests => error(StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
        ResendMode::EmailUnavailable => error(StatusCode::SERVICE_UNAVAILABLE, "email_unavailable"),
        ResendMode::EmailFailed => error(StatusCode::BAD_GATEWAY, "email_failed"),
        ResendMode::LegacyMessage => {
            Json(json!({"message":"Verification email sent"})).into_response()
        }
        ResendMode::LegacyFailed => error(StatusCode::INTERNAL_SERVER_ERROR, "email_failed"),
        ResendMode::Unauthorized => error(StatusCode::UNAUTHORIZED, "invalid_token"),
        ResendMode::NotFound => error(StatusCode::NOT_FOUND, "not_found"),
        ResendMode::Timeout => {
            tokio::time::sleep(Duration::from_millis(3500)).await;
            (
                StatusCode::ACCEPTED,
                Json(json!({"status":"provider_accepted"})),
            )
                .into_response()
        }
    }
}
