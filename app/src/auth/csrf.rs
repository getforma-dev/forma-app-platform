//! ADR-006 cross-site protection (design §9, M45). Every `POST` needs
//! `Sec-Fetch-Site: same-origin`, or, when that header is absent, an exact
//! `Origin` match; a `POST` made under a pre-access or workspace cookie also
//! carries a per-form synchronizer token tied to that cookie: an HMAC under a
//! random process-memory key over the form name and the cookie value. The two
//! anonymous forms rely on fetch metadata and the limits. A failure is 403
//! `cross_site`; nothing else runs first.
use axum::http::HeaderMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{hmac, rand::SystemRandom};

/// The fetch-metadata rule; a `false` is answered with `http::cross_site()`.
pub fn same_origin(headers: &HeaderMap, origin: &str) -> bool {
    let sites: Vec<_> = headers.get_all("sec-fetch-site").iter().collect();
    let allowed = match sites.as_slice() {
        [site] => site.as_bytes() == b"same-origin",
        [] => {
            let origins: Vec<_> = headers.get_all("origin").iter().collect();
            matches!(origins.as_slice(), [value] if value.as_bytes() == origin.as_bytes())
        }
        _ => false,
    };
    allowed
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Form {
    Resend,
    SignOut,
    SignOutAll,
    /// APP-013: the record command form (D7.16), tied to the workspace session.
    Command,
}
impl Form {
    fn name(self) -> &'static [u8] {
        match self {
            Self::Resend => b"resend",
            Self::SignOut => b"signout",
            Self::SignOutAll => b"signout-all",
            Self::Command => b"command",
        }
    }
}

pub struct CsrfKey(hmac::Key);
impl CsrfKey {
    pub fn new() -> Option<Self> {
        hmac::Key::generate(hmac::HMAC_SHA256, &SystemRandom::new())
            .ok()
            .map(Self)
    }
    fn message(form: Form, cookie: &str) -> Vec<u8> {
        let mut message = b"forma:csrf:v1\x00".to_vec();
        message.extend_from_slice(form.name());
        message.push(0);
        message.extend_from_slice(cookie.as_bytes());
        message
    }
    pub fn token(&self, form: Form, cookie: &str) -> String {
        URL_SAFE_NO_PAD.encode(hmac::sign(&self.0, &Self::message(form, cookie)))
    }
    /// Constant-time verification of the one submitted token.
    pub fn verify(&self, form: Form, cookie: &str, token: &str) -> bool {
        token.len() == 43
            && URL_SAFE_NO_PAD
                .decode(token)
                .is_ok_and(|tag| hmac::verify(&self.0, &Self::message(form, cookie), &tag).is_ok())
    }
}

/// The single `csrf` field of a form body; any other field refuses.
pub fn submitted(fields: &[(String, String)]) -> Option<&str> {
    match fields {
        [(name, value)] if name == "csrf" => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const ORIGIN: &str = "https://beta.getforma.dev";

    #[test]
    fn fetch_metadata_or_exact_origin_only() {
        let check = |pairs: &[(&'static str, &'static str)]| {
            let mut headers = HeaderMap::new();
            for (k, v) in pairs {
                headers.append(*k, HeaderValue::from_static(v));
            }
            same_origin(&headers, ORIGIN)
        };
        assert!(check(&[("sec-fetch-site", "same-origin")]));
        assert!(check(&[("origin", ORIGIN)]));
        assert!(check(&[
            ("sec-fetch-site", "same-origin"),
            ("origin", "https://evil.test")
        ]));
        for refused in [
            &[][..],
            &[("sec-fetch-site", "cross-site")][..],
            &[("sec-fetch-site", "same-site")][..],
            &[("sec-fetch-site", "none")][..],
            &[("sec-fetch-site", "cross-site"), ("origin", ORIGIN)][..],
            &[("origin", "https://beta.getforma.dev.evil.test")][..],
            &[("origin", "null")][..],
            &[("origin", ORIGIN), ("origin", ORIGIN)][..],
            &[
                ("sec-fetch-site", "same-origin"),
                ("sec-fetch-site", "same-origin"),
            ][..],
        ] {
            assert!(!check(refused), "{refused:?}");
        }
    }

    #[test]
    fn tokens_bind_form_cookie_and_process_key() {
        let key = CsrfKey::new().unwrap();
        let token = key.token(Form::SignOut, "cookie-a");
        assert!(key.verify(Form::SignOut, "cookie-a", &token));
        assert!(!key.verify(Form::SignOutAll, "cookie-a", &token));
        assert!(!key.verify(Form::SignOut, "cookie-b", &token));
        assert!(!key.verify(Form::SignOut, "cookie-a", &token[..42]));
        assert!(!CsrfKey::new()
            .unwrap()
            .verify(Form::SignOut, "cookie-a", &token));
        assert_eq!(
            submitted(&[("csrf".into(), token.clone())]),
            Some(token.as_str())
        );
        assert_eq!(submitted(&[]), None);
        assert_eq!(
            submitted(&[
                ("csrf".into(), token.clone()),
                ("grant".into(), "admin".into())
            ]),
            None
        );
    }
}
