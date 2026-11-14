//! APP-074 tests. The unit tests need nothing but the in-process router and
//! the cfg(test) synthetic loopback issuer; `native_invite_campaign`
//! (`#[ignore]`) drives the public invitation entry end to end over the real
//! router against local PostgreSQL 16.13 with verified TLS (the APP-008
//! Docker fixture), the real operator command issuing server-only provider
//! invitations through the loopback management stand-in (`fake_fauth.py`,
//! one per fixture tenant, one shared capability directory), the reviewed
//! verifier and the synthetic issuer (PAR, hosted registration, token).
//! SIMULATED provider; synthetic identifiers only; never hosted acceptance.
use super::{
    synthetic_issuer::{InvitationState, Issuer, ParMode, ISSUER},
    tests::{
        app, config, deliver_auth_configuration, field, full_app, tenants, Browser, Log, Reply,
        TestPages, APP, APP_ORIGIN, CANARY_HANDLE, CANARY_TENANT, NORTHSTAR_HANDLE,
        NORTHSTAR_TENANT, ORG, OTHER, UNKNOWN_HANDLE,
    },
    *,
};
use crate::{
    http::{self, proxy::TrustedProxies},
    storage::{
        admission_qualification::{
            accepted, cli_command, consume_last_slot, lock_wait, operator, operator_with, paused,
            wait, FakeProvider, FakeSecret,
        },
        code_digest,
        qualification::{fixture_python, Fixture},
        FlowKind, Stores,
    },
};
use axum::{
    http::{header, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::{
    io::{Read as _, Write as _},
    net::SocketAddr,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// A well-formed access code that was never issued (base32 of 32 random bytes).
fn unissued_code() -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let bytes = capability::random_bytes::<32>().unwrap();
    let (mut out, mut buffer, mut bits) = (String::new(), 0u32, 0u32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
        buffer &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    let grouped: Vec<String> = out
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8(c.to_vec()).unwrap())
        .collect();
    grouped.join("-")
}

const NEUTRAL: &str = "page=invite;failed=true;limited=false";
/// Review m4: the limited page names its window (this network's minute).
const LIMITED: &str = "page=invite;failed=false;limited=minute";
const LIMITED_SERVICE: &str = "page=invite;failed=false;limited=service";
const ENTRY: &str = "page=invite;failed=false;limited=false";
/// A well-formed organization id that no registry binds.
const UNREGISTERED: &str = "00000000-0000-4000-8000-0000000000ff";
/// The seeded saved work of both fixture organizations (tenant_seed).
const SEEDED_WORK: &str = "00000000-0000-4000-8000-000000000069";
/// The seeded source the seeded work reads, and the seeded owner principal
/// (tenant_seed.py; both fixture organizations hold the same ids).
const SOURCE: &str = "00000000-0000-4000-8000-00000000006e";
const PRINCIPAL: &str = "00000000-0000-4000-8000-000000000066";
/// `upload-sources`' real digests of the fixture source (as the query
/// campaign records them): check 9 registers that revision and binds a new
/// work to it in each organization (`register-source-revision --record-json`,
/// `create-bound-work`), so an admitted member can open the work page and
/// reach the query route; the seeded work itself stays untouched.
const CONTENT: &str = "sha256:6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647";
const NORMALIZED: &str = "sha256:0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082";
const CONTRACT: &str = "sha256:b92d4b96754e5e9de1863dff09e615880ded8d57de22e7546b1f59f2f9178f64";
/// The seeded work's second source input (inventory), read beside SOURCE.
const INVENTORY: &str = "00000000-0000-4000-8000-000000000070";
const BOUND_WORK: &str = "00000000-0000-4000-8000-000000000a74";

/// APP-074 review M3: the protected route families campaign check 9 probes
/// as every principal, admitted members included: (family, method, path,
/// mounted at this tip). Only `/workspace` is mounted here. The hook for
/// APP-012 (plan Task 9: `POST /v1/work/{app}/{work}/queries/{view}` and
/// `GET /w/{app}/{work}`) and APP-013 (the record command routes): the ticket
/// that mounts a route sets its `mounted` to true (and corrects its path to
/// the mounted shape) and re-runs `native_invite_campaign`; check 9 then
/// requires an admitted member to reach the handler while every non-member
/// keeps the byte-identical uniform 404. Item 2 is claimed for a family only
/// after that run passes; an unmounted path that starts answering fails the
/// check until its entry is flipped.
/// Check 9's probe table (family, method, path, mounted at this tip). APP-012
/// mounted `POST /v1/work/{app}/{work}/queries/{view}` and `GET /w/{app}/{work}`;
/// APP-013 mounted `POST /v1/work/{app}/{work}/commands/{command}` (its record
/// page lives at `/work/{app}/{work}/records/{collection}/{record}`, so the
/// `/w/...` record path below stays unmounted). The import, generation and
/// export paths are not mounted.
/// Check 9's probe body: well-formed JSON that no route accepts, so a
/// mounted route answers an admitted member with its own refusal (400/422)
/// once the session gate has passed, while every non-member keeps the
/// uniform 404 before any body is judged.
const PROBE_BODY: &str = "[]";
const ROUTE_FAMILIES: &[(&str, &str, &str, bool)] = &[
    ("workspace", "GET", "/workspace", true),
    ("query", "POST", "/v1/work/{app}/{work}/queries/total", true),
    ("query", "GET", "/w/{app}/{work}", true),
    ("import", "POST", "/v1/imports", false),
    ("import", "POST", "/v1/sources/{work}/revisions", false),
    ("generation", "POST", "/v1/generate", false),
    ("generation", "POST", "/v1/apps/{app}/definitions", false),
    ("export", "GET", "/v1/work/{app}/{work}/export", false),
    (
        "record",
        "POST",
        "/v1/work/{app}/{work}/commands/approve",
        true,
    ),
    ("record", "GET", "/w/{app}/{work}/records/{work}", false),
];

/// `upload-sources`' content-derived revision identity for `organization`
/// (`admission_operator.revision_id`: uuid5 re-stamped as version 4), from
/// the operator's own code.
fn derived_revision(organization: &str) -> String {
    let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(fixture_python())
        .current_dir(app.join("runtime"))
        .args([
            "-c",
            "import sys, admission_operator; print(admission_operator.revision_id(sys.argv[1], sys.argv[2]))",
            organization,
            CONTENT,
        ])
        .output()
        .expect("operator module available");
    assert!(output.status.success(), "revision_id");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn anonymous_app(issuer: &Issuer, proxies: TrustedProxies) -> (Arc<Auth>, Router) {
    let auth = Arc::new(
        Auth::loopback(config(&issuer.origin), None, Arc::new(TestPages))
            .with_trusted_proxies(proxies),
    );
    (auth.clone(), app(auth))
}

/// A capability-shaped cookie (ADR-006 amendment (3): the organization's 16
/// bytes, then 32 random bytes) that the server never issued.
fn forged_cookie(organization: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let hex: String = organization.chars().filter(|c| *c != '-').collect();
    let mut raw: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    raw.extend(capability::random_bytes::<32>().unwrap());
    URL_SAFE_NO_PAD.encode(raw)
}

// --- unit tests (no Docker) ------------------------------------------------

/// APP-074 review M2: `GET /auth/callback` is limited per address class and
/// across the service (sized at the sum of the starts) before the query, the
/// cookie or any store or provider work; the limited answer is the neutral
/// page with 429 and `Retry-After`, and it consumes nothing (the pre-login
/// cookie is kept, so the real return can be retried).
#[tokio::test]
async fn callback_floods_hit_class_and_global_limits_before_any_work() {
    let issuer = Issuer::start(tenants()).await;
    let (auth, app) = anonymous_app(&issuer, TrustedProxies::none());
    let log: Log = Arc::default();
    let neutral_signin = "page=signin;action=-;failed=true";
    let query = |browser: &mut Browser| {
        browser
            .jar
            .insert(cookies::PRELOGIN.into(), forged_cookie(ORG));
        format!(
            "/auth/callback?code={}&state={}",
            capability::random_capability().unwrap(),
            capability::random_capability().unwrap()
        )
    };
    let mut flood = Browser::new("198.51.100.50:5000", &log);
    for i in 0..http::limits::CALLBACK_ADDRESS[0].limit {
        let uri = query(&mut flood);
        let reply = flood.get(&app, &uri).await;
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (StatusCode::BAD_REQUEST, neutral_signin),
            "callback {i}"
        );
    }
    let uri = query(&mut flood);
    let limited = flood.get(&app, &uri).await;
    assert_eq!(
        (limited.status, limited.body.as_str()),
        (StatusCode::TOO_MANY_REQUESTS, neutral_signin),
        "the 16th callback from one class"
    );
    let retry: u64 = limited.headers[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry));
    assert!(
        limited.headers.get(header::SET_COOKIE).is_none(),
        "nothing consumed or cleared"
    );
    // A query or cookie that is not even well formed is limited the same way:
    // the limit precedes parsing.
    let malformed = flood.get(&app, "/auth/callback?code=x").await;
    assert_eq!(malformed.status, StatusCode::TOO_MANY_REQUESTS);
    // Across the service: 800 an hour, whatever the classes.
    auth.limits.clear();
    let mut admitted = 0;
    'classes: for a in 0..=255u16 {
        for b in 0..4u16 {
            let mut browser = Browser::new(&format!("100.64.{a}.{b}:1"), &log);
            for _ in 0..http::limits::CALLBACK_ADDRESS[0].limit {
                if admitted == http::limits::CALLBACK_GLOBAL[0].limit {
                    break 'classes;
                }
                let uri = query(&mut browser);
                assert_eq!(
                    browser.get(&app, &uri).await.status,
                    StatusCode::BAD_REQUEST
                );
                admitted += 1;
            }
        }
    }
    let mut fresh = Browser::new("100.127.9.9:1", &log);
    let uri = query(&mut fresh);
    assert_eq!(
        fresh.get(&app, &uri).await.status,
        StatusCode::TOO_MANY_REQUESTS,
        "the 801st callback of the hour from a fresh class"
    );
    assert_eq!(
        (issuer.count(|c| &c.exchange), issuer.count(|c| &c.par)),
        (0, 0),
        "no provider call"
    );
}

/// APP-074 review M2: forged session and pre-access cookies on `GET
/// /workspace` and `GET /auth/pending` whose organization the closed registry
/// does not bind are the uniform 404 with no lookup, and failed lookups are
/// counted per address class (then 429 for that class only).
#[tokio::test]
async fn forged_cookies_are_refused_before_lookup_and_counted_per_class() {
    let issuer = Issuer::start(tenants()).await;
    let (_auth, app) = anonymous_app(&issuer, TrustedProxies::none());
    let log: Log = Arc::default();
    for (path, name) in [
        ("/workspace", cookies::SESSION),
        ("/auth/pending", cookies::PRE_ACCESS),
    ] {
        let mut forger = Browser::new(
            if path == "/workspace" {
                "198.51.100.60:1"
            } else {
                "198.51.100.61:1"
            },
            &log,
        );
        for i in 0..http::limits::LOOKUP_FAILURE_ADDRESS[0].limit {
            forger.jar.insert(name.into(), forged_cookie(UNREGISTERED));
            assert!(forger.get(&app, path).await.uniform_404(), "{path} {i}");
        }
        forger.jar.insert(name.into(), forged_cookie(UNREGISTERED));
        let limited = forger.get(&app, path).await;
        assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS, "{path}");
        assert!(limited.headers.contains_key(header::RETRY_AFTER));
        // Another class is unaffected; a request without a cookie is never limited.
        let mut other = Browser::new("198.51.100.62:1", &log);
        other.jar.insert(name.into(), forged_cookie(UNREGISTERED));
        assert!(other.get(&app, path).await.uniform_404());
        forger.jar.clear();
        assert!(forger.get(&app, path).await.uniform_404());
    }
}

/// The entry page reads and echoes nothing; every refusal before a code is
/// judged happens without storage, hashing or a provider call: fetch
/// metadata, the 4 KiB body, client-supplied fields, malformed codes; and
/// an unconfigured service refuses every code neutrally.
#[tokio::test]
async fn invite_entry_refuses_before_any_work() {
    let issuer = Issuer::start(tenants()).await;
    let (auth, app) = anonymous_app(&issuer, TrustedProxies::none());
    let log: Log = Arc::default();
    let mut browser = Browser::new("198.51.100.30:4000", &log);
    let entry = browser.get(&app, "/invite").await;
    assert_eq!((entry.status, entry.body.as_str()), (StatusCode::OK, ENTRY));
    let queried = browser
        .get(&app, "/invite?code=AAAA-BBBB&organization=x")
        .await;
    assert_eq!(queried.body, ENTRY, "nothing from a URL is read or shown");
    assert!(browser.get(&app, "/invite/validate").await.uniform_404());
    for headers in [
        &[][..],
        &[("sec-fetch-site", "cross-site")][..],
        &[("sec-fetch-site", "same-site")][..],
        &[("sec-fetch-site", "none")][..],
        &[("origin", "https://evil.example.test")][..],
        &[
            ("sec-fetch-site", "same-origin"),
            ("sec-fetch-site", "same-origin"),
        ][..],
    ] {
        let reply = browser
            .send(
                &app,
                "POST",
                "/invite/validate",
                &format!("code={}", unissued_code()),
                headers,
            )
            .await;
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (
                StatusCode::FORBIDDEN,
                "{\"code\":\"cross_site\",\"status\":403}"
            ),
            "{headers:?}"
        );
    }
    let exact_origin = browser
        .send(
            &app,
            "POST",
            "/invite/validate",
            &format!("code={}", unissued_code()),
            &[("origin", APP_ORIGIN)],
        )
        .await;
    assert_eq!(
        (exact_origin.status, exact_origin.body.as_str()),
        (StatusCode::BAD_REQUEST, NEUTRAL),
        "an exact Origin passes; without storage every code is refused neutrally"
    );
    let oversize = browser
        .post(
            &app,
            "/invite/validate",
            &format!("code={}", "A".repeat(http::FORM_BODY_LIMIT)),
        )
        .await;
    assert_eq!(
        (oversize.status, oversize.body.as_str()),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "{\"code\":\"payload_too_large\",\"status\":413}"
        )
    );
    let code = unissued_code();
    let mut neutral = Vec::new();
    // Trial plan D5: the form is exactly `code` and `consent=1`; a missing,
    // different or repeated consent refuses like any extra field.
    for body in [
        String::new(),
        "code=".into(),
        "code=&consent=1".into(),
        "code=not-a-code&consent=1".into(),
        format!("code={code}"),
        "consent=1".into(),
        format!("code={code}&consent=0"),
        format!("code={code}&consent=on"),
        format!("code={code}&consent=1&consent=1"),
        format!("code={code}&consent=1&organization_id={ORG}"),
        format!("code={code}&consent=1&tenant={NORTHSTAR_TENANT}"),
        format!("code={code}&consent=1&role=admin"),
        format!("code={code}&consent=1&roles=admin"),
        format!("code={code}&consent=1&redirect_uri=https://evil.example.test/cb"),
        format!("code={code}&code={code}&consent=1"),
        format!("invite_token=tinv_x&code={code}&consent=1"),
    ] {
        auth.limits.clear();
        let reply = browser.post(&app, "/invite/validate", &body).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{body}");
        assert!(reply.headers.get(header::SET_COOKIE).is_none());
        neutral.push(reply.body);
    }
    assert!(neutral.iter().all(|b| b == NEUTRAL));
    assert_eq!(issuer.count(|c| &c.par), 0);
}

/// ADR-006 invitation limits and ADR-007 D7.14: 5/minute per address class,
/// then the honest limited page (429, `Retry-After`) with no provider call;
/// a forged `X-Forwarded-For` from an untrusted hop is ignored (rotating it
/// never evades the peer's budget); behind the trusted load balancer the
/// rightmost hop is the client and forged leftmost entries change nothing;
/// 200/day across the service; sign-in floods across many handles hit the
/// same per-class and global windows.
#[tokio::test]
async fn invite_and_signin_floods_hit_class_and_global_limits_without_provider_calls() {
    let issuer = Issuer::start(tenants()).await;
    let alb = TrustedProxies::parse("10.72.0.0/24").unwrap();
    let (auth, app) = anonymous_app(&issuer, alb);
    let log: Log = Arc::default();
    // Untrusted peer, forged and rotating forwarded headers.
    let mut direct = Browser::new("198.51.100.31:4000", &log);
    let mut statuses = Vec::new();
    for i in 0..7 {
        let forged = format!("203.0.113.{i}");
        let reply = direct
            .send(
                &app,
                "POST",
                "/invite/validate",
                &format!("code={}", unissued_code()),
                &[
                    ("sec-fetch-site", "same-origin"),
                    ("x-forwarded-for", &forged),
                ],
            )
            .await;
        statuses.push(reply.status.as_u16());
        if reply.status == StatusCode::TOO_MANY_REQUESTS {
            assert_eq!(reply.body, LIMITED);
            let retry: u64 = reply.headers[header::RETRY_AFTER]
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!((1..=60).contains(&retry));
            assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
        }
    }
    assert_eq!(statuses, [400, 400, 400, 400, 400, 429, 429]);
    // Behind the load balancer: the rightmost hop decides.
    let mut balancer = Browser::new("10.72.0.10:443", &log);
    let via = |client: &'static str, forged: String| {
        let header = format!("{forged}, {client}");
        (header, client)
    };
    let mut alb_statuses = Vec::new();
    for i in 0..6 {
        let (xff, _) = via("192.0.2.77", format!("198.51.100.{i}"));
        let reply = balancer
            .send(
                &app,
                "POST",
                "/invite/validate",
                &format!("code={}", unissued_code()),
                &[("sec-fetch-site", "same-origin"), ("x-forwarded-for", &xff)],
            )
            .await;
        alb_statuses.push(reply.status.as_u16());
    }
    assert_eq!(alb_statuses, [400, 400, 400, 400, 400, 429]);
    let (xff, _) = via("192.0.2.78", "192.0.2.77".into());
    let other = balancer
        .send(
            &app,
            "POST",
            "/invite/validate",
            &format!("code={}", unissued_code()),
            &[("sec-fetch-site", "same-origin"), ("x-forwarded-for", &xff)],
        )
        .await;
    assert_eq!(
        other.status,
        StatusCode::BAD_REQUEST,
        "another real client behind the balancer has its own budget"
    );
    // Global: 200 per day across the service (11 counted above).
    let mut admitted = 11;
    let mut peer = 0u32;
    while admitted < 200 {
        let mut b = Browser::new(&format!("100.64.{}.{}:1", peer / 250, peer % 250), &log);
        for _ in 0..5 {
            if admitted == 200 {
                break;
            }
            let reply = b
                .post(
                    &app,
                    "/invite/validate",
                    &format!("code={}", unissued_code()),
                )
                .await;
            assert_eq!(reply.status, StatusCode::BAD_REQUEST);
            admitted += 1;
        }
        peer += 1;
    }
    let mut fresh = Browser::new("100.127.0.1:1", &log);
    let global = fresh
        .post(
            &app,
            "/invite/validate",
            &format!("code={}", unissued_code()),
        )
        .await;
    assert_eq!(
        (global.status, global.body.as_str()),
        (StatusCode::TOO_MANY_REQUESTS, LIMITED_SERVICE),
        "the 201st attempt of the day is refused from a fresh class, naming the service window"
    );
    // Sign-in: many handles from one class share the per-class window.
    let mut signin = Browser::new("198.51.100.32:4000", &log);
    let mut signin_statuses = Vec::new();
    for i in 0..11 {
        let handle = format!("{}{}", &UNKNOWN_HANDLE[..25], (b'a' + i) as char);
        signin_statuses.push(
            signin
                .post(&app, &format!("/signin/{handle}"), "")
                .await
                .status
                .as_u16(),
        );
    }
    assert_eq!(signin_statuses[..10], [400; 10]);
    assert_eq!(signin_statuses[10], 429);
    // Sign-in global: 600 per hour across the service.
    auth.limits.clear();
    let mut count = 0;
    let mut peer = 0u32;
    while count < 600 {
        let mut b = Browser::new(&format!("100.65.{}.{}:1", peer / 250, peer % 250), &log);
        for _ in 0..10 {
            if count == 600 {
                break;
            }
            let reply = b.post(&app, &format!("/signin/{UNKNOWN_HANDLE}"), "").await;
            assert_eq!(reply.status, StatusCode::BAD_REQUEST);
            count += 1;
        }
        peer += 1;
    }
    let mut fresh = Browser::new("100.126.0.1:1", &log);
    assert_eq!(
        fresh
            .post(&app, &format!("/signin/{UNKNOWN_HANDLE}"), "")
            .await
            .status,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        issuer.count(|c| &c.par),
        0,
        "no denied request reached the provider"
    );
}

/// D7.14 handler deadline over a real socket: a body that never completes is
/// answered with the uniform 503 problem at 5 seconds, not held open.
#[tokio::test]
async fn slow_invitation_body_hits_the_five_second_deadline() {
    let issuer = Issuer::start(tenants()).await;
    let (_, app) = anonymous_app(&issuer, TrustedProxies::none());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let reply = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let started = Instant::now();
        stream
            .write_all(
                b"POST /invite/validate HTTP/1.1\r\nHost: beta.getforma.dev\r\nSec-Fetch-Site: same-origin\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 200\r\n\r\ncode=",
            )
            .unwrap();
        let mut response = Vec::new();
        let mut buffer = [0u8; 1024];
        while !response.windows(4).any(|w| w == b"\r\n\r\n") || !response.ends_with(b"}") {
            let n = stream.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..n]);
        }
        (started.elapsed(), String::from_utf8(response).unwrap())
    })
    .await
    .unwrap();
    let (elapsed, text) = reply;
    assert!(text.starts_with("HTTP/1.1 503"), "{text}");
    assert!(
        text.contains("application/problem+json")
            && text.ends_with("{\"code\":\"unavailable\",\"status\":503}")
    );
    assert!(
        elapsed >= Duration::from_millis(4900) && elapsed < Duration::from_secs(8),
        "{elapsed:?}"
    );
    assert_eq!(issuer.count(|c| &c.par), 0);
}

// --- the Docker campaign -------------------------------------------------------

/// Captured `tracing` output: the safe-log inspection reads what the service
/// actually logged during the campaign.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Capture;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

struct Code {
    id: String,
    raw: String,
    digest: String,
    email: String,
    token: Option<String>,
    invitation: Option<String>,
}

struct Run {
    fixture: Fixture,
    stores: Arc<Stores>,
    issuer: Issuer,
    auth: Arc<Auth>,
    app: Router,
    log: Log,
    checks: Vec<Value>,
    northstar: FakeProvider,
    canary: FakeProvider,
    capabilities: PathBuf,
    issued: usize,
}
impl Run {
    fn browser(&self, peer: &str) -> Browser {
        Browser::new(peer, &self.log)
    }
    fn record(&mut self, name: &str) {
        self.checks.push(json!({"name": name, "passed": true}));
        println!("APP074 check: {name}");
    }
    async fn scalar(&self, sql: &str) -> i64 {
        self.fixture
            .operator()
            .await
            .client
            .query_one(sql, &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    }
    async fn superuser(&self, sql: &str) {
        self.fixture
            .operator()
            .await
            .client
            .batch_execute(sql)
            .await
            .unwrap();
    }
    /// A one-redemption code for `org` (Northstar or Canary), with its
    /// server-only provider invitation when `invited`, issued by the real
    /// operator command; the synthetic issuer learns the same invitation.
    fn issue(&mut self, org: &str, invited: bool) -> Code {
        self.issued += 1;
        let email = format!("member-{}@invite.example.test", self.issued);
        let (provider, tenant, slug) = if org == ORG {
            (&self.northstar, NORTHSTAR_TENANT, "northstar")
        } else {
            (&self.canary, CANARY_TENANT, "canary")
        };
        // APP-076's journey grant set: the app, the bound work check 9 created
        // and the two sources it reads, so an admitted member reaches the
        // mounted query and record routes.
        let grants = [
            format!("discover:{APP}"),
            format!("run:{APP}"),
            format!("record_read:{APP}:{BOUND_WORK}"),
            format!("record_write:{APP}:{BOUND_WORK}"),
            format!("source_read:{SOURCE}"),
            format!("source_read:{INVENTORY}"),
        ];
        let mut args = vec![
            "issue",
            "--organization",
            org,
            "--issued-by",
            "invite-campaign",
        ];
        // The canary organization holds the same seeded app id (tenant_seed).
        for grant in &grants {
            args.push("--grant");
            args.push(grant);
        }
        if invited {
            args.extend([
                "--provider-email",
                &email,
                "--provider-role",
                "user",
                "--provider-issuer",
                ISSUER,
                "--provider-tenant",
                tenant,
            ]);
        }
        let out = operator_with(&self.fixture, provider, &args);
        let raw = out["code"].as_str().unwrap().to_owned();
        let digest = code_digest(&raw).unwrap();
        let (token, invitation) = if invited {
            let invitation = out["provider_invitation"]["invitation_id"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(out["provider_invitation"]["capability_stored"], true);
            let record: Value = serde_json::from_slice(
                &std::fs::read(
                    self.capabilities
                        .join(format!("{org}.{invitation}.capability")),
                )
                .unwrap(),
            )
            .unwrap();
            // The service's file is exactly the operator's secret entry.
            assert_eq!(
                provider.secret.record(org, &invitation),
                Some(record.clone())
            );
            assert_eq!(record["code_digest"], digest.as_str());
            let token = record["invite_token"].as_str().unwrap().to_owned();
            self.issuer.invitation(&token, slug, &email);
            (Some(token), Some(invitation))
        } else {
            (None, None)
        };
        Code {
            id: out["code_id"].as_str().unwrap().to_owned(),
            raw,
            digest,
            email,
            token,
            invitation,
        }
    }
    async fn validate(&self, browser: &mut Browser, raw: &str) -> Reply {
        browser
            .post(
                &self.app,
                "/invite/validate",
                &format!("code={raw}&consent=1"),
            )
            .await
    }
    /// The whole invited signup: validate → hosted registration → callback.
    async fn signup(&self, browser: &mut Browser, code: &Code, subject: &str) -> Reply {
        let entry = self.validate(browser, &code.raw).await;
        assert_eq!(entry.status, StatusCode::SEE_OTHER, "{}", entry.body);
        let query = self
            .issuer
            .register(&entry.location(), subject, &code.email)
            .expect("hosted registration");
        browser
            .get(&self.app, &format!("/auth/callback?{query}"))
            .await
    }
    async fn sign_in(&self, browser: &mut Browser, handle: &str, subject: &str) -> Reply {
        let start = browser
            .post(&self.app, &format!("/signin/{handle}"), "")
            .await;
        assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
        let query = self.issuer.login(&start.location(), subject).unwrap();
        browser
            .get(&self.app, &format!("/auth/callback?{query}"))
            .await
    }
    /// A member through the whole public path: invited signup (pending),
    /// mailbox verification, then the ordinary sign-in that redeems.
    async fn member(&mut self, org: &str, label: &str) -> (String, Browser) {
        let code = self.issue(org, true);
        let subject = format!("usr_00000000-0000-4000-8000-{label:0>12}");
        let mut browser = self.browser(&format!("192.0.2.{}:7000", 100 + self.issued));
        self.auth.limits.clear();
        let pending = self.signup(&mut browser, &code, &subject).await;
        assert_eq!(pending.location(), "/auth/pending");
        self.issuer.set_verified(&subject, true);
        let handle = if org == ORG {
            NORTHSTAR_HANDLE
        } else {
            CANARY_HANDLE
        };
        let arrived = self.sign_in(&mut browser, handle, &subject).await;
        assert_eq!(arrived.location(), "/workspace", "{label}");
        (subject, browser)
    }
    async fn counts(&self) -> (usize, usize, usize, usize, i64, i64) {
        (
            self.issuer.count(|c| &c.par),
            self.issuer.count(|c| &c.par_invited),
            self.issuer.count(|c| &c.exchange),
            self.issuer.count(|c| &c.register),
            self.scalar("SELECT count(*) FROM genui.code_reservations")
                .await,
            self.scalar("SELECT count(*) FROM genui.prelogin_flows")
                .await,
        )
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_invite_campaign() {
    std::thread::Builder::new()
        .name("app074-invite-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn campaign() {
    let capture = Capture::default();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(capture.clone())
            .finish(),
    )
    .expect("the campaign runs alone and captures every log line");
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    // Check 9's data behind the mounted query and record routes: the fixture
    // source revision registered (the managed inline form) and a new work
    // bound to it in each organization, through the operator channel.
    for org in [ORG, OTHER] {
        let revision = derived_revision(org);
        let record = json!({
            "organization_id": org, "source_id": SOURCE, "revision_id": revision,
            "content_digest": CONTENT, "normalized_digest": NORMALIZED, "object_digest": NORMALIZED,
            "row_count": 6, "contract_digest": CONTRACT,
        })
        .to_string();
        let registered = operator(
            &fixture,
            &[
                "register-source-revision",
                "--organization",
                org,
                "--record-json",
                &record,
            ],
        );
        assert_eq!(registered["changed"], true);
        let bound = operator(
            &fixture,
            &[
                "create-bound-work",
                "--organization",
                org,
                "--app",
                APP,
                "--from-work",
                SEEDED_WORK,
                "--work",
                BOUND_WORK,
                "--creator",
                PRINCIPAL,
                "--input",
                "events",
                "--source",
                SOURCE,
                "--revision",
                &revision,
            ],
        );
        assert_eq!(bound["changed"], true);
    }
    // APP-078 B1: the bindings through `bind-provider-tenant`, the routing and
    // registry from the composed AuthConfiguration secret (loopback stand-in),
    // materialized by the supervisor's own writer.
    let issuer = Issuer::start(tenants()).await;
    let (auth_config, registry, _auth_secret) = deliver_auth_configuration(
        &fixture,
        &issuer.origin,
        "00000000-0000-4000-8000-000000000a74",
    );
    // ADR-003 amendment 2026-10-30: one capability secret for the environment
    // (the operator writes it), one service view of it (the supervisor's
    // writer materializes it after each operator command).
    let capabilities = fixture.directory.0.join("capabilities");
    let secret = Arc::new(FakeSecret::start());
    let northstar = FakeProvider::start_at(
        &fixture,
        NORTHSTAR_TENANT,
        "northstar",
        Some(&capabilities),
        Some(&secret),
    );
    let canary = FakeProvider::start_at(
        &fixture,
        CANARY_TENANT,
        "canary",
        Some(&capabilities),
        Some(&secret),
    );
    let stores = Arc::new(Stores::new(fixture.settings_with_registry(registry)).unwrap());
    let auth = Arc::new(
        Auth::loopback(auth_config, Some(stores.clone()), Arc::new(TestPages))
            .with_trusted_proxies(TrustedProxies::parse("10.72.0.0/24").unwrap())
            .with_invitations(Some(invite::Invitations::open(&capabilities).unwrap())),
    );
    let mut c = Run {
        app: full_app(auth.clone()),
        fixture,
        stores,
        issuer,
        auth,
        log: Arc::default(),
        checks: Vec::new(),
        northstar,
        canary,
        capabilities,
        issued: 0,
    };
    c.record("APP-078 B1: both organizations' provider tenant bindings come from `admission_operator.py bind-provider-tenant` (created once, `existing` on repeat, one provider_tenant.bound event on create and none on the repeat), and the provider routing and closed registry come from the composed AuthConfiguration secret value put into the loopback Secrets Manager stand-in and materialized by runtime.write_auth_configuration (the supervisor's own writer) as the two 0600 files that AuthConfig::load and the storage registry read; only the auth origin is the cfg(test) loopback issuer's (plain HTTP on 127.0.0.1) in place of the file's https origin");
    let mut secrets: Vec<String> = Vec::new();

    // --- 1. A valid code: digest validation, backend-only PAR, no slot --------
    let a = c.issue(ORG, true);
    secrets.extend([a.raw.clone(), a.token.clone().unwrap()]);
    let before = c.counts().await;
    let mut maya = c.browser("192.0.2.10:2000");
    let entry = maya.get(&c.app, "/invite").await;
    assert_eq!(entry.body, ENTRY);
    // Lowercase, spaced and dashed input normalizes to the same digest.
    let typed = a.raw.to_lowercase().replace('-', " ");
    let handoff = c.validate(&mut maya, &typed).await;
    assert_eq!(handoff.status, StatusCode::SEE_OTHER, "{}", handoff.body);
    let authorize = url::Url::parse(&handoff.location()).unwrap();
    assert_eq!(authorize.path(), "/t/northstar/auth/authorize");
    let pairs: Vec<(String, String)> = authorize.query_pairs().into_owned().collect();
    assert_eq!(
        pairs.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
        ["client_id", "request_uri"]
    );
    assert_eq!(pairs[0].1, "northstar-platform");
    assert!(pairs[1].1.starts_with("urn:ietf:params:oauth:request_uri:"));
    assert_eq!(handoff.headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(handoff.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        handoff
            .sets(cookies::PRELOGIN)
            .unwrap()
            .split_once("; ")
            .unwrap()
            .1,
        "Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600"
    );
    assert_eq!(handoff.set_cookies().len(), 1, "only the pre-login cookie");
    let after = c.counts().await;
    assert_eq!(
        (after.0, after.1, after.2),
        (before.0 + 1, before.1 + 1, before.2)
    );
    assert_eq!(
        c.issuer.state.presented_invitations.lock().unwrap().last(),
        a.token.as_ref(),
        "the backend PAR carried the capability's invite_token"
    );
    let flow = c.fixture.operator().await.client.query_one(
        "SELECT kind,signup_handoff_id::text,reservation_id IS NOT NULL,expires_at-created_at=interval '10 minutes',client_address_class FROM genui.prelogin_flows",
        &[],
    ).await.unwrap();
    assert_eq!(flow.get::<_, String>(0), "signup");
    assert_eq!(flow.get::<_, Option<String>>(1), a.invitation.clone());
    assert!(flow.get::<_, bool>(2) && flow.get::<_, bool>(3));
    assert_eq!(flow.get::<_, String>(4), "v4:192.0.2.10");
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.code_reservations WHERE code_id='{}' AND kind='presignup' AND state='live' AND expires_at BETWEEN now()+interval '9 minutes 50 seconds' AND now()+interval '10 minutes'", a.id)).await, 1);
    // Trial plan D5: the reservation records the notice version the page
    // showed and the visitor agreed to; the same code without the consent
    // box (or with any other value) is the neutral refusal before any
    // reservation, flow or provider call.
    assert_eq!(c.scalar(&format!("SELECT notice_version::bigint FROM genui.code_reservations WHERE code_id='{}' AND kind='presignup'", a.id)).await, i64::from(notice::NOTICE_VERSION));
    let counted = c.counts().await;
    let mut unconsenting = c.browser("192.0.2.11:2000");
    for body in [
        format!("code={}", a.raw),
        format!("code={}&consent=0", a.raw),
        format!("code={}&consent=on", a.raw),
    ] {
        c.auth.limits.clear();
        let refused = unconsenting.post(&c.app, "/invite/validate", &body).await;
        assert_eq!(
            (refused.status, refused.body.as_str()),
            (StatusCode::BAD_REQUEST, NEUTRAL),
            "{body:.8}"
        );
        assert!(refused.headers.get(header::SET_COOKIE).is_none());
    }
    assert_eq!(
        c.counts().await,
        counted,
        "no reservation, flow or PAR without consent"
    );
    c.record("D5: a valid code is refused without `consent=1` (or with any other consent value) before any reservation, flow or provider call, and the reservation the consented form makes records the notice version it agreed to");
    assert_eq!(
        c.scalar(&format!(
            "SELECT redeemed_count FROM genui.access_codes WHERE code_id='{}' AND status='active'",
            a.id
        ))
        .await,
        0
    );
    let listed = operator(&c.fixture, &["list", "--organization", ORG]);
    let entry = listed["codes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["code_id"] == a.id.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (
            entry["redeemed_count"].as_i64(),
            entry["live_pending_admissions"].as_i64()
        ),
        (Some(0), Some(0))
    );
    c.record("item 1: GET /invite renders the entry; POST /invite/validate validates the code by digest (normalized input) and, without consuming a redemption slot (redeemed_count 0, no pending admission), creates the 10-minute single-use signup handoff (flow kind signup bound to a presignup reservation and the provider invitation id) and performs the backend PAR with the capability's invite_token; the browser receives only a 303 to /t/{slug}/auth/authorize with client_id and request_uri and the __Host- pre-login cookie");

    // --- 2. Hosted registration → pending; the account alone mints nothing ---
    let maya_subject = "usr_00000000-0000-4000-8000-0000000074aa";
    let query = c
        .issuer
        .register(&handoff.location(), maya_subject, &a.email)
        .unwrap();
    assert_eq!(
        c.issuer.invitation_state(a.token.as_ref().unwrap()),
        Some(InvitationState::Accepted)
    );
    let pending = maya.get(&c.app, &format!("/auth/callback?{query}")).await;
    assert_eq!(pending.location(), "/auth/pending");
    for sql in [
        format!("SELECT count(*) FROM genui.principals WHERE provider_subject='{maya_subject}'"),
        format!(
            "SELECT count(*) FROM genui.code_redemptions WHERE code_id='{}'",
            a.id
        ),
        "SELECT count(*) FROM genui.sessions".into(),
    ] {
        assert_eq!(c.scalar(&sql).await, 0, "{sql}");
    }
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.code_reservations WHERE provider_subject='{maya_subject}' AND kind='pending' AND state='live'")).await, 1);
    assert!(maya.get(&c.app, "/workspace").await.uniform_404());
    c.issuer.set_verified(maya_subject, true);
    let arrived = c.sign_in(&mut maya, NORTHSTAR_HANDLE, maya_subject).await;
    assert_eq!(arrived.location(), "/workspace");
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.access_codes WHERE code_id='{}' AND status='exhausted' AND redeemed_count=1", a.id)).await, 1);
    // Trial plan D5: the redemption stored the consent the reservation
    // carried, in the admission transaction, and appended admission.consented
    // right after admission.redeemed with the version's digest.
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.member_consents c JOIN genui.principals p ON p.organization_id=c.organization_id AND p.principal_id=c.principal_id WHERE p.provider_subject='{maya_subject}' AND c.notice_version={}", notice::NOTICE_VERSION)).await, 1);
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.member_consents c JOIN genui.audit_events e ON e.organization_id=c.organization_id AND e.event_id=c.audit_event_id JOIN genui.audit_events r ON r.organization_id=e.organization_id AND r.writer_role='admission' AND r.chain_sequence=e.chain_sequence-1 JOIN genui.principals p ON p.organization_id=c.organization_id AND p.principal_id=c.principal_id WHERE p.provider_subject='{maya_subject}' AND e.kind='admission.consented' AND e.actor_principal_id=c.principal_id AND e.code_id='{}' AND r.kind='admission.redeemed' AND r.actor_principal_id=c.principal_id", a.id)).await, 1);
    c.record("D5: the verified sign-in that redeems also stores the member's consent to the notice version (one member_consents row naming its admission.consented event, the event right after admission.redeemed on the admission chain)");
    c.record("item 2: the hosted registration consumes the bound invitation and the callback lands on /auth/pending with a 24-hour pending admission; the registered account and its pre-access session mint no principal, redemption or workspace session (uniform 404); only the later verified sign-in redeems (code exhausted at 1 of 1)");

    // --- 3. Honest, identical refusals; no slot, no provider call ------------
    let unissued = unissued_code();
    let expired = c.issue(ORG, true);
    c.superuser(&format!("UPDATE genui.access_codes SET expires_at=issued_at+interval '1 millisecond' WHERE code_id='{}'", expired.id)).await;
    let revoked = c.issue(ORG, true);
    operator(
        &c.fixture,
        &["revoke", "--organization", ORG, "--code-id", &revoked.id],
    );
    assert!(
        c.capabilities
            .join(format!(
                "{ORG}.{}.capability",
                revoked.invitation.as_ref().unwrap()
            ))
            .exists()
            && c.northstar
                .secret
                .record(ORG, revoked.invitation.as_ref().unwrap())
                .is_some(),
        "revoked without provider or secret settings: the capability remains in the secret and the service's view, the code still refuses"
    );
    let without_invitation = c.issue(ORG, false);
    secrets.extend([
        expired.raw.clone(),
        revoked.raw.clone(),
        without_invitation.raw.clone(),
    ]);
    let before = c.counts().await;
    let mut probe = c.browser("192.0.2.11:2100");
    let mut bodies = Vec::new();
    for (label, raw) in [
        ("unknown", unissued.as_str()),
        ("malformed", "AAAA-BBBB-CCCC"),
        ("expired", expired.raw.as_str()),
        ("revoked", revoked.raw.as_str()),
        ("exhausted", a.raw.as_str()),
        ("no provider invitation", without_invitation.raw.as_str()),
    ] {
        c.auth.limits.clear();
        let reply = c.validate(&mut probe, raw).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{label}");
        assert!(reply.headers.get(header::SET_COOKIE).is_none(), "{label}");
        bodies.push((reply.body, format!("{:?}", reply.headers)));
    }
    assert!(bodies.windows(2).all(|w| w[0] == w[1]), "{bodies:?}");
    assert_eq!(bodies[0].0, NEUTRAL);
    assert_eq!(
        c.counts().await,
        before,
        "no PAR, reservation or flow for any refused code"
    );
    c.record("item 1 (honest states): unknown, malformed, expired, revoked, exhausted codes and a code without a provider invitation receive the same neutral 400 page, headers and body included (\"could not be used\"), consume nothing and make no provider call; the code is never shown back");

    // --- 4. The limits: honest 429, forged hops, per-code reservation bound --
    c.auth.limits.clear();
    let before = c.counts().await;
    let mut flood = c.browser("198.51.100.40:3000");
    let mut statuses = Vec::new();
    for i in 0..6 {
        let forged = format!("203.0.113.{i}");
        statuses.push(
            flood
                .send(
                    &c.app,
                    "POST",
                    "/invite/validate",
                    &format!("code={}", unissued_code()),
                    &[
                        ("sec-fetch-site", "same-origin"),
                        ("x-forwarded-for", &forged),
                    ],
                )
                .await,
        );
    }
    assert_eq!(
        statuses
            .iter()
            .map(|r| r.status.as_u16())
            .collect::<Vec<_>>(),
        [400, 400, 400, 400, 400, 429]
    );
    assert_eq!(statuses[5].body, LIMITED);
    assert!(statuses[5].headers.contains_key(header::RETRY_AFTER));
    // A valid code is limited too, before it is judged: no PAR, no reservation.
    let valid_late = c.issue(ORG, true);
    secrets.push(valid_late.raw.clone());
    let limited = c.validate(&mut flood, &valid_late.raw).await;
    assert_eq!(
        (limited.status, limited.body.as_str()),
        (StatusCode::TOO_MANY_REQUESTS, LIMITED)
    );
    assert_eq!(c.counts().await, before);
    // Behind the trusted balancer the rightmost hop is the client.
    let mut alb = c.browser("10.72.0.10:443");
    let mut via_alb = Vec::new();
    for i in 0..6 {
        let xff = format!("198.51.100.{i}, 192.0.2.200");
        via_alb.push(
            alb.send(
                &c.app,
                "POST",
                "/invite/validate",
                &format!("code={}", unissued_code()),
                &[("sec-fetch-site", "same-origin"), ("x-forwarded-for", &xff)],
            )
            .await
            .status
            .as_u16(),
        );
    }
    assert_eq!(via_alb, [400, 400, 400, 400, 400, 429]);
    // Per code: at most 10 live presignup reservations (D7.1), then neutral.
    let crowded = c.issue(ORG, true);
    secrets.push(crowded.raw.clone());
    let before = c.counts().await;
    let mut accepted = 0;
    for round in 0..11 {
        c.auth.limits.clear();
        let mut b = c.browser(&format!("192.0.2.{}:2200", 20 + round));
        let reply = c.validate(&mut b, &crowded.raw).await;
        if round < 10 {
            assert_eq!(reply.status, StatusCode::SEE_OTHER, "round {round}");
            accepted += 1;
        } else {
            assert_eq!(
                (reply.status, reply.body.as_str()),
                (StatusCode::BAD_REQUEST, NEUTRAL)
            );
        }
    }
    let after = c.counts().await;
    assert_eq!(accepted, 10);
    assert_eq!(after.1, before.1 + 10, "the eleventh made no PAR");
    assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.code_reservations WHERE code_id='{}' AND kind='presignup' AND state='live'", crowded.id)).await, 10);
    assert_eq!(
        c.scalar(&format!(
            "SELECT redeemed_count FROM genui.access_codes WHERE code_id='{}'",
            crowded.id
        ))
        .await,
        0
    );
    c.record("item 4/5: invitation floods hit 5/minute per address class with the honest limited page (429, Retry-After) before any code is judged, valid codes included; rotating forged X-Forwarded-For from an untrusted peer never evades the peer's budget; behind the trusted load balancer the rightmost hop is the client; one code holds at most 10 live presignup reservations (the eleventh is neutral with no PAR) and none consumes a redemption slot; dispatch counters show no provider call for any denied request");

    // --- 4b. Redemption floods and forged cookies (review M2) -----------------
    c.auth.limits.clear();
    let before = c.counts().await;
    let exchanges = c.issuer.count(|x| &x.exchange);
    let callback_uri = || {
        format!(
            "/auth/callback?code={}&state={}",
            capability::random_capability().unwrap(),
            capability::random_capability().unwrap()
        )
    };
    // The database paused: a forged cookie of an organization no registry
    // binds is refused before any database work, so each answers at once.
    let docker = |verb: &str| {
        let status = std::process::Command::new("docker")
            .args([verb, &c.fixture.name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "docker {verb}");
    };
    docker("pause");
    let mut paused = Vec::new();
    for (path, name) in [
        (callback_uri(), cookies::PRELOGIN),
        ("/workspace".to_owned(), cookies::SESSION),
        ("/auth/pending".to_owned(), cookies::PRE_ACCESS),
    ] {
        let mut b = c.browser("198.51.100.70:7000");
        b.jar.insert(name.into(), forged_cookie(UNREGISTERED));
        let reply = tokio::time::timeout(Duration::from_secs(2), b.get(&c.app, &path)).await;
        paused.push((path, reply));
    }
    docker("unpause");
    for (path, reply) in paused {
        let reply = reply.unwrap_or_else(|_| panic!("{path}: waited on the paused database"));
        if path.starts_with("/auth/callback") {
            assert_eq!(
                (reply.status, reply.body.as_str()),
                (StatusCode::BAD_REQUEST, "page=signin;action=-;failed=true")
            );
        } else {
            assert!(reply.uniform_404(), "{path}");
        }
    }
    // A registered organization's forged pre-login cookie still costs one
    // flow lookup, at most 15 a minute from one class: the 16th is 429 with
    // `Retry-After` before any work.
    let mut flood = c.browser("198.51.100.71:7100");
    let mut statuses = Vec::new();
    for _ in 0..=http::limits::CALLBACK_ADDRESS[0].limit {
        flood
            .jar
            .insert(cookies::PRELOGIN.into(), forged_cookie(ORG));
        let reply = flood.get(&c.app, &callback_uri()).await;
        statuses.push(reply.status.as_u16());
        if reply.status == StatusCode::TOO_MANY_REQUESTS {
            assert!(reply.headers.contains_key(header::RETRY_AFTER));
        }
    }
    assert_eq!(statuses.iter().filter(|s| **s == 400).count(), 15);
    assert_eq!(statuses.last(), Some(&429));
    assert_eq!(c.counts().await, before);
    assert_eq!(c.issuer.count(|x| &x.exchange), exchanges);
    c.auth.limits.clear();
    c.record("item 4 (redemption, review M2): GET /auth/callback is limited per address class (15/minute, 80/hour) and across the service (800/hour), sized at the sum of the sign-in and invitation starts, before the query, the cookie or any database work (the 16th forged callback from one class is 429 with Retry-After and the neutral page); with the database paused, forged pre-login, session and pre-access cookies of an organization the closed registry does not bind answer at once (neutral page, uniform 404) because nothing reaches the store; no flood consumed a slot, created a record or reached the token endpoint");

    // --- 5. Provider invitation mismatches cannot create an account ----------
    c.auth.limits.clear();
    let b_code = c.issue(ORG, true);
    secrets.extend([b_code.raw.clone(), b_code.token.clone().unwrap()]);
    let mut leak = c.browser("192.0.2.50:2500");
    let start = c.validate(&mut leak, &b_code.raw).await;
    let refused_before = c.issuer.count(|x| &x.register_refused);
    // Wrong email on the invited request (a leaked code without the mailbox).
    assert!(c
        .issuer
        .register(
            &start.location(),
            "usr_00000000-0000-4000-8000-0000000074bb",
            "attacker@example.test"
        )
        .is_none());
    // A sign-in request (no invitation) cannot register: no generic signup.
    let mut direct = c.browser("192.0.2.51:2501");
    let plain = direct
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    assert!(c
        .issuer
        .register(
            &plain.location(),
            "usr_00000000-0000-4000-8000-0000000074bc",
            &b_code.email
        )
        .is_none());
    // A Northstar invitation on a Canary request (another tenant) cannot register.
    let canary_code = c.issue(OTHER, true);
    secrets.extend([canary_code.raw.clone(), canary_code.token.clone().unwrap()]);
    let mut cross = c.browser("192.0.2.52:2502");
    let canary_start = c.validate(&mut cross, &canary_code.raw).await;
    assert_eq!(
        url::Url::parse(&canary_start.location()).unwrap().path(),
        "/t/canary/auth/authorize"
    );
    assert!(c
        .issuer
        .register(
            &canary_start.location(),
            "usr_00000000-0000-4000-8000-0000000074bd",
            &b_code.email
        )
        .is_none());
    assert_eq!(c.issuer.count(|x| &x.register_refused), refused_before + 3);
    // Review m1(a): an existing identity signing in on another email's
    // invited request is refused, as fAuth's hosted sign-in is; the request
    // and the invitation stay as they were.
    let login_refused = c.issuer.count(|x| &x.login_refused);
    assert!(c.issuer.login(&start.location(), maya_subject).is_none());
    assert_eq!(c.issuer.count(|x| &x.login_refused), login_refused + 1);
    // Review m1(b): an existing account for the invited email is fAuth's 409:
    // refused without consuming the invitation or the request.
    c.issuer.user(
        "usr_00000000-0000-4000-8000-0000000074be",
        &b_code.email,
        false,
    );
    assert!(c
        .issuer
        .register(
            &start.location(),
            "usr_00000000-0000-4000-8000-0000000074bf",
            &b_code.email
        )
        .is_none());
    assert_eq!(c.issuer.count(|x| &x.register_refused), refused_before + 4);
    assert_eq!(
        c.issuer.invitation_state(b_code.token.as_ref().unwrap()),
        Some(InvitationState::Pending)
    );
    // Revoked or consumed at the provider: the PAR is refused, the flow discarded.
    let flows = c.scalar("SELECT count(*) FROM genui.prelogin_flows").await;
    c.issuer.revoke_invitation(b_code.token.as_ref().unwrap());
    let par_refused = c.issuer.count(|x| &x.par_refused);
    let mut late = c.browser("192.0.2.53:2503");
    let refused = c.validate(&mut late, &b_code.raw).await;
    assert_eq!(
        (refused.status, refused.body.as_str()),
        (StatusCode::BAD_REQUEST, NEUTRAL)
    );
    assert_eq!(c.issuer.count(|x| &x.par_refused), par_refused + 1);
    assert_eq!(
        c.scalar("SELECT count(*) FROM genui.prelogin_flows").await,
        flows
    );
    c.auth.limits.clear();
    let replayed = c.validate(&mut late, &a.raw).await;
    assert_eq!(replayed.body, NEUTRAL, "a used code replays nothing");
    assert_eq!(c.scalar("SELECT count(*) FROM genui.principals WHERE provider_subject LIKE 'usr_00000000-0000-4000-8000-0000000074b%'").await, 0);
    c.record("item 2: missing, wrong-email and other-tenant provider invitations cannot create an account through the hosted registration (refused, invitation untouched), an existing account for the invited email is refused (fAuth's 409) and an existing identity's sign-in on another email's invited request is refused, both with the invitation still pending (review m1), a sign-in request offers no generic signup, and a provider-revoked or already-consumed invitation is refused at the backend PAR (neutral page, flow discarded); a leaked or replayed used code obtains nothing");

    // --- 6. Wrong, replayed, expired and cross-flow state ---------------------
    c.auth.limits.clear();
    let d = c.issue(ORG, true);
    secrets.push(d.raw.clone());
    let mut dee = c.browser("192.0.2.60:2600");
    let d_start = c.validate(&mut dee, &d.raw).await;
    let d_query = c
        .issuer
        .register(
            &d_start.location(),
            "usr_00000000-0000-4000-8000-0000000074dd",
            &d.email,
        )
        .unwrap();
    let (code_part, _) = d_query.split_once('&').unwrap();
    let exchanges = c.issuer.count(|x| &x.exchange);
    let wrong = dee
        .get(
            &c.app,
            &format!(
                "/auth/callback?{code_part}&state={}",
                capability::random_capability().unwrap()
            ),
        )
        .await;
    assert_eq!(
        (wrong.status, field(&wrong.body, "page")),
        (StatusCode::BAD_REQUEST, "signin".into())
    );
    let replay = dee.get(&c.app, &format!("/auth/callback?{d_query}")).await;
    assert_eq!(
        replay.status,
        StatusCode::BAD_REQUEST,
        "the mismatched state consumed the flow"
    );
    assert_eq!(
        c.issuer.count(|x| &x.exchange),
        exchanges,
        "neither reached the token endpoint"
    );
    // Cross-flow injection: a Canary code/state presented to a Northstar flow's cookie.
    c.auth.limits.clear();
    let e = c.issue(ORG, true);
    let f = c.issue(OTHER, true);
    secrets.extend([e.raw.clone(), f.raw.clone()]);
    let mut north = c.browser("192.0.2.61:2601");
    let mut south = c.browser("192.0.2.62:2602");
    let _ = c.validate(&mut north, &e.raw).await;
    let f_start = c.validate(&mut south, &f.raw).await;
    let f_query = c
        .issuer
        .register(
            &f_start.location(),
            "usr_00000000-0000-4000-8000-0000000074ff",
            &f.email,
        )
        .unwrap();
    let injected = north
        .get(&c.app, &format!("/auth/callback?{f_query}"))
        .await;
    assert_eq!(injected.status, StatusCode::BAD_REQUEST);
    assert_eq!(c.scalar("SELECT count(*) FROM genui.pre_access_sessions WHERE provider_subject='usr_00000000-0000-4000-8000-0000000074ff'").await, 0);
    // An expired handoff (pre-login flow past 10 minutes) admits nothing.
    c.auth.limits.clear();
    let g = c.issue(ORG, true);
    secrets.push(g.raw.clone());
    let mut gus = c.browser("192.0.2.63:2603");
    let g_start = c.validate(&mut gus, &g.raw).await;
    let g_query = c
        .issuer
        .register(
            &g_start.location(),
            "usr_00000000-0000-4000-8000-00000000746a",
            &g.email,
        )
        .unwrap();
    c.superuser("UPDATE genui.prelogin_flows SET created_at=now()-interval '11 minutes',expires_at=now()-interval '1 minute'").await;
    let lapsed = gus.get(&c.app, &format!("/auth/callback?{g_query}")).await;
    assert_eq!(lapsed.status, StatusCode::BAD_REQUEST);
    assert_eq!(c.scalar("SELECT count(*) FROM genui.pre_access_sessions WHERE provider_subject='usr_00000000-0000-4000-8000-00000000746a'").await, 0);
    c.record("item 3: a wrong state consumes the signup flow and a replay of the right one then finds nothing (no token request); a Canary code and state injected into a Northstar flow's browser are refused (no pre-access); a signup handoff past its 10 minutes admits nothing");

    // --- 7. Client-supplied authority and cross-site redemption ----------------
    c.auth.limits.clear();
    let h = c.issue(ORG, true);
    secrets.push(h.raw.clone());
    let before = c.counts().await;
    let mut hal = c.browser("192.0.2.64:2604");
    for extra in [
        format!("organization_id={OTHER}"),
        format!("tenant={CANARY_TENANT}"),
        "role=admin".into(),
        "roles=admin,reviewer".into(),
        "redirect_uri=https://evil.example.test/auth/callback".into(),
        "return_to=/workspace".into(),
        format!("invite_token={}", h.token.as_ref().unwrap()),
    ] {
        c.auth.limits.clear();
        let reply = hal
            .post(
                &c.app,
                "/invite/validate",
                &format!("code={}&consent=1&{extra}", h.raw),
            )
            .await;
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (StatusCode::BAD_REQUEST, NEUTRAL),
            "{extra}"
        );
    }
    c.auth.limits.clear();
    for headers in [
        &[("sec-fetch-site", "cross-site")][..],
        &[("origin", "https://evil.example.test")][..],
        &[][..],
    ] {
        let reply = hal
            .send(
                &c.app,
                "POST",
                "/invite/validate",
                &format!("code={}&consent=1", h.raw),
                headers,
            )
            .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN);
    }
    assert_eq!(c.counts().await, before, "no reservation, flow or PAR");
    // The storage boundary: a Canary code cannot reserve under Northstar's scope.
    let pkce = pkce::Pkce::fresh().unwrap();
    let sealed = c
        .auth
        .sealer
        .seal(&routes::verifier_context(ORG), pkce.verifier.as_bytes())
        .unwrap();
    let crossed = c
        .stores
        .workspace
        .begin_flow(
            ORG,
            FlowKind::SignUp {
                code_digest: &f.digest,
                handoff: None,
                notice_version: notice::NOTICE_VERSION,
            },
            &sealed,
            "v4:192.0.2.64",
        )
        .await
        .unwrap();
    assert!(crossed.is_none());
    assert!(c
        .auth
        .invitations
        .as_ref()
        .unwrap()
        .find(ORG, &f.digest)
        .await
        .is_none());
    assert!(c
        .auth
        .invitations
        .as_ref()
        .unwrap()
        .find(OTHER, &f.digest)
        .await
        .is_some());
    assert_eq!(c.counts().await, before);
    c.record("item 3: client-provided organization, tenant, role(s), redirect, destination and invite_token fields refuse with the neutral page; cross-site, foreign-Origin and metadata-less posts are 403 cross_site; none reserves, starts a flow or calls the provider; under Northstar's scope a Canary code reserves nothing and its capability is not found");

    // --- 8. Two isolated trial organizations -----------------------------------
    let (north_member, mut north_browser) = c.member(ORG, "7401").await;
    let (south_member, mut south_browser) = c.member(OTHER, "7402").await;
    for (subject, org) in [(&north_member, ORG), (&south_member, OTHER)] {
        assert_eq!(c.scalar(&format!("SELECT count(*) FROM genui.principals WHERE provider_subject='{subject}' AND organization_id='{org}'")).await, 1);
        assert_eq!(
            c.scalar(&format!(
                "SELECT count(*) FROM genui.principals WHERE provider_subject='{subject}'"
            ))
            .await,
            1
        );
    }
    assert_eq!(
        field(&north_browser.get(&c.app, "/workspace").await.body, "page"),
        "workspace"
    );
    assert_eq!(
        field(&south_browser.get(&c.app, "/workspace").await.body, "page"),
        "workspace"
    );
    // A Canary member's verified identity signing in at Northstar is denied.
    let mut stray = c.browser("192.0.2.70:2700");
    let start = stray
        .post(&c.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    let q = c.issuer.login(&start.location(), &south_member).unwrap();
    let denied = stray.get(&c.app, &format!("/auth/callback?{q}")).await;
    assert_eq!(field(&denied.body, "page"), "denied");
    c.record("item 2/3: an operator-preprovisioned Canary code hands off to the Canary tenant only and its member joins only Canary; the Northstar member only Northstar; each workspace opens for its own member; a Canary identity at Northstar's sign-in is denied with nothing minted");

    // --- 9. Protected route families refuse every non-member identically -----
    let session_of = |b: &Browser| b.cookie(cookies::SESSION).unwrap();
    let north_cookie = session_of(&north_browser);
    let (expired_member, expired_browser) = c.member(ORG, "7403").await;
    let expired_cookie = session_of(&expired_browser);
    let expired_principal: String = c
        .fixture
        .operator()
        .await
        .client
        .query_one(
            "SELECT principal_id::text FROM genui.principals WHERE provider_subject=$1",
            &[&expired_member],
        )
        .await
        .unwrap()
        .get(0);
    c.superuser(&format!("UPDATE genui.sessions SET created_at=now()-interval '2 hours',last_seen_at=now()-interval '61 minutes',absolute_expires_at=now()+interval '9 hours' WHERE principal_id='{expired_principal}'")).await;
    let (revoked_member, revoked_browser) = c.member(ORG, "7404").await;
    let revoked_cookie = session_of(&revoked_browser);
    let revoked_principal: String = c
        .fixture
        .operator()
        .await
        .client
        .query_one(
            "SELECT principal_id::text FROM genui.principals WHERE provider_subject=$1",
            &[&revoked_member],
        )
        .await
        .unwrap()
        .get(0);
    operator(
        &c.fixture,
        &[
            "revoke-entitlement",
            "--organization",
            ORG,
            "--principal",
            &revoked_principal,
        ],
    );
    // A pre-access identity (registered, unverified).
    c.auth.limits.clear();
    let p = c.issue(ORG, true);
    secrets.push(p.raw.clone());
    let mut pre = c.browser("192.0.2.71:2701");
    let reply = c
        .signup(&mut pre, &p, "usr_00000000-0000-4000-8000-0000000074ee")
        .await;
    assert_eq!(reply.location(), "/auth/pending");
    let pre_cookie = pre.cookie(cookies::PRE_ACCESS).unwrap();
    // A Northstar session value re-scoped under Canary's prefix (foreign).
    let raw = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        &north_cookie,
    )
    .unwrap();
    let mut foreign = raw.clone();
    foreign[..16].copy_from_slice(&raw_uuid(OTHER));
    let foreign_cookie =
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, foreign);
    let principals: Vec<(&str, Vec<(String, String)>)> = vec![
        ("anonymous", vec![]),
        (
            "malformed cookie",
            vec![(cookies::SESSION.into(), "A".repeat(64))],
        ),
        (
            "pre-access",
            vec![(cookies::PRE_ACCESS.into(), pre_cookie.clone())],
        ),
        (
            "pre-access as session",
            vec![(cookies::SESSION.into(), pre_cookie.clone())],
        ),
        (
            "expired session",
            vec![(cookies::SESSION.into(), expired_cookie)],
        ),
        (
            "revoked entitlement",
            vec![(cookies::SESSION.into(), revoked_cookie)],
        ),
        (
            "foreign-scoped session",
            vec![(cookies::SESSION.into(), foreign_cookie)],
        ),
    ];
    // APP-074 review M3: every family is probed as every principal, admitted
    // members included, and the evidence says which routes this tip mounts.
    let families: Vec<(&str, &str, String, bool)> = ROUTE_FAMILIES
        .iter()
        .map(|(family, method, path, mounted)| {
            (
                *family,
                *method,
                path.replace("{app}", APP).replace("{work}", BOUND_WORK),
                *mounted,
            )
        })
        .collect();
    let dispatch = (
        c.issuer.count(|x| &x.par),
        c.issuer.count(|x| &x.exchange),
        c.issuer.count(|x| &x.register),
        c.issuer.count(|x| &x.logout),
    );
    let mut refusals = Vec::new();
    for (who, jar) in &principals {
        for (family, method, path, _) in &families {
            let mut b = c.browser("192.0.2.80:2800");
            b.jar = jar.iter().cloned().collect();
            let reply = b
                .send(
                    &c.app,
                    method,
                    path,
                    PROBE_BODY,
                    &[("sec-fetch-site", "same-origin")],
                )
                .await;
            assert!(
                reply.uniform_404(),
                "{who} {family} {method} {path}: {} {}",
                reply.status,
                reply.body
            );
            refusals.push(format!("{who}|{family}|{method}|{path}"));
        }
    }
    // Admitted members, over every family: a mounted route reaches its
    // handler (PROBE_BODY is well-formed JSON no route accepts, so a member
    // gets the handler's own shape refusal after the session gate, without
    // any fixture data behind the route); an unmounted path answers exactly
    // the router's fallback (the same bytes as a path no ticket names), so
    // nothing unmounted can be counted as implemented. When APP-012/013 mount a route, its entry in
    // ROUTE_FAMILIES flips to mounted and this loop then requires a member to
    // reach it while every non-member above keeps the uniform 404.
    let mut member_probes = 0;
    for (who, cookie) in [
        ("northstar member", north_cookie.clone()),
        ("canary member", session_of(&south_browser)),
    ] {
        let mut b = c.browser("192.0.2.81:2801");
        b.jar.insert(cookies::SESSION.into(), cookie.clone());
        let fallback = b
            .send(
                &c.app,
                "POST",
                "/v1/no-ticket-names-this-path",
                PROBE_BODY,
                &[("sec-fetch-site", "same-origin")],
            )
            .await;
        assert!(fallback.uniform_404(), "{who}: the router fallback");
        for (family, method, path, mounted) in &families {
            let mut b = c.browser("192.0.2.81:2801");
            b.jar.insert(cookies::SESSION.into(), cookie.clone());
            let reply = b
                .send(
                    &c.app,
                    method,
                    path,
                    PROBE_BODY,
                    &[("sec-fetch-site", "same-origin")],
                )
                .await;
            member_probes += 1;
            if *mounted {
                assert!(
                    !reply.uniform_404(),
                    "{who} {family} {method} {path}: a mounted route must reach its handler ({} {})",
                    reply.status,
                    reply.body
                );
            } else {
                assert!(
                    reply.uniform_404()
                        && reply.body == fallback.body
                        && reply.headers == fallback.headers,
                    "{who} {family} {method} {path} answered {} {}: it is mounted at this tip, so set its ROUTE_FAMILIES entry to mounted and re-run the seven-principal probe before item 2 is claimed for {family}",
                    reply.status,
                    reply.body
                );
            }
        }
    }
    // Review m8: the probes dispatched no PAR, exchange or registration; the
    // expired and the entitlement-revoked sessions' destruction revoked each
    // one's refresh token once (provider logout), as ADR-006 requires.
    assert_eq!(
        (
            c.issuer.count(|x| &x.par),
            c.issuer.count(|x| &x.exchange),
            c.issuer.count(|x| &x.register),
            c.issuer.count(|x| &x.logout)
        ),
        (dispatch.0, dispatch.1, dispatch.2, dispatch.3 + 2),
        "no protected-route refusal dispatched provider work beyond the two ended sessions' revocations"
    );
    let pre_rows = c.scalar("SELECT count(*) FROM genui.principals WHERE provider_subject='usr_00000000-0000-4000-8000-0000000074ee'").await;
    assert_eq!(pre_rows, 0);
    let mounted: Vec<&str> = families
        .iter()
        .filter(|f| f.3)
        .map(|f| f.2.as_str())
        .collect();
    let unmounted: Vec<&str> = families
        .iter()
        .filter(|f| !f.3)
        .map(|f| f.0)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    c.record(&format!("item 2/5: mounted protected routes at this tip: {mounted:?}; the {unmounted:?} paths are not mounted and answer the router's uniform fallback 404 to every principal, admitted members included, so none is counted as implemented. {} probes by anonymous, malformed, pre-access (also presented as a session), expired, entitlement-revoked and foreign-scoped principals all answer the byte-identical uniform 404 (problem+json, no-store, nosniff, no Set-Cookie); {member_probes} member probes over all {} paths: the mounted route reaches its handler and every unmounted path is byte-identical to the fallback; no PAR, exchange or registration was dispatched, and the expired and entitlement-revoked sessions' destruction revoked each refresh token once (ADR-006); a registered pre-access identity holds no principal. APP-012/APP-013 re-run this check with their routes marked mounted before item 2 is claimed for the query or record family", refusals.len(), families.len()));

    // --- 10. Provider faults at the PAR ---------------------------------------
    for mode in [ParMode::Unavailable, ParMode::Timeout] {
        c.auth.limits.clear();
        let k = c.issue(ORG, true);
        secrets.push(k.raw.clone());
        let flows = c.scalar("SELECT count(*) FROM genui.prelogin_flows").await;
        c.issuer.par_mode(mode);
        let started = Instant::now();
        let mut b = c.browser("192.0.2.90:2900");
        let reply = c.validate(&mut b, &k.raw).await;
        c.issuer.par_mode(ParMode::Normal);
        assert_eq!(
            (reply.status, reply.body.as_str()),
            (StatusCode::BAD_REQUEST, NEUTRAL),
            "{mode:?}"
        );
        assert!(
            started.elapsed() < http::HANDLER_DEADLINE,
            "{mode:?} {:?}",
            started.elapsed()
        );
        assert_eq!(
            c.scalar("SELECT count(*) FROM genui.prelogin_flows").await,
            flows,
            "{mode:?}: flow discarded"
        );
        assert_eq!(
            c.scalar(&format!(
                "SELECT redeemed_count FROM genui.access_codes WHERE code_id='{}'",
                k.id
            ))
            .await,
            0
        );
    }
    c.record("provider faults at the backend PAR (503, and an answer past the 3-second client deadline) give the neutral page inside the 5-second handler deadline, discard the flow and consume no slot; the presignup reservation simply lapses in 10 minutes (the create/revoke timeout and reconciliation exercise is auth::invite_tests::native_invite_race_campaign)");

    // --- 11. No code, invitation token or bearer credential leaks -----------
    let replies = c.log.lock().unwrap().clone();
    let issued = c.issuer.state.issued.lock().unwrap().clone();
    let presented = c.issuer.state.presented_invitations.lock().unwrap().clone();
    // The observer itself: a negative control with a deliberately planted
    // raw code, invitation token and issued capability must be detected.
    let leaked = |text: &str| {
        secrets
            .iter()
            .chain(issued.iter())
            .chain(presented.iter())
            .any(|secret| text.contains(secret.as_str()))
    };
    assert!(
        leaked(&format!("<p>{}</p>", secrets[0])),
        "negative control: raw code"
    );
    assert!(
        leaked(&format!("invite_token={}", presented[0])),
        "negative control: invite_token"
    );
    assert!(
        leaked(&format!("Location: /x?code={}", issued[0])),
        "negative control: issued code"
    );
    assert!(!leaked("page=invite;failed=true;limited=false"));
    let mut scanned = 0;
    for reply in &replies {
        let mut text = reply.body.clone();
        for (name, value) in reply.headers.iter() {
            if name == header::LOCATION && value.to_str().unwrap().contains("/auth/authorize?") {
                continue; // the bounded request_uri is the one permitted front-channel value
            }
            text.push_str(value.to_str().unwrap_or(""));
        }
        for secret in secrets.iter().chain(issued.iter()).chain(presented.iter()) {
            assert!(
                !text.contains(secret.as_str()),
                "a response carried a secret"
            );
            let lower = secret.to_lowercase();
            assert!(!text.to_lowercase().contains(&lower) || secret.len() < 8);
        }
        assert!(
            !text.contains("access_token")
                && !text.contains("refresh_token")
                && !text.contains("invite_token")
        );
        scanned += 1;
    }
    let dump = {
        let output = std::process::Command::new("docker")
            .args([
                "exec",
                &c.fixture.name,
                "pg_dump",
                "--username=postgres",
                "--dbname=genui",
                "--data-only",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    for secret in secrets.iter().chain(presented.iter()) {
        assert!(
            !dump.contains(secret.as_str()),
            "storage holds a raw code or invitation token"
        );
        assert!(!dump.contains(&secret.replace('-', "")));
    }
    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("invite.validate")
            && logs.contains("outcome=\"handoff\"")
            && logs.contains("outcome=\"limited\"")
    );
    for secret in secrets.iter().chain(issued.iter()).chain(presented.iter()) {
        assert!(
            !logs.contains(secret.as_str()),
            "a log line carried a secret"
        );
    }
    for (_, jar) in &principals {
        for (_, value) in jar {
            assert!(!logs.contains(value.as_str()));
        }
    }
    let log_lines = logs.lines().count();
    c.record(&format!("item 1: the observer detects a deliberately planted raw code, invite_token and issued code (negative control); {scanned} responses carry no raw code, invite_token, issued code, token or request_uri outside the authorize redirect; the database ({} bytes dumped) holds no raw code or invitation token; {log_lines} captured log lines carry none of them and no cookie value, while recording invite.validate outcomes", dump.len()));

    let report = json!({
        "schema_version": 1,
        "scope": "APP-074 local public invitation entry on the native PostgreSQL TLS fixture with the cfg(test) synthetic loopback issuer and the loopback fAuth management stand-in",
        "managed_acceptance": false,
        "hosted_acceptance": false,
        "server_version": c.fixture.server_version,
        "checks": c.checks,
        "dispatch": {
            "par": c.issuer.count(|x| &x.par),
            "par_invited": c.issuer.count(|x| &x.par_invited),
            "par_refused": c.issuer.count(|x| &x.par_refused),
            "register": c.issuer.count(|x| &x.register),
            "register_refused": c.issuer.count(|x| &x.register_refused),
            "exchange": c.issuer.count(|x| &x.exchange),
        },
        "responses_scanned": scanned,
        "log_lines_scanned": log_lines,
        "passed": true,
        "limits": [
            "The provider is the cfg(test) synthetic loopback issuer (PAR, hosted registration, token) plus the loopback management stand-in app/runtime/fake_fauth.py; the operator writes each capability into the environment's secret (app/runtime/local_secrets.py, a loopback Secrets Manager stand-in reached through the real boto3 client; ADR-003 amendment 2026-10-30), the supervisor's own writer (runtime.write_capabilities) materializes it as the service's 0600 files after each operator command, and the test copies each invitation token from the service's file into the issuer (one simulated provider). Never the deployed fAuth; hosted PAR, real mail and browsers are APP-076/079.",
            "Synthetic local password authentication over TLS, not RDS IAM; synthetic identifiers.",
            "Rate-limit windows are in-process (D7.5, D7.18).",
            "The route-level admission race repeat and the provider create/revoke fault exercise are auth::invite_tests::native_invite_race_campaign (APP-082).",
            "Raw codes and invitation tokens are never written to this report."
        ]
    });
    if let Ok(path) = std::env::var("GENUI_INVITE_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!(
        "APP074 invite campaign: {} checks passed",
        report["checks"].as_array().unwrap().len()
    );
}

// --- APP-082: route-level races and provider faults ----------------------------

fn duplicate(browser: &Browser) -> Browser {
    Browser {
        jar: browser.jar.clone(),
        peer: browser.peer,
        log: browser.log.clone(),
    }
}
fn spawn_get(app: Router, browser: Browser, path: String) -> tokio::task::JoinHandle<Reply> {
    tokio::spawn(async move {
        let mut browser = browser;
        browser.get(&app, &path).await
    })
}
fn spawn_post(
    app: Router,
    browser: Browser,
    path: &'static str,
    body: String,
) -> tokio::task::JoinHandle<Reply> {
    tokio::spawn(async move {
        let mut browser = browser;
        browser.post(&app, path, &body).await
    })
}
fn session_value(reply: &Reply) -> String {
    reply
        .sets(cookies::SESSION)
        .expect("workspace session cookie")
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned()
}
fn neutral_refusal(reply: &Reply) -> bool {
    reply.status == StatusCode::BAD_REQUEST && reply.body == NEUTRAL
}
/// A denial clears an existing session cookie. That clear is not a minted session.
fn no_live_session(reply: &Reply) -> bool {
    reply
        .sets(cookies::SESSION)
        .is_none_or(|cookie| cookie.contains("Max-Age=0"))
}
fn denied_page(reply: &Reply) -> bool {
    reply.status == StatusCode::FORBIDDEN && field(&reply.body, "page") == "denied"
}
fn peer_of(n: u16) -> String {
    format!("192.0.2.{}:{}", 200 + (n % 50), 8100 + n)
}
fn subject_for(n: u32) -> String {
    format!(
        "usr_00000000-0000-4000-8000-{:012x}",
        0x0820_0000_0001u64 + u64::from(n)
    )
}
fn posts(provider: &mut FakeProvider) -> i64 {
    provider.control(json!({"op": "count", "method": "POST", "path": "/manage/invites"}))["count"]
        .as_i64()
        .unwrap()
}
fn remember(secrets: &mut Vec<String>, code: &Code) {
    secrets.push(code.raw.clone());
    if let Some(token) = &code.token {
        secrets.push(token.clone());
    }
}
/// A provider-backed issue that may fail the remote create. Unlike `Run::issue`,
/// a missing capability is a result to assert, not a harness failure.
fn issue_invited(run: &Run, email: &str) -> Value {
    let grants = [
        format!("discover:{APP}"),
        format!("run:{APP}"),
        format!("record_read:{APP}:{BOUND_WORK}"),
        format!("record_write:{APP}:{BOUND_WORK}"),
        format!("source_read:{SOURCE}"),
        format!("source_read:{INVENTORY}"),
    ];
    let mut owned = vec![
        "issue".to_owned(),
        "--organization".to_owned(),
        ORG.to_owned(),
        "--issued-by".to_owned(),
        "invite-race".to_owned(),
        "--max-redemptions".to_owned(),
        "1".to_owned(),
        "--provider-email".to_owned(),
        email.to_owned(),
        "--provider-role".to_owned(),
        "user".to_owned(),
        "--provider-issuer".to_owned(),
        ISSUER.to_owned(),
        "--provider-tenant".to_owned(),
        NORTHSTAR_TENANT.to_owned(),
    ];
    for grant in &grants {
        owned.push("--grant".to_owned());
        owned.push(grant.clone());
    }
    let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    operator_with(&run.fixture, &run.northstar, &refs)
}
fn reconcile(run: &Run) -> Value {
    operator_with(
        &run.fixture,
        &run.northstar,
        &["reconcile", "--organization", ORG],
    )
}
async fn cell(run: &Run, sql: &str) -> String {
    run.fixture
        .operator()
        .await
        .client
        .query_one(sql, &[])
        .await
        .unwrap()
        .get(0)
}
async fn principals(run: &Run, subject: &str) -> i64 {
    run.scalar(&format!(
        "SELECT count(*) FROM genui.principals WHERE provider_subject='{subject}'"
    ))
    .await
}
/// Every principal and entitlement row in the run's database. A refused public
/// validate starts no sign-in, so it has no subject to look up by: compare the
/// whole tables before and after it (APP-082 review N2).
async fn minted_rows(run: &Run) -> (i64, i64) {
    (
        run.scalar("SELECT count(*) FROM genui.principals").await,
        run.scalar("SELECT count(*) FROM genui.entitlements").await,
    )
}
async fn active_entitlements(run: &Run, subject: &str) -> i64 {
    run.scalar(&format!("SELECT count(*) FROM genui.entitlements e JOIN genui.principals p ON p.organization_id=e.organization_id AND p.principal_id=e.principal_id WHERE p.provider_subject='{subject}' AND e.status='active' AND e.expires_at>clock_timestamp()")).await
}
async fn entitlement_rows(run: &Run, subject: &str) -> i64 {
    run.scalar(&format!("SELECT count(*) FROM genui.entitlements e JOIN genui.principals p ON p.organization_id=e.organization_id AND p.principal_id=e.principal_id WHERE p.provider_subject='{subject}'")).await
}
async fn sessions_of(run: &Run, subject: &str) -> i64 {
    run.scalar(&format!("SELECT count(*) FROM genui.sessions s JOIN genui.principals p ON p.organization_id=s.organization_id AND p.principal_id=s.principal_id WHERE p.provider_subject='{subject}'")).await
}
async fn live_pending(run: &Run, code_id: &str) -> i64 {
    run.scalar(&format!("SELECT count(*) FROM genui.code_reservations WHERE code_id='{code_id}' AND kind='pending' AND state='live' AND expires_at>now()")).await
}
async fn denied_pending(run: &Run, subject: &str) -> i64 {
    run.scalar(&format!("SELECT count(*) FROM genui.code_reservations WHERE provider_subject='{subject}' AND state='denied_for_reconciliation'")).await
}
async fn redeemed(run: &Run, code_id: &str) -> i64 {
    run.scalar(&format!(
        "SELECT redeemed_count FROM genui.access_codes WHERE code_id='{code_id}'"
    ))
    .await
}
async fn code_status(run: &Run, code_id: &str) -> String {
    cell(
        run,
        &format!("SELECT status FROM genui.access_codes WHERE code_id='{code_id}'"),
    )
    .await
}
async fn audits(run: &Run, code_id: &str, kind: &str) -> i64 {
    run.scalar(&format!(
        "SELECT count(*) FROM genui.audit_events WHERE code_id='{code_id}' AND kind='{kind}'"
    ))
    .await
}
async fn provider_states(run: &Run, code_id: &str) -> Vec<(String, String)> {
    let operator = run.fixture.operator().await;
    let rows = operator.client.query("SELECT operation,state FROM genui.provider_invitations WHERE organization_id=$1::text::uuid AND code_id=$2::text::uuid ORDER BY operation,created_at", &[&ORG, &code_id]).await.unwrap();
    rows.iter().map(|row| (row.get(0), row.get(1))).collect()
}
async fn age_rows(run: &Run, code_id: &str) {
    run.superuser(&format!("UPDATE genui.provider_invitations SET created_at=created_at-make_interval(secs=>900), updated_at=updated_at-make_interval(secs=>900) WHERE organization_id='{ORG}' AND code_id='{code_id}'::uuid")).await;
}
fn shift(provider: &mut FakeProvider, email: &str) {
    let shifted = provider.control(json!({"op": "shift", "email": email, "seconds": 900}));
    assert!(shifted.get("ids").is_some(), "{shifted}");
}
fn fault(provider: &mut FakeProvider, route: &str, mode: &str, extra: &[(&str, Value)]) {
    let mut command = json!({"op": "fault", "route": route, "mode": mode});
    for (key, value) in extra {
        command[*key] = value.clone();
    }
    assert_eq!(provider.control(command)["ok"], true);
}
async fn prepared_sign_in(run: &Run, browser: &mut Browser, subject: &str) -> String {
    run.auth.limits.clear();
    let start = browser
        .post(&run.app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    run.issuer
        .login(&start.location(), subject)
        .expect("hosted sign-in")
}
async fn sign_in_on(run: &Run, app: &Router, browser: &mut Browser, subject: &str) -> Reply {
    run.auth.limits.clear();
    let start = browser
        .post(app, &format!("/signin/{NORTHSTAR_HANDLE}"), "")
        .await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let query = run
        .issuer
        .login(&start.location(), subject)
        .expect("hosted sign-in");
    browser.get(app, &format!("/auth/callback?{query}")).await
}
fn install_session(log: &Log, address: &str, reply: &Reply) -> Browser {
    let mut browser = Browser::new(address, log);
    browser
        .jar
        .insert(cookies::SESSION.into(), session_value(reply));
    browser
}

struct Notes(Vec<Value>);
impl Notes {
    fn record(&mut self, name: &str) {
        println!("APP082 check: {name}");
        self.0.push(json!({"name": name, "passed": true}));
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_invite_race_campaign() {
    std::thread::Builder::new()
        .name("app082-invite-race-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap()
                .block_on(race_campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn race_campaign() {
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }
    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    // The same grant template Run::issue writes. The bound work is the one
    // the invite campaign registers; without it the template's foreign key
    // refuses the code.
    for org in [ORG, OTHER] {
        let revision = derived_revision(org);
        let record = json!({
            "organization_id": org, "source_id": SOURCE, "revision_id": revision,
            "content_digest": CONTENT, "normalized_digest": NORMALIZED, "object_digest": NORMALIZED,
            "row_count": 6, "contract_digest": CONTRACT,
        })
        .to_string();
        let registered = operator(
            &fixture,
            &[
                "register-source-revision",
                "--organization",
                org,
                "--record-json",
                &record,
            ],
        );
        assert_eq!(registered["changed"], true);
        let bound = operator(
            &fixture,
            &[
                "create-bound-work",
                "--organization",
                org,
                "--app",
                APP,
                "--from-work",
                SEEDED_WORK,
                "--work",
                BOUND_WORK,
                "--creator",
                PRINCIPAL,
                "--input",
                "events",
                "--source",
                SOURCE,
                "--revision",
                &revision,
            ],
        );
        assert_eq!(bound["changed"], true);
    }
    let issuer = Issuer::start(tenants()).await;
    let (auth_config, registry, _auth_secret) = deliver_auth_configuration(
        &fixture,
        &issuer.origin,
        "00000000-0000-4000-8000-000000000a82",
    );
    let capabilities = fixture.directory.0.join("capabilities");
    let secret = Arc::new(crate::storage::admission_qualification::FakeSecret::start());
    let northstar = FakeProvider::start_at(
        &fixture,
        NORTHSTAR_TENANT,
        "northstar",
        Some(&capabilities),
        Some(&secret),
    );
    let canary = FakeProvider::start_at(
        &fixture,
        CANARY_TENANT,
        "canary",
        Some(&capabilities),
        Some(&secret),
    );
    let stores = Arc::new(Stores::new(fixture.settings_with_registry(registry)).unwrap());
    let hooks = stores.hooks();
    let auth = Arc::new(
        Auth::loopback(
            auth_config.clone(),
            Some(stores.clone()),
            Arc::new(TestPages),
        )
        .with_invitations(Some(invite::Invitations::open(&capabilities).unwrap())),
    );
    let mut c = Run {
        app: app(auth.clone()),
        fixture,
        stores,
        issuer,
        auth,
        log: Arc::default(),
        checks: Vec::new(),
        northstar,
        canary,
        capabilities,
        issued: 0,
    };
    let mut notes = Notes(Vec::new());
    let mut secrets = Vec::new();
    let mut peers = 0u16;
    let mut next_peer = || {
        peers += 1;
        peer_of(peers)
    };

    // --- 1. Last slot: signup callback holds the code row; a second validate waits.
    // One server-only invitation is single-use and email-bound, so a second
    // subject cannot finish hosted registration on it. The public last-slot
    // race is this hold against POST /invite/validate.
    let slot = c.issue(ORG, true);
    remember(&mut secrets, &slot);
    let slot_subject = subject_for(1);
    let mut holder = c.browser(&next_peer());
    c.auth.limits.clear();
    let entry = c.validate(&mut holder, &slot.raw).await;
    assert_eq!(entry.status, StatusCode::SEE_OTHER, "{}", entry.body);
    let query = c
        .issuer
        .register(&entry.location(), &slot_subject, &slot.email)
        .expect("hosted registration");
    let par_at_hold = c.issuer.count(|counter| &counter.par);
    let creates_at_hold = posts(&mut c.northstar);
    hooks.arm("pending");
    let held = spawn_get(
        c.app.clone(),
        duplicate(&holder),
        format!("/auth/callback?{query}"),
    );
    paused(&hooks).await;
    assert_eq!(
        live_pending(&c, &slot.id).await,
        0,
        "the pending row is not committed while the callback is stopped"
    );
    c.auth.limits.clear();
    let loser = spawn_post(
        c.app.clone(),
        c.browser(&next_peer()),
        "/invite/validate",
        format!("code={}&consent=1", slot.raw),
    );
    lock_wait(&c.fixture, 1).await;
    hooks.release.notify_one();
    let held = held.await.unwrap();
    let loser = loser.await.unwrap();
    assert_eq!(held.location(), "/auth/pending", "{}", held.body);
    assert!(
        neutral_refusal(&loser),
        "second validate {}: {}",
        loser.status,
        loser.body
    );
    assert_eq!(c.issuer.count(|counter| &counter.par), par_at_hold);
    assert_eq!(posts(&mut c.northstar), creates_at_hold);
    assert_eq!(live_pending(&c, &slot.id).await, 1);
    assert_eq!(redeemed(&c, &slot.id).await, 0);
    assert_eq!(principals(&c, &slot_subject).await, 0);
    assert_eq!(audits(&c, &slot.id, "admission.pending").await, 1);
    assert_eq!(audits(&c, &slot.id, "admission.redeemed").await, 0);
    c.issuer.set_verified(&slot_subject, true);
    c.auth.limits.clear();
    let admitted = c
        .sign_in(&mut holder, NORTHSTAR_HANDLE, &slot_subject)
        .await;
    assert_eq!(admitted.location(), "/workspace", "{}", admitted.body);
    let slot_session = session_value(&admitted);
    assert_eq!(redeemed(&c, &slot.id).await, 1);
    assert_eq!(code_status(&c, &slot.id).await, "exhausted");
    assert_eq!(principals(&c, &slot_subject).await, 1);
    assert_eq!(active_entitlements(&c, &slot_subject).await, 1);
    assert_eq!(audits(&c, &slot.id, "admission.redeemed").await, 1);
    c.auth.limits.clear();
    let mut later = c.browser(&next_peer());
    let again = c.validate(&mut later, &slot.raw).await;
    assert!(neutral_refusal(&again), "{} {}", again.status, again.body);
    assert_eq!(principals(&c, &slot_subject).await, 1);
    assert_eq!(posts(&mut c.northstar), creates_at_hold);
    notes.record("concurrent last slot (observed lock wait): the signup callback holds the code row before commit; the waiting POST /invite/validate is the neutral refusal, performs no PAR and no second create; exactly one pending admission is then admitted, and a later validate stays refused");

    // --- 2. Two verified sign-ins of one pending identity.
    let retry = c.issue(ORG, true);
    remember(&mut secrets, &retry);
    let retry_subject = subject_for(2);
    let mut retry_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    let pending = c.signup(&mut retry_browser, &retry, &retry_subject).await;
    assert_eq!(pending.location(), "/auth/pending", "{}", pending.body);
    c.issuer.set_verified(&retry_subject, true);
    let mut first_browser = c.browser(&next_peer());
    let mut second_browser = c.browser(&next_peer());
    let first_query = prepared_sign_in(&c, &mut first_browser, &retry_subject).await;
    let second_query = prepared_sign_in(&c, &mut second_browser, &retry_subject).await;
    hooks.arm("admit");
    let first = spawn_get(
        c.app.clone(),
        first_browser,
        format!("/auth/callback?{first_query}"),
    );
    paused(&hooks).await;
    assert_eq!(
        redeemed(&c, &retry.id).await,
        0,
        "the admission is not committed while the first sign-in is stopped"
    );
    let second = spawn_get(
        c.app.clone(),
        second_browser,
        format!("/auth/callback?{second_query}"),
    );
    lock_wait(&c.fixture, 1).await;
    hooks.release.notify_one();
    let first = first.await.unwrap();
    let second = second.await.unwrap();
    assert_eq!(first.location(), "/workspace", "{}", first.body);
    assert_eq!(second.location(), "/workspace", "{}", second.body);
    assert_ne!(session_value(&first), session_value(&second));
    assert_eq!(redeemed(&c, &retry.id).await, 1);
    assert_eq!(principals(&c, &retry_subject).await, 1);
    assert_eq!(active_entitlements(&c, &retry_subject).await, 1);
    assert_eq!(entitlement_rows(&c, &retry_subject).await, 1);
    assert_eq!(audits(&c, &retry.id, "admission.redeemed").await, 1);
    assert_eq!(sessions_of(&c, &retry_subject).await, 2);
    c.auth.limits.clear();
    for reply in [&first, &second] {
        let mut browser = install_session(&c.log, &next_peer(), reply);
        let page = browser.get(&c.app, "/workspace").await;
        assert_eq!(field(&page.body, "page"), "workspace", "{}", page.body);
    }
    notes.record("identical retries (observed lock wait): two verified sign-ins of one pending identity return one stored admission, one principal, one entitlement and one admission.redeemed; each completion mints its own session");

    // --- 3. A sign-in already inside mint cannot revive an expired entitlement.
    // A later sign-in of an admitted member does not call admit again (the
    // pending row is consumed). It reaches mint, which re-reads the
    // entitlement after the member fence. Stop there, expire the row, and
    // let the re-read run.
    let mut late = c.browser(&next_peer());
    let late_query = prepared_sign_in(&c, &mut late, &retry_subject).await;
    hooks.arm("mint");
    let late_task = spawn_get(c.app.clone(), late, format!("/auth/callback?{late_query}"));
    paused(&hooks).await;
    c.superuser(&format!("UPDATE genui.entitlements e SET expires_at=e.starts_at+interval '1 millisecond' FROM genui.principals p WHERE e.organization_id=p.organization_id AND e.principal_id=p.principal_id AND p.provider_subject='{retry_subject}'")).await;
    hooks.release.notify_one();
    let late_reply = late_task.await.unwrap();
    assert!(
        denied_page(&late_reply),
        "late sign-in {}: {}",
        late_reply.status,
        late_reply.body
    );
    assert!(
        no_live_session(&late_reply),
        "late sign-in set a session: {:?}",
        late_reply.sets(cookies::SESSION)
    );
    assert_eq!(active_entitlements(&c, &retry_subject).await, 0);
    assert_eq!(entitlement_rows(&c, &retry_subject).await, 1);
    assert_eq!(redeemed(&c, &retry.id).await, 1);
    assert_eq!(principals(&c, &retry_subject).await, 1);
    c.auth.limits.clear();
    for reply in [&first, &second] {
        let mut browser = install_session(&c.log, &next_peer(), reply);
        assert!(
            browser.get(&c.app, "/workspace").await.uniform_404(),
            "expired entitlement invalidates the session"
        );
    }
    c.auth.limits.clear();
    let mut after = c.browser(&next_peer());
    let after_reply = c
        .sign_in(&mut after, NORTHSTAR_HANDLE, &retry_subject)
        .await;
    assert!(
        denied_page(&after_reply),
        "retry after expiry {}: {}",
        after_reply.status,
        after_reply.body
    );
    assert_eq!(active_entitlements(&c, &retry_subject).await, 0);
    assert_eq!(entitlement_rows(&c, &retry_subject).await, 1);
    notes.record("expired entitlement is not revived: a sign-in stopped after the member fence, before the entitlement re-read, is denied once the expiry is committed; the existing sessions are the uniform 404, a later sign-in stays denied, and no entitlement row is added");

    // --- 4. Entitlement revocation waits on the member fence, then wins.
    let slot_principal = cell(
        &c,
        &format!(
            "SELECT principal_id::text FROM genui.principals WHERE provider_subject='{slot_subject}'"
        ),
    )
    .await;
    let mut racing_member = c.browser(&next_peer());
    let racing_member_query = prepared_sign_in(&c, &mut racing_member, &slot_subject).await;
    hooks.arm("mint");
    let racing_member_task = spawn_get(
        c.app.clone(),
        racing_member,
        format!("/auth/callback?{racing_member_query}"),
    );
    paused(&hooks).await;
    let revoke_entitlement = cli_command(
        &c.fixture,
        "genui_migration",
        "synthetic-migration",
        &[
            "revoke-entitlement",
            "--organization",
            ORG,
            "--principal",
            &slot_principal,
        ],
    )
    .spawn()
    .unwrap();
    lock_wait(&c.fixture, 1).await;
    hooks.release.notify_one();
    let racing_member_reply = racing_member_task.await.unwrap();
    let revoke_entitlement = accepted(
        tokio::task::spawn_blocking(move || wait(revoke_entitlement, "revoke-entitlement"))
            .await
            .unwrap(),
        "revoke-entitlement",
    );
    assert_eq!(
        racing_member_reply.location(),
        "/workspace",
        "{}",
        racing_member_reply.body
    );
    assert_eq!(revoke_entitlement["changed"], true);
    assert_eq!(active_entitlements(&c, &slot_subject).await, 0);
    assert_eq!(entitlement_rows(&c, &slot_subject).await, 1);
    assert_eq!(redeemed(&c, &slot.id).await, 1);
    c.auth.limits.clear();
    let mut minted_then_revoked = install_session(&c.log, &next_peer(), &racing_member_reply);
    assert!(minted_then_revoked
        .get(&c.app, "/workspace")
        .await
        .uniform_404());
    let mut earlier_session = c.browser(&next_peer());
    earlier_session
        .jar
        .insert(cookies::SESSION.into(), slot_session);
    assert!(earlier_session
        .get(&c.app, "/workspace")
        .await
        .uniform_404());
    c.auth.limits.clear();
    let mut after_revoke = c.browser(&next_peer());
    let after_revoke = c
        .sign_in(&mut after_revoke, NORTHSTAR_HANDLE, &slot_subject)
        .await;
    assert!(
        denied_page(&after_revoke),
        "sign-in after entitlement revocation {}: {}",
        after_revoke.status,
        after_revoke.body
    );
    assert_eq!(active_entitlements(&c, &slot_subject).await, 0);
    notes.record("entitlement revocation racing a sign-in (observed lock wait): the operator waits on the member fence until the mint commits, then the new session and the earlier session are the uniform 404 and a later sign-in is denied without a new entitlement");

    // --- 5. Admission first, then code revocation.
    let admit_first = c.issue(ORG, true);
    remember(&mut secrets, &admit_first);
    let admit_subject = subject_for(3);
    let mut admit_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    assert_eq!(
        c.signup(&mut admit_browser, &admit_first, &admit_subject)
            .await
            .location(),
        "/auth/pending"
    );
    c.issuer.set_verified(&admit_subject, true);
    let mut racing = c.browser(&next_peer());
    let racing_query = prepared_sign_in(&c, &mut racing, &admit_subject).await;
    hooks.arm("admit");
    let racing_task = spawn_get(
        c.app.clone(),
        racing,
        format!("/auth/callback?{racing_query}"),
    );
    paused(&hooks).await;
    let revoke = cli_command(
        &c.fixture,
        "genui_migration",
        "synthetic-migration",
        &[
            "revoke",
            "--organization",
            ORG,
            "--code-id",
            &admit_first.id,
        ],
    )
    .spawn()
    .unwrap();
    lock_wait(&c.fixture, 1).await;
    hooks.release.notify_one();
    let racing_reply = racing_task.await.unwrap();
    let revoke_out = accepted(
        tokio::task::spawn_blocking(move || wait(revoke, "revoke"))
            .await
            .unwrap(),
        "revoke",
    );
    assert_eq!(
        racing_reply.location(),
        "/workspace",
        "{}",
        racing_reply.body
    );
    assert_eq!(revoke_out["changed"], true);
    assert_eq!(
        (
            code_status(&c, &admit_first.id).await,
            redeemed(&c, &admit_first.id).await
        ),
        ("revoked".to_owned(), 1)
    );
    assert_eq!(principals(&c, &admit_subject).await, 1);
    assert_eq!(active_entitlements(&c, &admit_subject).await, 1);
    c.auth.limits.clear();
    let mut stood = install_session(&c.log, &next_peer(), &racing_reply);
    assert_eq!(
        field(&stood.get(&c.app, "/workspace").await.body, "page"),
        "workspace"
    );
    c.auth.limits.clear();
    let mut replay = c.browser(&next_peer());
    let replay = c
        .sign_in(&mut replay, NORTHSTAR_HANDLE, &admit_subject)
        .await;
    assert_eq!(replay.location(), "/workspace", "{}", replay.body);
    assert_eq!(redeemed(&c, &admit_first.id).await, 1);
    assert_eq!(audits(&c, &admit_first.id, "admission.redeemed").await, 1);
    notes.record("admission-first: code revocation waits on the code row until the sign-in commits; the admission stands, the code is then revoked, and a later sign-in replays the stored admission without a second slot");

    // --- 6. Revocation first: the sign-in waits, then the code is revoked.
    let revoke_first = c.issue(ORG, true);
    remember(&mut secrets, &revoke_first);
    let revoke_subject = subject_for(4);
    let mut revoke_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    assert_eq!(
        c.signup(&mut revoke_browser, &revoke_first, &revoke_subject)
            .await
            .location(),
        "/auth/pending"
    );
    c.issuer.set_verified(&revoke_subject, true);
    let mut blocked = c.browser(&next_peer());
    let blocked_query = prepared_sign_in(&c, &mut blocked, &revoke_subject).await;
    let writer = c.fixture.operator().await;
    writer.client.batch_execute(&format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; UPDATE genui.access_codes SET status='revoked',revoked_at=clock_timestamp() WHERE organization_id='{ORG}' AND code_id='{}'", revoke_first.id)).await.unwrap();
    let blocked_task = spawn_get(
        c.app.clone(),
        blocked,
        format!("/auth/callback?{blocked_query}"),
    );
    lock_wait(&c.fixture, 1).await;
    assert_eq!(principals(&c, &revoke_subject).await, 0);
    writer.client.batch_execute("COMMIT").await.unwrap();
    drop(writer);
    let blocked_reply = blocked_task.await.unwrap();
    assert!(
        denied_page(&blocked_reply),
        "revocation-first {}: {}",
        blocked_reply.status,
        blocked_reply.body
    );
    assert!(
        no_live_session(&blocked_reply),
        "revocation-first set a session: {:?}",
        blocked_reply.sets(cookies::SESSION)
    );
    assert_eq!(principals(&c, &revoke_subject).await, 0);
    assert_eq!(active_entitlements(&c, &revoke_subject).await, 0);
    assert_eq!(entitlement_rows(&c, &revoke_subject).await, 0);
    assert_eq!(redeemed(&c, &revoke_first.id).await, 0);
    assert_eq!(code_status(&c, &revoke_first.id).await, "revoked");
    assert_eq!(denied_pending(&c, &revoke_subject).await, 1);
    assert_eq!(
        audits(&c, &revoke_first.id, "admission.denied_for_reconciliation").await,
        1
    );
    c.auth.limits.clear();
    let mut blocked_again = c.browser(&next_peer());
    let blocked_again = c
        .sign_in(&mut blocked_again, NORTHSTAR_HANDLE, &revoke_subject)
        .await;
    assert!(
        denied_page(&blocked_again),
        "late completion {}: {}",
        blocked_again.status,
        blocked_again.body
    );
    assert_eq!(principals(&c, &revoke_subject).await, 0);
    notes.record("revocation-first (observed lock wait): the verified sign-in waits behind the code-row update and is denied with no principal, membership or entitlement; the pending admission is denied_for_reconciliation and a later sign-in stays denied");

    // --- 7. Provider create timeout, lost response, and reconciliation.
    let baseline_creates = posts(&mut c.northstar);
    fault(&mut c.northstar, "create", "drop_before", &[]);
    let lost_email = "drop-before@invite.example.test";
    let lost = issue_invited(&c, lost_email);
    secrets.push(lost["code"].as_str().unwrap().to_owned());
    let lost_id = lost["code_id"].as_str().unwrap().to_owned();
    assert_eq!(
        (
            lost["provider_invitation"]["state"].as_str(),
            lost["provider_invitation"]["detail"].as_str(),
            lost["provider_invitation"]["capability_stored"].as_bool(),
            lost["provider_invitation"]["remote_invitation_id"].is_null()
        ),
        (
            Some("unknown"),
            Some("transport_ambiguous"),
            Some(false),
            true
        )
    );
    assert_eq!(posts(&mut c.northstar), baseline_creates + 1);
    let young = reconcile(&c);
    assert!(young["deferred"].as_i64().unwrap() >= 1);
    assert_eq!(
        provider_states(&c, &lost_id).await,
        vec![("create".to_owned(), "unknown".to_owned())]
    );
    age_rows(&c, &lost_id).await;
    shift(&mut c.northstar, lost_email);
    let settled = reconcile(&c);
    assert_eq!(settled["provider_phase"], "completed");
    assert_eq!(
        provider_states(&c, &lost_id).await,
        vec![("create".to_owned(), "failed".to_owned())]
    );
    assert_eq!(posts(&mut c.northstar), baseline_creates + 1);
    let par_before_lost = c.issuer.count(|counter| &counter.par);
    let minted_before_lost = minted_rows(&c).await;
    c.auth.limits.clear();
    let mut lost_browser = c.browser(&next_peer());
    let lost_reply = c
        .validate(&mut lost_browser, lost["code"].as_str().unwrap())
        .await;
    assert!(
        neutral_refusal(&lost_reply),
        "unconfirmed create {}: {}",
        lost_reply.status,
        lost_reply.body
    );
    assert_eq!(c.issuer.count(|counter| &counter.par), par_before_lost);
    assert_eq!(minted_rows(&c).await, minted_before_lost);
    notes.record("create dropped before a response is unknown, not sent again; reconcile inside the grace leaves it unknown and, after the row is aged, records failed by inspection with no second POST; public validate is the neutral refusal and mints nothing");

    fault(
        &mut c.northstar,
        "create",
        "delay_after",
        &[("seconds", json!(13))],
    );
    let slow_email = "create-timeout@invite.example.test";
    let slow_started = Instant::now();
    let slow = issue_invited(&c, slow_email);
    let slow_elapsed = slow_started.elapsed();
    secrets.push(slow["code"].as_str().unwrap().to_owned());
    let slow_id = slow["code_id"].as_str().unwrap().to_owned();
    assert!(
        slow_elapsed >= Duration::from_secs(8) && slow_elapsed < Duration::from_secs(18),
        "create timeout returned in {slow_elapsed:?}"
    );
    assert_eq!(
        (
            slow["provider_invitation"]["state"].as_str(),
            slow["provider_invitation"]["detail"].as_str(),
            slow["provider_invitation"]["capability_stored"].as_bool()
        ),
        (Some("unknown"), Some("transport_ambiguous"), Some(false))
    );
    let creates_after_timeout = posts(&mut c.northstar);
    assert_eq!(creates_after_timeout, baseline_creates + 2);
    let still_young = reconcile(&c);
    assert!(still_young["deferred"].as_i64().unwrap() >= 1);
    assert_eq!(
        provider_states(&c, &slow_id).await,
        vec![("create".to_owned(), "unknown".to_owned())]
    );
    age_rows(&c, &slow_id).await;
    shift(&mut c.northstar, slow_email);
    assert_eq!(reconcile(&c)["provider_phase"], "completed");
    assert_eq!(
        provider_states(&c, &slow_id).await,
        vec![
            ("create".to_owned(), "confirmed".to_owned()),
            ("revoke".to_owned(), "confirmed".to_owned())
        ]
    );
    assert_eq!(posts(&mut c.northstar), creates_after_timeout);
    let slow_remote = cell(
        &c,
        &format!("SELECT remote_invitation_id FROM genui.provider_invitations WHERE code_id='{slow_id}' AND operation='create'"),
    )
    .await;
    assert_eq!(
        c.northstar
            .control(json!({"op": "status", "id": slow_remote}))["status"]
            .as_str(),
        Some("revoked")
    );
    let par_before_slow = c.issuer.count(|counter| &counter.par);
    let minted_before_slow = minted_rows(&c).await;
    c.auth.limits.clear();
    let mut slow_browser = c.browser(&next_peer());
    let slow_reply = c
        .validate(&mut slow_browser, slow["code"].as_str().unwrap())
        .await;
    assert!(
        neutral_refusal(&slow_reply),
        "timed-out create {}: {}",
        slow_reply.status,
        slow_reply.body
    );
    assert_eq!(c.issuer.count(|counter| &counter.par), par_before_slow);
    assert_eq!(minted_rows(&c).await, minted_before_slow);
    notes.record("create timeout: one POST, an unknown outcome inside the client deadline, no capability; reconcile does not judge it inside the grace and afterwards confirms the invitation by inspection and revokes it (the token was lost) without a second create; public validate mints nothing");

    // --- 8. Revoke timeout, retry, and signup after local revocation.
    let remote_revoke = c.issue(ORG, true);
    remember(&mut secrets, &remote_revoke);
    let remote_subject = subject_for(5);
    let mut remote_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    assert_eq!(
        c.signup(&mut remote_browser, &remote_revoke, &remote_subject)
            .await
            .location(),
        "/auth/pending"
    );
    let creates_before_revoke = posts(&mut c.northstar);
    fault(
        &mut c.northstar,
        "revoke",
        "delay_after",
        &[("seconds", json!(13))],
    );
    let revoke_started = Instant::now();
    let revoke_unknown = operator_with(
        &c.fixture,
        &c.northstar,
        &[
            "revoke",
            "--organization",
            ORG,
            "--code-id",
            &remote_revoke.id,
        ],
    );
    let revoke_elapsed = revoke_started.elapsed();
    assert!(
        revoke_elapsed >= Duration::from_secs(8) && revoke_elapsed < Duration::from_secs(18),
        "revoke timeout returned in {revoke_elapsed:?}"
    );
    assert_eq!(revoke_unknown["changed"], true);
    assert_eq!(
        revoke_unknown["provider_revocations"][0]["state"],
        "unknown"
    );
    assert_eq!(code_status(&c, &remote_revoke.id).await, "revoked");
    assert_eq!(posts(&mut c.northstar), creates_before_revoke);
    assert!(
        !c.capabilities
            .join(format!(
                "{ORG}.{}.capability",
                remote_revoke.invitation.as_deref().unwrap()
            ))
            .exists(),
        "revocation removes the capability before the remote answer"
    );
    c.issuer.set_verified(&remote_subject, true);
    c.auth.limits.clear();
    let remote_denied = c
        .sign_in(&mut remote_browser, NORTHSTAR_HANDLE, &remote_subject)
        .await;
    assert!(
        denied_page(&remote_denied),
        "signup after revocation {}: {}",
        remote_denied.status,
        remote_denied.body
    );
    assert_eq!(principals(&c, &remote_subject).await, 0);
    assert_eq!(active_entitlements(&c, &remote_subject).await, 0);
    assert_eq!(reconcile(&c)["provider_phase"], "completed");
    assert_eq!(
        provider_states(&c, &remote_revoke.id).await,
        vec![
            ("create".to_owned(), "confirmed".to_owned()),
            ("revoke".to_owned(), "confirmed".to_owned())
        ]
    );
    assert_eq!(posts(&mut c.northstar), creates_before_revoke);
    c.auth.limits.clear();
    let remote_denied_again = c
        .sign_in(&mut remote_browser, NORTHSTAR_HANDLE, &remote_subject)
        .await;
    assert!(denied_page(&remote_denied_again));
    assert_eq!(principals(&c, &remote_subject).await, 0);
    notes.record("revoke timeout: the local code stays revoked and the capability is gone when the remote answer is unknown; the verified sign-in mints nothing; reconcile retries the revocation to confirmed without a second create, and the sign-in stays denied");

    // --- 9. Signup after local expiry and exhaustion.
    let expired_code = c.issue(ORG, true);
    remember(&mut secrets, &expired_code);
    let expired_subject = subject_for(6);
    let mut expired_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    assert_eq!(
        c.signup(&mut expired_browser, &expired_code, &expired_subject)
            .await
            .location(),
        "/auth/pending"
    );
    c.superuser(&format!("UPDATE genui.access_codes SET expires_at=issued_at+interval '1 millisecond' WHERE code_id='{}'", expired_code.id)).await;
    c.issuer.set_verified(&expired_subject, true);
    c.auth.limits.clear();
    let expired_sign_in = c
        .sign_in(&mut expired_browser, NORTHSTAR_HANDLE, &expired_subject)
        .await;
    assert!(
        denied_page(&expired_sign_in),
        "signup after expiry {}: {}",
        expired_sign_in.status,
        expired_sign_in.body
    );
    assert_eq!(principals(&c, &expired_subject).await, 0);
    assert_eq!(active_entitlements(&c, &expired_subject).await, 0);
    assert_eq!(denied_pending(&c, &expired_subject).await, 1);
    assert_eq!(
        audits(&c, &expired_code.id, "admission.denied_for_reconciliation").await,
        1
    );
    // The handoff is prepared while the code is still valid. A competing
    // fixture admission then takes the last slot. The original callback is
    // what must be refused. A fresh validate after exhaustion is a different
    // boundary and is not this check.
    let exhausted = c.issue(ORG, true);
    remember(&mut secrets, &exhausted);
    let exhausted_subject = subject_for(7);
    let mut exhausted_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    let entry = c.validate(&mut exhausted_browser, &exhausted.raw).await;
    assert_eq!(entry.status, StatusCode::SEE_OTHER, "{}", entry.body);
    let exhausted_query = c
        .issuer
        .register(&entry.location(), &exhausted_subject, &exhausted.email)
        .expect("hosted registration");
    assert_eq!(redeemed(&c, &exhausted.id).await, 0);
    let slot_taker = consume_last_slot(
        &c.stores,
        ORG,
        &exhausted.digest,
        ISSUER,
        NORTHSTAR_TENANT,
        "a82-slot",
    )
    .await;
    assert_eq!(code_status(&c, &exhausted.id).await, "exhausted");
    assert_eq!(redeemed(&c, &exhausted.id).await, 1);
    c.auth.limits.clear();
    let exhausted_reply = exhausted_browser
        .get(&c.app, &format!("/auth/callback?{exhausted_query}"))
        .await;
    assert!(
        denied_page(&exhausted_reply),
        "callback after exhaustion {}: {}",
        exhausted_reply.status,
        exhausted_reply.body
    );
    assert!(no_live_session(&exhausted_reply));
    assert_eq!(principals(&c, &exhausted_subject).await, 0);
    assert_eq!(active_entitlements(&c, &exhausted_subject).await, 0);
    assert_eq!(
        c.scalar(&format!("SELECT count(*) FROM genui.memberships m JOIN genui.principals p ON p.organization_id=m.organization_id AND p.principal_id=m.principal_id WHERE p.provider_subject='{exhausted_subject}'")).await,
        0
    );
    // record_pending_admission returns exhausted before it writes a pending
    // row, so this callback is not the admit_redemption path that marks a
    // pending admission denied_for_reconciliation. The storage campaign
    // records that same early return. The slot taken above is the only
    // admission.redeemed.
    assert_eq!(denied_pending(&c, &exhausted_subject).await, 0);
    assert_eq!(
        audits(&c, &exhausted.id, "admission.denied_for_reconciliation").await,
        0
    );
    assert_eq!(audits(&c, &exhausted.id, "admission.redeemed").await, 1);
    assert_eq!(principals(&c, &slot_taker).await, 1);
    assert_eq!(redeemed(&c, &exhausted.id).await, 1);
    notes.record("signup after local expiry is denied and recorded for reconciliation, with no principal or entitlement; an outstanding signup callback after a competing fixture admission takes the last slot is denied, with no live session, principal, membership, entitlement, or second redeemed slot. record_pending_admission returns exhausted without a denied_for_reconciliation row; that audit belongs to admit_redemption when a pending admission already exists");

    // --- 10. Remote revocation propagates through reconcile.
    let drifted = c.issue(ORG, true);
    remember(&mut secrets, &drifted);
    let drifted_remote = cell(
        &c,
        &format!("SELECT remote_invitation_id FROM genui.provider_invitations WHERE code_id='{}' AND operation='create'", drifted.id),
    )
    .await;
    let creates_before_drift = posts(&mut c.northstar);
    assert_eq!(
        c.northstar
            .control(json!({"op": "set_status", "id": drifted_remote, "status": "revoked"}))
            ["status"]
            .as_str(),
        Some("revoked")
    );
    let drift = reconcile(&c);
    assert_eq!(drift["provider_phase"], "completed");
    assert_eq!(
        provider_states(&c, &drifted.id).await,
        vec![
            ("create".to_owned(), "confirmed".to_owned()),
            ("revoke".to_owned(), "confirmed".to_owned())
        ]
    );
    assert!(!c
        .capabilities
        .join(format!(
            "{ORG}.{}.capability",
            drifted.invitation.as_deref().unwrap()
        ))
        .exists());
    assert_eq!(code_status(&c, &drifted.id).await, "active");
    assert_eq!(posts(&mut c.northstar), creates_before_drift);
    let par_before_drift = c.issuer.count(|counter| &counter.par);
    let minted_before_drift = minted_rows(&c).await;
    c.auth.limits.clear();
    let mut drifted_browser = c.browser(&next_peer());
    let drifted_reply = c.validate(&mut drifted_browser, &drifted.raw).await;
    assert!(
        neutral_refusal(&drifted_reply),
        "remote revocation {}: {}",
        drifted_reply.status,
        drifted_reply.body
    );
    assert_eq!(c.issuer.count(|counter| &counter.par), par_before_drift);
    assert_eq!(minted_rows(&c).await, minted_before_drift);
    notes.record("remote revocation propagation: reconcile records the provider's revoked invitation, removes the capability and does not create another; the code stays active and public validate mints nothing");

    // --- 11. Restart and reopen.
    let (reopen_subject, mut reopen_browser) = c.member(ORG, "a82reopen").await;
    let reopen_page = reopen_browser.get(&c.app, "/workspace").await;
    assert_eq!(field(&reopen_page.body, "page"), "workspace");
    let pending_subject = subject_for(8);
    let pending_code = c.issue(ORG, true);
    remember(&mut secrets, &pending_code);
    let mut pending_browser = c.browser(&next_peer());
    c.auth.limits.clear();
    assert_eq!(
        c.signup(&mut pending_browser, &pending_code, &pending_subject)
            .await
            .location(),
        "/auth/pending"
    );
    let pending_page = pending_browser.get(&c.app, "/auth/pending").await;
    let old_csrf = field(&pending_page.body, "resend");
    let mut unfinished = c.browser(&next_peer());
    let unfinished_query = prepared_sign_in(&c, &mut unfinished, &reopen_subject).await;
    let fresh = c.issue(ORG, true);
    remember(&mut secrets, &fresh);
    let principals_before_restart = principals(&c, &reopen_subject).await;
    let restarted = Arc::new(
        Auth::loopback(auth_config, Some(c.stores.clone()), Arc::new(TestPages))
            .with_invitations(Some(invite::Invitations::open(&c.capabilities).unwrap())),
    );
    let restarted_app = app(restarted);
    assert!(reopen_browser
        .get(&restarted_app, "/workspace")
        .await
        .uniform_404());
    let unfinished_reply = unfinished
        .get(
            &restarted_app,
            &format!("/auth/callback?{unfinished_query}"),
        )
        .await;
    assert_eq!(unfinished_reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(field(&unfinished_reply.body, "page"), "signin");
    assert_eq!(
        principals(&c, &reopen_subject).await,
        principals_before_restart
    );
    let stale = pending_browser
        .post(
            &restarted_app,
            "/auth/pending/resend",
            &format!("csrf={old_csrf}"),
        )
        .await;
    assert_eq!(stale.status, StatusCode::FORBIDDEN);
    let fresh_page = pending_browser.get(&restarted_app, "/auth/pending").await;
    assert_eq!(fresh_page.status, StatusCode::OK);
    let resent = pending_browser
        .post(
            &restarted_app,
            "/auth/pending/resend",
            &format!("csrf={}", field(&fresh_page.body, "resend")),
        )
        .await;
    assert_eq!(resent.location(), "/signin");
    let mut reopened = c.browser(&next_peer());
    let reopened = sign_in_on(&c, &restarted_app, &mut reopened, &reopen_subject).await;
    assert_eq!(reopened.location(), "/workspace", "{}", reopened.body);
    assert_eq!(
        field(
            &reopened_page(&c, &restarted_app, &reopened).await.body,
            "page"
        ),
        "workspace"
    );
    let mut still_revoked = c.browser(&next_peer());
    let still_revoked = sign_in_on(&c, &restarted_app, &mut still_revoked, &slot_subject).await;
    assert!(
        denied_page(&still_revoked),
        "reopen of a revoked member {}: {}",
        still_revoked.status,
        still_revoked.body
    );
    assert_eq!(active_entitlements(&c, &slot_subject).await, 0);
    c.auth.limits.clear();
    let mut fresh_browser = Browser::new(&next_peer(), &c.log);
    let fresh_reply = fresh_browser
        .post(
            &restarted_app,
            "/invite/validate",
            &format!("code={}&consent=1", fresh.raw),
        )
        .await;
    assert_eq!(
        fresh_reply.status,
        StatusCode::SEE_OTHER,
        "{}",
        fresh_reply.body
    );
    notes.record("restart/reopen: a replaced Auth and router in this same process, on the same stores, admits no earlier session, completes no earlier pre-login flow and rejects the earlier synchronizer token; the pending identity can resend from a fresh page; a current member signs in again; a revoked member stays denied; an unspent code still validates from the capability directory. This is auth-memory and CSRF replacement, not an operating-system process or storage-pool restart");

    let report = json!({
        "schema_version": 1,
        "scope": "APP-082 route-level invitation races and provider faults on the local PostgreSQL TLS fixture with the synthetic issuer and loopback fAuth stand-in",
        "managed_acceptance": false,
        "hosted_acceptance": false,
        "server_version": c.fixture.server_version,
        "checks": notes.0,
        "dispatch": {
            "par": c.issuer.count(|counter| &counter.par),
            "register": c.issuer.count(|counter| &counter.register),
            "exchange": c.issuer.count(|counter| &counter.exchange),
            "provider_creates": posts(&mut c.northstar),
        },
        "passed": true,
        "limits": [
            "The provider is the cfg(test) synthetic loopback issuer plus app/runtime/fake_fauth.py. No deployed fAuth, mail, browser or AWS call.",
            "Races stop on the cfg(test) admission checkpoints or on a PostgreSQL lock wait (pg_stat_activity). The provider deadline is the create/revoke timeout itself.",
            "One server-only invitation is single-use and email-bound, so two subjects do not both complete hosted registration on one code. The outstanding callback is prepared while the code is valid; a fixture admission then takes the last slot; the original callback is completed after that. The replaced Auth in the restart check is the same process and the same stores.",
            "Raw codes and invitation tokens are not written to this report."
        ]
    });
    let rendered = serde_json::to_string_pretty(&report).unwrap();
    for secret in &secrets {
        assert!(
            !rendered.contains(secret.as_str()),
            "the report carried a raw code or invitation token"
        );
    }
    if let Ok(path) = std::env::var("GENUI_INVITE_RACE_REPORT") {
        std::fs::write(&path, rendered.as_bytes()).unwrap();
    }
    println!(
        "APP082 invite race campaign: {} checks passed",
        report["checks"].as_array().unwrap().len()
    );
}

async fn reopened_page(run: &Run, app: &Router, reply: &Reply) -> Reply {
    let mut browser = install_session(&run.log, "192.0.2.250:8250", reply);
    browser.get(app, "/workspace").await
}

fn raw_uuid(id: &str) -> [u8; 16] {
    let hex: Vec<u8> = id.bytes().filter(|b| *b != b'-').collect();
    let mut out = [0u8; 16];
    for (i, pair) in hex.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    out
}
