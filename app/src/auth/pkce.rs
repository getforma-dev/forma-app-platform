//! RFC 7636 S256 PKCE: a fresh 43-character verifier per flow (32 random
//! bytes) and its 43-character challenge, as fAuth's PAR requires.
use super::capability::random_capability;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}
impl Pkce {
    pub fn fresh() -> Option<Self> {
        let verifier = random_capability()?;
        let challenge = challenge(&verifier);
        Some(Self {
            verifier,
            challenge,
        })
    }
}
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s256_matches_rfc7636_appendix_b_and_every_flow_is_fresh() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let a = Pkce::fresh().unwrap();
        let b = Pkce::fresh().unwrap();
        assert_eq!((a.verifier.len(), a.challenge.len()), (43, 43));
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.challenge, challenge(&a.verifier));
    }
}
