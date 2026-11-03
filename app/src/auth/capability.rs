//! Random capabilities and the process-memory sealing key (ADR-006, D7.11).
//!
//! `Sealer` holds one random AES-256-GCM key that exists only in this
//! process's memory: the PKCE verifier stored in a flow row and every vault
//! entry are sealed under it, so a restart makes both unusable (the user
//! starts a fresh sign-in; D7.18 deploys stop-then-start).
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN},
    rand::{SecureRandom, SystemRandom},
};

pub fn random_bytes<const N: usize>() -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    SystemRandom::new().fill(&mut bytes).ok()?;
    Some(bytes)
}

/// 32 random bytes as 43 base64url characters (PKCE verifier).
pub fn random_capability() -> Option<String> {
    Some(URL_SAFE_NO_PAD.encode(random_bytes::<32>()?))
}

/// `N` random bytes as `2N` lowercase hex digits (ids and idempotency keys).
pub fn random_hex<const N: usize>() -> Option<String> {
    Some(
        random_bytes::<N>()?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// A random canonical UUID (version 4, RFC variant), for vault references.
pub fn random_uuid() -> Option<String> {
    let mut bytes = random_bytes::<16>()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

pub struct Sealer {
    key: LessSafeKey,
}
impl Sealer {
    pub fn new() -> Option<Self> {
        let raw = random_bytes::<32>()?;
        Some(Self {
            key: LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &raw).ok()?),
        })
    }
    /// `nonce ‖ ciphertext ‖ tag`, bound to `context` as associated data.
    pub fn seal(&self, context: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
        let nonce = random_bytes::<NONCE_LEN>()?;
        let mut out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(context),
                &mut out,
            )
            .ok()?;
        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&out);
        Some(sealed)
    }
    pub fn open(&self, context: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < NONCE_LEN + AES_256_GCM.tag_len() {
            return None;
        }
        let (nonce, body) = sealed.split_at(NONCE_LEN);
        let mut body = body.to_vec();
        let plain = self
            .key
            .open_in_place(
                Nonce::try_assume_unique_for_key(nonce).ok()?,
                Aad::from(context),
                &mut body,
            )
            .ok()?;
        Some(plain.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealing_is_bound_to_its_context_and_to_this_process_key() {
        let sealer = Sealer::new().unwrap();
        let sealed = sealer.seal(b"ctx", b"verifier").unwrap();
        assert_eq!(sealer.open(b"ctx", &sealed).unwrap(), b"verifier");
        assert!(sealer.open(b"other", &sealed).is_none());
        // A restart: a new key cannot open what the old one sealed.
        assert!(Sealer::new().unwrap().open(b"ctx", &sealed).is_none());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(sealer.open(b"ctx", &tampered).is_none());
        assert!(sealer.open(b"ctx", &sealed[..20]).is_none());
        let id = random_uuid().unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
        assert_eq!(random_capability().unwrap().len(), 43);
    }
}
