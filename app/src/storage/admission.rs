//! APP-073 admission operations (ADR-007 D7.1, D7.2, D7.9, D7.17; ADR-003;
//! ADR-006). Each operation is one READ COMMITTED transaction on the workspace
//! pool that calls exactly one `genui_admit` definer function. Lookups before
//! scope and the three admission writes use the private nonmember entry
//! (organization scope, no principal); sign-out uses the member's own trusted
//! request. No SQL text, client, SQLSTATE or permission name leaves storage,
//! and no caller can supply an organization's authority: the code's
//! organization, grants, expiry and entitlement are fixed server-side.
use super::{
    authorization::Transaction, pool::RolePool, require, valid_id, Error, Result, TrustedRequest,
    Workspace,
};
use sha2::{Digest, Sha256};
use tokio_postgres::{error::SqlState, Row};

/// A provider identity verified by the session adapter (APP-009) but not yet a
/// member. Constructed only inside `storage`; it carries no organization: the
/// organization always comes from the code or handle through the definer
/// lookups, and must agree with the closed registry for this identity.
///
/// ```compile_fail
/// use genui::storage::VerifiedNonmember;
/// let forged = VerifiedNonmember {
///     issuer: String::new(), provider_tenant: String::new(),
///     provider_subject: String::new(), registry_sha256: String::new(),
///     email_verified: true,
/// };
/// ```
pub struct VerifiedNonmember {
    pub(super) issuer: String,
    pub(super) provider_tenant: String,
    pub(super) provider_subject: String,
    pub(super) registry_sha256: String,
    /// The provider exchange's backend `user.email_verified` (ADR-006). An
    /// unverified mailbox may hold a pending admission but never redeems.
    pub(super) email_verified: bool,
}

/// `reserve_code` outcomes (D7.1). A pre-signup reservation lasts 10 minutes,
/// never counts against remaining redemptions and authorizes no work.
#[derive(Debug, Eq, PartialEq)]
pub enum Reservation {
    Reserved {
        reservation: String,
        /// Unix seconds.
        expires_at: i64,
    },
    Expired,
    Revoked,
    Exhausted,
    TooManyReservations,
}

/// `record_pending_admission` outcomes (D7.1). A pending admission lasts 24
/// hours, is bound to the verified identity, counts against the code's
/// remaining redemptions and grants nothing.
#[derive(Debug, Eq, PartialEq)]
pub enum PendingAdmission {
    Pending {
        pending_admission: String,
        /// Unix seconds.
        expires_at: i64,
    },
    AlreadyMember,
    ReservationExpired,
    Expired,
    Revoked,
    Exhausted,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PendingReason {
    /// The exchange did not report `email_verified` true; nothing was written.
    EmailUnverified,
}

/// D7.9. `Admitted` is also the answer to an identical retry, which returns
/// the stored result and consumes nothing. Every code-side, expiry, identity
/// or payload refusal is the neutral `Refused`.
#[derive(Debug, Eq, PartialEq)]
pub enum Admission {
    Admitted {
        principal: String,
        epoch: i64,
        entitlement: String,
    },
    Pending {
        reason: PendingReason,
    },
    AlreadyMember,
    Refused,
}

/// The two `advance_member_epoch` reasons (D7.1); nothing else advances an
/// epoch from the application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignOut {
    One,
    All,
}
impl SignOut {
    fn reason(self) -> &'static str {
        match self {
            Self::One => "session.sign_out",
            Self::All => "session.sign_out_all",
        }
    }
}

const CODE_DOMAIN: &[u8] = b"forma:access-code:v1";
const HANDLE_DOMAIN: &[u8] = b"forma:signin-handle:v1";
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 without padding, canonical only: every trailing bit zero.
fn base32(text: &[u8], bytes: usize) -> Option<Vec<u8>> {
    if text.len() != (bytes * 8).div_ceil(5) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &c in text {
        let value = ALPHABET.iter().position(|&a| a == c)? as u32;
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    (out.len() == bytes && buffer == 0).then_some(out)
}
fn domain_digest(domain: &[u8], raw: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update([0u8]);
    hash.update(raw);
    let hash = hash.finalize();
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for byte in hash.iter() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
fn digest_shaped(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The stored digest of an operator-issued access code: the 32 secret bytes
/// (52 base32 characters, hyphens, spaces and case ignored) under the
/// `forma:access-code:v1` domain. Anything else is no code at all; callers
/// answer it with the same neutral refusal as an unknown code.
pub fn code_digest(code: &str) -> Option<String> {
    if code.len() > 256 {
        return None;
    }
    let compact: Vec<u8> = code
        .bytes()
        .filter(|b| *b != b'-' && !b.is_ascii_whitespace())
        .map(|b| b.to_ascii_uppercase())
        .collect();
    base32(&compact, 32).map(|raw| domain_digest(CODE_DOMAIN, &raw))
}

/// The stored digest of an organization sign-in handle (ADR-006): 16 random
/// bytes as 26 lowercase base32 characters, under `forma:signin-handle:v1`.
pub fn handle_digest(handle: &str) -> Option<String> {
    if handle.len() != 26
        || !handle
            .bytes()
            .all(|b| b.is_ascii_lowercase() || matches!(b, b'2'..=b'7'))
    {
        return None;
    }
    base32(handle.to_ascii_uppercase().as_bytes(), 16).map(|raw| domain_digest(HANDLE_DOMAIN, &raw))
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

/// One row of a definer call: (identifier, expiry in Unix seconds, outcome).
type Outcome = (Option<String>, Option<i64>, String);
fn outcome(rows: &[Row]) -> Result<Outcome> {
    let row = rows.first().ok_or(Error::Denied)?;
    Ok((
        optional(row, 0)?,
        row.try_get(1).map_err(|_| Error::Unavailable)?,
        text(row, 2)?,
    ))
}

/// The verified identity must map to this organization through the closed
/// registry, and the scoped provider binding must exist and be active
/// (`genui_admit` deliberately cannot read bindings; review m3).
fn identity_agrees(
    pool: &RolePool,
    identity: &VerifiedNonmember,
    organization: &str,
) -> Result<()> {
    require(
        valid_id(organization)
            && pool.registry.identity.sha256 == identity.registry_sha256
            && pool
                .registry
                .agrees(&identity.issuer, &identity.provider_tenant, organization)
            && (1..=256).contains(&identity.provider_subject.len()),
    )
}
async fn binding_active(
    transaction: &Transaction<'_>,
    identity: &VerifiedNonmember,
    organization: &str,
) -> Result<()> {
    let rows = transaction.query("SELECT 1 FROM genui.provider_tenant_bindings WHERE organization_id=$1::text::uuid AND issuer=$2 AND provider_tenant_id=$3 AND active", &[&organization,&identity.issuer,&identity.provider_tenant]).await?;
    require(rows.len() == 1)
}

impl Workspace {
    /// D7.2: the organization of a known code digest, before any scope exists.
    /// Unknown and malformed digests are `None`, never an error.
    pub async fn resolve_code(&self, code_digest: &str) -> Result<Option<String>> {
        self.resolve("SELECT genui_private.resolve_code($1)::text", code_digest)
            .await
    }
    /// D7.2: the organization of a live sign-in handle digest.
    pub async fn resolve_handle(&self, handle_digest: &str) -> Result<Option<String>> {
        self.resolve(
            "SELECT genui_private.resolve_handle($1)::text",
            handle_digest,
        )
        .await
    }
    async fn resolve(&self, sql: &'static str, digest: &str) -> Result<Option<String>> {
        if !digest_shaped(digest) {
            return Ok(None);
        }
        let transaction = Transaction::begin_nonmember(&self.inner, None).await?;
        let result = async {
            let rows = transaction.query(sql, &[&digest]).await?;
            optional(rows.first().ok_or(Error::Unavailable)?, 0)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// D7.1 pre-signup validation: holds no redemption slot and grants nothing.
    /// A digest that is not this organization's code is `Err(Denied)`, as is a
    /// malformed argument; callers render both like any other refusal.
    pub async fn reserve_code(
        &self,
        organization: &str,
        code_digest: &str,
        context_digest: &str,
    ) -> Result<Reservation> {
        require(
            valid_id(organization) && digest_shaped(code_digest) && digest_shaped(context_digest),
        )?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            let rows = transaction.query("SELECT reservation_id::text,floor(extract(epoch FROM expires_at))::bigint,outcome FROM genui_private.reserve_code($1::text::uuid,$2,$3)", &[&organization,&code_digest,&context_digest]).await?;
            #[cfg(test)]
            transaction.lease.checkpoint("reserve").await;
            outcome(&rows)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        let row = result?;
        Ok(match (row.2.as_str(), row.0, row.1) {
            ("reserved", Some(reservation), Some(expires_at)) => Reservation::Reserved {
                reservation,
                expires_at,
            },
            ("expired", ..) => Reservation::Expired,
            ("revoked", ..) => Reservation::Revoked,
            ("exhausted", ..) => Reservation::Exhausted,
            ("too_many_reservations", ..) => Reservation::TooManyReservations,
            _ => return Err(Error::Unavailable),
        })
    }

    /// D7.1, ADR-006 callback: binds a live pre-signup reservation to the
    /// verified identity as a 24-hour pending admission. Independent of the
    /// mailbox state; it commits no slot, membership or entitlement.
    pub async fn record_pending_admission(
        &self,
        identity: &VerifiedNonmember,
        organization: &str,
        reservation: &str,
    ) -> Result<PendingAdmission> {
        identity_agrees(&self.inner, identity, organization)?;
        require(valid_id(reservation))?;
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            binding_active(&transaction, identity, organization).await?;
            let rows = transaction.query("SELECT pending_admission_id::text,floor(extract(epoch FROM expires_at))::bigint,outcome FROM genui_private.record_pending_admission($1::text::uuid,$2::text::uuid,$3,$4,$5)", &[&organization,&reservation,&identity.issuer,&identity.provider_tenant,&identity.provider_subject]).await?;
            #[cfg(test)]
            transaction.lease.checkpoint("pending").await;
            outcome(&rows)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        let row = result?;
        Ok(match (row.2.as_str(), row.0, row.1) {
            ("pending", Some(pending_admission), Some(expires_at)) => PendingAdmission::Pending {
                pending_admission,
                expires_at,
            },
            ("already_member", ..) => PendingAdmission::AlreadyMember,
            ("reservation_expired", ..) => PendingAdmission::ReservationExpired,
            ("expired", ..) => PendingAdmission::Expired,
            ("revoked", ..) => PendingAdmission::Revoked,
            ("exhausted", ..) => PendingAdmission::Exhausted,
            _ => return Err(Error::Unavailable),
        })
    }

    /// D7.1 `admit_redemption`: rechecks expiry, revocation and remaining
    /// redemptions and, in this one transaction, commits the principal, the
    /// epoch-1 membership, the code's grant template, the 30-day entitlement,
    /// the redemption and its audit event. Only for an identity whose provider
    /// exchange reported `email_verified` true; otherwise nothing is written.
    pub async fn admit(
        &self,
        identity: &VerifiedNonmember,
        organization: &str,
        pending_admission: &str,
        payload_digest: &str,
    ) -> Result<Admission> {
        identity_agrees(&self.inner, identity, organization)?;
        require(valid_id(pending_admission) && digest_shaped(payload_digest))?;
        if !identity.email_verified {
            return Ok(Admission::Pending {
                reason: PendingReason::EmailUnverified,
            });
        }
        let transaction = Transaction::begin_nonmember(&self.inner, Some(organization)).await?;
        let result = async {
            binding_active(&transaction, identity, organization).await?;
            let rows = transaction.query_classified("SELECT principal_id::text,membership_epoch,entitlement_id::text,outcome FROM genui_private.admit_redemption($1::text::uuid,$2::text::uuid,$3,$4,$5,$6)", &[&organization,&pending_admission,&identity.issuer,&identity.provider_tenant,&identity.provider_subject,&payload_digest]).await?;
            #[cfg(test)]
            transaction.lease.checkpoint("admit").await;
            // 0003 answers a concurrent same-identity admission through another
            // code with already_member; a residual unique violation is the same
            // business fact and is never disclosed as an error (D7.9).
            let rows = match rows {
                Ok(rows) => rows,
                Err(state) if state == SqlState::UNIQUE_VIOLATION => return Ok((false, Admission::AlreadyMember)),
                Err(_) => return Err(Error::Unavailable),
            };
            let row = rows.first().ok_or(Error::Denied)?;
            let admission = match text(row, 3)?.as_str() {
                "admitted" | "replayed" => Admission::Admitted {
                    principal: text(row, 0)?,
                    epoch: integer(row, 1)?,
                    entitlement: text(row, 2)?,
                },
                "already_member" => Admission::AlreadyMember,
                "expired" | "revoked" | "exhausted" | "mismatch" => Admission::Refused,
                _ => return Err(Error::Unavailable),
            };
            Ok((true, admission))
        }
        .await;
        let commit = matches!(result, Ok((true, _)));
        transaction.finish(commit).await?;
        result.map(|(_, admission)| admission)
    }

    /// D7.1 `advance_member_epoch` for sign-out. It is called instead of the
    /// member fence, never after it: the definer checks the same context and
    /// current epoch under `FOR UPDATE`, which is this transaction's fence, so
    /// two concurrent sign-outs queue rather than deadlock (review m1). Sign-out
    /// needs no current entitlement.
    pub async fn advance_epoch(&self, request: &TrustedRequest, reason: SignOut) -> Result<i64> {
        let transaction = Transaction::begin(&self.inner, request).await?;
        let result = async {
            let rows = transaction
                .query(
                    "SELECT genui_private.advance_member_epoch($1::text::uuid,$2::text::uuid,$3)",
                    &[&request.organization, &request.principal, &reason.reason()],
                )
                .await?;
            #[cfg(test)]
            transaction.lease.checkpoint("sign_out").await;
            rows.first()
                .ok_or(Error::Unavailable)?
                .try_get::<_, Option<i64>>(0)
                .map_err(|_| Error::Unavailable)?
                .ok_or(Error::Denied)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shared with app/runtime/test_admission_operator.py: the operator command
    // stores exactly the digest this module computes from a typed code.
    const VECTOR_CODE: &str = "AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4IB-CEQT-CQKR-MFYY-DENB-WHA5-DYPQ";
    const VECTOR_DIGEST: &str =
        "sha256:3d6abee22a05d2ade33f3017a45c52630d05311687d4f77a28601bb2343752cc";
    const HANDLE: &str = "mrswmz3infvgw3dnnzxxa4lsom";
    const HANDLE_DIGEST: &str =
        "sha256:4a08c5c04799367f81a02414698be03c99f175ca70405e354ebd814a672e7c22";

    #[test]
    fn code_digest_matches_the_operator_vector_for_every_accepted_spelling() {
        let compact = VECTOR_CODE.replace('-', "");
        for spelling in [
            VECTOR_CODE.to_owned(),
            VECTOR_CODE.to_lowercase(),
            compact.clone(),
            format!(" {}\n", VECTOR_CODE.replace('-', " ")),
        ] {
            assert_eq!(code_digest(&spelling).as_deref(), Some(VECTOR_DIGEST));
        }
        assert_eq!(
            base32(compact.as_bytes(), 32),
            Some((0u8..32).collect::<Vec<_>>())
        );
    }

    #[test]
    fn malformed_codes_have_no_digest() {
        let compact = VECTOR_CODE.replace('-', "");
        let last = compact.len() - 1;
        for bad in [
            String::new(),
            compact[..last].to_owned(),
            format!("{compact}A"),
            format!("{}R", &compact[..last]), // nonzero trailing bits
            format!("{}1", &compact[..last]),
            compact.replacen('A', "0", 1),
            "x".repeat(400),
            format!("{compact}\u{00e9}"),
        ] {
            assert_eq!(code_digest(&bad), None, "{bad:.12}");
        }
    }

    #[test]
    fn handle_digest_matches_the_operator_vector_and_is_strict() {
        assert_eq!(handle_digest(HANDLE).as_deref(), Some(HANDLE_DIGEST));
        for bad in [
            HANDLE.to_uppercase(),
            HANDLE[..25].to_owned(),
            format!("{HANDLE}a"),
            format!("{}b", &HANDLE[..25]),
            HANDLE.replacen('m', "1", 1),
        ] {
            assert_eq!(handle_digest(&bad), None, "{bad}");
        }
    }

    #[test]
    fn digest_shape_and_sign_out_reasons_are_closed() {
        assert!(digest_shaped(VECTOR_DIGEST));
        assert!(!digest_shaped(&VECTOR_DIGEST.to_uppercase()));
        assert!(!digest_shaped(&VECTOR_DIGEST[..70]));
        assert!(!digest_shaped("sha512:00"));
        assert_eq!(SignOut::One.reason(), "session.sign_out");
        assert_eq!(SignOut::All.reason(), "session.sign_out_all");
    }
}
