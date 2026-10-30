//! APP-074: the public invitation entry (ADR-006 `GET /invite` and
//! `POST /invite/validate`; ADR-003 §public entry; ADR-007 D7.1, D7.2, D7.14).
//!
//! A visitor types a GenUI access code into a same-origin form. The server
//! digests it, resolves its organization through `resolve_code` (D7.2), finds
//! the code's server-only provider invitation capability (APP-073's 0600
//! capability file, matched by the code digest it records), reserves the code
//! for this browser's new pre-login flow (D7.1 `reserve_code`: 10 minutes,
//! never counted against remaining redemptions, at most 10 live per code) and
//! performs the backend PAR with the confidential `invite_token`. The browser
//! receives only the `303` to fAuth's authorize URL carrying `client_id` and
//! the bounded `request_uri`, then (at `/auth/callback`) the code and state.
//!
//! The flow record *is* the bounded signup handoff ADR-006 names: the code
//! (through its reservation), the server-bound organization, the provider
//! invitation reference (`signup_handoff_id` = the invitation id) and the
//! 10-minute expiry, single use. No client-supplied organization, tenant,
//! role or destination exists in the form: any field but `code` refuses.
//!
//! Every refusal of a code (malformed, unknown, expired, revoked, exhausted,
//! foreign, without a usable invitation, not reservable, provider refusal or
//! deadline) is the same neutral page with no provider call before the
//! decision; only the limits (429), the body bound (413), cross-site posts
//! (403) and a failure of the first lookup (503, a store outage before any
//! code is judged) differ, and none of those depends on the code.
use super::{cookies, csrf, pkce::Pkce, routes, Auth, InviteView};
use crate::{
    http::{self, limits},
    storage::{code_digest, FlowKind},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use serde::Deserialize;
use std::{
    fmt,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

/// The private directory of APP-073's provider invitation capabilities.
pub const CAPABILITY_DIR_ENV: &str = "GENUI_PROVIDER_CAPABILITY_DIR";
const CAPABILITY_FORMAT: &str = "forma:fauth-invite-capability:v1";
const CAPABILITY_LIMIT: u64 = 4096;
/// Directory entries examined per lookup, and capability files of one
/// organization read per lookup: a bounded scan whatever the directory holds.
const SCAN_ENTRIES: usize = 4096;
const SCAN_FILES: usize = 256;
/// The provider invitation must outlive fAuth's five-minute hosted request.
const INVITATION_MARGIN_SECONDS: u64 = 300;

/// APP-073's capability record (`provider_management.write_capability`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    format: String,
    organization_id: String,
    invitation_id: String,
    remote_invitation_id: String,
    provider_expires_at: String,
    invite_token: String,
    /// Written since APP-074; a record without it is never handed off.
    #[serde(default)]
    code_digest: Option<String>,
}

/// The confidential provider invitation of one code: its GenUI invitation id
/// (the flow's `signup_handoff_id`) and the raw fAuth `invite_token`, which
/// travels only in the PAR body.
pub(crate) struct Capability {
    pub invitation: String,
    pub token: String,
}
impl fmt::Debug for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Capability([redacted])")
    }
}

/// `tinv_` and a lowercase 8-4-4-4-12 hex UUID: fAuth's invitation token and
/// id shape (`provider_management.REMOTE_ID`/`TOKEN`).
fn provider_token_shaped(value: &str) -> bool {
    value.strip_prefix("tinv_").is_some_and(|id| {
        id.len() == 36
            && id.bytes().enumerate().all(|(i, b)| {
                if matches!(i, 8 | 13 | 18 | 23) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
    })
}
fn same(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0u8, |d, (a, b)| d | (a ^ b))
            == 0
}
/// `YYYY-MM-DDTHH:MM:SSZ` (the operator's `timestamp`) as Unix seconds.
fn unix_seconds(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> {
        let part = text.get(from..to)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Days from the civil date (H. Hinnant's algorithm), proleptic Gregorian.
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second).ok()
}

/// The service's read-only view of the operator's capability directory.
pub struct Invitations {
    directory: PathBuf,
}
impl fmt::Debug for Invitations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invitations([private directory])")
    }
}
impl Invitations {
    /// An absolute same-UID 0700 directory, never a link.
    pub fn open(path: &Path) -> Result<Self, &'static str> {
        let refused = "provider capability directory refused";
        if !path.is_absolute() {
            return Err(refused);
        }
        let metadata = path.symlink_metadata().map_err(|_| refused)?;
        let uid = unsafe { libc::geteuid() };
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(refused);
        }
        Ok(Self {
            directory: path.to_owned(),
        })
    }
    /// The deployment setting; absent: no code can obtain a signup handoff.
    pub fn from_env() -> Result<Option<Self>, &'static str> {
        match std::env::var(CAPABILITY_DIR_ENV) {
            Ok(path) => Self::open(Path::new(&path)).map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err("provider capability directory refused"),
        }
    }
    /// The one live capability of this organization recording this code
    /// digest, or `None` (none, several, malformed, unsafe or expiring).
    pub(crate) async fn find(&self, organization: &str, code_digest: &str) -> Option<Capability> {
        let directory = self.directory.clone();
        let organization = organization.to_owned();
        let digest = code_digest.to_owned();
        tokio::task::spawn_blocking(move || scan(&directory, &organization, &digest))
            .await
            .ok()
            .flatten()
    }
}

fn read_private(path: &Path) -> Option<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
        || metadata.len() > CAPABILITY_LIMIT
    {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(CAPABILITY_LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= CAPABILITY_LIMIT).then_some(bytes)
}

fn scan(directory: &Path, organization: &str, digest: &str) -> Option<Capability> {
    let prefix = format!("{organization}.");
    let mut found: Option<Capability> = None;
    let mut read = 0usize;
    for entry in std::fs::read_dir(directory).ok()?.take(SCAN_ENTRIES) {
        let name = entry.ok()?.file_name();
        let Some(invitation) = name
            .to_str()
            .and_then(|n| n.strip_prefix(&prefix))
            .and_then(|n| n.strip_suffix(".capability"))
            .filter(|id| crate::storage::valid_organization(id))
            .map(str::to_owned)
        else {
            continue;
        };
        read += 1;
        if read > SCAN_FILES {
            return None;
        }
        let Some(record) = read_private(&directory.join(&name))
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
        else {
            continue;
        };
        let live = unix_seconds(&record.provider_expires_at)
            .is_some_and(|at| at >= super::now() + INVITATION_MARGIN_SECONDS);
        let matches = record.format == CAPABILITY_FORMAT
            && record.organization_id == organization
            && record.invitation_id == invitation
            && provider_token_shaped(&record.remote_invitation_id)
            && provider_token_shaped(&record.invite_token)
            && record
                .code_digest
                .as_deref()
                .is_some_and(|recorded| same(recorded, digest));
        if matches && live {
            if found.is_some() {
                // Two live invitations for one code: hand off neither.
                return None;
            }
            found = Some(Capability {
                invitation,
                token: record.invite_token,
            });
        }
    }
    found
}

/// Why a code obtained no handoff (safe server-side log field; the browser
/// sees the one neutral page whatever it is).
#[derive(Clone, Copy, Debug)]
enum Refusal {
    Malformed,
    Unconfigured,
    Unavailable,
    Unknown,
    Unrouted,
    NoCapability,
    NotReserved,
    Provider,
}

fn page(auth: &Auth, view: InviteView, status: StatusCode) -> Response {
    routes::with_status(auth.pages.invite(&view), status)
}
fn neutral(auth: &Auth) -> Response {
    page(
        auth,
        InviteView {
            failed: true,
            limited: false,
        },
        StatusCode::BAD_REQUEST,
    )
}

/// ADR-006 `GET /invite`: the entry form. Nothing from the request is read
/// or echoed; a code is never accepted from a URL.
pub(crate) async fn entry(State(auth): State<Arc<Auth>>) -> Response {
    page(
        &auth,
        InviteView {
            failed: false,
            limited: false,
        },
        StatusCode::OK,
    )
}

/// Judge the code and start the signup flow; `Ok` carries the pre-login
/// cookie and the authorize URL.
async fn handoff(auth: &Auth, code: &str, class: &str) -> Result<(String, String), Refusal> {
    let digest = code_digest(code).ok_or(Refusal::Malformed)?;
    let (Some(provider), Some(stores), Some(invitations)) =
        (&auth.provider, &auth.stores, &auth.invitations)
    else {
        return Err(Refusal::Unconfigured);
    };
    let organization = stores
        .workspace
        .resolve_code(&digest)
        .await
        .map_err(|_| Refusal::Unavailable)?
        .ok_or(Refusal::Unknown)?;
    let route = provider
        .config
        .route(&organization)
        .ok_or(Refusal::Unrouted)?;
    let capability = invitations
        .find(&organization, &digest)
        .await
        .ok_or(Refusal::NoCapability)?;
    let pkce = Pkce::fresh().ok_or(Refusal::NotReserved)?;
    let sealed = auth
        .sealer
        .seal(
            &routes::verifier_context(&organization),
            pkce.verifier.as_bytes(),
        )
        .ok_or(Refusal::NotReserved)?;
    let flow = stores
        .workspace
        .begin_flow(
            &organization,
            FlowKind::SignUp {
                code_digest: &digest,
                handoff: Some(&capability.invitation),
            },
            &sealed,
            class,
        )
        .await
        .map_err(|_| Refusal::NotReserved)?
        .ok_or(Refusal::NotReserved)?;
    match provider
        .client
        .par(
            &route.tenant_slug,
            &route.client_id,
            &provider.config.callback,
            &flow.state,
            &pkce.challenge,
            Some(&capability.token),
        )
        .await
    {
        Ok(request_uri) => Ok((
            flow.cookie,
            provider
                .client
                .authorize_url(&route.tenant_slug, &route.client_id, &request_uri),
        )),
        Err(failure) => {
            tracing::info!(failure = ?failure, "invite.par");
            // Single use either way; the reservation lapses in 10 minutes.
            let _ = stores.workspace.discard_flow(&flow.cookie).await;
            Err(Refusal::Provider)
        }
    }
}

/// ADR-006 `POST /invite/validate`. Order: fetch metadata (403), the limits
/// (429), the 4 KiB body (413), then the single `code` field, and only then
/// hashing, database or provider work.
pub(crate) async fn validate(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let class = auth.address_class(&parts);
    if let Err(retry) = auth.limits.admit(&[
        (&format!("invite:{class}"), limits::INVITE_ADDRESS),
        ("invite:global", limits::INVITE_GLOBAL),
    ]) {
        tracing::info!(outcome = "limited", "invite.validate");
        let mut response = page(
            &auth,
            InviteView {
                failed: false,
                limited: true,
            },
            StatusCode::TOO_MANY_REQUESTS,
        );
        response.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&retry.max(1).to_string()).expect("decimal"),
        );
        return response;
    }
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    // Exactly one `code` field: an organization, tenant, role, destination
    // or any other client-supplied field refuses the request.
    let code = match fields.as_slice() {
        [(name, value)] if name == "code" => value.as_str(),
        _ => {
            tracing::info!(outcome = "refused", stage = ?Refusal::Malformed, "invite.validate");
            return neutral(&auth);
        }
    };
    match handoff(&auth, code, &class).await {
        Ok((cookie, location)) => {
            tracing::info!(outcome = "handoff", "invite.validate");
            let mut response = http::see_other(&location);
            routes::add_cookie(&mut response, cookies::set(cookies::PRELOGIN, &cookie));
            response
        }
        // The first lookup failed before any code was judged: honest and
        // independent of the code.
        Err(Refusal::Unavailable) => {
            tracing::info!(outcome = "unavailable", "invite.validate");
            http::unavailable()
        }
        Err(stage) => {
            tracing::info!(outcome = "refused", stage = ?stage, "invite.validate");
            neutral(&auth)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_timestamps_and_token_shapes() {
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_seconds("2026-11-02T13:00:00Z"), Some(1_793_624_400));
        assert_eq!(unix_seconds("2000-02-29T23:59:59Z"), Some(951_868_799));
        for bad in [
            "2026-11-02T13:00:00",
            "2026-11-02 13:00:00Z",
            "2026-13-02T13:00:00Z",
            "2026-11-02T24:00:00Z",
            "2026-11-02T13:00:00.5Z",
            "+026-11-02T13:00:00Z",
        ] {
            assert_eq!(unix_seconds(bad), None, "{bad}");
        }
        assert!(provider_token_shaped(
            "tinv_0b9d3c8e-5f2a-4c1d-9e8f-7a6b5c4d3e2f"
        ));
        assert!(!provider_token_shaped(
            "tinv_0B9D3C8E-5F2A-4C1D-9E8F-7A6B5C4D3E2F"
        ));
        assert!(!provider_token_shaped(
            "0b9d3c8e-5f2a-4c1d-9e8f-7a6b5c4d3e2f"
        ));
        assert!(format!(
            "{:?}",
            Capability {
                invitation: "i".into(),
                token: "tinv_secret".into()
            }
        )
        .contains("redacted"));
    }
}
