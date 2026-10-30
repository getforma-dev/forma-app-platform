//! Slack request signatures (Slack's "Verifying requests from Slack"):
//! `X-Slack-Signature: v0=hex(HMAC-SHA256(signing_secret, "v0:" ts ":" body))`
//! over the raw body, with `X-Slack-Request-Timestamp` within
//! [`MAX_SKEW_SECONDS`] of now. Judged before any byte of the body is parsed;
//! the comparison is `ring::hmac::verify` (constant time). A signature seen
//! once is refused for the rest of its window (a replay of the same signed
//! request), and a body re-sent under a new timestamp fails the MAC.
use ring::hmac;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

/// Slack's replay window.
pub const MAX_SKEW_SECONDS: u64 = 300;

/// Refused signature checks since start (the only trace a refusal leaves).
static REFUSED: AtomicU64 = AtomicU64::new(0);
/// How many requests were refused by the signature check.
pub fn refused_count() -> u64 {
    REFUSED.load(Ordering::Relaxed)
}
pub(super) fn count_refusal() {
    REFUSED.fetch_add(1, Ordering::Relaxed);
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
/// `v0=` and 64 lowercase hex digits, as the 32-byte tag.
fn tag(signature: &str) -> Option<[u8; 32]> {
    let hex = signature.strip_prefix("v0=")?.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in hex.chunks(2).enumerate() {
        out[i] = hex_digit(pair[0])? << 4 | hex_digit(pair[1])?;
    }
    Some(out)
}
/// A decimal Unix second of at most 12 digits.
fn seconds(timestamp: &str) -> Option<u64> {
    if timestamp.is_empty()
        || timestamp.len() > 12
        || !timestamp.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    timestamp.parse().ok()
}

/// Whether `body` carries Slack's valid signature for `secret` at `now`.
pub fn verify(
    secret: &[u8],
    timestamp: Option<&str>,
    signature: Option<&str>,
    body: &[u8],
    now: u64,
) -> bool {
    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return false;
    };
    let Some(at) = seconds(timestamp) else {
        return false;
    };
    if at.abs_diff(now) > MAX_SKEW_SECONDS {
        return false;
    }
    let Some(expected) = tag(signature) else {
        return false;
    };
    let mut message = Vec::with_capacity(body.len() + timestamp.len() + 4);
    message.extend_from_slice(b"v0:");
    message.extend_from_slice(timestamp.as_bytes());
    message.push(b':');
    message.extend_from_slice(body);
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    hmac::verify(&key, &message, &expected).is_ok()
}

/// Signatures accepted within the window: a second delivery of the same
/// signed request is refused.
#[derive(Default)]
pub struct Replays {
    seen: Mutex<HashMap<String, u64>>,
}
impl Replays {
    /// Records `signature` accepted at `now`; false when it was already seen
    /// within the window.
    pub fn first(&self, signature: &str, now: u64) -> bool {
        let Ok(mut seen) = self.seen.lock() else {
            return false;
        };
        if seen.len() > 4096 {
            seen.retain(|_, at| now.saturating_sub(*at) <= 2 * MAX_SKEW_SECONDS);
        }
        match seen.get(signature) {
            Some(at) if now.saturating_sub(*at) <= 2 * MAX_SKEW_SECONDS => false,
            _ => {
                seen.insert(signature.to_owned(), now);
                true
            }
        }
    }
}

/// The signature header a test or the loopback stub sends.
#[cfg(test)]
pub fn sign(secret: &[u8], timestamp: &str, body: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    let mut message = b"v0:".to_vec();
    message.extend_from_slice(timestamp.as_bytes());
    message.push(b':');
    message.extend_from_slice(body);
    let tag = hmac::sign(&key, &message);
    format!(
        "v0={}",
        tag.as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
