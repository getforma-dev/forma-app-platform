//! The command payload identity (ADR-002; contracts/validate.py
//! `payload_digest`): `forma:command-payload:v1` over the forma-json-v1 bytes
//! of the complete `command_request` document without `idempotency_key`. The
//! same JSON under another domain has another identity; a retry's identity
//! equals the original's exactly when every value and premise is unchanged.
use crate::{admission, storage::CommandRequest};

/// `None` when the request's values are not forma-json-v1 (a float, an
/// unsafe integer, a duplicate key); storage refuses such a request.
pub fn digest(request: &CommandRequest) -> Option<String> {
    let mut document = request.document();
    document.as_object_mut()?.remove("idempotency_key");
    let bytes = admission::encode(&document).ok()?;
    Some(admission::digest::digest(
        admission::digest::COMMAND_PAYLOAD,
        &bytes,
    ))
}
