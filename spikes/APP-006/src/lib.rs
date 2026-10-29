//! Source-bound fAuth identity verification. This is not an application principal,
//! membership decision, session implementation, or deployed-service qualification.
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{header, redirect, Client, Url};
use ring::{digest, signature};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::{timeout_at, Instant};

pub const MAX_TOKEN_BYTES: usize = 8192;
pub const MAX_JWKS_BYTES: usize = 65536;
pub const VERIFY_DEADLINE: Duration = Duration::from_secs(2);
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
pub const MAX_CACHE_AGE: Duration = Duration::from_secs(3600);
const FUTURE_IAT_SKEW: u64 = 60;
pub const MAX_ACCESS_KEYS: usize = 10; // current + next + eight previous (248 manifest cap)

#[derive(Clone, Copy)]
pub enum ContractProfile {
    Forma247,
    Forma248AccessOnly,
}

/// Fixed codes only: errors never embed a token, claim, URL, key or response body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Configuration,
    MalformedToken,
    UnsupportedToken,
    InvalidClaims,
    ForeignTenant,
    InvalidSignature,
    UnknownKey,
    RefreshThrottled,
    KeysetUnavailable,
    KeysetMalformed,
    UnqualifiedKeyPurpose,
    Deadline,
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "auth_refused:{self:?}")
    }
}
impl std::error::Error for Refusal {}

/// Explicit expected deployment values, never inferred from token contents,
/// the configured endpoint's hostname or an OAuth client identifier.
pub struct Config {
    pub profile: ContractProfile,
    pub issuer: String,
    pub audience: String,
    pub jwks_url: String,
    /// Must be supplied from the chosen issuer's deployment contract/receipt.
    pub max_access_lifetime: Duration,
}

/// Cryptographically verified external identity only. The application must still
/// resolve invited membership and current organization authority independently.
/// Deliberately excludes role/email/name assertions as application permissions.
#[derive(PartialEq, Eq)]
pub struct VerifiedIdentity {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
    pub key_id: String,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl fmt::Debug for VerifiedIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerifiedIdentity([redacted])")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    alg: String,
    typ: String,
    kid: String,
}

// Fields/types derived from FORMA-247 signing::Claims at the recorded source pin.
// Serde derive refuses duplicate fields, including fields written with escapes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    sub: String,
    email: String,
    name: Option<String>,
    tenant_id: Option<String>,
    iss: String,
    aud: String,
    exp: u64,
    iat: u64,
    roles: Vec<String>,
    #[serde(default = "password")]
    auth_method: String,
    #[serde(default)]
    mfa_verified: bool,
    cnf: Option<serde_json::Value>,
}
fn password() -> String {
    "password".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Jwks {
    keys: Vec<Jwk>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Jwk {
    kty: String,
    #[serde(rename = "use")]
    usage: String,
    kid: String,
    alg: String,
    crv: String,
    x: String,
}
struct Key {
    kid: String,
    bytes: Vec<u8>,
}
struct KeyLease {
    bytes: Vec<u8>,
    generation: u64,
    fresh_until: Instant,
}
#[derive(Default)]
struct Cache {
    keys: Vec<Key>,
    fresh_until: Option<Instant>,
    attempted_at: Option<Instant>,
    generation: u64,
    refresh_attempts: u64,
    accepted_keysets: u64,
}

/// A bounded, coherent observation of this verifier instance. No key IDs,
/// identities, tokens, URLs or clock values are exposed. Attempts are reserved
/// before I/O; cancellation may prevent a reserved attempt reaching the issuer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostics {
    pub refresh_attempts: u64,
    pub accepted_keysets: u64,
    pub cached_access_keys: usize,
    pub generation: u64,
}

/// Share one instance per configured issuer in a process. The attempt budget is
/// per instance, not a distributed fleet limit; no caller can supply a fetch URL.
pub struct Verifier {
    config: Config,
    endpoint: Url,
    client: Client,
    cache: Arc<Mutex<Cache>>,
    wall_now: fn() -> Result<u64, Refusal>,
}
impl Verifier {
    pub fn new(config: Config) -> Result<Self, Refusal> {
        Self::construct(config, false, unix_time)
    }

    /// Observe counters without fetching, changing freshness or resetting the
    /// attempt budget. A concurrent operation cannot expose a partial snapshot.
    pub async fn diagnostics(&self) -> Result<Diagnostics, Refusal> {
        let cache = timeout_at(Instant::now() + VERIFY_DEADLINE, self.cache.lock())
            .await
            .map_err(|_| Refusal::Deadline)?;
        Ok(Diagnostics {
            refresh_attempts: cache.refresh_attempts,
            accepted_keysets: cache.accepted_keysets,
            cached_access_keys: cache.keys.len(),
            generation: cache.generation,
        })
    }

    fn construct(
        config: Config,
        test_loopback_http: bool,
        wall_now: fn() -> Result<u64, Refusal>,
    ) -> Result<Self, Refusal> {
        let endpoint = Url::parse(&config.jwks_url).map_err(|_| Refusal::Configuration)?;
        let loopback = endpoint
            .host_str()
            .and_then(|host| host.parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if !bounded_text(&config.issuer, 512)
            || !bounded_text(&config.audience, 512)
            || config.max_access_lifetime.is_zero()
            || config.max_access_lifetime > Duration::from_secs(86400)
            || (endpoint.scheme() != "https"
                && !(test_loopback_http && endpoint.scheme() == "http" && loopback))
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/auth/jwks"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(Refusal::Configuration);
        }
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
            .timeout(VERIFY_DEADLINE)
            .build()
            .map_err(|_| Refusal::Configuration)?;
        Ok(Self {
            config,
            endpoint,
            client,
            cache: Arc::new(Mutex::new(Cache::default())),
            wall_now,
        })
    }

    /// `expected_tenant` is trusted server context, not a request header or picker.
    /// A valid identity does not establish membership, invitation, or resource access.
    pub async fn verify(
        &self,
        token: &str,
        expected_tenant: &str,
    ) -> Result<VerifiedIdentity, Refusal> {
        let deadline = Instant::now() + VERIFY_DEADLINE;
        timeout_at(
            deadline,
            self.verify_inner(token, expected_tenant, deadline),
        )
        .await
        .map_err(|_| Refusal::Deadline)?
    }

    async fn verify_inner(
        &self,
        token: &str,
        expected_tenant: &str,
        deadline: Instant,
    ) -> Result<VerifiedIdentity, Refusal> {
        let (head, claims, signed, signature) = parse_token(token)?;
        self.validate_claims(&claims, expected_tenant)?;
        let key = self.key(&head.kid, deadline).await?;
        signature::UnparsedPublicKey::new(&signature::ED25519, &key.bytes)
            .verify(signed.as_bytes(), &signature)
            .map_err(|_| Refusal::InvalidSignature)?;
        // Hold the current cache generation through identity release. Another
        // request cannot invalidate/replace authority between this fence and
        // the returned identity; expiry cannot be extended by a late fetch.
        let cache = self.cache.lock().await;
        self.validate_claims(&claims, expected_tenant)?;
        if Instant::now() >= deadline {
            return Err(Refusal::Deadline);
        }
        validate_lease(&key, &cache)?;
        Ok(VerifiedIdentity {
            issuer: claims.iss,
            subject: claims.sub,
            tenant_id: claims.tenant_id.ok_or(Refusal::InvalidClaims)?,
            key_id: head.kid,
            issued_at: claims.iat,
            expires_at: claims.exp,
        })
    }

    fn validate_claims(&self, c: &Claims, expected_tenant: &str) -> Result<(), Refusal> {
        let now = (self.wall_now)()?;
        if c.iss != self.config.issuer
            || c.aud != self.config.audience
            || !bounded_text(&c.sub, 256)
            || !bounded_text(&c.email, 320)
            || c.name.as_ref().is_some_and(|s| s.len() > 256)
            || !c.tenant_id.as_ref().is_some_and(|s| bounded_text(s, 256))
            || !bounded_text(expected_tenant, 256)
            || c.exp <= now
            || c.iat > now.saturating_add(FUTURE_IAT_SKEW)
            || c.exp <= c.iat
            || c.exp - c.iat > self.config.max_access_lifetime.as_secs()
            || c.roles.len() > 64
            || c.roles.iter().any(|s| !bounded_text(s, 64))
            || !bounded_text(&c.auth_method, 64)
            || c.cnf.is_some()
        {
            return Err(Refusal::InvalidClaims);
        }
        // An MFA assertion is not an app permission or a performed local MFA check.
        let _ = c.mfa_verified;
        if c.tenant_id.as_deref() != Some(expected_tenant) {
            return Err(Refusal::ForeignTenant);
        }
        Ok(())
    }

    async fn key(&self, kid: &str, deadline: Instant) -> Result<KeyLease, Refusal> {
        let mut cache = self.cache.lock().await;
        let now = Instant::now();
        if cache.fresh_until.is_some_and(|until| now < until) {
            if let Some(key) = cache.keys.iter().find(|key| key.kid == kid) {
                return Ok(KeyLease {
                    bytes: key.bytes.clone(),
                    generation: cache.generation,
                    fresh_until: cache.fresh_until.ok_or(Refusal::KeysetUnavailable)?,
                });
            }
        }
        if cache
            .attempted_at
            .is_some_and(|last| now.saturating_duration_since(last) < REFRESH_INTERVAL)
        {
            return Err(Refusal::RefreshThrottled);
        }
        // Reserve before I/O; cancellation, failures and concurrent misses cannot
        // create extra attempts. Invalidate first, so cancellation cannot leave
        // stale authority. A failed unknown-kid refresh deliberately denies all
        // new sign-ins until an allowed successful refresh; existing app sessions
        // are outside this verifier. An unknown kid never becomes a cached key.
        cache.attempted_at = Some(now);
        cache.refresh_attempts = cache.refresh_attempts.saturating_add(1);
        cache.keys.clear();
        cache.fresh_until = None;
        cache.generation = cache
            .generation
            .checked_add(1)
            .ok_or(Refusal::KeysetUnavailable)?;
        let (keys, ttl) = timeout_at(deadline, self.fetch())
            .await
            .map_err(|_| Refusal::Deadline)??;
        let fresh_until = now + ttl; // request-start freshness, not response arrival
        if Instant::now() >= fresh_until {
            return Err(Refusal::KeysetUnavailable);
        }
        let matches = keys.iter().any(|key| key.kid == kid);
        cache.keys = keys;
        cache.fresh_until = Some(fresh_until);
        cache.accepted_keysets = cache.accepted_keysets.saturating_add(1);
        if !matches {
            return Err(Refusal::UnknownKey);
        }
        Ok(KeyLease {
            bytes: cache
                .keys
                .iter()
                .find(|key| key.kid == kid)
                .ok_or(Refusal::UnknownKey)?
                .bytes
                .clone(),
            generation: cache.generation,
            fresh_until,
        })
    }

    async fn fetch(&self) -> Result<(Vec<Key>, Duration), Refusal> {
        let mut response = self
            .client
            .get(self.endpoint.clone())
            .header(header::ACCEPT, "application/json")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::CACHE_CONTROL, "no-cache")
            .send()
            .await
            .map_err(|_| Refusal::KeysetUnavailable)?;
        if response.status() != reqwest::StatusCode::OK
            || response
                .content_length()
                .is_some_and(|n| n > MAX_JWKS_BYTES as u64)
            || response
                .headers()
                .get(header::CONTENT_ENCODING)
                .is_some_and(|x| x != "identity")
            || response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_none_or(|s| s.split(';').next() != Some("application/json"))
        {
            return Err(Refusal::KeysetUnavailable);
        }
        let ttl = cache_ttl(response.headers())?;
        let mut body = Vec::new();
        while let Some(part) = response
            .chunk()
            .await
            .map_err(|_| Refusal::KeysetUnavailable)?
        {
            if body.len().saturating_add(part.len()) > MAX_JWKS_BYTES {
                return Err(Refusal::KeysetMalformed);
            }
            body.extend_from_slice(&part);
        }
        Ok((parse_jwks(&body, self.config.profile)?, ttl))
    }
}

fn validate_lease(key: &KeyLease, cache: &Cache) -> Result<(), Refusal> {
    if Instant::now() >= key.fresh_until
        || cache.generation != key.generation
        || cache.fresh_until != Some(key.fresh_until)
        || cache.keys.is_empty()
    {
        return Err(Refusal::KeysetUnavailable);
    }
    Ok(())
}

fn unix_time() -> Result<u64, Refusal> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Refusal::InvalidClaims)
}
fn bounded_text(value: &str, cap: usize) -> bool {
    !value.is_empty() && value.len() <= cap && !value.chars().any(char::is_control)
}
fn decode(value: &str) -> Result<Vec<u8>, Refusal> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| Refusal::MalformedToken)
}
fn parse_token(token: &str) -> Result<(Header, Claims, &str, Vec<u8>), Refusal> {
    if token.len() > MAX_TOKEN_BYTES || !token.is_ascii() {
        return Err(Refusal::MalformedToken);
    }
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(Refusal::MalformedToken);
    }
    let header: Header =
        serde_json::from_slice(&decode(parts[0])?).map_err(|_| Refusal::MalformedToken)?;
    if header.alg != "EdDSA" || header.typ != "JWT" || decode(&header.kid)?.len() != 8 {
        return Err(Refusal::UnsupportedToken);
    }
    let claims = serde_json::from_slice(&decode(parts[1])?).map_err(|_| Refusal::MalformedToken)?;
    let signature = decode(parts[2])?;
    if signature.len() != 64 {
        return Err(Refusal::MalformedToken);
    }
    let signed = &token[..parts[0].len() + 1 + parts[1].len()];
    Ok((header, claims, signed, signature))
}
fn key_id(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(&digest::digest(&digest::SHA256, bytes).as_ref()[..8])
}
fn parse_jwks(bytes: &[u8], profile: ContractProfile) -> Result<Vec<Key>, Refusal> {
    let jwks: Jwks = serde_json::from_slice(bytes).map_err(|_| Refusal::KeysetMalformed)?;
    // Cardinality is explicit source-profile policy, never inferred from a
    // mixed-purpose set. Both profiles trust only the fixed /auth/jwks endpoint.
    let limit = match profile {
        ContractProfile::Forma247 => 1,
        ContractProfile::Forma248AccessOnly => MAX_ACCESS_KEYS,
    };
    if jwks.keys.is_empty() || jwks.keys.len() > limit {
        return Err(Refusal::UnqualifiedKeyPurpose);
    }
    let mut keys: Vec<Key> = Vec::with_capacity(jwks.keys.len());
    for key in jwks.keys {
        let bytes = URL_SAFE_NO_PAD
            .decode(key.x)
            .map_err(|_| Refusal::KeysetMalformed)?;
        if key.kty != "OKP"
            || key.usage != "sig"
            || key.alg != "EdDSA"
            || key.crv != "Ed25519"
            || bytes.len() != 32
            || key.kid != key_id(&bytes)
            || keys.iter().any(|seen| seen.kid == key.kid)
        {
            return Err(Refusal::KeysetMalformed);
        }
        keys.push(Key {
            kid: key.kid,
            bytes,
        });
    }
    Ok(keys)
}
fn cache_ttl(headers: &header::HeaderMap) -> Result<Duration, Refusal> {
    if headers.get_all(header::CACHE_CONTROL).iter().count() != 1
        || headers.get_all(header::AGE).iter().count() > 1
    {
        return Err(Refusal::KeysetMalformed);
    }
    let value = headers
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .ok_or(Refusal::KeysetMalformed)?;
    let mut max_age = None;
    for part in value.split(',').map(str::trim) {
        if part.eq_ignore_ascii_case("no-store") || part.eq_ignore_ascii_case("no-cache") {
            return Err(Refusal::KeysetUnavailable);
        }
        if let Some(age) = part.strip_prefix("max-age=") {
            if max_age.is_some() {
                return Err(Refusal::KeysetMalformed);
            }
            max_age = Some(age.parse::<u64>().map_err(|_| Refusal::KeysetMalformed)?);
        }
    }
    let age = headers
        .get(header::AGE)
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or(Refusal::KeysetMalformed)
        })
        .transpose()?
        .unwrap_or(0);
    let ttl = max_age
        .ok_or(Refusal::KeysetMalformed)?
        .min(MAX_CACHE_AGE.as_secs())
        .saturating_sub(age);
    if ttl == 0 {
        return Err(Refusal::KeysetUnavailable);
    }
    Ok(Duration::from_secs(ttl))
}

#[cfg(test)]
mod tests;
