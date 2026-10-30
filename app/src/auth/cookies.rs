//! ADR-006 cookies: `__Host-` prefixed, `Secure`, `HttpOnly`, `Path=/`,
//! `SameSite=Lax`, opaque values whose server-side records hold the state.
//! A cookie without a live record is treated exactly like no cookie; a
//! duplicated or oversized cookie is no cookie at all.
use crate::storage::{PRELOGIN_SECONDS, PRE_ACCESS_ABSOLUTE_SECONDS, SESSION_ABSOLUTE_SECONDS};
use axum::http::{header, HeaderMap, HeaderValue};

pub const PRELOGIN: &str = "__Host-genui_prelogin";
pub const PRE_ACCESS: &str = "__Host-genui_pre";
pub const SESSION: &str = "__Host-genui_session";
/// Routing convenience only (the organization's sign-in handle), never authority.
pub const ORGANIZATION_HINT: &str = "__Host-genui_org";
pub const ORGANIZATION_HINT_SECONDS: u64 = 30 * 86_400;
const MAX_COOKIE_HEADER: usize = 4096;
const MAX_VALUE: usize = 128;

/// The lifetime each cookie is issued with (ADR-006 table).
pub fn lifetime(name: &str) -> u64 {
    match name {
        PRELOGIN => PRELOGIN_SECONDS,
        PRE_ACCESS => PRE_ACCESS_ABSOLUTE_SECONDS,
        SESSION => SESSION_ABSOLUTE_SECONDS,
        _ => ORGANIZATION_HINT_SECONDS,
    }
}

/// The single value of `name`, if it occurs exactly once among bounded
/// `Cookie` headers and is a plain token.
pub fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut total = 0;
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        total += value.len();
        if total > MAX_COOKIE_HEADER {
            return None;
        }
        for pair in value.to_str().ok()?.split(';') {
            let Some((key, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if key == name {
                if found.is_some() {
                    return None;
                }
                found = Some(value.to_owned());
            }
        }
    }
    found.filter(|v| {
        !v.is_empty()
            && v.len() <= MAX_VALUE
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

pub fn set(name: &str, value: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={}",
        lifetime(name)
    ))
    .expect("base64url cookie values")
}

pub fn clear(name: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{name}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"
    ))
    .expect("fixed cookie names")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_and_lifetimes_follow_adr_006() {
        assert_eq!(
            set(SESSION, "abc").to_str().unwrap(),
            "__Host-genui_session=abc; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=43200"
        );
        assert!(set(PRELOGIN, "a")
            .to_str()
            .unwrap()
            .ends_with("Max-Age=600"));
        assert!(set(PRE_ACCESS, "a")
            .to_str()
            .unwrap()
            .ends_with("Max-Age=1800"));
        assert!(set(ORGANIZATION_HINT, "a")
            .to_str()
            .unwrap()
            .ends_with("Max-Age=2592000"));
        assert!(clear(PRE_ACCESS).to_str().unwrap().ends_with("Max-Age=0"));
        for name in [PRELOGIN, PRE_ACCESS, SESSION, ORGANIZATION_HINT] {
            assert!(name.starts_with("__Host-"));
        }
    }

    #[test]
    fn exactly_one_bounded_token_value_is_a_cookie() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("a=1; __Host-genui_pre=xyz_-9"),
        );
        assert_eq!(read(&headers, PRE_ACCESS).as_deref(), Some("xyz_-9"));
        assert_eq!(read(&headers, SESSION), None);
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("__Host-genui_pre=other"),
        );
        assert_eq!(read(&headers, PRE_ACCESS), None, "duplicate");
        let mut odd = HeaderMap::new();
        odd.append(
            header::COOKIE,
            HeaderValue::from_static("__Host-genui_pre=a+b"),
        );
        assert_eq!(read(&odd, PRE_ACCESS), None);
        let mut long = HeaderMap::new();
        long.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("__Host-genui_pre={}", "a".repeat(129))).unwrap(),
        );
        assert_eq!(read(&long, PRE_ACCESS), None);
    }
}
