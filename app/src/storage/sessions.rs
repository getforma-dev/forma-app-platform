//! APP-009 sessions (ADR-006; ADR-007 D7.1, D7.3, D7.5, D7.9, D7.11).
//!
//! Three organization-leading relations (migration 0006): the single-use
//! pre-login flow, the restricted pre-access session of a verified identity
//! that is not (yet) a member, and the workspace session. Every cookie value
//! is a capability `base64url(organization ‖ 32 random bytes)`: the leading
//! organization only selects the RLS scope, and the row is found by the
//! SHA-256 digest of the whole value, so a forged or foreign prefix finds
//! nothing. Only digests are stored; the PKCE verifier is stored as the
//! caller's ciphertext; the fAuth refresh token never enters a row, which
//! holds only a reference to the caller's process-memory vault (D7.11).
//!
//! `TrustedRequest` is constructed here and nowhere else in production: from
//! the session row (or, APP-100, a live Slack link row:
//! `resolve_slack_link`), the member's *current* `membership_epoch` read
//! under the `lock_member_authority` fence, the principal's registered
//! provider identity and this process's registry identity. A `PreAccess` is a distinct
//! type with no conversion into a `TrustedRequest`.
//!
//! APP-099 (ADR-011 pitch): the second production constructor is
//! [`Workspace::resolve_agent_token`], for an MCP client's bearer agent token
//! (migration 0010). It is the session path's twin: the token is a capability
//! of the same shape behind the `fmat_` prefix, found by its domain-separated
//! digest, and becomes a `TrustedRequest` only under the same member fence,
//! at the member's current epoch (which must still equal the epoch at
//! minting), with a live entitlement, an active provider binding, the
//! principal's registered identity and this process's registry identity.
//!
//! Lock order (D7.9): the member fence always precedes any session-row write;
//! sign-out calls `advance_member_epoch` directly (it takes the member row
//! `FOR UPDATE` itself) and then deletes session rows. The expiry sweep takes
//! no member lock and therefore skips locked rows instead of waiting.
use super::{
    admission::{binding_active, digest_shaped, domain_digest, Admission, VerifiedNonmember},
    authorization::Transaction,
    definitions::{append_as, Chain, Event},
    pool::Workspace,
    require, valid_id, Error, Result, TrustedRequest,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use genui_auth_verifier::VerifiedIdentity;
use ring::rand::{SecureRandom, SystemRandom};
use std::fmt;
use tokio_postgres::Row;

/// ADR-006 lifetimes, mirrored by migration 0006's CHECKs and SQL intervals.
pub const PRELOGIN_SECONDS: u64 = 600;
pub const PRE_ACCESS_IDLE_SECONDS: u64 = 600;
pub const PRE_ACCESS_ABSOLUTE_SECONDS: u64 = 1_800;
pub const SESSION_IDLE_SECONDS: u64 = 3_600;
pub const SESSION_ABSOLUTE_SECONDS: u64 = 43_200;
/// Rows removed per relation and organization by one sweep pass.
const SWEEP_LIMIT: i64 = 100;

const PRELOGIN_DOMAIN: &[u8] = b"forma:prelogin-cookie:v1";
const PRE_ACCESS_DOMAIN: &[u8] = b"forma:pre-access-cookie:v1";
const SESSION_DOMAIN: &[u8] = b"forma:session-cookie:v1";
const STATE_DOMAIN: &[u8] = b"forma:oauth-state:v1";
const CONTEXT_DOMAIN: &[u8] = b"forma:prelogin-context:v1";
const ADMISSION_PAYLOAD_DOMAIN: &[u8] = b"forma:admission-payload:v1";
const AGENT_TOKEN_DOMAIN: &[u8] = b"forma:agent-token:v1";
/// The visible prefix of every agent token (APP-099), so secret scanners and
/// people recognize one; the rest is a session-shaped capability.
pub const AGENT_TOKEN_PREFIX: &str = "fmat_";

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    Ok(bytes)
}
fn uuid_bytes(id: &str) -> Option<[u8; 16]> {
    if !valid_id(id) {
        return None;
    }
    let hex: Vec<u8> = id.bytes().filter(|b| *b != b'-').collect();
    let mut out = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}
fn uuid_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(36);
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
fn canonical_base64url(value: &str, bytes: usize) -> Option<Vec<u8>> {
    if value.len() != (bytes * 8).div_ceil(6)
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    let raw = URL_SAFE_NO_PAD.decode(value).ok()?;
    (raw.len() == bytes && URL_SAFE_NO_PAD.encode(&raw) == value).then_some(raw)
}
/// A fresh cookie capability for an organization (64 characters).
fn capability(organization: &str) -> Result<String> {
    let organization = uuid_bytes(organization).ok_or(Error::Denied)?;
    let secret: [u8; 32] = random()?;
    let mut raw = [0u8; 48];
    raw[..16].copy_from_slice(&organization);
    raw[16..].copy_from_slice(&secret);
    Ok(URL_SAFE_NO_PAD.encode(raw))
}
/// The scope a cookie capability names, or None for anything malformed.
pub fn capability_organization(value: &str) -> Option<String> {
    let raw = canonical_base64url(value, 48)?;
    let organization = uuid_string(&raw[..16]);
    valid_id(&organization).then_some(organization)
}
/// A fresh OAuth `state` (32 random bytes, 43 base64url characters).
fn fresh_state() -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(random::<32>()?))
}
/// True for the exact shape of every 32-byte base64url capability this
/// contract exchanges (our `state`, fAuth's code and refresh token).
pub fn capability_shaped(value: &str) -> bool {
    canonical_base64url(value, 32).is_some()
}
/// A fresh agent token for an organization (APP-099): `fmat_` and a
/// 64-character capability. Shown to its member once; only its digest is
/// stored.
pub(super) fn fresh_agent_token(organization: &str) -> Result<String> {
    Ok(format!("{AGENT_TOKEN_PREFIX}{}", capability(organization)?))
}
/// The stored digest of an agent token.
pub(super) fn agent_token_digest(token: &str) -> String {
    digest(AGENT_TOKEN_DOMAIN, token)
}
/// The scope an agent token names, or None for anything malformed.
pub fn agent_token_organization(token: &str) -> Option<String> {
    capability_organization(token.strip_prefix(AGENT_TOKEN_PREFIX)?)
}
fn digest(domain: &[u8], value: &str) -> String {
    domain_digest(domain, value.as_bytes())
}
fn same(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0u8, |d, (a, b)| d | (a ^ b))
            == 0
}

/// A Slack team or user id as Slack issues them (uppercase letters and
/// digits), mirrored by migration 0009's CHECKs.
pub fn slack_id(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn integer(row: &Row, index: usize) -> Result<i64> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn boolean(row: &Row, index: usize) -> Result<bool> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

/// The two pre-login flow kinds (0006 `kind`). A signup flow is created by
/// APP-074's invitation entry: it reserves the code (D7.1 `reserve_code`)
/// bound to this flow's context digest in the same transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlowKind<'a> {
    SignIn,
    SignUp {
        code_digest: &'a str,
        handoff: Option<&'a str>,
    },
}

/// A started flow: the pre-login cookie value and the `state` to send.
pub struct FlowStart {
    pub cookie: String,
    pub state: String,
}
impl fmt::Debug for FlowStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FlowStart([redacted])")
    }
}

/// A consumed flow record; only storage constructs one, so its organization
/// is the one the flow was started for.
pub struct FlowRecord {
    organization: String,
    reservation: Option<String>,
    sealed_verifier: Vec<u8>,
}
impl FlowRecord {
    pub fn organization(&self) -> &str {
        &self.organization
    }
    pub fn sealed_verifier(&self) -> &[u8] {
        &self.sealed_verifier
    }
    pub fn is_signup(&self) -> bool {
        self.reservation.is_some()
    }
}
impl fmt::Debug for FlowRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FlowRecord([redacted])")
    }
}

/// ADR-006 pre-access session: a verified provider identity that may finish
/// its bound admission, read `/auth/pending`, request a resend or sign out.
/// It is not a member and has no conversion into a `TrustedRequest`.
///
/// ```compile_fail
/// use genui::storage::PreAccess;
/// let forged = PreAccess { organization: String::new(), session: String::new(),
///     issuer: String::new(), provider_tenant: String::new(),
///     provider_subject: String::new(), email_verified: true, pending: None,
///     refresh_ref: String::new(), registry_sha256: String::new() };
/// ```
pub struct PreAccess {
    organization: String,
    session: String,
    issuer: String,
    provider_tenant: String,
    provider_subject: String,
    email_verified: bool,
    pending: Option<(String, i64)>,
    refresh_ref: String,
    registry_sha256: String,
}
impl PreAccess {
    pub fn organization(&self) -> &str {
        &self.organization
    }
    pub fn email_verified(&self) -> bool {
        self.email_verified
    }
    /// The vault reference of this identity's stored fAuth refresh token.
    pub fn refresh_ref(&self) -> &str {
        &self.refresh_ref
    }
    /// Unix seconds at which the bound pending admission lapses, if any.
    pub fn pending_expires_at(&self) -> Option<i64> {
        self.pending.as_ref().map(|(_, at)| *at)
    }
    /// The account key for the per-account resend limits (never displayed).
    pub fn account_key(&self) -> String {
        domain_digest(
            b"forma:resend-account:v1",
            format!(
                "{}\0{}\0{}\0{}",
                self.organization, self.issuer, self.provider_tenant, self.provider_subject
            )
            .as_bytes(),
        )
    }
    fn nonmember(&self) -> VerifiedNonmember {
        VerifiedNonmember {
            issuer: self.issuer.clone(),
            provider_tenant: self.provider_tenant.clone(),
            provider_subject: self.provider_subject.clone(),
            registry_sha256: self.registry_sha256.clone(),
            email_verified: self.email_verified,
        }
    }
}
impl fmt::Debug for PreAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreAccess([redacted])")
    }
}

/// The callback's outcome for the pre-access mint: the new cookie, the
/// record, and for a signup flow the D7.1 `record_pending_admission` result.
pub struct PreAccessStart {
    pub cookie: String,
    pub pre: PreAccess,
    pub signup: Option<super::PendingAdmission>,
}

/// A member whose admission completed (redeemed now, or already current).
pub struct AdmittedMember {
    principal: String,
    entitlement: String,
}
impl fmt::Debug for AdmittedMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmittedMember([redacted])")
    }
}
pub enum Completion {
    Member(AdmittedMember),
    /// No live pending admission and no current membership and entitlement:
    /// the neutral denial page with sign-out only (ADR-006).
    Denied,
}

/// A record that was destroyed; its vault reference must be revoked with the
/// provider (ADR-006: sign-out, sign-out-all and the expiry sweep).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ended {
    pub organization: String,
    pub refresh_ref: String,
}

/// A live agent token (APP-099): the member's trusted request, which tools
/// pass to the same storage operations the canvas routes call, and the
/// token's identity and label for the log and the "via" chip. It grants
/// nothing beyond the member's own request.
pub struct AgentCaller {
    pub request: TrustedRequest,
    pub token_id: String,
    pub label: String,
    /// APP-115 (0013): the scope the member chose at minting; it only ever
    /// narrows what the member may do through this token (`mcp::scope`).
    pub scope: super::governance::AgentScope,
}
impl AgentCaller {
    /// Test executable only: a caller for unit tests that never reach
    /// storage (draft bookkeeping, JSON-RPC shape). Never compiled into a
    /// release build.
    #[cfg(test)]
    pub(crate) fn for_tests(organization: &str, principal: &str, token_id: &str) -> Self {
        Self {
            request: TrustedRequest {
                organization: organization.into(),
                principal: principal.into(),
                epoch: 1,
                issuer: String::new(),
                provider_tenant: String::new(),
                provider_subject: String::new(),
                registry_sha256: String::new(),
            },
            token_id: token_id.into(),
            label: "Claude".into(),
            scope: super::governance::AgentScope::default(),
        }
    }
}
impl fmt::Debug for AgentCaller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentCaller")
            .field("token_id", &self.token_id)
            .finish_non_exhaustive()
    }
}

pub enum Lookup<T> {
    Live(T),
    /// The record existed but no longer admits anything and was deleted.
    Ended(Ended),
    Absent,
}

/// Sign-out of one workspace session: the destroyed record and the epoch the
/// member advanced to (`advance_member_epoch`, reason `session.sign_out`).
pub struct SignedOut {
    pub ended: Ended,
    pub epoch: i64,
}

async fn set_member_context(
    transaction: &Transaction<'_>,
    principal: &str,
    epoch: i64,
) -> Result<()> {
    transaction
        .query(
            "SELECT set_config('genui.principal_id',$1,true),set_config('genui.membership_epoch',$2,true)",
            &[&principal, &epoch.to_string()],
        )
        .await
        .map(|_| ())
}
async fn current_epoch(
    transaction: &Transaction<'_>,
    organization: &str,
    principal: &str,
) -> Result<i64> {
    let rows = transaction.query("SELECT membership_epoch FROM genui.memberships WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid", &[&organization,&principal]).await?;
    integer(rows.first().ok_or(Error::Denied)?, 0)
}
/// ADR-004 fence for a principal read from a session row: install its
/// context at the current epoch and take `lock_member_authority`. Returns
/// (organization active, member active, epoch). One re-read covers an epoch
/// that moved between the read and the lock (a concurrent sign-out).
async fn fence(
    transaction: &Transaction<'_>,
    organization: &str,
    principal: &str,
    mut epoch: i64,
) -> Result<(bool, bool, i64)> {
    for _ in 0..2 {
        set_member_context(transaction, principal, epoch).await?;
        let rows = transaction.query("SELECT organization_active,member_active,membership_epoch FROM genui_private.lock_member_authority($1::text::uuid,$2::text::uuid,$3)", &[&organization,&principal,&epoch]).await?;
        if let Some(row) = rows.first() {
            return Ok((boolean(row, 0)?, boolean(row, 1)?, integer(row, 2)?));
        }
        epoch = current_epoch(transaction, organization, principal).await?;
    }
    Err(Error::Denied)
}
/// D7.1 `advance_member_epoch` in the caller's transaction, with the member's
/// context at its current epoch. The definer takes the member row FOR UPDATE
/// before comparing, so after one refusal the re-read epoch is stable.
async fn advance(
    transaction: &Transaction<'_>,
    organization: &str,
    principal: &str,
    mut epoch: i64,
    reason: &'static str,
) -> Result<i64> {
    for _ in 0..2 {
        set_member_context(transaction, principal, epoch).await?;
        let rows = transaction
            .query(
                "SELECT genui_private.advance_member_epoch($1::text::uuid,$2::text::uuid,$3)",
                &[&organization, &principal, &reason],
            )
            .await?;
        let advanced: Option<i64> = rows
            .first()
            .ok_or(Error::Unavailable)?
            .try_get(0)
            .map_err(|_| Error::Unavailable)?;
        if let Some(advanced) = advanced {
            return Ok(advanced);
        }
        epoch = current_epoch(transaction, organization, principal).await?;
    }
    Err(Error::Denied)
}

impl Workspace {
    /// The provider tenant this organization is bound to for `issuer` in the
    /// closed registry: the verifier's trusted expected tenant (ADR-006).
    pub fn provider_tenant(&self, organization: &str, issuer: &str) -> Option<String> {
        self.inner
            .registry
            .tenant_for(organization, issuer)
            .map(str::to_owned)
    }

    /// ADR-006 `POST /signin/{handle}` (and APP-074's invitation entry): a
    /// fresh single-use flow record for an organization the caller resolved
    /// through `resolve_handle` or `resolve_code`. `Ok(None)` is the neutral
    /// refusal (inactive organization, or the code would not reserve).
    pub async fn begin_flow(
        &self,
        organization: &str,
        kind: FlowKind<'_>,
        sealed_verifier: &[u8],
        address_class: &str,
    ) -> Result<Option<FlowStart>> {
        require(
            valid_id(organization)
                && (1..=256).contains(&sealed_verifier.len())
                && (1..=64).contains(&address_class.len())
                && address_class.bytes().all(|b| b.is_ascii_graphic()),
        )?;
        let (signup_code, handoff) = match kind {
            FlowKind::SignIn => (None, None),
            FlowKind::SignUp {
                code_digest,
                handoff,
            } => {
                require(digest_shaped(code_digest) && handoff.is_none_or(valid_id))?;
                (Some(code_digest), handoff)
            }
        };
        let cookie = capability(organization)?;
        let state = fresh_state()?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            let active = transaction
                .query(
                    "SELECT status='active' FROM genui.organizations WHERE organization_id=$1::text::uuid",
                    &[&organization],
                )
                .await?;
            if !active.first().map(|r| boolean(r, 0)).transpose()?.unwrap_or(false) {
                return Ok(None);
            }
            let mut reservation: Option<String> = None;
            if let Some(code_digest) = signup_code {
                let context = digest(CONTEXT_DOMAIN, &cookie);
                let rows = transaction.query("SELECT reservation_id::text,outcome FROM genui_private.reserve_code($1::text::uuid,$2,$3)", &[&organization,&code_digest,&context]).await?;
                match rows.first() {
                    Some(row) if text(row, 1)? == "reserved" => reservation = optional(row, 0)?,
                    _ => return Ok(None),
                }
                require(reservation.is_some())?;
            }
            let kind = if reservation.is_some() { "signup" } else { "signin" };
            transaction.query("INSERT INTO genui.prelogin_flows(organization_id,flow_id,cookie_digest,state_digest,kind,reservation_id,signup_handoff_id,code_verifier_ciphertext,client_address_class,created_at,expires_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text,$3::text,$4,$5::text::uuid,$6::text::uuid,$7,$8,now(),now()+interval '10 minutes')",
                &[&organization,&digest(PRELOGIN_DOMAIN,&cookie),&digest(STATE_DOMAIN,&state),&kind,&reservation,&handoff,&sealed_verifier,&address_class]).await?;
            Ok(Some(()))
        }
        .await;
        let started = matches!(result, Ok(Some(())));
        transaction.finish(started).await?;
        Ok(result?.map(|()| FlowStart { cookie, state }))
    }

    /// Discards a flow whose provider hand-off failed (single use either way).
    pub async fn discard_flow(&self, cookie: &str) -> Result<()> {
        let Some(organization) = capability_organization(cookie) else {
            return Ok(());
        };
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = transaction
            .query(
                "DELETE FROM genui.prelogin_flows WHERE organization_id=$1::text::uuid AND cookie_digest=$2",
                &[&organization, &digest(PRELOGIN_DOMAIN, cookie)],
            )
            .await
            .map(|_| ());
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// ADR-006 callback, first step: consume the flow named by the pre-login
    /// cookie. The record is deleted whatever happens next (single use), so a
    /// replayed or mismatched callback can never mint anything.
    pub async fn complete_flow(&self, cookie: &str, state: &str) -> Result<FlowRecord> {
        let organization = capability_organization(cookie).ok_or(Error::Denied)?;
        require(capability_shaped(state))?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = transaction.query("DELETE FROM genui.prelogin_flows WHERE organization_id=$1::text::uuid AND cookie_digest=$2 RETURNING state_digest::text,reservation_id::text,code_verifier_ciphertext,expires_at>clock_timestamp()", &[&organization,&digest(PRELOGIN_DOMAIN,cookie)]).await;
        // Commit the consumption even when the flow is then refused.
        transaction.finish(result.is_ok()).await?;
        let rows = result?;
        let row = rows.first().ok_or(Error::Denied)?;
        require(boolean(row, 3)? && same(&text(row, 0)?, &digest(STATE_DOMAIN, state)))?;
        Ok(FlowRecord {
            organization,
            reservation: optional(row, 1)?,
            sealed_verifier: row.try_get(2).map_err(|_| Error::Unavailable)?,
        })
    }

    /// ADR-006 callback after the exchange and verification: the identity
    /// must map through the closed registry to the flow's organization. A
    /// signup flow records its bounded pending admission (D7.1) in the same
    /// transaction. `refresh_ref` names the caller's vault entry.
    pub async fn mint_pre_access(
        &self,
        flow: &FlowRecord,
        identity: &VerifiedIdentity,
        email_verified: bool,
        refresh_ref: &str,
    ) -> Result<PreAccessStart> {
        let organization = flow.organization.as_str();
        let registry = &self.inner.registry;
        require(
            registry.agrees(&identity.issuer, &identity.tenant_id, organization)
                && (1..=256).contains(&identity.subject.len())
                && valid_id(refresh_ref),
        )?;
        let cookie = capability(organization)?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let nonmember = VerifiedNonmember {
            issuer: identity.issuer.clone(),
            provider_tenant: identity.tenant_id.clone(),
            provider_subject: identity.subject.clone(),
            registry_sha256: registry.identity.sha256.clone(),
            email_verified,
        };
        let result = async {
            binding_active(&transaction, &nonmember, organization).await?;
            let mut signup = None;
            let mut pending: Option<(String, i64)> = None;
            if let Some(reservation) = &flow.reservation {
                let rows = transaction.query("SELECT pending_admission_id::text,floor(extract(epoch FROM expires_at))::bigint,outcome FROM genui_private.record_pending_admission($1::text::uuid,$2::text::uuid,$3,$4,$5)", &[&organization,&reservation,&nonmember.issuer,&nonmember.provider_tenant,&nonmember.provider_subject]).await?;
                let row = rows.first().ok_or(Error::Denied)?;
                let id = optional(row, 0)?;
                let at: Option<i64> = row.try_get(1).map_err(|_| Error::Unavailable)?;
                signup = Some(match (text(row, 2)?.as_str(), id, at) {
                    ("pending", Some(id), Some(at)) => {
                        pending = Some((id.clone(), at));
                        super::PendingAdmission::Pending { pending_admission: id, expires_at: at }
                    }
                    ("already_member", ..) => super::PendingAdmission::AlreadyMember,
                    ("reservation_expired", ..) => super::PendingAdmission::ReservationExpired,
                    ("expired", ..) => super::PendingAdmission::Expired,
                    ("revoked", ..) => super::PendingAdmission::Revoked,
                    ("exhausted", ..) => super::PendingAdmission::Exhausted,
                    _ => return Err(Error::Unavailable),
                });
            }
            let kind = if flow.reservation.is_some() { "signup" } else { "signin" };
            let pending_id = pending.as_ref().map(|(id, _)| id.clone());
            let pending_at = pending.as_ref().map(|(_, at)| *at);
            let rows = transaction.query("INSERT INTO genui.pre_access_sessions(organization_id,session_id,cookie_digest,issuer,provider_tenant_id,provider_subject,email_verified,flow_kind,pending_admission_id,pending_expires_at,refresh_token_ref,created_at,last_seen_at,expires_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text,$3,$4,$5,$6,$7,$8::text::uuid,to_timestamp($9::bigint),$10::text::uuid,now(),now(),now()+interval '30 minutes') RETURNING session_id::text",
                &[&organization,&digest(PRE_ACCESS_DOMAIN,&cookie),&nonmember.issuer,&nonmember.provider_tenant,&nonmember.provider_subject,&email_verified,&kind,&pending_id,&pending_at,&refresh_ref]).await?;
            let session = text(rows.first().ok_or(Error::Unavailable)?, 0)?;
            Ok((session, signup, pending))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        let (session, signup, pending) = result?;
        Ok(PreAccessStart {
            cookie,
            pre: PreAccess {
                organization: organization.to_owned(),
                session,
                issuer: nonmember.issuer,
                provider_tenant: nonmember.provider_tenant,
                provider_subject: nonmember.provider_subject,
                email_verified,
                pending,
                refresh_ref: refresh_ref.to_owned(),
                registry_sha256: nonmember.registry_sha256,
            },
            signup,
        })
    }

    /// The live pre-access session a cookie names. An idle, expired or no
    /// longer registry-bound record is deleted and reported `Ended`.
    pub async fn resolve_pre_access(&self, cookie: &str) -> Result<Lookup<PreAccess>> {
        let Some(organization) = capability_organization(cookie) else {
            return Ok(Lookup::Absent);
        };
        let registry = &self.inner.registry;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT session_id::text,issuer,provider_tenant_id,provider_subject,email_verified,pending_admission_id::text,floor(extract(epoch FROM pending_expires_at))::bigint,refresh_token_ref::text,(revoked_at IS NULL AND expires_at>now() AND last_seen_at>now()-interval '10 minutes'),last_seen_at<=now()-interval '60 seconds' FROM genui.pre_access_sessions WHERE organization_id=$1::text::uuid AND cookie_digest=$2", &[&organization,&digest(PRE_ACCESS_DOMAIN,cookie)]).await?;
            let Some(row) = rows.first() else {
                return Ok(Lookup::Absent);
            };
            let pre = PreAccess {
                organization: organization.clone(),
                session: text(row, 0)?,
                issuer: text(row, 1)?,
                provider_tenant: text(row, 2)?,
                provider_subject: text(row, 3)?,
                email_verified: boolean(row, 4)?,
                pending: match (optional(row, 5)?, row.try_get::<_, Option<i64>>(6).map_err(|_| Error::Unavailable)?) {
                    (Some(id), Some(at)) => Some((id, at)),
                    _ => None,
                },
                refresh_ref: text(row, 7)?,
                registry_sha256: registry.identity.sha256.clone(),
            };
            if !boolean(row, 8)? || !registry.agrees(&pre.issuer, &pre.provider_tenant, &organization) {
                transaction.query("DELETE FROM genui.pre_access_sessions WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid", &[&organization,&pre.session]).await?;
                return Ok(Lookup::Ended(Ended { organization: organization.clone(), refresh_ref: pre.refresh_ref }));
            }
            if boolean(row, 9)? {
                transaction.query("UPDATE genui.pre_access_sessions SET last_seen_at=now() WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid", &[&organization,&pre.session]).await?;
            }
            Ok(Lookup::Live(pre))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Destroys the pre-access record a cookie names (sign-out, an
    /// `already_verified` resend, a provider 401/404, a failed refresh).
    pub async fn destroy_pre_access(&self, cookie: &str) -> Result<Option<Ended>> {
        let Some(organization) = capability_organization(cookie) else {
            return Ok(None);
        };
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = transaction.query("DELETE FROM genui.pre_access_sessions WHERE organization_id=$1::text::uuid AND cookie_digest=$2 RETURNING refresh_token_ref::text", &[&organization,&digest(PRE_ACCESS_DOMAIN,cookie)]).await;
        transaction.finish(result.is_ok()).await?;
        let rows = result?;
        rows.first()
            .map(|row| {
                Ok(Ended {
                    organization: organization.clone(),
                    refresh_ref: text(row, 0)?,
                })
            })
            .transpose()
    }

    /// ADR-006 callback when the exchange reported `email_verified` true: a
    /// live pending admission of this identity is redeemed through APP-073's
    /// atomic `admit_redemption`; otherwise a current membership with an
    /// active entitlement is used as is. Anything else is `Denied`.
    pub async fn complete_admission(&self, pre: &PreAccess) -> Result<Completion> {
        if !pre.email_verified
            || !self
                .inner
                .registry
                .agrees(&pre.issuer, &pre.provider_tenant, &pre.organization)
        {
            return Ok(Completion::Denied);
        }
        let organization = pre.organization.as_str();
        let pending = match &pre.pending {
            Some((id, _)) => Some(id.clone()),
            None => {
                let transaction =
                    Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
                let result = transaction.query("SELECT reservation_id::text FROM genui.code_reservations WHERE organization_id=$1::text::uuid AND kind='pending' AND state='live' AND issuer=$2 AND provider_tenant_id=$3 AND provider_subject=$4 ORDER BY (expires_at>clock_timestamp()) DESC,expires_at DESC LIMIT 1", &[&organization,&pre.issuer,&pre.provider_tenant,&pre.provider_subject]).await;
                transaction.finish(result.is_ok()).await?;
                result?.first().map(|row| text(row, 0)).transpose()?
            }
        };
        if let Some(pending) = pending {
            let payload = domain_digest(
                ADMISSION_PAYLOAD_DOMAIN,
                format!(
                    "{pending}\0{}\0{}\0{}",
                    pre.issuer, pre.provider_tenant, pre.provider_subject
                )
                .as_bytes(),
            );
            if let Admission::Admitted {
                principal,
                entitlement,
                ..
            } = self
                .admit(&pre.nonmember(), organization, &pending, &payload)
                .await?
            {
                return Ok(Completion::Member(AdmittedMember {
                    principal,
                    entitlement,
                }));
            }
        }
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = transaction.query("SELECT p.principal_id::text,e.entitlement_id::text FROM genui.principals p JOIN genui.memberships m ON m.organization_id=p.organization_id AND m.principal_id=p.principal_id JOIN genui.organizations o ON o.organization_id=p.organization_id JOIN genui.entitlements e ON e.organization_id=p.organization_id AND e.principal_id=p.principal_id WHERE p.organization_id=$1::text::uuid AND p.issuer=$2 AND p.provider_tenant_id=$3 AND p.provider_subject=$4 AND m.status='active' AND o.status='active' AND e.status='active' AND e.expires_at>clock_timestamp() ORDER BY e.expires_at DESC,e.entitlement_id LIMIT 1", &[&organization,&pre.issuer,&pre.provider_tenant,&pre.provider_subject]).await;
        transaction.finish(result.is_ok()).await?;
        Ok(match result?.first() {
            Some(row) => Completion::Member(AdmittedMember {
                principal: text(row, 0)?,
                entitlement: text(row, 1)?,
            }),
            None => Completion::Denied,
        })
    }

    /// Mints the workspace session once: under the member fence, rechecks the
    /// membership, the entitlement and the identity, consumes the pre-access
    /// record (so a second completion finds nothing), transfers its refresh
    /// reference and appends `session.signed_in` (D7.8 workspace chain).
    pub async fn mint_session(&self, pre: &PreAccess, member: &AdmittedMember) -> Result<String> {
        let organization = pre.organization.as_str();
        let registry = &self.inner.registry;
        require(
            pre.email_verified
                && registry.agrees(&pre.issuer, &pre.provider_tenant, organization)
                && valid_id(&member.principal)
                && valid_id(&member.entitlement),
        )?;
        let cookie = capability(organization)?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT m.membership_epoch,p.issuer,p.provider_tenant_id,p.provider_subject FROM genui.memberships m JOIN genui.principals p ON p.organization_id=m.organization_id AND p.principal_id=m.principal_id WHERE m.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid", &[&organization,&member.principal]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            require(text(row, 1)? == pre.issuer && text(row, 2)? == pre.provider_tenant && text(row, 3)? == pre.provider_subject)?;
            let (organization_active, member_active, epoch) =
                fence(&transaction, organization, &member.principal, integer(row, 0)?).await?;
            require(organization_active && member_active)?;
            #[cfg(test)]
            transaction.lease.checkpoint("mint").await;
            let entitled = transaction.query("SELECT 1 FROM genui.entitlements WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND entitlement_id=$3::text::uuid AND status='active' AND expires_at>clock_timestamp()", &[&organization,&member.principal,&member.entitlement]).await?;
            require(entitled.len() == 1)?;
            binding_active(&transaction, &pre.nonmember(), organization).await?;
            let consumed = transaction.query("DELETE FROM genui.pre_access_sessions WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid RETURNING refresh_token_ref::text", &[&organization,&pre.session]).await?;
            require(consumed.len() == 1 && text(&consumed[0], 0)? == pre.refresh_ref)?;
            transaction.query("INSERT INTO genui.sessions(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,refresh_token_ref,created_at,last_seen_at,absolute_expires_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text,$3::text::uuid,$4::bigint,$5::text::uuid,$6::text,$7::text::uuid,now(),now(),now()+interval '12 hours')",
                &[&organization,&digest(SESSION_DOMAIN,&cookie),&member.principal,&epoch,&member.entitlement,&registry.identity.sha256,&pre.refresh_ref]).await?;
            append_as(&transaction, Chain::Workspace, organization, &member.principal, Event { kind: "session.signed_in", id: None, app: None, work: None, record: None, outcome: None }).await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result.map(|()| cookie)
    }

    /// Test executable only (APP-013 command campaign): a workspace session
    /// for a seeded active member with an active entitlement, bypassing the
    /// provider exchange. Everything the route then does (`resolve_session`,
    /// the fence, the data role's command) is the production path. Never
    /// compiled into a release build.
    #[cfg(test)]
    pub(crate) async fn mint_session_for_tests(
        &self,
        organization: &str,
        principal: &str,
        refresh_ref: &str,
    ) -> Result<String> {
        require(valid_id(organization) && valid_id(principal) && valid_id(refresh_ref))?;
        let registry = &self.inner.registry;
        let cookie = capability(organization)?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT m.membership_epoch,e.entitlement_id::text FROM genui.memberships m JOIN genui.entitlements e ON e.organization_id=m.organization_id AND e.principal_id=m.principal_id AND e.status='active' AND e.expires_at>clock_timestamp() WHERE m.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid AND m.status='active' LIMIT 1", &[&organization,&principal]).await?;
            let row = rows.first().ok_or(Error::Denied)?;
            let (epoch, entitlement) = (integer(row, 0)?, text(row, 1)?);
            transaction.query("INSERT INTO genui.sessions(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,refresh_token_ref,created_at,last_seen_at,absolute_expires_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text,$3::text::uuid,$4::bigint,$5::text::uuid,$6::text,$7::text::uuid,now(),now(),now()+interval '12 hours')",
                &[&organization,&digest(SESSION_DOMAIN,&cookie),&principal,&epoch,&entitlement,&registry.identity.sha256,&refresh_ref]).await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result.map(|()| cookie)
    }

    /// Resolves a workspace session cookie into a `TrustedRequest` at the
    /// member's current epoch, under the member fence. Idle (1 hour), absolute
    /// (12 hours), revoked membership, inactive organization, expired or
    /// revoked entitlement, an inactive provider binding, another registry
    /// identity, or a vault that no longer holds the refresh reference (a
    /// restart: D7.11, D7.18) each delete the record and report `Ended`.
    ///
    /// ADR-007 D7.9 (Phase 1): a lock wait past `lock_timeout` (or a lost
    /// deadlock) during session resolution itself, such as an operator
    /// membership change holding this member's row `FOR UPDATE` while the
    /// fence takes it `FOR SHARE`, is `Error::Denied`, the uniform 404 like
    /// any other session-lookup failure (it fails closed; a typed busy
    /// answer on session routes is APP-086). Only a genuine store outage is
    /// `Error::Unavailable` (503). APP-012 review M4.
    pub async fn resolve_session(
        &self,
        cookie: &str,
        refresh_live: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<Lookup<TrustedRequest>> {
        let Some(organization) = capability_organization(cookie) else {
            return Ok(Lookup::Absent);
        };
        let registry = &self.inner.registry;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT s.session_id::text,s.principal_id::text,s.registry_sha256,s.refresh_token_ref::text,(s.revoked_at IS NULL AND s.absolute_expires_at>now() AND s.last_seen_at>now()-interval '1 hour'),s.last_seen_at<=now()-interval '60 seconds',p.issuer,p.provider_tenant_id,p.provider_subject,m.membership_epoch,EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=s.organization_id AND e.principal_id=s.principal_id AND e.entitlement_id=s.entitlement_id AND e.status='active' AND e.expires_at>clock_timestamp()),EXISTS(SELECT 1 FROM genui.provider_tenant_bindings b WHERE b.organization_id=p.organization_id AND b.issuer=p.issuer AND b.provider_tenant_id=p.provider_tenant_id AND b.active) FROM genui.sessions s JOIN genui.principals p ON p.organization_id=s.organization_id AND p.principal_id=s.principal_id JOIN genui.memberships m ON m.organization_id=s.organization_id AND m.principal_id=s.principal_id WHERE s.organization_id=$1::text::uuid AND s.cookie_digest=$2", &[&organization,&digest(SESSION_DOMAIN,cookie)]).await?;
            let Some(row) = rows.first() else {
                return Ok(Lookup::Absent);
            };
            let session = text(row, 0)?;
            let principal = text(row, 1)?;
            let refresh_ref = text(row, 3)?;
            let issuer = text(row, 6)?;
            let provider_tenant = text(row, 7)?;
            let (organization_active, member_active, epoch) =
                fence(&transaction, &organization, &principal, integer(row, 9)?).await?;
            let valid = boolean(row, 4)?
                && organization_active
                && member_active
                && boolean(row, 10)?
                && boolean(row, 11)?
                && text(row, 2)? == registry.identity.sha256
                && registry.agrees(&issuer, &provider_tenant, &organization)
                && refresh_live(&refresh_ref);
            if !valid {
                transaction.query("DELETE FROM genui.sessions WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid", &[&organization,&session]).await?;
                return Ok(Lookup::Ended(Ended { organization: organization.clone(), refresh_ref }));
            }
            if boolean(row, 5)? {
                transaction.query("UPDATE genui.sessions SET last_seen_at=now() WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid", &[&organization,&session]).await?;
            }
            Ok(Lookup::Live(TrustedRequest {
                organization: organization.clone(),
                principal,
                epoch,
                issuer,
                provider_tenant,
                provider_subject: text(row, 8)?,
                registry_sha256: registry.identity.sha256.clone(),
            }))
        }
        .await;
        // D7.9: a lock wait on this cookie's own member row is a session-lookup
        // failure (uniform 404), not an outage; the aborted transaction is
        // rolled back either way.
        let lock_wait = result.is_err() && transaction.lock_timed_out();
        let finished = transaction.finish(result.is_ok()).await;
        if lock_wait {
            return Err(Error::Denied);
        }
        finished?;
        result
    }

    /// APP-100 (ADR-011 pitch): the second `TrustedRequest` constructor, for
    /// a Slack request from workspace `team` by Slack user `user` (both
    /// already checked against Slack's request signature by the caller). Each
    /// registry organization is scanned under its own scope (RLS) for an
    /// unrevoked `genui.slack_links` row; a row admits only when, under the
    /// member fence, the organization and membership are active, the
    /// member's **current** epoch equals the epoch recorded at link time (a
    /// sign-out or an operator revocation advanced it: the link has ended),
    /// an entitlement is live, the principal's provider binding is active and
    /// the registry agrees, exactly the session path's judgment. `None` for
    /// no row, a stale or revoked row, or more than one organization
    /// answering (an ambiguous Slack user is never guessed at). Nothing is
    /// written. A lock wait is `Denied` as in `resolve_session`.
    pub async fn resolve_slack_link(
        &self,
        team: &str,
        user: &str,
    ) -> Result<Option<TrustedRequest>> {
        if !(slack_id(team) && slack_id(user)) {
            return Ok(None);
        }
        let registry = &self.inner.registry;
        let mut found = Vec::new();
        for organization in registry.organizations() {
            let transaction =
                Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
            let result = async {
                let rows = transaction.query("SELECT l.principal_id::text,l.membership_epoch_at_link,p.issuer,p.provider_tenant_id,p.provider_subject,m.membership_epoch,EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=l.organization_id AND e.principal_id=l.principal_id AND e.status='active' AND e.expires_at>clock_timestamp()),EXISTS(SELECT 1 FROM genui.provider_tenant_bindings b WHERE b.organization_id=p.organization_id AND b.issuer=p.issuer AND b.provider_tenant_id=p.provider_tenant_id AND b.active) FROM genui.slack_links l JOIN genui.principals p ON p.organization_id=l.organization_id AND p.principal_id=l.principal_id JOIN genui.memberships m ON m.organization_id=l.organization_id AND m.principal_id=l.principal_id WHERE l.organization_id=$1::text::uuid AND l.team_id=$2 AND l.slack_user_id=$3 AND l.revoked_at IS NULL", &[&organization,&team,&user]).await?;
                let Some(row) = rows.first() else {
                    return Ok(None);
                };
                let principal = text(row, 0)?;
                let issuer = text(row, 2)?;
                let provider_tenant = text(row, 3)?;
                let (organization_active, member_active, epoch) =
                    fence(&transaction, &organization, &principal, integer(row, 5)?).await?;
                let valid = organization_active
                    && member_active
                    && epoch == integer(row, 1)?
                    && boolean(row, 6)?
                    && boolean(row, 7)?
                    && registry.agrees(&issuer, &provider_tenant, &organization);
                if !valid {
                    return Ok(None);
                }
                Ok(Some(TrustedRequest {
                    organization: organization.clone(),
                    principal,
                    epoch,
                    issuer,
                    provider_tenant,
                    provider_subject: text(row, 4)?,
                    registry_sha256: registry.identity.sha256.clone(),
                }))
            }
            .await;
            let lock_wait = result.is_err() && transaction.lock_timed_out();
            let finished = transaction.finish(result.is_ok()).await;
            if lock_wait {
                return Err(Error::Denied);
            }
            finished?;
            if let Some(request) = result? {
                found.push(request);
            }
        }
        Ok(if found.len() == 1 { found.pop() } else { None })
    }

    /// APP-100: the unrevoked Slack links of `organization`, as
    /// `(team, slack user, principal)`, for the approver notifier. A listing
    /// confers nothing: every use resolves the link again through
    /// [`Workspace::resolve_slack_link`].
    pub async fn slack_links(&self, organization: &str) -> Result<Vec<(String, String, String)>> {
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT team_id,slack_user_id,principal_id::text FROM genui.slack_links WHERE organization_id=$1::text::uuid AND revoked_at IS NULL ORDER BY linked_at LIMIT 256", &[&organization]).await?;
            rows.iter()
                .map(|row| Ok((text(row, 0)?, text(row, 1)?, text(row, 2)?)))
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// APP-100: links Slack user `user` of workspace `team` to the member
    /// behind `request` (a live workspace session, judged by the caller's
    /// session resolution), in the member's own organization, at the member's
    /// current epoch (re-read under the fence). A re-link of the same Slack
    /// user in this organization replaces the earlier member and clears a
    /// revocation. `Denied` for a member that is no longer current.
    pub async fn link_slack(&self, request: &TrustedRequest, team: &str, user: &str) -> Result<()> {
        require(slack_id(team) && slack_id(user))?;
        let transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let (organization_active, member_active, epoch) =
                fence(&transaction, &request.organization, &request.principal, request.epoch).await?;
            require(organization_active && member_active && epoch == request.epoch)?;
            transaction.query("INSERT INTO genui.slack_links(organization_id,link_id,principal_id,team_id,slack_user_id,membership_epoch_at_link,linked_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text::uuid,$3,$4,$5::bigint,now()) ON CONFLICT (organization_id,team_id,slack_user_id) DO UPDATE SET principal_id=EXCLUDED.principal_id,membership_epoch_at_link=EXCLUDED.membership_epoch_at_link,linked_at=EXCLUDED.linked_at,revoked_at=NULL", &[&request.organization,&request.principal,&team,&user,&epoch]).await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// APP-100 `/forma unlink`: revokes the link of `team`/`user` held by the
    /// member behind `request` (resolved from that same link). Revoking only
    /// takes authority away. `true` when a live link was revoked.
    pub async fn unlink_slack(
        &self,
        request: &TrustedRequest,
        team: &str,
        user: &str,
    ) -> Result<bool> {
        require(slack_id(team) && slack_id(user))?;
        let transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let rows = transaction.query("UPDATE genui.slack_links SET revoked_at=now() WHERE organization_id=$1::text::uuid AND team_id=$2 AND slack_user_id=$3 AND principal_id=$4::text::uuid AND revoked_at IS NULL RETURNING link_id::text", &[&request.organization,&team,&user,&request.principal]).await?;
            Ok(!rows.is_empty())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Resolves an MCP client's bearer agent token (APP-099) into the
    /// member's `TrustedRequest`, exactly as [`Workspace::resolve_session`]
    /// resolves a cookie: the row by the digest under the token's scope, the
    /// member fence at the current epoch, which must equal the epoch at
    /// minting (a sign-out, sign-out-all or any operator epoch change ends
    /// every token of the member), the token neither revoked nor past its
    /// `expires_at` (0012: 30 days from minting), an active
    /// organization and membership, a live entitlement, an active provider
    /// binding, the principal's registered identity agreeing with this
    /// process's registry and the registry identity at minting. `None` for
    /// anything else (malformed, unknown, foreign, revoked, stale): the
    /// caller answers the uniform 404 for all of them alike. A refused token
    /// row is kept (its attributions keep their label); only `last_used_at`
    /// is written, at most once a minute. A lock wait on the member's own row
    /// is `Denied`, as for a session.
    pub async fn resolve_agent_token(&self, token: &str) -> Result<Option<AgentCaller>> {
        let Some(organization) = agent_token_organization(token) else {
            return Ok(None);
        };
        let registry = &self.inner.registry;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = async {
            let rows = transaction.query(&format!("SELECT t.token_id::text,t.principal_id::text,t.label,t.registry_sha256,t.membership_epoch_at_mint,(t.revoked_at IS NULL AND t.expires_at>clock_timestamp()),(t.last_used_at IS NULL OR t.last_used_at<=now()-interval '60 seconds'),p.issuer,p.provider_tenant_id,p.provider_subject,m.membership_epoch,EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=t.organization_id AND e.principal_id=t.principal_id AND e.status='active' AND e.expires_at>clock_timestamp()),EXISTS(SELECT 1 FROM genui.provider_tenant_bindings b WHERE b.organization_id=p.organization_id AND b.issuer=p.issuer AND b.provider_tenant_id=p.provider_tenant_id AND b.active),{} FROM genui.agent_tokens t JOIN genui.principals p ON p.organization_id=t.organization_id AND p.principal_id=t.principal_id JOIN genui.memberships m ON m.organization_id=t.organization_id AND m.principal_id=t.principal_id WHERE t.organization_id=$1::text::uuid AND t.token_digest=$2", super::governance::SCOPE_COLUMNS), &[&organization,&agent_token_digest(token)]).await?;
            let Some(row) = rows.first() else {
                return Ok(None);
            };
            let token_id = text(row, 0)?;
            let principal = text(row, 1)?;
            let issuer = text(row, 7)?;
            let provider_tenant = text(row, 8)?;
            let (organization_active, member_active, epoch) =
                fence(&transaction, &organization, &principal, integer(row, 10)?).await?;
            let valid = boolean(row, 5)?
                && organization_active
                && member_active
                && epoch == integer(row, 4)?
                && boolean(row, 11)?
                && boolean(row, 12)?
                && text(row, 3)? == registry.identity.sha256
                && registry.agrees(&issuer, &provider_tenant, &organization);
            if !valid {
                return Ok(None);
            }
            if boolean(row, 6)? {
                transaction.query("UPDATE genui.agent_tokens SET last_used_at=now() WHERE organization_id=$1::text::uuid AND token_id=$2::text::uuid", &[&organization,&token_id]).await?;
            }
            Ok(Some(AgentCaller {
                request: TrustedRequest {
                    organization: organization.clone(),
                    principal,
                    epoch,
                    issuer,
                    provider_tenant,
                    provider_subject: text(row, 9)?,
                    registry_sha256: registry.identity.sha256.clone(),
                },
                token_id,
                label: text(row, 2)?,
                scope: super::governance::AgentScope::from_row(row, 13)?,
            }))
        }
        .await;
        let lock_wait = result.is_err() && transaction.lock_timed_out();
        let finished = transaction.finish(result.is_ok()).await;
        if lock_wait {
            return Err(Error::Denied);
        }
        finished?;
        result
    }

    /// ADR-006 `POST /auth/signout` for a workspace session: advances the
    /// member's epoch (D7.1, reason `session.sign_out`; D7.9: directly, never
    /// after `lock_member_authority`) and then deletes the record, in one
    /// transaction. Needs no current entitlement. `None` when there is no
    /// record (sign-out is idempotent).
    pub async fn sign_out(&self, cookie: &str) -> Result<Option<SignedOut>> {
        let Some(organization) = capability_organization(cookie) else {
            return Ok(None);
        };
        let transaction = Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT s.session_id::text,s.principal_id::text,s.refresh_token_ref::text,m.membership_epoch FROM genui.sessions s JOIN genui.memberships m ON m.organization_id=s.organization_id AND m.principal_id=s.principal_id WHERE s.organization_id=$1::text::uuid AND s.cookie_digest=$2", &[&organization,&digest(SESSION_DOMAIN,cookie)]).await?;
            let Some(row) = rows.first() else {
                return Ok(None);
            };
            let principal = text(row, 1)?;
            let epoch = advance(&transaction, &organization, &principal, integer(row, 3)?, "session.sign_out").await?;
            #[cfg(test)]
            transaction.lease.checkpoint("sign_out_session").await;
            transaction.query("DELETE FROM genui.sessions WHERE organization_id=$1::text::uuid AND session_id=$2::text::uuid", &[&organization,&text(row, 0)?]).await?;
            Ok(Some(SignedOut { ended: Ended { organization: organization.clone(), refresh_ref: text(row, 2)? }, epoch }))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// ADR-006 `POST /auth/signout-all`: advances the epoch once (reason
    /// `session.sign_out_all`) and deletes every workspace session of this
    /// member in this organization, in one transaction.
    pub async fn sign_out_all(&self, request: &TrustedRequest) -> Result<(i64, Vec<Ended>)> {
        let transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let epoch = advance(&transaction, &request.organization, &request.principal, request.epoch, "session.sign_out_all").await?;
            let rows = transaction.query("DELETE FROM genui.sessions WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid RETURNING refresh_token_ref::text", &[&request.organization,&request.principal]).await?;
            let ended = rows
                .iter()
                .map(|row| Ok(Ended { organization: request.organization.clone(), refresh_ref: text(row, 0)? }))
                .collect::<Result<Vec<_>>>()?;
            Ok((epoch, ended))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// ADR-006 expiry sweep, per registry organization: deletes expired flows
    /// and idle, absolute-expired or revoked pre-access and workspace records
    /// (at most `SWEEP_LIMIT` each), skipping rows another transaction holds.
    /// Returns the destroyed records whose refresh tokens must be revoked.
    pub async fn sweep_expired(&self) -> Result<Vec<Ended>> {
        let mut ended = Vec::new();
        for organization in self.inner.registry.organizations() {
            let transaction =
                Transaction::begin_nonmember(&self.inner, Some(&organization)).await?;
            let result = async {
                transaction.query("DELETE FROM genui.prelogin_flows WHERE organization_id=$1::text::uuid AND expires_at<=now()", &[&organization]).await?;
                let pre = transaction.query("DELETE FROM genui.pre_access_sessions WHERE (organization_id,session_id) IN (SELECT organization_id,session_id FROM genui.pre_access_sessions WHERE organization_id=$1::text::uuid AND (revoked_at IS NOT NULL OR expires_at<=now() OR last_seen_at<=now()-interval '10 minutes') LIMIT $2 FOR UPDATE SKIP LOCKED) RETURNING refresh_token_ref::text", &[&organization,&SWEEP_LIMIT]).await?;
                let sessions = transaction.query("DELETE FROM genui.sessions WHERE (organization_id,session_id) IN (SELECT organization_id,session_id FROM genui.sessions WHERE organization_id=$1::text::uuid AND (revoked_at IS NOT NULL OR absolute_expires_at<=now() OR last_seen_at<=now()-interval '1 hour') LIMIT $2 FOR UPDATE SKIP LOCKED) RETURNING refresh_token_ref::text", &[&organization,&SWEEP_LIMIT]).await?;
                pre.iter()
                    .chain(sessions.iter())
                    .map(|row| Ok(Ended { organization: organization.clone(), refresh_ref: text(row, 0)? }))
                    .collect::<Result<Vec<_>>>()
            }
            .await;
            transaction.finish(result.is_ok()).await?;
            ended.extend(result?);
        }
        Ok(ended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORG: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn capabilities_carry_only_their_scope_and_refuse_every_other_shape() {
        let cookie = capability(ORG).unwrap();
        assert_eq!(cookie.len(), 64);
        assert_eq!(capability_organization(&cookie).as_deref(), Some(ORG));
        assert_ne!(capability(ORG).unwrap(), cookie);
        let other = capability("00000000-0000-4000-8000-000000000002").unwrap();
        assert_ne!(capability_organization(&other).as_deref(), Some(ORG));
        let mut padded = cookie.clone();
        padded.push('=');
        for bad in [
            String::new(),
            cookie[..63].to_owned(),
            format!("{cookie}A"),
            padded,
            cookie.replace(['-', '_'], "+"),
            "A".repeat(64), // version nibble 0: not a canonical v4/v7 identifier
        ] {
            if bad != cookie {
                assert_eq!(capability_organization(&bad), None, "{bad}");
            }
        }
        let state = fresh_state().unwrap();
        assert!(capability_shaped(&state));
        assert!(!capability_shaped(&cookie));
        assert!(!capability_shaped(&state[..42]));
    }

    #[test]
    fn agent_tokens_are_prefixed_capabilities_with_their_own_digest_domain() {
        let token = fresh_agent_token(ORG).unwrap();
        assert!(token.starts_with(AGENT_TOKEN_PREFIX) && token.len() == 69);
        assert_eq!(agent_token_organization(&token).as_deref(), Some(ORG));
        // The bare capability (a session cookie's shape) is not a token.
        assert_eq!(agent_token_organization(&token[5..]), None);
        assert_eq!(agent_token_organization(&format!("{token}A")), None);
        assert_eq!(agent_token_organization("fmat_"), None);
        assert_eq!(agent_token_organization(""), None);
        let digest = agent_token_digest(&token);
        assert!(digest_shaped(&digest) && !digest.contains(&token[5..21]));
        assert_ne!(digest, super::digest(SESSION_DOMAIN, &token));
        assert_ne!(digest, super::digest(SESSION_DOMAIN, &token[5..]));
    }

    #[test]
    fn digests_are_domain_separated_sha256() {
        let value = capability(ORG).unwrap();
        let a = digest(PRELOGIN_DOMAIN, &value);
        let b = digest(PRE_ACCESS_DOMAIN, &value);
        let c = digest(SESSION_DOMAIN, &value);
        assert!(digest_shaped(&a) && digest_shaped(&b) && digest_shaped(&c));
        assert!(a != b && b != c && a != c);
        assert!(!a.contains(&value[..16]));
        assert!(same(&a, &a.clone()) && !same(&a, &b) && !same(&a, &a[..70]));
        assert_eq!(uuid_string(&uuid_bytes(ORG).unwrap()), ORG);
    }
}
