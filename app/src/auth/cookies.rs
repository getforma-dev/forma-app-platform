//! ADR-006 cookies: `__Host-` prefixed, `Secure`, `HttpOnly`, `Path=/`,
//! `SameSite=Lax`, opaque values whose server-side records hold the state.
//! A cookie without a live record is treated exactly like no cookie; a
//! duplicated or oversized cookie is no cookie at all.
//!
//! The one exception to "opaque" is the APP-012 run cookie ([`RUN`]): it
//! carries no authority and no secret, only the normalized, validated run
//! parameters of one saved work, tagged under the process key to that work
//! and to the session cookie, so that a reload of the work page restores its
//! views (ADR-010 §1 act 4) without a re-submitted form, a query string or a
//! stored record. It is `SameSite=Strict` (a cross-site navigation never
//! carries it, so a link from elsewhere runs nothing) and short-lived.
use crate::storage::{PRELOGIN_SECONDS, PRE_ACCESS_ABSOLUTE_SECONDS, SESSION_ABSOLUTE_SECONDS};
use axum::http::{header, HeaderMap, HeaderValue};

pub const PRELOGIN: &str = "__Host-genui_prelogin";
pub const PRE_ACCESS: &str = "__Host-genui_pre";
pub const SESSION: &str = "__Host-genui_session";
/// Routing convenience only (the organization's sign-in handle), never authority.
pub const ORGANIZATION_HINT: &str = "__Host-genui_org";
pub const ORGANIZATION_HINT_SECONDS: u64 = 30 * 86_400;
/// APP-012: the last successful run's parameters of one work page (tag and
/// parameters, never authority), `SameSite=Strict`, 10 minutes, refreshed by
/// each run it restores.
pub const RUN: &str = "__Host-genui_run";
pub const RUN_SECONDS: u64 = 600;
/// The run cookie's largest value: a 43-character tag plus the base64url of
/// the parameters (the page bounds its destination at 256 characters), well
/// inside the 4 KiB `Cookie` header bound with the session cookie beside it.
pub const RUN_MAX_VALUE: usize = 2560;
const MAX_COOKIE_HEADER: usize = 4096;
const MAX_VALUE: usize = 128;

/// The lifetime each cookie is issued with (ADR-006 table).
pub fn lifetime(name: &str) -> u64 {
    match name {
        PRELOGIN => PRELOGIN_SECONDS,
        PRE_ACCESS => PRE_ACCESS_ABSOLUTE_SECONDS,
        SESSION => SESSION_ABSOLUTE_SECONDS,
        RUN => RUN_SECONDS,
        _ => ORGANIZATION_HINT_SECONDS,
    }
}

/// The single value of `name`, if it occurs exactly once among bounded
/// `Cookie` headers and is a plain token of at most 128 characters.
pub fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    read_bounded(headers, name, MAX_VALUE)
}

/// As [`read`], with the value bound `max_value` (the run cookie's
/// [`RUN_MAX_VALUE`]).
pub fn read_bounded(headers: &HeaderMap, name: &str, max_value: usize) -> Option<String> {
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
            && v.len() <= max_value
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

/// The run cookie: as [`set`] but `SameSite=Strict`, so no cross-site
/// navigation (a link from another site to the work page) carries it.
pub fn set_run(value: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{RUN}={value}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={RUN_SECONDS}"
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
        for name in [PRELOGIN, PRE_ACCESS, SESSION, ORGANIZATION_HINT, RUN] {
            assert!(name.starts_with("__Host-"));
        }
        // APP-012: the run cookie is Strict and short; clearing it needs no
        // attribute beyond name and path.
        assert_eq!(
            set_run("abc").to_str().unwrap(),
            "__Host-genui_run=abc; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=600"
        );
        assert!(clear(RUN)
            .to_str()
            .unwrap()
            .starts_with("__Host-genui_run=; Path=/;"));
    }

    #[test]
    fn the_run_cookie_reads_under_its_own_bound() {
        let value = "x".repeat(RUN_MAX_VALUE);
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("{RUN}={value}; {SESSION}=abc")).unwrap(),
        );
        assert_eq!(read(&headers, SESSION).as_deref(), Some("abc"));
        assert_eq!(read(&headers, RUN), None, "128 is the opaque bound");
        assert_eq!(read_bounded(&headers, RUN, RUN_MAX_VALUE), Some(value));
        let mut long = HeaderMap::new();
        long.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("{RUN}={}", "x".repeat(RUN_MAX_VALUE + 1))).unwrap(),
        );
        assert_eq!(read_bounded(&long, RUN, RUN_MAX_VALUE), None);
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
