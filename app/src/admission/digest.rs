//! Domain-separated content identities: `sha256:` and the lowercase hex of
//! SHA-256(domain || 0x00 || canonical bytes). Hashes are identity, not signatures
//! or authorization.
use sha2::{Digest, Sha256};

/// Admitted definition content.
pub const APP_DEFINITION: &str = "forma:app-definition:v1";
/// One input schema contract.
pub const INPUT_CONTRACT: &str = "forma:input-contract:v1";
/// One record schema.
pub const RECORD_SCHEMA: &str = "forma:record-schema:v1";
/// One view's typed query.
pub const QUERY: &str = "forma:query:v1";
/// A query receipt without its own `digest`.
pub const QUERY_RECEIPT: &str = "forma:query-receipt:v1";
/// A command request without its `idempotency_key`.
pub const COMMAND_PAYLOAD: &str = "forma:command-payload:v1";
/// One saved-work source binding.
pub const SOURCE_BINDING: &str = "forma:source-binding:v1";
/// A query or job result.
pub const RESULT: &str = "forma:result:v1";
/// Every registered domain.
pub const DOMAINS: [&str; 8] = [
    APP_DEFINITION,
    INPUT_CONTRACT,
    RECORD_SCHEMA,
    QUERY,
    QUERY_RECEIPT,
    COMMAND_PAYLOAD,
    SOURCE_BINDING,
    RESULT,
];

/// The identity of canonical bytes within one registered domain:
/// `sha256:` and the lowercase hex of SHA-256(domain || 0x00 || canonical).
///
/// # Panics
///
/// When `domain` is not one of [`DOMAINS`]. Domains are code, never input; an
/// unregistered one would mint identities outside every contract.
pub fn digest(domain: &str, canonical: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    assert!(DOMAINS.contains(&domain), "unregistered digest domain");
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(canonical);
    let mut identity = String::with_capacity(71);
    identity.push_str("sha256:");
    for byte in hasher.finalize().iter() {
        identity.push(char::from(HEX[usize::from(byte >> 4)]));
        identity.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    identity
}
