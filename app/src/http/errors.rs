//! APP-217: the themed page a person sees when a browser navigation meets
//! one of the shared refusals. The refusal is decided exactly as before.
//! This pass sits inside `finish`, the outermost layer of `uniform` and
//! `uniform_with_many`. It replaces the body only when the request is a
//! top-level navigation and the answer is a covered `application/problem+json`
//! refusal. The status and every header the refusal carried stay
//! (`no-store`, `nosniff`, `no-referrer`, `Retry-After`). The page adds its
//! `text/html` type and its nonce policy, as every other SSR page does.
//! Anything else passes through untouched. A fetch, an image, a frame, or an
//! API, MCP or Slack caller keeps its JSON byte for byte.
//!
//! ADR-006 uniform refusal: the page reads nothing from the request but two
//! things. One is the closed viewer variant: which session cookie the browser
//! sent, never whether it is live. The other is the refusal's own
//! `Retry-After`. No path, query, id or name reaches it. An unknown path, a
//! protected route the caller cannot see, a wrong method and an undecodable
//! segment therefore stay one answer for the same request headers. A page
//! that cannot be rendered leaves the original refusal, never a partial body.
//!
//! The renderer arrives as an [`ErrorPageLayer`] request extension that the
//! binaries add around the composed router ([`ErrorPageLayer::serve`]). A
//! router without it, such as a test's, answers exactly as before.
use axum::{
    body::{to_bytes, Body, HttpBody as _},
    extract::Request,
    http::{header, response::Parts, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
    Extension, Router,
};
use std::{collections::HashSet, sync::Arc};

/// The largest refusal body read to find its `code`. A covered refusal is a
/// few dozen bytes. A body with no exact length, or a longer one, is not read.
pub const PROBLEM_READ_LIMIT: usize = 4096;

/// The refusals that have a page, with the status and `code` they carry today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// 404 `not_found`, the uniform refusal.
    NotFound,
    /// 403 `cross_site`: fetch metadata or the synchronizer token.
    CrossSite,
    /// 413 `payload_too_large`.
    PayloadTooLarge,
    /// 429 `too_many_requests`, with `Retry-After`.
    TooManyRequests,
    /// 503 `unavailable`.
    Unavailable,
}
impl Refusal {
    const ALL: [Refusal; 5] = [
        Refusal::NotFound,
        Refusal::CrossSite,
        Refusal::PayloadTooLarge,
        Refusal::TooManyRequests,
        Refusal::Unavailable,
    ];
    pub fn status(self) -> StatusCode {
        match self {
            Refusal::NotFound => StatusCode::NOT_FOUND,
            Refusal::CrossSite => StatusCode::FORBIDDEN,
            Refusal::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Refusal::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Refusal::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Refusal::NotFound => "not_found",
            Refusal::CrossSite => "cross_site",
            Refusal::PayloadTooLarge => "payload_too_large",
            Refusal::TooManyRequests => "too_many_requests",
            Refusal::Unavailable => "unavailable",
        }
    }
    fn for_status(status: StatusCode) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|refusal| refusal.status() == status)
    }
}

/// Who is looking, from cookie presence alone: no session is resolved and no
/// store is called. A member's cookie may have expired, and the copy allows
/// for that.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Viewer {
    /// Neither session cookie.
    SignedOut,
    /// The workspace session cookie.
    Member,
    /// Only the shared-app session cookie.
    Guest,
}

/// Everything the page shows. Nothing else reaches the renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorView {
    pub refusal: Refusal,
    /// Only the 404 page varies by viewer. Every other refusal has `None`, so
    /// its page cannot depend on cookies.
    pub viewer: Option<Viewer>,
    /// The 429 wait in plain words ([`wait`]). Empty for every other refusal.
    pub wait: String,
}

/// The renderer: a complete SSR page response for `view` (status 200, its own
/// `text/html` type and nonce policy), or `None` when it cannot render one.
pub trait ErrorPages: Send + Sync {
    fn render(&self, view: &ErrorView) -> Option<Response>;
}

/// The renderer and the viewer test, as the request extension the uniform
/// layer reads.
#[derive(Clone)]
pub struct ErrorPageLayer {
    pages: Arc<dyn ErrorPages>,
    viewer: fn(&HeaderMap) -> Viewer,
}
impl ErrorPageLayer {
    /// `viewer` is `auth::cookies::viewer` in the binaries: only `auth` reads
    /// cookies.
    pub fn new(pages: Arc<dyn ErrorPages>, viewer: fn(&HeaderMap) -> Viewer) -> Self {
        Self { pages, viewer }
    }
    /// `router`, a `uniform` or `uniform_with_many` composition, with these
    /// pages for its browser navigations.
    pub fn serve(self, router: Router) -> Router {
        router.layer(Extension(self))
    }
}

/// The pass `uniform` installs inside `finish`.
pub(crate) async fn error_pages(mut request: Request, next: Next) -> Response {
    // The handlers never see the renderer.
    let layer = request.extensions_mut().remove::<ErrorPageLayer>();
    let Some(layer) = layer.filter(|_| navigation(request.method(), request.headers())) else {
        return next.run(request).await;
    };
    let viewer = (layer.viewer)(request.headers());
    let response = next.run(request).await;
    page_for(response, viewer, layer.pages.as_ref()).await
}

/// A top-level browser navigation. With fetch metadata, `Sec-Fetch-Mode:
/// navigate` and `Sec-Fetch-Dest: document`, each sent exactly once. Without
/// `Sec-Fetch-Mode` (an older browser), a `GET` or `HEAD` that accepts
/// `text/html` and names no destination other than `document`. A fetch, a
/// frame, an image or a duplicated header is not a navigation.
pub fn navigation(method: &Method, headers: &HeaderMap) -> bool {
    let dest = single(headers, "sec-fetch-dest");
    match single(headers, "sec-fetch-mode") {
        Field::One(mode) => mode == b"navigate" && dest == Field::One(b"document"),
        Field::Many => false,
        Field::Absent => {
            matches!(dest, Field::Absent | Field::One(b"document"))
                && (method == Method::GET || method == Method::HEAD)
                && accepts_html(headers)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Field<'a> {
    Absent,
    One(&'a [u8]),
    Many,
}
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Field<'a> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (None, _) => Field::Absent,
        (Some(value), None) => Field::One(value.as_bytes()),
        _ => Field::Many,
    }
}
fn accepts_html(headers: &HeaderMap) -> bool {
    headers.get_all(header::ACCEPT).iter().any(|value| {
        value
            .to_str()
            .is_ok_and(|value| value.to_ascii_lowercase().contains("text/html"))
    })
}

/// The 429 wait in plain words, from the refusal's `Retry-After` seconds.
/// A missing or non-numeric value (an HTTP date) is "a little while".
pub fn wait(retry_after: Option<&HeaderValue>) -> String {
    let Some(seconds) = retry_after
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
    else {
        return "a little while".into();
    };
    let rounded = |unit: u64| seconds.saturating_add(unit / 2) / unit;
    match seconds {
        0..=10 => "a few seconds".into(),
        11..=44 => "less than a minute".into(),
        _ => match (rounded(60), rounded(3600), rounded(86_400)) {
            (1, _, _) => "about a minute".into(),
            (minutes, _, _) if minutes < 60 => format!("about {minutes} minutes"),
            (_, 1, _) => "about an hour".into(),
            (_, hours, _) if hours < 24 => format!("about {hours} hours"),
            (_, _, 1) => "about a day".into(),
            (_, _, days) => format!("about {days} days"),
        },
    }
}

async fn page_for(response: Response, viewer: Viewer, pages: &dyn ErrorPages) -> Response {
    let Some(refusal) = Refusal::for_status(response.status()) else {
        return response;
    };
    if !is_problem(response.headers()) {
        return response;
    }
    if !matches!(response.body().size_hint().exact(), Some(length) if length <= PROBLEM_READ_LIMIT as u64)
    {
        return response;
    }
    let (parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, PROBLEM_READ_LIMIT).await else {
        // A buffered body of known length does not fail to read. If one ever
        // did, its bytes are gone: answer the plain 503, never a partial body.
        return super::unavailable();
    };
    let page = carries(&bytes, refusal)
        .then(|| ErrorView {
            refusal,
            viewer: (refusal == Refusal::NotFound).then_some(viewer),
            wait: if refusal == Refusal::TooManyRequests {
                wait(parts.headers.get(header::RETRY_AFTER))
            } else {
                String::new()
            },
        })
        .and_then(|view| pages.render(&view))
        .filter(is_page);
    match page {
        Some(page) => merge(parts, page),
        None => Response::from_parts(parts, Body::from(bytes)),
    }
}

/// Exactly one `Content-Type: application/problem+json`.
fn is_problem(headers: &HeaderMap) -> bool {
    single(headers, header::CONTENT_TYPE.as_str()) == Field::One(b"application/problem+json")
}

/// The body is exactly `{"code":…,"status":…}` for this refusal. A problem
/// with other members, another code or another status keeps its JSON.
fn carries(bytes: &[u8], refusal: Refusal) -> bool {
    let Ok(serde_json::Value::Object(members)) = serde_json::from_slice(bytes) else {
        return false;
    };
    members.len() == 2
        && members.get("code").and_then(|code| code.as_str()) == Some(refusal.code())
        && members.get("status").and_then(|status| status.as_u64())
            == Some(u64::from(refusal.status().as_u16()))
}

/// A complete page: 200, HTML, and a policy. Anything else is a failed render.
fn is_page(page: &Response) -> bool {
    page.status() == StatusCode::OK
        && page
            .headers()
            .get(header::CONTENT_TYPE)
            .map(HeaderValue::as_bytes)
            == Some(b"text/html; charset=utf-8")
        && page.headers().contains_key(header::CONTENT_SECURITY_POLICY)
}

/// The refusal's status and headers, with the page's body, type and the page
/// headers the refusal does not already set (its CSP, `Permissions-Policy`
/// and opener policy). Where both set one (`Cache-Control`, `nosniff`,
/// `Referrer-Policy`), the refusal's stricter value stays.
fn merge(mut parts: Parts, page: Response) -> Response {
    let (page_parts, body) = page.into_parts();
    parts.headers.remove(header::CONTENT_TYPE);
    parts.headers.remove(header::CONTENT_LENGTH);
    let kept: HashSet<HeaderName> = parts.headers.keys().cloned().collect();
    for (name, value) in &page_parts.headers {
        if !kept.contains(name) {
            parts.headers.append(name.clone(), value.clone());
        }
    }
    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{self, uniform, UniformPath};
    use axum::{
        response::IntoResponse,
        routing::{get, post},
    };
    use tower::ServiceExt as _;

    const NAVIGATE: &[(&str, &str)] = &[
        ("sec-fetch-mode", "navigate"),
        ("sec-fetch-dest", "document"),
        ("sec-fetch-site", "same-origin"),
        ("accept", "text/html,application/xhtml+xml,*/*;q=0.8"),
    ];

    /// A renderer whose page names only what it was given, with the headers
    /// a `PageResponse` writes. It counts its calls.
    #[derive(Default)]
    struct Echo {
        calls: std::sync::atomic::AtomicUsize,
    }
    impl ErrorPages for Echo {
        fn render(&self, view: &ErrorView) -> Option<Response> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut page = format!(
                "<!DOCTYPE html><p>{} {:?} {:?} [{}]</p>",
                view.refusal.status().as_u16(),
                view.refusal,
                view.viewer,
                view.wait
            )
            .into_response();
            let headers = page.headers_mut();
            for (name, value) in [
                ("content-type", "text/html; charset=utf-8"),
                ("cache-control", "no-store"),
                (
                    "content-security-policy",
                    "default-src 'none'; frame-ancestors 'none'",
                ),
                ("x-content-type-options", "nosniff"),
                ("referrer-policy", "strict-origin-when-cross-origin"),
                (
                    "permissions-policy",
                    "geolocation=(), camera=(), microphone=()",
                ),
                ("cross-origin-opener-policy", "same-origin"),
            ] {
                headers.insert(name, HeaderValue::from_static(value));
            }
            Some(page)
        }
    }
    /// A renderer that always fails.
    struct Broken;
    impl ErrorPages for Broken {
        fn render(&self, _: &ErrorView) -> Option<Response> {
            None
        }
    }
    /// A renderer that answers something other than a page (as `respond`'s
    /// 500 would).
    struct NotAPage;
    impl ErrorPages for NotAPage {
        fn render(&self, _: &ErrorView) -> Option<Response> {
            Some((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response())
        }
    }

    /// A one-chunk stream: a body with no exact length.
    struct Once(Option<axum::body::Bytes>);
    impl futures_core::Stream for Once {
        type Item = Result<axum::body::Bytes, std::convert::Infallible>;
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            std::task::Poll::Ready(self.get_mut().0.take().map(Ok))
        }
    }

    fn app(pages: Option<Arc<dyn ErrorPages>>) -> Router {
        async fn segment(UniformPath(id): UniformPath<u32>) -> String {
            id.to_string()
        }
        let router = uniform(
            Router::new()
                .route("/x/{id}", post(segment))
                .route("/forbidden", post(|| async { http::cross_site() }))
                .route("/large", post(|| async { http::payload_too_large() }))
                .route("/busy", get(|| async { http::too_many_requests(300) }))
                .route("/soon", get(|| async { http::too_many_requests(2) }))
                .route("/down", get(|| async { http::unavailable() }))
                .route(
                    "/conflict",
                    get(|| async { http::problem(StatusCode::CONFLICT, "conflict") }),
                )
                .route(
                    "/other-code",
                    get(|| async { http::problem(StatusCode::NOT_FOUND, "missing") }),
                )
                .route(
                    "/extra",
                    get(|| async {
                        http::problem_with(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "unavailable",
                            serde_json::json!({"retry": true}),
                        )
                    }),
                )
                .route("/plain-404", get(|| async { StatusCode::NOT_FOUND }))
                .route(
                    "/json",
                    get(|| async { axum::Json(serde_json::json!({"ok": true})) }),
                )
                .route(
                    "/large-body",
                    get(|| async {
                        let mut response = http::not_found();
                        *response.body_mut() = Body::from(vec![b' '; PROBLEM_READ_LIMIT + 1]);
                        response
                    }),
                )
                .route(
                    "/streamed",
                    get(|| async {
                        let mut response = http::not_found();
                        *response.body_mut() = Body::from_stream(Once(Some(
                            axum::body::Bytes::from_static(br#"{"code":"not_found","status":404}"#),
                        )));
                        response
                    }),
                ),
        );
        match pages {
            Some(pages) => ErrorPageLayer::new(pages, crate::auth::cookies::viewer).serve(router),
            None => router,
        }
    }

    type Answer = (StatusCode, HeaderMap, axum::body::Bytes);
    async fn send(app: &Router, method: &str, uri: &str, headers: &[(&str, &str)]) -> Answer {
        let mut request = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let mut headers = response.headers().clone();
        headers.remove(header::DATE);
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, headers, body)
    }
    fn text(answer: &Answer) -> &str {
        std::str::from_utf8(&answer.2).unwrap()
    }

    #[test]
    fn a_navigation_is_navigate_to_a_document_or_an_old_browser_asking_for_html() {
        let headers = |pairs: &[(&str, &str)]| {
            let mut map = HeaderMap::new();
            for (name, value) in pairs {
                map.append(
                    HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    HeaderValue::from_str(value).unwrap(),
                );
            }
            map
        };
        let html = ("accept", "text/html");
        for (method, pairs, expected) in [
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "navigate"),
                    ("sec-fetch-dest", "document"),
                ],
                true,
            ),
            (
                "POST",
                vec![
                    ("sec-fetch-mode", "navigate"),
                    ("sec-fetch-dest", "document"),
                ],
                true,
            ),
            (
                "GET",
                vec![("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "iframe")],
                false,
            ),
            (
                "GET",
                vec![("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "frame")],
                false,
            ),
            ("GET", vec![("sec-fetch-mode", "navigate")], false),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "cors"),
                    ("sec-fetch-dest", "empty"),
                    html,
                ],
                false,
            ),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "no-cors"),
                    ("sec-fetch-dest", "image"),
                    html,
                ],
                false,
            ),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "same-origin"),
                    ("sec-fetch-dest", "document"),
                ],
                false,
            ),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "Navigate"),
                    ("sec-fetch-dest", "document"),
                ],
                false,
            ),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "navigate"),
                    ("sec-fetch-mode", "navigate"),
                    ("sec-fetch-dest", "document"),
                ],
                false,
            ),
            (
                "GET",
                vec![
                    ("sec-fetch-mode", "navigate"),
                    ("sec-fetch-dest", "document"),
                    ("sec-fetch-dest", "document"),
                ],
                false,
            ),
            // Older browsers: no fetch metadata at all.
            ("GET", vec![html], true),
            ("HEAD", vec![html], true),
            ("GET", vec![("accept", "TEXT/HTML;q=0.9, */*")], true),
            ("GET", vec![("accept", "*/*")], false),
            ("GET", vec![], false),
            ("POST", vec![html], false),
            ("GET", vec![html, ("sec-fetch-dest", "image")], false),
            ("GET", vec![html, ("sec-fetch-dest", "document")], true),
        ] {
            assert_eq!(
                navigation(&method.parse().unwrap(), &headers(&pairs)),
                expected,
                "{method} {pairs:?}"
            );
        }
    }

    #[test]
    fn the_wait_is_plain_words_from_retry_after() {
        let words = |value: &str| wait(Some(&HeaderValue::from_str(value).unwrap()));
        for (value, expected) in [
            ("1", "a few seconds"),
            ("10", "a few seconds"),
            ("11", "less than a minute"),
            ("44", "less than a minute"),
            ("45", "about a minute"),
            ("60", "about a minute"),
            ("89", "about a minute"),
            ("90", "about 2 minutes"),
            ("300", "about 5 minutes"),
            ("3569", "about 59 minutes"),
            ("3600", "about an hour"),
            ("7200", "about 2 hours"),
            ("84599", "about 23 hours"),
            ("86400", "about a day"),
            ("172800", "about 2 days"),
            (" 60 ", "about a minute"),
            ("Wed, 21 Oct 2026 07:28:00 GMT", "a little while"),
            ("-5", "a little while"),
            ("18446744073709551615", "about 213503982334601 days"),
        ] {
            assert_eq!(words(value), expected, "{value}");
        }
        assert_eq!(wait(None), "a little while");
    }

    /// The uniform 404, for every way a path can miss, is one page per
    /// viewer for a navigation, and the exact JSON for everything else.
    #[tokio::test]
    async fn navigations_get_the_page_and_everyone_else_the_exact_refusal() {
        let echo = Arc::new(Echo::default());
        let app = app(Some(echo.clone()));
        let plain = self::app(None);
        let json = br#"{"code":"not_found","status":404}"#;
        for (cookie, viewer) in [
            (None, "Some(SignedOut)"),
            (Some("__Host-genui_session=abc"), "Some(Member)"),
            (Some("__Host-genui_shared=abc"), "Some(Guest)"),
            (
                Some("__Host-genui_shared=abc; __Host-genui_session=def"),
                "Some(Member)",
            ),
            // A duplicated cookie is no cookie at all (auth::cookies).
            (
                Some("__Host-genui_session=a; __Host-genui_session=b"),
                "Some(SignedOut)",
            ),
        ] {
            let mut navigate = NAVIGATE.to_vec();
            navigate.extend(cookie.map(|cookie| ("cookie", cookie)));
            let unknown = send(&app, "GET", "/nothing-here-MARKER?q=MARKER", &navigate).await;
            assert_eq!(unknown.0, StatusCode::NOT_FOUND);
            assert_eq!(
                text(&unknown),
                format!("<!DOCTYPE html><p>404 NotFound {viewer} []</p>")
            );
            assert_eq!(unknown.1[header::CONTENT_TYPE], "text/html; charset=utf-8");
            assert_eq!(unknown.1.get_all(header::CONTENT_TYPE).iter().count(), 1);
            assert!(unknown.1.contains_key(header::CONTENT_SECURITY_POLICY));
            // The refusal's headers stay; `finish` still adds HSTS last.
            assert_eq!(unknown.1[header::CACHE_CONTROL], "no-store");
            assert_eq!(unknown.1[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
            assert_eq!(unknown.1[header::REFERRER_POLICY], "no-referrer");
            assert_eq!(unknown.1.get_all(header::REFERRER_POLICY).iter().count(), 1);
            assert_eq!(
                unknown.1[header::STRICT_TRANSPORT_SECURITY],
                http::STRICT_TRANSPORT_SECURITY
            );
            assert!(unknown.1.get(header::SET_COOKIE).is_none());
            assert!(unknown.1.get(header::ALLOW).is_none());
            for (method, uri) in [
                ("GET", "/elsewhere"),
                ("DELETE", "/x/1"),
                ("GET", "/x/1"),
                ("POST", "/x/%FF"),
                ("POST", "/x/not-a-number"),
                ("PUT", "/down"),
            ] {
                assert_eq!(
                    send(&app, method, uri, &navigate).await,
                    unknown,
                    "{method} {uri}"
                );
            }
            // Not navigations: the exact original bytes and headers.
            let mut fetches = vec![
                vec![("sec-fetch-mode", "cors"), ("sec-fetch-dest", "empty")],
                vec![("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")],
                vec![("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "iframe")],
                vec![("accept", "*/*")],
                vec![],
            ];
            for headers in &mut fetches {
                headers.extend(cookie.map(|cookie| ("cookie", cookie)));
                for (method, uri) in [
                    ("GET", "/elsewhere"),
                    ("DELETE", "/x/1"),
                    ("POST", "/x/%FF"),
                ] {
                    let answer = send(&app, method, uri, headers).await;
                    assert_eq!(answer, send(&plain, method, uri, headers).await);
                    assert_eq!(&answer.2[..], json, "{method} {uri} {headers:?}");
                    assert_eq!(answer.1[header::CONTENT_TYPE], "application/problem+json");
                }
            }
        }
        // An old browser asking for HTML gets the page; one asking for anything gets JSON.
        let old = send(&app, "GET", "/elsewhere", &[("accept", "text/html")]).await;
        assert_eq!(
            text(&old),
            "<!DOCTYPE html><p>404 NotFound Some(SignedOut) []</p>"
        );
        let any = send(&app, "GET", "/elsewhere", &[("accept", "*/*")]).await;
        assert_eq!(&any.2[..], json);
        // The served routes are untouched by the layer.
        let served = send(&app, "POST", "/x/7", NAVIGATE).await;
        assert_eq!((served.0, &served.2[..]), (StatusCode::OK, &b"7"[..]));
        assert_eq!(served, send(&plain, "POST", "/x/7", NAVIGATE).await);
    }

    #[tokio::test]
    async fn the_other_refusals_keep_their_status_and_headers_and_name_no_viewer() {
        let app = app(Some(Arc::new(Echo::default())));
        let member = [NAVIGATE, &[("cookie", "__Host-genui_session=abc")]].concat();
        for (method, uri, status, page) in [
            ("POST", "/forbidden", 403, "403 CrossSite None []"),
            ("POST", "/large", 413, "413 PayloadTooLarge None []"),
            (
                "GET",
                "/busy",
                429,
                "429 TooManyRequests None [about 5 minutes]",
            ),
            (
                "GET",
                "/soon",
                429,
                "429 TooManyRequests None [a few seconds]",
            ),
            ("GET", "/down", 503, "503 Unavailable None []"),
        ] {
            for headers in [NAVIGATE, &member[..]] {
                let answer = send(&app, method, uri, headers).await;
                assert_eq!(answer.0.as_u16(), status, "{uri}");
                assert_eq!(
                    text(&answer),
                    format!("<!DOCTYPE html><p>{page}</p>"),
                    "{uri}"
                );
                assert_eq!(answer.1[header::CONTENT_TYPE], "text/html; charset=utf-8");
                assert_eq!(answer.1[header::CACHE_CONTROL], "no-store");
                assert_eq!(answer.1[header::REFERRER_POLICY], "no-referrer");
            }
        }
        assert_eq!(
            send(&app, "GET", "/busy", NAVIGATE).await.1[header::RETRY_AFTER],
            "300"
        );
        assert_eq!(
            send(&app, "GET", "/soon", NAVIGATE).await.1[header::RETRY_AFTER],
            "2"
        );
    }

    /// Anything the layer does not recognize, and any failed render, is the
    /// original answer byte for byte.
    #[tokio::test]
    async fn unrecognized_answers_and_failed_renders_are_the_original_refusal() {
        let echo = Arc::new(Echo::default());
        let app = app(Some(echo.clone()));
        let plain = self::app(None);
        for uri in [
            "/conflict",
            "/other-code",
            "/extra",
            "/plain-404",
            "/json",
            "/large-body",
            "/streamed",
        ] {
            assert_eq!(
                send(&app, "GET", uri, NAVIGATE).await,
                send(&plain, "GET", uri, NAVIGATE).await,
                "{uri}"
            );
        }
        assert_eq!(echo.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        for pages in [Arc::new(Broken) as Arc<dyn ErrorPages>, Arc::new(NotAPage)] {
            let failing = self::app(Some(pages));
            for (method, uri) in [
                ("GET", "/elsewhere"),
                ("POST", "/x/%FF"),
                ("POST", "/forbidden"),
                ("POST", "/large"),
                ("GET", "/busy"),
                ("GET", "/down"),
            ] {
                let answer = send(&failing, method, uri, NAVIGATE).await;
                assert_eq!(answer, send(&plain, method, uri, NAVIGATE).await, "{uri}");
                assert_eq!(answer.1[header::CONTENT_TYPE], "application/problem+json");
                assert!(answer.2.starts_with(b"{\"code\":"), "{uri}");
            }
        }
    }
}
