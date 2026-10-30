//! The shared HTTP contract (ADR-007 D7.14, D7.15; design §9; ADR-006):
//! `application/problem+json` refusals carrying only a stable `code`, the
//! uniform 404 for unknown and protected paths alike, `no-store` and
//! `nosniff` on every non-page response, form bodies bounded at 4 KiB and
//! JSON bodies at 64 KiB (256 KiB on the command route) with 413
//! `payload_too_large`, and the 5 s handler deadline. APP-009 added what its
//! routes need; APP-074 adds the invitation limits, the JSON bounds and the
//! trusted-proxy restriction to the load balancer's peers.
pub mod limits;
pub mod proxy;

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Duration;

/// D7.14: auth and admission forms.
pub const FORM_BODY_LIMIT: usize = 4096;
/// D7.14: JSON query routes.
pub const JSON_BODY_LIMIT: usize = 64 * 1024;
/// D7.14: the JSON command route (32 values of up to 2,000 characters).
pub const COMMAND_BODY_LIMIT: usize = 256 * 1024;
/// D7.14: every handler.
pub const HANDLER_DEADLINE: Duration = Duration::from_secs(5);

/// `Cache-Control: no-store`, `X-Content-Type-Options: nosniff` and
/// `Referrer-Policy: no-referrer` (a redirect may carry a front-channel
/// capability such as fAuth's `request_uri`).
pub fn secure(headers: &mut HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

/// A design §9 refusal: status, `application/problem+json`, `{"code":…}`.
pub fn problem(status: StatusCode, code: &'static str) -> Response {
    let mut response = (status, format!("{{\"code\":\"{code}\"}}")).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    secure(response.headers_mut());
    response
}
/// The uniform refusal: unknown, anonymous, pre-access, expired, revoked and
/// foreign callers of a protected route are indistinguishable (ADR-006).
pub fn not_found() -> Response {
    problem(StatusCode::NOT_FOUND, "not_found")
}
/// ADR-006: a failed fetch-metadata or synchronizer-token check.
pub fn cross_site() -> Response {
    problem(StatusCode::FORBIDDEN, "cross_site")
}
pub fn payload_too_large() -> Response {
    problem(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
}
/// A store or provider outage, or the handler deadline (design §9: 503).
pub fn unavailable() -> Response {
    problem(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
}
/// A server-side limit (ADR-006 limits; design §9: 429 with `Retry-After`).
pub fn too_many_requests(retry_after: u64) -> Response {
    let mut response = problem(StatusCode::TOO_MANY_REQUESTS, "too_many_requests");
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&retry_after.max(1).to_string()).expect("decimal"),
    );
    response
}
/// `303 See Other` with the security headers.
pub fn see_other(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    secure(response.headers_mut());
    response
}

/// Router fallback and method-mismatch answer: the uniform 404.
pub async fn fallback() -> Response {
    not_found()
}

/// D7.14 handler deadline; a request that outlives it is unavailable.
pub async fn deadline(request: Request, next: Next) -> Response {
    tokio::time::timeout(HANDLER_DEADLINE, next.run(request))
        .await
        .unwrap_or_else(|_| unavailable())
}

/// An `application/x-www-form-urlencoded` body of at most 4 KiB, as pairs.
/// `None` (anything larger) is answered with 413 `payload_too_large`; the
/// caller judges the fields.
pub async fn form(body: Body) -> Option<Vec<(String, String)>> {
    let bytes = to_bytes(body, FORM_BODY_LIMIT).await.ok()?;
    Some(url::form_urlencoded::parse(&bytes).into_owned().collect())
}

/// A request body of at most `limit` bytes (`JSON_BODY_LIMIT` or
/// `COMMAND_BODY_LIMIT` for the JSON routes). `None` (anything larger) is
/// answered with 413 `payload_too_large`; the caller parses the bytes.
pub async fn bounded(body: Body, limit: usize) -> Option<axum::body::Bytes> {
    to_bytes(body, limit).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refusals_are_problem_json_with_a_stable_code_and_no_store() {
        for (response, status, code) in [
            (not_found(), 404, "not_found"),
            (cross_site(), 403, "cross_site"),
            (payload_too_large(), 413, "payload_too_large"),
            (unavailable(), 503, "unavailable"),
            (too_many_requests(7), 429, "too_many_requests"),
        ] {
            assert_eq!(response.status().as_u16(), status);
            let headers = response.headers().clone();
            assert_eq!(headers[header::CONTENT_TYPE], "application/problem+json");
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            assert_eq!(body, format!("{{\"code\":\"{code}\"}}").as_bytes());
        }
        assert_eq!(too_many_requests(7).headers()[header::RETRY_AFTER], "7");
    }

    #[tokio::test]
    async fn forms_are_bounded_at_four_kibibytes() {
        let ok = form(Body::from("csrf=a&x=b")).await.unwrap();
        assert_eq!(
            ok,
            vec![("csrf".into(), "a".into()), ("x".into(), "b".into())]
        );
        assert!(form(Body::from(vec![b'a'; FORM_BODY_LIMIT]))
            .await
            .is_some());
        assert!(form(Body::from(vec![b'a'; FORM_BODY_LIMIT + 1]))
            .await
            .is_none());
    }

    #[tokio::test]
    async fn json_bodies_are_bounded_at_64_and_256_kibibytes() {
        for limit in [JSON_BODY_LIMIT, COMMAND_BODY_LIMIT] {
            assert_eq!(
                bounded(Body::from(vec![b'{'; limit]), limit)
                    .await
                    .map(|b| b.len()),
                Some(limit)
            );
            assert!(bounded(Body::from(vec![b'{'; limit + 1]), limit)
                .await
                .is_none());
        }
        assert_eq!((JSON_BODY_LIMIT, COMMAND_BODY_LIMIT), (65_536, 262_144));
    }
}
