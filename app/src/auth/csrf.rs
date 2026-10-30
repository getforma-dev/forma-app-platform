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
            // An unconfigured (empty) app origin matches nothing, not an
            // empty `Origin:` header (APP-009 review m3).
            matches!(origins.as_slice(), [value] if !origin.is_empty() && value.as_bytes() == origin.as_bytes())
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
    /// APP-012: the work page's run form (`POST /w/{app}/{work}`).
    Query,
}
impl Form {
    fn name(self) -> &'static [u8] {
        match self {
            Self::Resend => b"resend",
            Self::SignOut => b"signout",
            Self::SignOutAll => b"signout-all",
            Self::Command => b"command",
            Self::Query => b"query",
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

    /// APP-012 run cookie (reload restores): the tag over one work path
    /// (`app`, `work`), the session cookie and the canonical parameters, so
    /// the cookie restores only the run of that work for that session, and
    /// only as submitted. A restart (a new key) or any other work, session
    /// or parameter bytes leaves the cookie ignored.
    fn run_message(app: &str, work: &str, cookie: &str, parameters: &[u8]) -> Vec<u8> {
        let mut message = b"forma:run-cookie:v1\x00".to_vec();
        for part in [app.as_bytes(), work.as_bytes(), cookie.as_bytes()] {
            message.extend_from_slice(part);
            message.push(0);
        }
        message.extend_from_slice(parameters);
        message
    }
    pub fn run_tag(&self, app: &str, work: &str, cookie: &str, parameters: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(hmac::sign(
            &self.0,
            &Self::run_message(app, work, cookie, parameters),
        ))
    }
    /// Constant-time verification of a run cookie's 43-character tag.
    pub fn verify_run(
        &self,
        app: &str,
        work: &str,
        cookie: &str,
        parameters: &[u8],
        tag: &str,
    ) -> bool {
        tag.len() == 43
            && URL_SAFE_NO_PAD.decode(tag).is_ok_and(|tag| {
                hmac::verify(
                    &self.0,
                    &Self::run_message(app, work, cookie, parameters),
                    &tag,
                )
                .is_ok()
            })
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
        // APP-009 review m3: with no configured origin, an empty `Origin:`
        // (or any Origin) never passes; fetch metadata still decides.
        let mut empty = HeaderMap::new();
        empty.append("origin", HeaderValue::from_static(""));
        assert!(!same_origin(&empty, ""));
        assert!(!same_origin(&empty, ORIGIN));
        let mut fetch = HeaderMap::new();
        fetch.append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(same_origin(&fetch, ""));
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

    #[test]
    fn run_tags_bind_work_session_parameters_and_process_key() {
        let key = CsrfKey::new().unwrap();
        let tag = key.run_tag("app-a", "work-a", "cookie-a", b"{\"window\":1}");
        assert_eq!(tag.len(), 43);
        assert!(key.verify_run("app-a", "work-a", "cookie-a", b"{\"window\":1}", &tag));
        assert!(!key.verify_run("app-b", "work-a", "cookie-a", b"{\"window\":1}", &tag));
        assert!(!key.verify_run("app-a", "work-b", "cookie-a", b"{\"window\":1}", &tag));
        assert!(!key.verify_run("app-a", "work-a", "cookie-b", b"{\"window\":1}", &tag));
        assert!(!key.verify_run("app-a", "work-a", "cookie-a", b"{\"window\":2}", &tag));
        assert!(!key.verify_run("app-a", "work-a", "cookie-a", b"{\"window\":1}", &tag[..42]));
        // The run tag and a form token never stand in for each other.
        assert!(!key.verify(Form::Query, "cookie-a", &tag));
        assert!(!CsrfKey::new().unwrap().verify_run(
            "app-a",
            "work-a",
            "cookie-a",
            b"{\"window\":1}",
            &tag
        ));
    }
}
