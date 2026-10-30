//! APP-009: fAuth-backed sign-in, verification-pending state and sign-out
//! (ADR-006 routes, cookies, lifetimes, CSRF rules and resend mapping;
//! ADR-007 D7.11, D7.13–D7.15; ADR-003 pre-access).
//!
//! The browser holds only opaque `__Host-` cookies. The back channel (PAR,
//! code exchange, refresh, logout, resend) runs here with bounded deadlines;
//! no fAuth token JSON, code, `request_uri` or issuer `Set-Cookie` ever
//! reaches a response, log or row. The organization always comes from the
//! sign-in handle's server-side lookup, confirmed at the callback by the
//! verified token's `(issuer, tenant_id)` through the closed registry, never
//! from a path, header or body. Rendering is the binary's (`Pages`): this
//! module decides the state, the status and the cookies.
//!
//! APP-074 adds the public invitation entry (`invite`): `GET /invite` and
//! `POST /invite/validate`, the backend PAR with the confidential provider
//! `invite_token`, and the trusted-proxy restriction of the limit keys.
pub mod capability;
pub mod cookies;
pub mod csrf;
pub mod invite;
pub mod pending;
pub mod pkce;
pub mod provider;
pub mod routes;
pub mod vault;

// Crate-visible to the APP-012 query campaign, which signs members in over
// the same routes; still compiled only into the test executable.
#[cfg(test)]
mod invite_tests;
#[cfg(test)]
pub(crate) mod synthetic_issuer;
#[cfg(test)]
pub(crate) mod tests;

use crate::{
    http::{limits::Limiter, proxy::TrustedProxies},
    storage::{capability_shaped, Ended, Stores},
};
use axum::{http::request::Parts, response::Response};
use genui_auth_verifier::{Config as VerifierConfig, ContractProfile, Verifier};
use serde::Deserialize;
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The private deployment file naming the provider routing (absent: every
/// sign-in fails neutrally and protected routes stay the uniform 404).
pub const CONFIG_ENV: &str = "GENUI_AUTH_CONFIG_FILE";
const CONFIG_LIMIT: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    auth_origin: String,
    app_origin: String,
    issuer: String,
    audience: String,
    max_access_lifetime_seconds: u64,
    organizations: Vec<RouteDocument>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteDocument {
    organization_id: String,
    tenant_slug: String,
    client_id: String,
}

/// Per-organization provider routing, read only after the organization is
/// known server-side (ADR-007 D7.2): fAuth tenant slug and OAuth client id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub organization: String,
    pub tenant_slug: String,
    pub client_id: String,
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub auth_origin: String,
    /// `https://beta.getforma.dev`: the exact `Origin` a `POST` may carry.
    pub app_origin: String,
    /// The exact registered redirect: `{app_origin}/auth/callback`.
    pub callback: String,
    pub issuer: String,
    pub audience: String,
    pub max_access_lifetime: Duration,
    routes: Vec<Route>,
}

fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| b.is_ascii_graphic())
}
fn exact_origin(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        u.scheme() == "https"
            && u.origin().ascii_serialization() == value
            && u.username().is_empty()
            && u.password().is_none()
    })
}

impl AuthConfig {
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let document: Document =
            serde_json::from_slice(bytes).map_err(|_| "auth configuration refused")?;
        let mut seen = std::collections::HashSet::new();
        if document.schema_version != 1
            || !exact_origin(&document.auth_origin)
            || !exact_origin(&document.app_origin)
            || !bounded(&document.issuer, 512)
            || !bounded(&document.audience, 512)
            || !(1..=86_400).contains(&document.max_access_lifetime_seconds)
            || document.organizations.is_empty()
            || document.organizations.len() > 32
            || document.organizations.iter().any(|r| {
                !crate::storage::valid_organization(&r.organization_id)
                    || !seen.insert(r.organization_id.clone())
                    || r.tenant_slug.is_empty()
                    || r.tenant_slug.len() > 64
                    || !r
                        .tenant_slug
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    || r.client_id.is_empty()
                    || r.client_id.len() > 128
                    || !r
                        .client_id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    || r.client_id.ends_with("-hosted")
            })
        {
            return Err("auth configuration refused");
        }
        Ok(Self {
            callback: format!("{}/auth/callback", document.app_origin),
            auth_origin: document.auth_origin,
            app_origin: document.app_origin,
            issuer: document.issuer,
            audience: document.audience,
            max_access_lifetime: Duration::from_secs(document.max_access_lifetime_seconds),
            routes: document
                .organizations
                .into_iter()
                .map(|r| Route {
                    organization: r.organization_id,
                    tenant_slug: r.tenant_slug,
                    client_id: r.client_id,
                })
                .collect(),
        })
    }
    /// Same-UID private file (0600 in a 0700 directory, no link), as the
    /// registry and the credential document are loaded.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let refused = "auth configuration file refused";
        if !path.is_absolute() {
            return Err(refused);
        }
        let uid = unsafe { libc::geteuid() };
        let parent = path
            .parent()
            .ok_or(refused)?
            .symlink_metadata()
            .map_err(|_| refused)?;
        if !parent.is_dir() || parent.uid() != uid || parent.mode() & 0o077 != 0 {
            return Err(refused);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| refused)?;
        let metadata = file.metadata().map_err(|_| refused)?;
        if !metadata.is_file()
            || metadata.uid() != uid
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
            || metadata.len() > CONFIG_LIMIT
        {
            return Err(refused);
        }
        let mut bytes = Vec::new();
        file.take(CONFIG_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| refused)?;
        if bytes.len() as u64 > CONFIG_LIMIT {
            return Err(refused);
        }
        Self::decode(&bytes)
    }
    pub fn route(&self, organization: &str) -> Option<&Route> {
        self.routes.iter().find(|r| r.organization == organization)
    }
}

/// What a page shows; rendering (forma-server SSR) belongs to the binary.
pub struct SigninView {
    /// `/signin/{handle}` when a well-formed handle is known (from the path
    /// or the routing hint); no lookup decides it, so it discloses nothing.
    pub action: Option<String>,
    /// The uniform neutral failure.
    pub failed: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResendResult {
    Requested,
    Unavailable,
}
pub struct PendingView {
    pub csrf_resend: String,
    pub csrf_signout: String,
    pub resend_available: bool,
    pub cooldown_seconds: u32,
    /// Whole hours (rounded up) left on the bound pending admission; 0 if none.
    pub hours_remaining: u32,
    pub result: Option<ResendResult>,
}
pub struct DeniedView {
    pub csrf_signout: String,
}
pub struct WorkspaceView {
    pub csrf_signout: String,
    pub csrf_signout_all: String,
    /// APP-076 G9: what the member may open now, judged under the session's
    /// fenced, scoped transaction (`Data::openable_works`). Empty (denied,
    /// nothing granted, or a listing the store could not answer): the page
    /// says only that there is nothing to open yet.
    pub items: Vec<WorkspaceItem>,
}
/// One saved work on the workspace page: a plain link to its work page and,
/// when the member may write its records, one link per record collection to
/// the record page of a fresh record id (the S05 `create` at version 0).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceItem {
    pub title: String,
    pub href: String,
    pub records: Vec<RecordLink>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLink {
    pub label: String,
    pub href: String,
}
/// APP-074 invitation entry. The submitted code is never shown back.
#[derive(Default)]
pub struct InviteView {
    /// The one neutral refusal of every unusable code.
    pub failed: bool,
    /// The invitation window that refused the attempt (429 with
    /// `Retry-After`); review m4: the page names it.
    pub limited: Option<InviteLimit>,
    /// A store or provider outage, or the entry's own deadline, before any
    /// code was judged (503): an HTML answer for a form, never raw JSON.
    pub unavailable: bool,
}
/// Which ADR-006 invitation window refused: this network's per-minute or
/// per-day window, or the service-wide daily window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InviteLimit {
    Minute,
    Day,
    Service,
}
impl InviteLimit {
    /// The page's wording key (`InviteProps.limit_window`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minute => "minute",
            Self::Day => "day",
            Self::Service => "service",
        }
    }
}
pub trait Pages: Send + Sync {
    fn invite(&self, view: &InviteView) -> Response;
    fn signin(&self, view: &SigninView) -> Response;
    fn pending(&self, view: &PendingView) -> Response;
    fn denied(&self, view: &DeniedView) -> Response;
    fn workspace(&self, view: &WorkspaceView) -> Response;
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The exchanged token's own shape (the verifier judges the JWT itself).
pub(crate) fn token_shaped(token: &provider::Token) -> bool {
    let user = &token.user;
    token.token_type == "Bearer"
        && capability_shaped(&token.refresh_token)
        && bounded(&token.access_token, genui_auth_verifier::MAX_TOKEN_BYTES)
        && (1..=86_400).contains(&token.expires_in)
        && token.expires_at > now()
        && user.id.starts_with("usr_")
        && bounded(&user.id, 256)
        && user.email.len() <= 320
        && user.name.as_ref().is_none_or(|v| v.len() <= 512)
        && user.avatar.as_ref().is_none_or(|v| v.len() <= 2048)
        && user.roles.len() <= 64
        && user.roles.iter().all(|r| r.len() <= 128)
        && user.auth_method.len() <= 64
}

/// Provider settings in effect (all three or none).
pub(crate) struct Provider {
    pub config: AuthConfig,
    pub client: provider::Client,
    pub verifier: Verifier,
}

pub struct Auth {
    pub(crate) provider: Option<Provider>,
    pub(crate) stores: Option<Arc<Stores>>,
    pub(crate) sealer: capability::Sealer,
    pub(crate) vault: vault::Vault,
    pub(crate) csrf: csrf::CsrfKey,
    pub(crate) limits: Limiter,
    /// D7.14: the load balancer's peers (none: every forwarded header ignored).
    pub(crate) proxies: TrustedProxies,
    /// APP-073's provider invitation capabilities (none: no signup handoff).
    pub(crate) invitations: Option<invite::Invitations>,
    pub(crate) pages: Arc<dyn Pages>,
}

impl Auth {
    fn assemble(
        provider: Option<Provider>,
        stores: Option<Arc<Stores>>,
        pages: Arc<dyn Pages>,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            provider,
            stores,
            sealer: capability::Sealer::new().ok_or("random source unavailable")?,
            vault: vault::Vault::new().ok_or("random source unavailable")?,
            csrf: csrf::CsrfKey::new().ok_or("random source unavailable")?,
            limits: Limiter::default(),
            proxies: TrustedProxies::none(),
            invitations: None,
            pages,
        })
    }
    /// D7.14: believe the rightmost `X-Forwarded-For` hop from these peers only.
    pub fn with_trusted_proxies(mut self, proxies: TrustedProxies) -> Self {
        self.proxies = proxies;
        self
    }
    /// APP-074: the provider invitation capabilities for signup handoffs.
    pub fn with_invitations(mut self, invitations: Option<invite::Invitations>) -> Self {
        self.invitations = invitations;
        self
    }
    /// The limit key of a request's client (D7.14 trusted proxy).
    pub(crate) fn address_class(&self, parts: &Parts) -> String {
        crate::http::proxy::address_class(&parts.headers, routes::peer(parts), &self.proxies)
    }
    /// Production: an `https` auth origin and the reviewed verifier with the
    /// pinned access-only profile and JWKS path (`{auth_origin}/auth/jwks`).
    pub fn new(
        config: Option<AuthConfig>,
        stores: Option<Arc<Stores>>,
        pages: Arc<dyn Pages>,
    ) -> Result<Self, &'static str> {
        let provider = match config {
            None => None,
            Some(config) => {
                let client =
                    provider::Client::new(&config.auth_origin).ok_or("auth origin refused")?;
                let verifier = Verifier::new(verifier_config(&config))
                    .map_err(|_| "verifier configuration refused")?;
                Some(Provider {
                    config,
                    client,
                    verifier,
                })
            }
        };
        Self::assemble(provider, stores, pages)
    }
    /// The cfg(test) synthetic loopback issuer, same verifier code.
    #[cfg(test)]
    pub(crate) fn loopback(
        config: AuthConfig,
        stores: Option<Arc<Stores>>,
        pages: Arc<dyn Pages>,
    ) -> Self {
        let client = provider::Client::loopback(&config.auth_origin).expect("loopback origin");
        let verifier =
            Verifier::new_loopback_for_tests(verifier_config(&config)).expect("loopback verifier");
        Self::assemble(
            Some(Provider {
                config,
                client,
                verifier,
            }),
            stores,
            pages,
        )
        .expect("random source")
    }
    /// The app origin a `POST` must come from (none configured: nothing matches).
    pub(crate) fn origin(&self) -> &str {
        self.provider
            .as_ref()
            .map(|p| p.config.app_origin.as_str())
            .unwrap_or("")
    }
    /// ADR-006: revokes the provider refresh token of a destroyed record
    /// (sign-out, sign-out-all, the expiry sweep). A provider `204` is not the
    /// negative receipt; the token is discarded whatever the answer.
    pub(crate) async fn revoke(&self, ended: &Ended) {
        let Some((slug, refresh)) = self.vault.take(&ended.refresh_ref).await else {
            return;
        };
        let Some(provider) = &self.provider else {
            return;
        };
        let outcome = provider.client.logout(&slug, &refresh).await;
        tracing::info!(outcome = ?outcome, "auth.logout");
    }
    /// The ADR-006 expiry sweep: destroys idle and expired records and
    /// revokes their refresh tokens. Run periodically by the binary.
    pub async fn sweep(&self) {
        if let Some(stores) = &self.stores {
            match stores.workspace.sweep_expired().await {
                Ok(ended) => {
                    for record in &ended {
                        self.revoke(record).await;
                    }
                }
                Err(error) => tracing::warn!(code = %error, "auth.sweep"),
            }
        }
        // APP-009 review m5: no row outlives the 12-hour absolute session, so
        // an older vault entry is referenced by nothing; end it.
        let stale = self.vault.stale(crate::storage::SESSION_ABSOLUTE_SECONDS);
        for reference in &stale {
            self.revoke(&Ended {
                organization: String::new(),
                refresh_ref: reference.clone(),
            })
            .await;
        }
        if !stale.is_empty() {
            tracing::info!(entries = stale.len(), "auth.vault_sweep");
        }
    }
}

fn verifier_config(config: &AuthConfig) -> VerifierConfig {
    VerifierConfig {
        profile: ContractProfile::Forma248AccessOnly,
        issuer: config.issuer.clone(),
        audience: config.audience.clone(),
        jwks_url: format!("{}/auth/jwks", config.auth_origin),
        max_access_lifetime: config.max_access_lifetime,
    }
}
