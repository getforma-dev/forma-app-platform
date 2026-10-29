//! Domain-separated content identities: `sha256:` and the lowercase hex of
//! SHA-256(domain || 0x00 || canonical bytes). Hashes are identity, not signatures
//! or authorization.
use super::{canonical, AdmissionError};
use serde_json::Value;
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
/// A normalized revision's typed rows in their sorted order (edition
/// impl-2026-10-29.2; ADR-009 D9.3 `rows_digest`).
pub const NORMALIZED_ROWS: &str = "forma:normalized-rows:v1";
/// A job's complete payload (edition impl-2026-10-29.2;
/// `genui.jobs.payload_digest`, ADR-009 D9.4).
pub const JOB_PAYLOAD: &str = "forma:job-payload:v1";
/// Every registered domain.
pub const DOMAINS: [&str; 10] = [
    APP_DEFINITION,
    INPUT_CONTRACT,
    RECORD_SCHEMA,
    QUERY,
    QUERY_RECEIPT,
    COMMAND_PAYLOAD,
    SOURCE_BINDING,
    RESULT,
    NORMALIZED_ROWS,
    JOB_PAYLOAD,
];

/// The identity of canonical bytes within one registered domain:
/// `sha256:` and the lowercase hex of SHA-256(domain || 0x00 || canonical).
///
/// # Panics
///
/// When `domain` is not one of [`DOMAINS`]. Domains are code, never input; an
/// unregistered one would mint identities outside every contract.
pub fn digest(domain: &str, canonical: &[u8]) -> String {
    let mut hasher = domain_hasher(domain);
    hasher.update(canonical);
    identity(hasher)
}

/// The [`NORMALIZED_ROWS`] identity of typed rows streamed in their sorted
/// order: exactly [`digest`] of the forma-json-v1 array of those rows, hashed
/// one row at a time so the stream is never held whole. Each row is one
/// forma-json-v1 value bounded like a document.
pub fn normalized_rows<'a>(
    rows: impl IntoIterator<Item = &'a Value>,
) -> Result<String, AdmissionError> {
    let mut hasher = domain_hasher(NORMALIZED_ROWS);
    hasher.update(b"[");
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            hasher.update(b",");
        }
        // Encoded as `[row]` so the row's depth is checked one level down, as
        // inside the stream; the brackets are dropped.
        let wrapped = canonical::encode(&Value::Array(vec![row.clone()]))?;
        hasher.update(&wrapped[1..wrapped.len() - 1]);
    }
    hasher.update(b"]");
    Ok(identity(hasher))
}

fn domain_hasher(domain: &str) -> Sha256 {
    assert!(DOMAINS.contains(&domain), "unregistered digest domain");
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher
}

fn identity(hasher: Sha256) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut identity = String::with_capacity(71);
    identity.push_str("sha256:");
    for byte in hasher.finalize().iter() {
        identity.push(char::from(HEX[usize::from(byte >> 4)]));
        identity.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    identity
}
