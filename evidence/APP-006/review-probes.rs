// Appended to a disposable copy of the candidate's tests module. The candidate
// library and existing tests remain byte-identical; private helpers are reused
// only to drive the real verifier with ephemeral keys and loopback HTTP.
#[tokio::test]
async fn review_external_critical_headers_and_optional_duplicates_are_closed() {
    let key = IssuerKey::fresh();
    let mock = Mock::start(ok(key.jwks())).await;
    let verifier = mock.verifier();
    for (field, value) in [
        ("b64", json!(false)),
        ("x5u", json!("https://attacker.invalid/certificate")),
        ("x5c", json!(["not-a-certificate"])),
        ("crit", json!([])),
    ] {
        let mut header = key.header(); header[field] = value;
        assert!(verifier.verify(&key.sign(&header, &claims()), TENANT).await.is_err());
    }
    let header = serde_json::to_string(&key.header()).unwrap();
    let body = serde_json::to_string(&claims()).unwrap();
    for prefix in ["\"cnf\":null,\"cnf\":null,", "\"name\":null,", "\"\\u0074enant_id\":null,"] {
        let duplicate = format!("{{{prefix}{}", &body[1..]);
        assert!(verifier.verify(&key.sign_raw(&header, &duplicate), TENANT).await.is_err());
    }
    assert_eq!(mock.hits(), 0);
}

static REVIEW_CLOCK_CALLS: AtomicUsize = AtomicUsize::new(0);
static REVIEW_CLOCK_RELEASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn review_block_second_clock() -> Result<u64, Refusal> {
    if REVIEW_CLOCK_CALLS.fetch_add(1, Ordering::SeqCst) == 1 {
        let until = std::time::Instant::now() + Duration::from_millis(1500);
        while !REVIEW_CLOCK_RELEASE.load(Ordering::SeqCst) && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(NOW)
}
fn review_slow_second_clock() -> Result<u64, Refusal> {
    if REVIEW_CLOCK_CALLS.fetch_add(1, Ordering::SeqCst) == 1 {
        std::thread::sleep(Duration::from_millis(1100));
    }
    Ok(NOW)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_short_jwks_ttl_expires_before_identity_release() {
    REVIEW_CLOCK_CALLS.store(0, Ordering::SeqCst);
    let key = IssuerKey::fresh();
    let mock = Mock::start(Reply::Body { status:200, headers:"Cache-Control: public, max-age=1\r\n".into(), body:key.jwks(), delay:Duration::ZERO, chunked:false }).await;
    let verifier = Verifier::construct(config(&mock.url), true, review_slow_second_clock).unwrap();
    let result = verifier.verify(&key.token(), TENANT).await;
    assert_eq!(result.err(), Some(Refusal::KeysetUnavailable));
    assert_eq!(mock.hits(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_delayed_release_and_failed_refresh_are_serialized() {
    REVIEW_CLOCK_CALLS.store(100, Ordering::SeqCst);
    REVIEW_CLOCK_RELEASE.store(false, Ordering::SeqCst);
    let known = IssuerKey::fresh(); let unknown = IssuerKey::fresh();
    let mock = Mock::start(ok(known.jwks())).await;
    let verifier = Arc::new(Verifier::construct(config(&mock.url), true, review_block_second_clock).unwrap());
    verifier.verify(&known.token(), TENANT).await.unwrap();
    allow_refresh(&verifier).await;
    REVIEW_CLOCK_CALLS.store(0, Ordering::SeqCst);
    let pending = { let verifier=verifier.clone(); let token=known.token(); tokio::spawn(async move { verifier.verify(&token,TENANT).await }) };
    while REVIEW_CLOCK_CALLS.load(Ordering::SeqCst) < 2 { tokio::task::yield_now().await; }
    mock.set(Reply::Body { status:503, headers:"Cache-Control: no-store\r\n".into(), body:b"{}".to_vec(), delay:Duration::ZERO, chunked:false });
    let refreshing = { let verifier=verifier.clone(); let token=unknown.token(); tokio::spawn(async move { verifier.verify(&token,TENANT).await }) };
    tokio::time::sleep(Duration::from_millis(20)).await;
    // The release decision holds the cache mutex. Refresh must not invalidate
    // the key between final validation and that already-linearized decision.
    assert_eq!(mock.hits(),1);
    REVIEW_CLOCK_RELEASE.store(true, Ordering::SeqCst);
    pending.await.unwrap().unwrap();
    assert_eq!(refreshing.await.unwrap().err(),Some(Refusal::KeysetUnavailable));
    assert!(verifier.verify(&known.token(),TENANT).await.is_err());
    assert_eq!(mock.hits(),2);
}
