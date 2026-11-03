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
    extract::{FromRequestParts, Path, Request},
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
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

/// How a bounded body read ended short of bytes.
#[derive(Debug, Eq, PartialEq)]
pub enum BodyRefused {
    /// More than the bound, or the stream failed.
    TooLarge,
    /// The bytes did not arrive within the deadline (composer review M1).
    Stalled,
}

/// A request body of at most `limit` bytes arriving within `deadline`: a
/// caller that trickles or stalls holds a route no longer than any other
/// handler. Routes read it only after the session check, so an anonymous
/// caller spends nothing.
pub async fn read_bounded(
    body: Body,
    limit: usize,
    deadline: Duration,
) -> Result<axum::body::Bytes, BodyRefused> {
    match tokio::time::timeout(deadline, to_bytes(body, limit)).await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Err(BodyRefused::TooLarge),
        Err(_) => Err(BodyRefused::Stalled),
    }
}

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

/// A design §9 refusal: status, `application/problem+json`,
/// `{"code":…,"status":…}` as forma-json-v1 (contract `problem` requires
/// `status` for every code; APP-013 review M2).
pub fn problem(status: StatusCode, code: &'static str) -> Response {
    problem_with(status, code, serde_json::Value::Null)
}
/// A design §9 refusal carrying its class-specific authorized members
/// (ADR-007 D7.15: the recovery view on 409, the current revision on 412):
/// `status`, `code` and `extra`'s members, as forma-json-v1 (contract
/// `problem`). A member that is not forma-json-v1 makes the answer a plain
/// `unavailable`, never a partial body.
pub fn problem_with(status: StatusCode, code: &'static str, extra: serde_json::Value) -> Response {
    let mut body = serde_json::Map::new();
    body.insert("status".into(), serde_json::Value::from(status.as_u16()));
    body.insert("code".into(), serde_json::Value::from(code));
    if let serde_json::Value::Object(members) = extra {
        body.extend(members);
    }
    let Ok(bytes) = crate::admission::encode(&serde_json::Value::Object(body)) else {
        return unavailable();
    };
    let mut response = (status, bytes).into_response();
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

/// The binary's composition of a product router (APP-076 G8): the uniform
/// 404 for unknown paths and for a method the matched route does not serve,
/// the D7.14 handler deadline, and no `Allow` header on any answer. axum's
/// method router adds `Allow` to whatever its method-mismatch fallback
/// returns, after every layer of the inner router, so a wrong method on an
/// existing route would differ from an unknown path by that one header and
/// disclose the route's shape; the outer router removes it, making the two
/// byte-identical (ADR-006; ADR-007 D7.9). No route of this service answers
/// `OPTIONS` or `405`, so no answer needs `Allow`.
pub fn uniform(router: Router) -> Router {
    let inner = router
        .fallback(fallback)
        .method_not_allowed_fallback(fallback)
        .layer(middleware::from_fn(deadline));
    Router::new()
        .fallback_service(inner)
        .layer(middleware::map_response(without_allow))
}
/// `uniform`, plus routes that wait on an external model and so run under
/// `COMPOSE_DEADLINE` instead of the D7.14 handler deadline (APP-093's
/// `POST /v1/compose`; ADR-011 §4 relaxes the deadline for that one route).
/// Those routes keep every other property: a method mismatch on them is the
/// uniform 404 and no answer carries `Allow` (the `long` router answers the
/// whole `long_path` as one service, so its method router's `Allow` is
/// removed by the outer layer like the inner router's).
#[cfg(test)]
pub fn uniform_with(router: Router, long_path: &'static str, long: Router) -> Router {
    uniform_with_many(router, vec![(long_path, long)])
}
/// `uniform_with` for several long-deadline paths (ADR-011 scenarios: the
/// composer's `/v1/compose` and the action path's `/v1/act` both wait on a
/// model). Each path is one service under `long_deadline`.
pub fn uniform_with_many(router: Router, long: Vec<(&'static str, Router)>) -> Router {
    let inner = router
        .fallback(fallback)
        .method_not_allowed_fallback(fallback)
        .layer(middleware::from_fn(deadline));
    let mut outer = Router::new();
    for (path, long) in long {
        let long = long
            .fallback(fallback)
            .method_not_allowed_fallback(fallback)
            .layer(middleware::from_fn(long_deadline));
        outer = outer.route_service(path, long.into_service::<Body>());
    }
    outer
        .fallback_service(inner)
        .layer(middleware::map_response(without_allow))
}
async fn without_allow(mut response: Response) -> Response {
    response.headers_mut().remove(header::ALLOW);
    response
}

/// A path extractor whose every rejection is the uniform 404 (APP-076 G7):
/// a segment that does not percent-decode to UTF-8, or does not deserialize
/// into `T`, names no resource, so a protected route answers it exactly as an
/// unknown path, never with axum's `400 text/plain` (which carries none of
/// the security headers). Every product route with path parameters extracts
/// through this; `GET|POST /signin/{handle}` is anonymous and keeps its
/// neutral sign-in page for an undecodable handle (APP-009 review m2).
pub struct UniformPath<T>(pub T);
impl<T, S> FromRequestParts<S> for UniformPath<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Response> {
        match Path::<T>::from_request_parts(parts, state).await {
            Ok(Path(value)) => Ok(Self(value)),
            Err(_) => Err(not_found()),
        }
    }
}

/// D7.14 handler deadline; a request that outlives it is unavailable.
pub async fn deadline(request: Request, next: Next) -> Response {
    // APP-108: the folder routes that parse, normalize or call a model run
    // under the composer's deadline (their paths carry ids, so they cannot
    // be `uniform_with_many`'s static long paths).
    let budget = if crate::boxes::routes::long_path(request.uri().path()) {
        COMPOSE_DEADLINE
    } else {
        HANDLER_DEADLINE
    };
    tokio::time::timeout(budget, next.run(request))
        .await
        .unwrap_or_else(|_| unavailable())
}
/// The composer's deadline (APP-093): the whole `POST /v1/compose`, model
/// calls included.
pub const COMPOSE_DEADLINE: Duration = crate::composer::COMPOSE_DEADLINE;
async fn long_deadline(request: Request, next: Next) -> Response {
    tokio::time::timeout(COMPOSE_DEADLINE, next.run(request))
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
            assert_eq!(
                body,
                format!("{{\"code\":\"{code}\",\"status\":{status}}}").as_bytes()
            );
        }
        assert_eq!(too_many_requests(7).headers()[header::RETRY_AFTER], "7");
    }

    /// APP-076 G7/G8: an undecodable segment, a wrong method on a mounted
    /// route and an unknown path answer byte-identical uniform 404s, with no
    /// `Allow` and the same header set.
    #[tokio::test]
    async fn method_mismatch_and_undecodable_paths_are_the_uniform_404() {
        use axum::routing::{get, post};
        use tower::ServiceExt as _;
        async fn named(UniformPath((a, b)): UniformPath<(String, String)>) -> String {
            format!("{a}/{b}")
        }
        let app = uniform(
            Router::new()
                .route("/x/{a}/{b}", post(named))
                .route("/y", get(|| async { "y" })),
        );
        let send = |method: &str, uri: &str| {
            let app = app.clone();
            let request = Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap();
            async move {
                let response = app.oneshot(request).await.unwrap();
                let status = response.status();
                let mut headers = response.headers().clone();
                headers.remove(header::DATE);
                let body = to_bytes(response.into_body(), 1024).await.unwrap();
                (status, headers, body)
            }
        };
        let unknown = send("GET", "/nothing-here").await;
        assert_eq!(unknown.0, StatusCode::NOT_FOUND);
        assert_eq!(unknown.2, &b"{\"code\":\"not_found\",\"status\":404}"[..]);
        assert!(unknown.1.get(header::ALLOW).is_none());
        for (method, uri) in [
            ("GET", "/x/a/b"),
            ("PUT", "/x/a/b"),
            ("DELETE", "/y"),
            ("POST", "/y"),
            ("POST", "/x/%FF/b"),
            ("POST", "/x/a/%C3%28"),
        ] {
            assert_eq!(send(method, uri).await, unknown, "{method} {uri}");
        }
        let served = send("POST", "/x/a%20b/c").await;
        assert_eq!((served.0, &served.2[..]), (StatusCode::OK, &b"a b/c"[..]));
        assert_eq!(send("GET", "/y").await.0, StatusCode::OK);
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
