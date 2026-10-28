use super::*;
use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex as StdMutex,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};

const NOW: u64 = 1_800_000_000;
const TENANT: &str = "00000000-0000-4000-8000-000000000001";
fn now() -> Result<u64, Refusal> {
    Ok(NOW)
}
fn claims() -> Value {
    serde_json::from_str(include_str!("../fixtures/claims-247.json")).unwrap()
}
struct IssuerKey(Ed25519KeyPair);
impl IssuerKey {
    fn fresh() -> Self {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        Self(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap())
    }
    fn kid(&self) -> String {
        key_id(self.0.public_key().as_ref())
    }
    fn jwk(&self) -> Value {
        json!({"kty":"OKP","use":"sig","kid":self.kid(),"alg":"EdDSA","crv":"Ed25519","x":URL_SAFE_NO_PAD.encode(self.0.public_key().as_ref())})
    }
    fn jwks(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({"keys":[self.jwk()]})).unwrap()
    }
    fn header(&self) -> Value {
        json!({"alg":"EdDSA","typ":"JWT","kid":self.kid()})
    }
    fn sign(&self, head: &Value, claims: &Value) -> String {
        self.sign_raw(
            &serde_json::to_string(head).unwrap(),
            &serde_json::to_string(claims).unwrap(),
        )
    }
    fn token(&self) -> String {
        self.sign(&self.header(), &claims())
    }
    fn sign_raw(&self, head: &str, body: &str) -> String {
        let signed = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(head),
            URL_SAFE_NO_PAD.encode(body)
        );
        format!(
            "{}.{}",
            signed,
            URL_SAFE_NO_PAD.encode(self.0.sign(signed.as_bytes()).as_ref())
        )
    }
}
#[derive(Clone)]
enum Reply {
    Body {
        status: u16,
        headers: String,
        body: Vec<u8>,
        delay: Duration,
        chunked: bool,
    },
    Trickle(Vec<u8>),
}
fn ok(body: Vec<u8>) -> Reply {
    Reply::Body {
        status: 200,
        headers: "Cache-Control: public, max-age=3600\r\n".into(),
        body,
        delay: Duration::ZERO,
        chunked: false,
    }
}
struct Mock {
    url: String,
    reply: Arc<StdMutex<Reply>>,
    hits: Arc<AtomicUsize>,
    requests: Arc<StdMutex<Vec<String>>>,
    task: JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Mock {
    async fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/auth/jwks", listener.local_addr().unwrap());
        let reply = Arc::new(StdMutex::new(reply));
        let hits = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let (r, h, q) = (reply.clone(), hits.clone(), requests.clone());
        let task = tokio::spawn(async move {
            let mut handlers = JoinSet::new();
            loop {
                tokio::select! {
                    incoming = listener.accept() => {
                        let (mut socket,_) = incoming.unwrap(); let (r,h,q)=(r.clone(),h.clone(),q.clone());
                        handlers.spawn(async move {
                            let mut request=Vec::new(); let mut byte=[0;1];
                            while request.len()<4096 && !request.ends_with(b"\r\n\r\n") {
                                if socket.read(&mut byte).await.unwrap_or(0)==0 { return; } request.push(byte[0]);
                            }
                            q.lock().unwrap().push(String::from_utf8(request).unwrap()); h.fetch_add(1,Ordering::SeqCst);
                            let reply=r.lock().unwrap().clone();
                            match reply {
                                Reply::Body {status,headers,body,delay,chunked} => {
                                    tokio::time::sleep(delay).await;
                                    let length=if chunked {"Transfer-Encoding: chunked\r\n".into()} else {format!("Content-Length: {}\r\n",body.len())};
                                    let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nConnection: close\r\n{length}{headers}\r\n");
                                    if socket.write_all(header.as_bytes()).await.is_err() {return;}
                                    if chunked {
                                        for chunk in body.chunks(1024) {
                                            if socket.write_all(format!("{:x}\r\n",chunk.len()).as_bytes()).await.is_err() {return;}
                                            if socket.write_all(chunk).await.is_err() {return;}
                                            if socket.write_all(b"\r\n").await.is_err() {return;}
                                        }
                                        let _=socket.write_all(b"0\r\n\r\n").await;
                                    } else {let _=socket.write_all(&body).await;}
                                }
                                Reply::Trickle(body) => {
                                    let header=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: public, max-age=3600\r\nContent-Length: {}\r\n\r\n",body.len());
                                    if socket.write_all(header.as_bytes()).await.is_err() {return;}
                                    for byte in body { if socket.write_all(&[byte]).await.is_err() {return;} tokio::time::sleep(Duration::from_millis(100)).await; }
                                }
                            }
                        });
                    }
                    _ = handlers.join_next(), if !handlers.is_empty() => {}
                }
            }
        });
        Self {
            url,
            reply,
            hits,
            requests,
            task,
        }
    }
    fn set(&self, reply: Reply) {
        *self.reply.lock().unwrap() = reply;
    }
    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
    fn verifier(&self) -> Verifier {
        Verifier::construct(config(&self.url), true, now).unwrap()
    }
}
fn config(endpoint: &str) -> Config {
    Config {
        profile: ContractProfile::Forma247,
        issuer: "synthetic-fauth-issuer".into(),
        audience: "synthetic-genui-audience".into(),
        jwks_url: endpoint.into(),
        max_access_lifetime: Duration::from_secs(900),
    }
}
async fn allow_refresh(v: &Verifier) {
    v.cache.lock().await.attempted_at = Some(Instant::now() - REFRESH_INTERVAL);
}

#[test]
fn public_source_profile_vector_has_independent_key_identity() {
    let key = parse_jwks(
        include_bytes!("../fixtures/jwks-247.json"),
        ContractProfile::Forma247,
    )
    .unwrap()
    .pop()
    .unwrap();
    assert_eq!(key.kid, "If4x36FUomE");
    assert_eq!(
        key.bytes,
        [
            0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
            0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
            0xf7, 0x07, 0x51, 0x1a,
        ]
    );
}

#[tokio::test]
async fn actual_ed25519_identity_contract_and_fresh_key_cache() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    let identity = v.verify(&key.token(), TENANT).await.unwrap();
    assert_eq!(identity.subject, claims()["sub"].as_str().unwrap());
    assert_eq!(identity.tenant_id, TENANT);
    assert_eq!(format!("{identity:?}"), "VerifiedIdentity([redacted])");
    for _ in 0..4 {
        v.verify(&key.token(), TENANT).await.unwrap();
    }
    assert_eq!(mock.hits(), 1);
    let requests = mock.requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /auth/jwks HTTP/1.1"));
    assert!(!requests[0].to_ascii_lowercase().contains("authorization:"));
}

#[tokio::test]
async fn invalid_signature_and_claims_never_become_identity() {
    let key = IssuerKey::fresh();
    let other = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    v.verify(&key.token(), TENANT).await.unwrap();
    assert_eq!(
        v.verify(&other.sign(&key.header(), &claims()), TENANT)
            .await
            .err(),
        Some(Refusal::InvalidSignature)
    );
    let cases = [
        ("iss", json!("other")),
        ("aud", json!("wrong-client-guess")),
        ("aud", json!(["synthetic-genui-audience"])),
        ("sub", json!("")),
        ("tenant_id", Value::Null),
        ("tenant_id", json!("")),
        ("exp", json!(NOW)),
        ("exp", json!(NOW - 1)),
        ("iat", json!(NOW + 61)),
        ("exp", json!(NOW + 901)),
        ("exp", json!(true)),
        ("exp", json!(1800000900.0)),
        ("roles", json!([1])),
        ("cnf", json!({"jkt":"unqualified-dpop"})),
    ];
    for (field, value) in cases {
        let mut c = claims();
        c[field] = value;
        assert!(
            v.verify(&key.sign(&key.header(), &c), TENANT)
                .await
                .is_err(),
            "field={field}"
        );
    }
    let mut c = claims();
    c["tenant_id"] = json!("00000000-0000-4000-8000-000000000002");
    assert_eq!(
        v.verify(&key.sign(&key.header(), &c), TENANT).await.err(),
        Some(Refusal::ForeignTenant)
    );
    assert_eq!(mock.hits(), 1);
}

#[tokio::test]
async fn roles_email_and_unknown_subject_do_not_create_membership() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    let mut c = claims();
    c["roles"] = json!(["admin", "owner"]);
    c["email"] = json!("not-a-member@example.invalid");
    c["sub"] = json!("unknown-to-application");
    let identity = v
        .verify(&key.sign(&key.header(), &c), TENANT)
        .await
        .unwrap();
    assert_eq!(identity.subject, "unknown-to-application");
    // Verification accurately returns an external identity. No membership API,
    // role grant, app Principal, session cookie or resource access exists here.
    assert_eq!(format!("{identity:?}"), "VerifiedIdentity([redacted])");
}

#[tokio::test]
async fn wrong_algorithm_type_external_keys_and_ambiguous_json_refuse_before_fetch() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    for (field, value) in [
        ("alg", json!("none")),
        ("alg", json!("HS256")),
        ("alg", json!("ES256")),
        ("typ", json!("secevent+jwt")),
        ("kid", json!("")),
        ("jku", json!("https://attacker.invalid/keys")),
        ("jwk", key.jwk()),
        ("crit", json!(["b64"])),
    ] {
        let mut h = key.header();
        h[field] = value;
        assert!(
            v.verify(&key.sign(&h, &claims()), TENANT).await.is_err(),
            "field={field}"
        );
    }
    let h = serde_json::to_string(&key.header()).unwrap();
    let c = serde_json::to_string(&claims()).unwrap();
    for duplicate in [
        format!("{{\"alg\":\"EdDSA\",{}", &h[1..]),
        format!("{{\"\\u0061lg\":\"EdDSA\",{}", &h[1..]),
    ] {
        assert!(v
            .verify(&key.sign_raw(&duplicate, &c), TENANT)
            .await
            .is_err());
    }
    let dup = format!("{{\"tenant_id\":\"{TENANT}\",{}", &c[1..]);
    assert!(v.verify(&key.sign_raw(&h, &dup), TENANT).await.is_err());
    let mut claim = claims();
    claim["organization_id"] = json!(TENANT);
    assert!(v
        .verify(&key.sign(&key.header(), &claim), TENANT)
        .await
        .is_err());
    for token in [
        "a.b".into(),
        "a.b.c.d".into(),
        "a..c".into(),
        "x".repeat(MAX_TOKEN_BYTES + 1),
        format!("{}=", key.token()),
    ] {
        assert!(v.verify(&token, TENANT).await.is_err());
    }
    assert_eq!(mock.hits(), 0);
}

#[tokio::test]
async fn jwks_closed_shapes_duplicate_fields_and_mixed_key_purpose_refuse() {
    let key = IssuerKey::fresh();
    let other = IssuerKey::fresh();
    for (field, value) in [
        ("kty", json!("RSA")),
        ("alg", json!("HS256")),
        ("use", json!("enc")),
        ("crv", json!("X25519")),
        ("kid", json!(other.kid())),
        ("x", json!(URL_SAFE_NO_PAD.encode([0; 31]))),
        ("d", json!("private-material-must-not-be-published")),
    ] {
        let mut k = key.jwk();
        k[field] = value;
        assert!(
            parse_jwks(
                &serde_json::to_vec(&json!({"keys":[k]})).unwrap(),
                ContractProfile::Forma247
            )
            .is_err(),
            "field={field}"
        );
    }
    for keys in [
        vec![],
        vec![key.jwk(), other.jwk()],
        vec![key.jwk(), key.jwk()],
    ] {
        assert_eq!(
            parse_jwks(
                &serde_json::to_vec(&json!({"keys":keys})).unwrap(),
                ContractProfile::Forma247
            )
            .err(),
            Some(Refusal::UnqualifiedKeyPurpose)
        );
    }
    let k = serde_json::to_string(&key.jwk()).unwrap();
    for raw in [
        format!("{{\"keys\":[{k}],\"keys\":[{k}]}}"),
        format!("{{\"keys\":[{{\"kid\":\"{}\",{}]}}", key.kid(), &k[1..]),
    ] {
        assert!(parse_jwks(raw.as_bytes(), ContractProfile::Forma247).is_err());
    }
}

#[tokio::test]
async fn unknown_key_cooldown_rotation_and_authoritative_replacement() {
    let old = IssuerKey::fresh();
    let new = IssuerKey::fresh();
    let mock = Mock::start(ok(old.jwks())).await;
    let v = mock.verifier();
    v.verify(&old.token(), TENANT).await.unwrap();
    mock.set(ok(new.jwks()));
    assert_eq!(
        v.verify(&new.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    assert_eq!(mock.hits(), 1);
    v.verify(&old.token(), TENANT).await.unwrap();
    allow_refresh(&v).await;
    v.verify(&new.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 2);
    assert_eq!(
        v.verify(&old.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    assert_eq!(v.cache.lock().await.keys[0].kid, new.kid());
}

#[tokio::test]
async fn unknown_key_failure_cannot_poison_extend_or_keep_cached_signin_authority() {
    let known = IssuerKey::fresh();
    let unknown = IssuerKey::fresh();
    let mock = Mock::start(ok(known.jwks())).await;
    let v = mock.verifier();
    v.verify(&known.token(), TENANT).await.unwrap();
    allow_refresh(&v).await;
    mock.set(Reply::Body {
        status: 503,
        headers: "Cache-Control: no-store\r\n".into(),
        body: b"{}".to_vec(),
        delay: Duration::ZERO,
        chunked: false,
    });
    assert_eq!(
        v.verify(&unknown.token(), TENANT).await.err(),
        Some(Refusal::KeysetUnavailable)
    );
    assert_eq!(mock.hits(), 2);
    assert!(v.cache.lock().await.keys.is_empty());
    assert!(v.cache.lock().await.fresh_until.is_none());
    assert_eq!(
        v.verify(&known.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    assert_eq!(mock.hits(), 2);
    mock.set(ok(known.jwks()));
    allow_refresh(&v).await;
    v.verify(&known.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 3);
}

#[tokio::test]
async fn unknown_key_success_never_installs_untrusted_request_key() {
    let known = IssuerKey::fresh();
    let unknown = IssuerKey::fresh();
    let mock = Mock::start(ok(known.jwks())).await;
    let v = mock.verifier();
    assert_eq!(
        v.verify(&unknown.token(), TENANT).await.err(),
        Some(Refusal::UnknownKey)
    );
    assert_eq!(v.cache.lock().await.keys[0].kid, known.kid());
    v.verify(&known.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_unknown_keys_share_one_attempt_budget_and_recover() {
    let known = IssuerKey::fresh();
    let mock = Mock::start(ok(known.jwks())).await;
    let v = Arc::new(mock.verifier());
    let mut jobs = JoinSet::new();
    for _ in 0..24 {
        let v = v.clone();
        let token = IssuerKey::fresh().token();
        jobs.spawn(async move { v.verify(&token, TENANT).await.err().unwrap() });
    }
    let mut unknown = 0;
    let mut throttled = 0;
    while let Some(result) = jobs.join_next().await {
        match result.unwrap() {
            Refusal::UnknownKey => unknown += 1,
            Refusal::RefreshThrottled => throttled += 1,
            other => panic!("unexpected fixed code: {other:?}"),
        }
    }
    assert_eq!((unknown, throttled, mock.hits()), (1, 23, 1));
    v.verify(&known.token(), TENANT).await.unwrap();
}

#[tokio::test]
async fn stale_cache_is_never_grace_authority_and_freshness_starts_at_request() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    let before = Instant::now();
    v.verify(&key.token(), TENANT).await.unwrap();
    assert!(
        v.cache.lock().await.fresh_until.unwrap()
            <= before + MAX_CACHE_AGE + Duration::from_millis(20)
    );
    {
        let mut c = v.cache.lock().await;
        c.fresh_until = Some(Instant::now());
    }
    assert_eq!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    allow_refresh(&v).await;
    v.verify(&key.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 2);
}

#[test]
fn cache_headers_cannot_create_unbounded_or_stale_lifetimes() {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=999999".parse().unwrap(),
    );
    assert_eq!(cache_ttl(&headers).unwrap(), MAX_CACHE_AGE);
    headers.insert(header::AGE, "3500".parse().unwrap());
    assert_eq!(cache_ttl(&headers).unwrap(), Duration::from_secs(100));
    headers.insert(header::AGE, "3600".parse().unwrap());
    assert_eq!(cache_ttl(&headers).err(), Some(Refusal::KeysetUnavailable));
    headers.remove(header::AGE);
    for value in [
        "no-store",
        "no-cache, max-age=3600",
        "max-age=1, max-age=2",
        "max-age=bad",
        "public",
    ] {
        headers.insert(header::CACHE_CONTROL, value.parse().unwrap());
        assert!(cache_ttl(&headers).is_err());
    }
}

#[tokio::test]
async fn actual_http_redirect_status_invalid_and_streamed_oversize_are_refused() {
    let key = IssuerKey::fresh();
    let destination = Mock::start(ok(key.jwks())).await;
    let redirect = Mock::start(Reply::Body {
        status: 302,
        headers: format!(
            "Location: {}\r\nCache-Control: public, max-age=3600\r\n",
            destination.url
        ),
        body: vec![],
        delay: Duration::ZERO,
        chunked: false,
    })
    .await;
    assert_eq!(
        redirect.verifier().verify(&key.token(), TENANT).await.err(),
        Some(Refusal::KeysetUnavailable)
    );
    assert_eq!(destination.hits(), 0);
    for (body, chunked) in [
        (b"not JSON".to_vec(), false),
        (vec![b' '; MAX_JWKS_BYTES + 1], false),
        (vec![b' '; MAX_JWKS_BYTES + 1], true),
    ] {
        let mock = Mock::start(Reply::Body {
            status: 200,
            headers: "Cache-Control: public, max-age=3600\r\n".into(),
            body,
            delay: Duration::ZERO,
            chunked,
        })
        .await;
        assert!(mock.verifier().verify(&key.token(), TENANT).await.is_err());
        assert_eq!(mock.hits(), 1);
    }
}

#[tokio::test]
async fn partial_trickle_cannot_reset_total_network_deadline() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(Reply::Trickle(key.jwks())).await;
    let v = mock.verifier();
    let started = Instant::now();
    assert!(matches!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::Deadline | Refusal::KeysetUnavailable)
    ));
    eprintln!("slow_body_refusal_ms={}", started.elapsed().as_millis());
    assert!(
        started.elapsed() >= Duration::from_millis(1800)
            && started.elapsed() < Duration::from_millis(3500)
    );
    assert!(v.cache.lock().await.keys.is_empty());
    assert_eq!(mock.hits(), 1);
    assert_eq!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
}

#[tokio::test]
async fn deadline_covers_concurrent_cache_lock_wait_without_launching_fetch() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let v = mock.verifier();
    let held = v.cache.lock().await;
    let started = Instant::now();
    assert_eq!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::Deadline)
    );
    eprintln!("cache_lock_refusal_ms={}", started.elapsed().as_millis());
    assert!(started.elapsed() < Duration::from_millis(3500));
    assert_eq!(mock.hits(), 0);
    drop(held);
    v.verify(&key.token(), TENANT).await.unwrap();
}

#[tokio::test]
async fn cancellation_reserves_budget_and_leaves_no_authority() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(Reply::Trickle(key.jwks())).await;
    let v = Arc::new(mock.verifier());
    let token = key.token();
    let clone = v.clone();
    let request = tokio::spawn(async move { clone.verify(&token, TENANT).await });
    while mock.hits() == 0 {
        tokio::task::yield_now().await;
    }
    request.abort();
    let _ = request.await;
    assert!(v.cache.lock().await.keys.is_empty());
    assert_eq!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    mock.set(ok(key.jwks()));
    allow_refresh(&v).await;
    v.verify(&key.token(), TENANT).await.unwrap();
}

static MOVING_WALL: AtomicU64 = AtomicU64::new(NOW);
fn moving_now() -> Result<u64, Refusal> {
    Ok(MOVING_WALL.load(Ordering::SeqCst))
}
#[tokio::test]
async fn expiry_is_rechecked_after_fetch_before_identity_release() {
    MOVING_WALL.store(NOW, Ordering::SeqCst);
    let key = IssuerKey::fresh();
    let mock = Mock::start(Reply::Body {
        status: 200,
        headers: "Cache-Control: public, max-age=3600\r\n".into(),
        body: key.jwks(),
        delay: Duration::from_millis(100),
        chunked: false,
    })
    .await;
    let v = Arc::new(Verifier::construct(config(&mock.url), true, moving_now).unwrap());
    let token = key.token();
    let clone = v.clone();
    let request = tokio::spawn(async move { clone.verify(&token, TENANT).await });
    while mock.hits() == 0 {
        tokio::task::yield_now().await;
    }
    MOVING_WALL.store(NOW + 900, Ordering::SeqCst);
    assert_eq!(request.await.unwrap().err(), Some(Refusal::InvalidClaims));
}

#[test]
fn production_configuration_is_explicit_https_fixed_endpoint_only() {
    for endpoint in [
        "http://127.0.0.1:3000/auth/jwks",
        "file:///auth/jwks",
        "https://u:p@example.invalid/auth/jwks",
        "https://example.invalid/other",
        "https://example.invalid/auth/jwks?url=elsewhere",
        "https://example.invalid/auth/jwks#keys",
    ] {
        assert!(Verifier::new(config(endpoint)).is_err());
    }
    assert!(Verifier::new(config("https://auth.example.invalid/auth/jwks")).is_ok());
    let mut c = config("https://auth.example.invalid/auth/jwks");
    c.issuer.clear();
    assert!(Verifier::new(c).is_err());
    let mut c = config("https://auth.example.invalid/auth/jwks");
    c.max_access_lifetime = Duration::ZERO;
    assert!(Verifier::new(c).is_err());
}

static RELEASE_CLOCK_CALLS: AtomicUsize = AtomicUsize::new(0);
fn delayed_release_now() -> Result<u64, Refusal> {
    if RELEASE_CLOCK_CALLS.fetch_add(1, Ordering::SeqCst) == 1 {
        // Controlled synchronous delay after signature verification; a fresh
        // key at acquisition must not authorize release after its TTL expires.
        std::thread::sleep(Duration::from_millis(60));
    }
    Ok(NOW)
}

#[tokio::test]
async fn key_expiry_is_rechecked_after_signature_before_identity_release() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let mut v = mock.verifier();
    v.verify(&key.token(), TENANT).await.unwrap();
    v.cache.lock().await.fresh_until = Some(Instant::now() + Duration::from_millis(20));
    RELEASE_CLOCK_CALLS.store(0, Ordering::SeqCst);
    v.wall_now = delayed_release_now;
    assert_eq!(
        v.verify(&key.token(), TENANT).await.err(),
        Some(Refusal::KeysetUnavailable)
    );
    assert_eq!(mock.hits(), 1);
}

#[tokio::test]
async fn acquired_key_lease_is_refused_after_authoritative_refresh() {
    let old = IssuerKey::fresh();
    let new = IssuerKey::fresh();
    let mock = Mock::start(ok(old.jwks())).await;
    let v = mock.verifier();
    let lease = v
        .key(&old.kid(), Instant::now() + VERIFY_DEADLINE)
        .await
        .unwrap();
    assert!(validate_lease(&lease, &*v.cache.lock().await).is_ok());
    allow_refresh(&v).await;
    mock.set(ok(new.jwks()));
    v.verify(&new.token(), TENANT).await.unwrap();
    assert_eq!(
        validate_lease(&lease, &*v.cache.lock().await).err(),
        Some(Refusal::KeysetUnavailable)
    );
}

fn access_set(keys: &[&IssuerKey]) -> Vec<u8> {
    serde_json::to_vec(&json!({"keys":keys.iter().map(|key| key.jwk()).collect::<Vec<_>>()}))
        .unwrap()
}
fn verifier248(mock: &Mock) -> Verifier {
    let mut cfg = config(&mock.url);
    cfg.profile = ContractProfile::Forma248AccessOnly;
    Verifier::construct(cfg, true, now).unwrap()
}

#[tokio::test]
async fn access_only_profile_prepublication_overlap_and_observed_retirement() {
    let previous = IssuerKey::fresh();
    let current = IssuerKey::fresh();
    let next = IssuerKey::fresh();
    let future = IssuerKey::fresh();
    let mock = Mock::start(ok(access_set(&[&previous, &current, &next]))).await;
    let v = verifier248(&mock);
    for key in [&current, &previous, &next] {
        v.verify(&key.token(), TENANT).await.unwrap();
    }
    // The prepublished next key works inside cooldown without another fetch.
    assert_eq!(mock.hits(), 1);
    mock.set(ok(access_set(&[&current, &next, &future])));
    // No remote-retirement claim: a matching fresh cached key still verifies.
    v.verify(&previous.token(), TENANT).await.unwrap();
    assert_eq!(
        v.verify(&future.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    allow_refresh(&v).await;
    v.verify(&future.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 2);
    assert_eq!(
        v.verify(&previous.token(), TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    allow_refresh(&v).await;
    assert_eq!(
        v.verify(&previous.token(), TENANT).await.err(),
        Some(Refusal::UnknownKey)
    );
    v.verify(&next.token(), TENANT).await.unwrap();
    assert_eq!(mock.hits(), 3);
}

#[tokio::test]
async fn ssf_signature_with_access_type_and_claims_cannot_select_access_key() {
    let current = IssuerKey::fresh();
    let next = IssuerKey::fresh();
    let ssf = IssuerKey::fresh();
    let mock = Mock::start(ok(access_set(&[&current, &next]))).await;
    let v = verifier248(&mock);
    let forged = ssf.token(); // Genuine SSF-key signature, but typ=JWT and valid access claims.
    let (_, _, input, sig) = parse_token(&forged).unwrap();
    signature::UnparsedPublicKey::new(&signature::ED25519, ssf.0.public_key().as_ref())
        .verify(input.as_bytes(), &sig)
        .unwrap();
    assert_eq!(
        v.verify(&forged, TENANT).await.err(),
        Some(Refusal::UnknownKey)
    );
    assert_eq!(
        v.verify(&forged, TENANT).await.err(),
        Some(Refusal::RefreshThrottled)
    );
    v.verify(&current.token(), TENANT).await.unwrap();
    allow_refresh(&v).await;
    assert_eq!(
        v.verify(&forged, TENANT).await.err(),
        Some(Refusal::UnknownKey)
    );
    // Rewriting kid to an admitted access kid cannot turn the SSF signature valid.
    assert_eq!(
        v.verify(&ssf.sign(&current.header(), &claims()), TENANT)
            .await
            .err(),
        Some(Refusal::InvalidSignature)
    );
    let mut event_header = ssf.header();
    event_header["typ"] = json!("secevent+jwt");
    assert_eq!(
        v.verify(&ssf.sign(&event_header, &claims()), TENANT)
            .await
            .err(),
        Some(Refusal::UnsupportedToken)
    );
    assert_eq!(mock.hits(), 2);
    assert!(mock
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.starts_with("GET /auth/jwks HTTP/1.1")));
    let mut cfg = config("https://auth.example.invalid/ssf/jwks");
    cfg.profile = ContractProfile::Forma248AccessOnly;
    assert!(Verifier::new(cfg).is_err());
}

#[tokio::test]
async fn access_only_set_has_bounded_unique_validated_keys_and_atomic_replacement() {
    let keys: Vec<_> = (0..MAX_ACCESS_KEYS + 1)
        .map(|_| IssuerKey::fresh())
        .collect();
    let refs: Vec<_> = keys.iter().collect();
    let mock = Mock::start(ok(access_set(&refs[..MAX_ACCESS_KEYS]))).await;
    let v = verifier248(&mock);
    for key in &keys[..MAX_ACCESS_KEYS] {
        v.verify(&key.token(), TENANT).await.unwrap();
    }
    assert_eq!(mock.hits(), 1);
    for bytes in [
        access_set(&refs),
        access_set(&[&keys[0], &keys[0]]),
        b"{\"keys\":[]}".to_vec(),
    ] {
        allow_refresh(&v).await;
        mock.set(ok(bytes));
        assert!(v
            .verify(&keys[MAX_ACCESS_KEYS].token(), TENANT)
            .await
            .is_err());
        assert!(v.cache.lock().await.keys.is_empty());
    }
    let mut corrupt = keys[1].jwk();
    corrupt["alg"] = json!("RS256");
    mock.set(ok(serde_json::to_vec(
        &json!({"keys":[keys[0].jwk(),corrupt]}),
    )
    .unwrap()));
    allow_refresh(&v).await;
    assert_eq!(
        v.verify(&keys[0].token(), TENANT).await.err(),
        Some(Refusal::KeysetMalformed)
    );
    assert!(v.cache.lock().await.keys.is_empty());
    mock.set(ok(access_set(&[&keys[0], &keys[1]])));
    allow_refresh(&v).await;
    v.verify(&keys[0].token(), TENANT).await.unwrap();
    v.verify(&keys[1].token(), TENANT).await.unwrap();
}
