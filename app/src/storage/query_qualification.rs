//! APP-012 query campaign. Isolated test executable only: local Docker,
//! OpenSSL, a throwaway CA and the locked Python environment with DuckDB
//! 2.0.0 (`GENUI_NATIVE_FIXTURE_PYTHON`, the operator lock), as the APP-008,
//! APP-073 and APP-010 campaigns.
//!
//! The operator package registers the uploaded Northstar events revision as a
//! NEW source revision with its real digests (`register-source-revision`) and
//! binds a new saved work to it (`create-bound-work`); nothing seeded is
//! rewritten. The data role then authorizes runs (`Data::run_query`), the
//! confined engine child executes them over the read-only cache, the release
//! is re-authorized (`Data::release_result`) and the receipt is built and
//! validated against the contract schema by the contract validator.
use super::{
    admission_qualification::{operator, operator_as, refused_for},
    definition_qualification::{grant, request},
    qualification::{docker, fixture_python, Fixture},
    qualification_cases::{check, APP, ORG, OTHER, PRINCIPAL, REVIEWER, WORK},
    *,
};
use crate::query::{
    receipt,
    tests::{
        cache_northstar, measure_latency, real_engine, repository, seeded_query, window, Directory,
    },
    Failure, QueryError,
};
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

const SOURCE: &str = "00000000-0000-4000-8000-00000000006e";
const SEEDED_REVISION: &str = "00000000-0000-4000-8000-00000000006f";
/// The new revision of the uploaded objects: `upload-sources`' identity,
/// uuid5(organization, content digest) re-stamped as version 4.
const REVISION: &str = "9161c1e4-27c0-4cbb-addf-eb7ce504bf45";
/// The saved work bound to it.
const BOUND: &str = "00000000-0000-4000-8000-0000000000f2";
/// evidence/APP-077: the uploaded objects' digests and the seeded contract.
const CONTENT: &str = "sha256:6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647";
const NORMALIZED: &str = "sha256:0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082";
const CONTRACT: &str = "sha256:b92d4b96754e5e9de1863dff09e615880ded8d57de22e7546b1f59f2f9178f64";
const PLACEHOLDER: &str = "sha256:bb16c6e424ae249ed26e04d801d5c3c4efbf3852f969a02b4293ffb3a785c119";
const DEADLINE: Duration = Duration::from_millis(1800);

fn record(fixture: &Fixture, name: &str, value: &Value) -> String {
    let path = fixture.directory.0.join(name);
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path.to_str().unwrap().to_owned()
}
async fn rows(fixture: &Fixture, sql: &str) -> Vec<String> {
    fixture
        .operator()
        .await
        .client
        .query(sql, &[])
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect()
}
/// The contract validator's verdict on one document (contracts/validate.py).
fn contract_valid(document: &Value) -> bool {
    let mut child = Command::new(fixture_python())
        .current_dir(repository().join("contracts"))
        .args([
            "-c",
            "import json,sys; from validate import validate; validate(json.load(sys.stdin)); print('valid')",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(document).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    output.status.success() && output.stdout == b"valid\n"
}
fn numbers(result: &crate::query::WorkerResult) -> Vec<Vec<Value>> {
    result.rows.clone()
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked Python environment with DuckDB 2.0.0"]
fn native_query_campaign() {
    std::thread::Builder::new()
        .name("app012-query-campaign".into())
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
    let fixture = Fixture::new();
    fixture.sql(
        "ALTER ROLE genui_migration PASSWORD 'synthetic-migration'",
        "genui",
    );
    let mut checks = Vec::new();
    for org in [ORG, OTHER] {
        operator(&fixture, &["backfill", "--organization", org]);
    }

    // --- The uploaded revision becomes a NEW revision row; nothing is rewritten ---
    let uploaded = json!({
        "organization_id": ORG, "source_id": SOURCE, "revision_id": REVISION,
        "content_digest": CONTENT, "normalized_digest": NORMALIZED, "object_digest": NORMALIZED,
        "row_count": 6, "contract_digest": CONTRACT,
    });
    let path = record(&fixture, "record.json", &uploaded);
    let registered = operator(
        &fixture,
        &[
            "register-source-revision",
            "--organization",
            ORG,
            "--record",
            &path,
        ],
    );
    assert_eq!(registered["changed"], true);
    assert_eq!(registered["row_count"], 6);
    let again = operator(
        &fixture,
        &[
            "register-source-revision",
            "--organization",
            ORG,
            "--record",
            &path,
        ],
    );
    assert_eq!(again["changed"], false);
    let mut differing = uploaded.clone();
    differing["normalized_digest"] = json!(format!("sha256:{}", "1".repeat(64)));
    differing["object_digest"] = differing["normalized_digest"].clone();
    let differing = record(&fixture, "differing.json", &differing);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "register-source-revision",
                "--organization",
                ORG,
                "--record",
                &differing,
            ],
        ),
        "revision_exists_differs",
    );
    let mut underived = uploaded.clone();
    underived["revision_id"] = json!("00000000-0000-4000-8000-0000000000f3");
    let underived = record(&fixture, "underived.json", &underived);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "register-source-revision",
                "--organization",
                ORG,
                "--record",
                &underived,
            ],
        ),
        "record_revision_refused",
    );
    let mut mismatched = uploaded.clone();
    mismatched["object_digest"] = json!(CONTENT);
    let mismatched = record(&fixture, "mismatched.json", &mismatched);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "register-source-revision",
                "--organization",
                ORG,
                "--record",
                &mismatched,
            ],
        ),
        "record_digest_refused",
    );
    let foreign = record(&fixture, "foreign.json", &uploaded);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "register-source-revision",
                "--organization",
                OTHER,
                "--record",
                &foreign,
            ],
        ),
        "record_organization_refused",
    );
    assert_eq!(
        rows(&fixture, &format!("SELECT revision_id::text||' '||normalized_digest::text FROM genui.source_revisions WHERE organization_id='{ORG}' AND source_id='{SOURCE}' ORDER BY revision_id")).await,
        [format!("{SEEDED_REVISION} {PLACEHOLDER}"), format!("{REVISION} {NORMALIZED}")]
    );
    check(
        &mut checks,
        "register-source-revision writes one new source revision with the uploaded objects' real digests; a rerun is unchanged; a differing record, a revision id not derived from the content digest, a disagreeing object digest and another organization are refused; the seeded placeholder revision is untouched",
    );

    let bound = operator(
        &fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            BOUND,
            "--creator",
            PRINCIPAL,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );
    assert_eq!(bound["changed"], true);
    let events = bound["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["input_id"] == "events")
        .unwrap()
        .clone();
    assert_eq!(events["source_revision_id"], REVISION);
    assert_eq!(events["source_digest"], NORMALIZED);
    let rerun = operator(
        &fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            BOUND,
            "--creator",
            PRINCIPAL,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );
    assert_eq!(rerun["changed"], false);
    refused_for(
        &operator_as(
            &fixture,
            "genui_migration",
            "synthetic-migration",
            &[
                "create-bound-work",
                "--organization",
                ORG,
                "--app",
                APP,
                "--from-work",
                WORK,
                "--work",
                "00000000-0000-4000-8000-0000000000f4",
                "--creator",
                REVIEWER,
                "--input",
                "events",
                "--source",
                SOURCE,
                "--revision",
                REVISION,
            ],
        ),
        "creator_lacks_authority",
    );
    assert_eq!(
        rows(&fixture, &format!("SELECT source_revision_id::text||' '||source_digest::text FROM genui.work_bindings WHERE organization_id='{ORG}' AND work_id='{WORK}' AND input_id='events'")).await,
        [format!("{SEEDED_REVISION} {PLACEHOLDER}")]
    );
    check(
        &mut checks,
        "create-bound-work writes a new saved work at version 1 on the same publication whose events input binds the new revision; the seeded work, its binding and its record are unchanged; a creator without run and source_read is refused",
    );

    // --- Authorized member: exact result, released, receipted ---
    let python = fixture_python();
    let root = Directory::new("native");
    assert_eq!(
        cache_northstar(&python, &root.0.join("cache"), ORG, REVISION),
        NORMALIZED
    );
    let engine = Arc::new(real_engine(&python, &root.0));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let member = request(&stores, ORG, PRINCIPAL, 1);
    let run = stores
        .data
        .run_query(&member, APP, BOUND, "total", &window())
        .await
        .unwrap();
    assert_eq!(run.source().normalized_digest(), NORMALIZED);
    assert_eq!(run.view_query(), &seeded_query("total"));
    let result = engine.execute(&run, DEADLINE).await.unwrap();
    assert_eq!(numbers(&result), vec![vec![json!("1140000")]]);
    stores.data.release_result(&member, &run).await.unwrap();
    let total = receipt::build(&run, &result).unwrap();
    assert!(contract_valid(&total.document), "{}", total.document);
    for (key, value) in [
        ("mode", json!("published")),
        ("organization_id", json!(ORG)),
        ("app_id", json!(APP)),
        ("work_id", json!(BOUND)),
        (
            "publication_id",
            json!(super::qualification_cases::PUBLICATION),
        ),
        ("principal_id", json!(PRINCIPAL)),
        ("membership_epoch", json!(1)),
        ("policy_epoch", json!(1)),
        ("filter_epoch", json!(0)),
        ("record_snapshot_sequence", json!(0)),
        ("partial", json!(false)),
        ("truncated_page", json!(false)),
        ("result_digest", json!(result.result_digest)),
    ] {
        assert_eq!(total.document[key], value, "{key}");
    }
    assert_eq!(
        total.document["source_revisions"],
        json!([{"input_id": "events", "source_id": SOURCE, "source_revision_id": REVISION, "source_digest": NORMALIZED}])
    );
    let mut filtered = window();
    filtered["destination"] = json!("198.51.100.7");
    filtered["filter_epoch"] = json!(1);
    let destination = stores
        .data
        .run_query(&member, APP, BOUND, "total", &filtered)
        .await
        .unwrap();
    let destination_result = engine.execute(&destination, DEADLINE).await.unwrap();
    assert_eq!(numbers(&destination_result), vec![vec![json!("900000")]]);
    stores
        .data
        .release_result(&member, &destination)
        .await
        .unwrap();
    let destination_receipt = receipt::build(&destination, &destination_result).unwrap();
    assert!(contract_valid(&destination_receipt.document));
    assert_eq!(destination_receipt.document["filter_epoch"], 1);
    assert_ne!(destination_receipt.digest, total.digest);
    let timeline = stores
        .data
        .run_query(&member, APP, BOUND, "timeline", &window())
        .await
        .unwrap();
    let timeline_result = engine.execute(&timeline, DEADLINE).await.unwrap();
    let buckets: Vec<(String, String)> = timeline_result
        .rows
        .iter()
        .map(|r| {
            (
                r[0].as_str().unwrap().to_owned(),
                r[2].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        buckets,
        [
            ("2026-10-22T10:00:00Z", "60000"),
            ("2026-10-22T10:01:00Z", "120000"),
            ("2026-10-22T10:02:00Z", "300000"),
            ("2026-10-22T10:03:00Z", "600000"),
            ("2026-10-22T10:05:00Z", "60000"),
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    stores
        .data
        .release_result(&member, &timeline)
        .await
        .unwrap();
    let timeline_receipt = receipt::build(&timeline, &timeline_result).unwrap();
    assert!(contract_valid(&timeline_receipt.document));
    check(
        &mut checks,
        "authorized_member_gets_exact_result_and_receipt_digest: total 1,140,000, destination 198.51.100.7 900,000 and the five timeline buckets of the APP-011 oracle, each released after re-authorization with a contract-valid published receipt pinning the new revision's real digest",
    );

    // --- The seeded placeholder work: authorized, but never a number ---
    let placeholder = stores
        .data
        .run_query(&member, APP, WORK, "total", &window())
        .await
        .unwrap();
    assert_eq!(placeholder.source().normalized_digest(), PLACEHOLDER);
    assert_eq!(
        engine.execute(&placeholder, DEADLINE).await,
        Err(QueryError::SourceUnavailable)
    );
    check(
        &mut checks,
        "the seeded work still binds the placeholder digest; with no object of that digest in the cache it is unavailable and no number is produced",
    );

    // --- Denials before release ---
    for (label, attempt) in [
        ("unknown view", stores.data.run_query(&member, APP, BOUND, "missing", &window()).await),
        ("form view without a query", stores.data.run_query(&member, APP, BOUND, "record_observation", &json!({})).await),
        ("two-input lookup view", stores.data.run_query(&member, APP, BOUND, "destinations", &json!({})).await),
        ("unclosed parameters", stores.data.run_query(&member, APP, BOUND, "total", &json!({"sql": "SELECT 1"})).await),
        ("money parameter", stores.data.run_query(&member, APP, BOUND, "total", &json!({"amount_cents": {"type": "money", "minor_units": "20000", "currency": "USD", "minor_unit_exponent": 2}})).await),
        ("foreign organization scope", stores.data.run_query(&request(&stores, OTHER, PRINCIPAL, 1), APP, BOUND, "total", &window()).await),
        ("member without run or work access", stores.data.run_query(&request(&stores, ORG, REVIEWER, 1), APP, BOUND, "total", &window()).await),
        ("stale epoch", stores.data.run_query(&request(&stores, ORG, PRINCIPAL, 2), APP, BOUND, "total", &window()).await),
    ] {
        assert_eq!(attempt.map(|_| ()), Err(Error::Denied), "{label}");
    }
    // A run of one member is never released to another.
    let other = request(&stores, ORG, REVIEWER, 1);
    assert_eq!(
        stores.data.release_result(&other, &run).await,
        Err(Error::Denied)
    );
    let canary = request(&stores, OTHER, PRINCIPAL, 1);
    assert_eq!(
        stores.data.release_result(&canary, &run).await,
        Err(Error::Denied)
    );
    check(
        &mut checks,
        "unknown and query-less views, the lookup view, unclosed or money parameters, another organization's scope, a member without run or work access and a stale epoch are denied before any engine runs; a run is never released to another principal or organization",
    );

    // --- Authorization changes between execution and delivery ---
    let pending = stores
        .data
        .run_query(&member, APP, BOUND, "total", &window())
        .await
        .unwrap();
    let computed = engine.execute(&pending, DEADLINE).await.unwrap();
    assert_eq!(numbers(&computed), vec![vec![json!("1140000")]]);
    let revoked = operator(
        &fixture,
        &[
            "revoke-permission",
            "--organization",
            ORG,
            "--principal",
            PRINCIPAL,
            "--permission",
            "source_read",
            "--source",
            SOURCE,
        ],
    );
    assert_eq!(revoked["membership_epoch"], 2);
    assert_eq!(
        stores.data.release_result(&member, &pending).await,
        Err(Error::Denied)
    );
    let current = request(&stores, ORG, PRINCIPAL, 2);
    // The current epoch cannot claim the old run either, nor start a new one.
    assert_eq!(
        stores.data.release_result(&current, &pending).await,
        Err(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .run_query(&current, APP, BOUND, "total", &window())
            .await
            .map(|_| ()),
        Err(Error::Denied)
    );
    grant(&fixture, PRINCIPAL, "source_read", &["--source", SOURCE]);
    let regranted = stores
        .data
        .run_query(&current, APP, BOUND, "total", &window())
        .await
        .unwrap();
    let regranted_result = engine.execute(&regranted, DEADLINE).await.unwrap();
    operator(
        &fixture,
        &[
            "revoke-creator-authority",
            "--organization",
            ORG,
            "--app",
            APP,
            "--work",
            BOUND,
        ],
    );
    assert_eq!(
        stores.data.release_result(&current, &regranted).await,
        Err(Error::Denied)
    );
    assert_eq!(numbers(&regranted_result), vec![vec![json!("1140000")]]);
    let after = request(&stores, ORG, PRINCIPAL, 3);
    assert_eq!(
        stores
            .data
            .run_query(&after, APP, BOUND, "total", &window())
            .await
            .map(|_| ()),
        Err(Error::Denied)
    );
    // Membership revocation while a run waits for delivery (the seeded work,
    // where the member holds work grants).
    let seeded = stores
        .data
        .run_query(&after, APP, WORK, "total", &window())
        .await
        .unwrap();
    operator(
        &fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            PRINCIPAL,
        ],
    );
    assert_eq!(
        stores.data.release_result(&after, &seeded).await,
        Err(Error::Denied)
    );
    let revoked_member = request(&stores, ORG, PRINCIPAL, 4);
    assert_eq!(
        stores
            .data
            .run_query(&revoked_member, APP, WORK, "total", &window())
            .await
            .map(|_| ()),
        Err(Error::Denied)
    );
    check(
        &mut checks,
        "foreign_or_revoked_scope_denied_before_release_including_change_between_execution_and_delivery: a computed result is not released after source_read revocation, creator-authority revocation or membership revocation between execution and delivery, and the new epoch can neither claim it nor start a run until authority is current again",
    );

    // --- Per-request latency through the full authorized path ---
    let latency_member = request(&stores, ORG, super::definition_qualification::AUTHOR, 1);
    grant(
        &fixture,
        super::definition_qualification::AUTHOR,
        "run",
        &["--app", APP],
    );
    grant(
        &fixture,
        super::definition_qualification::AUTHOR,
        "source_read",
        &["--source", SOURCE],
    );
    grant(
        &fixture,
        super::definition_qualification::AUTHOR,
        "source_read",
        &["--source", "00000000-0000-4000-8000-000000000070"],
    );
    let latency_work = "00000000-0000-4000-8000-0000000000f5";
    operator(
        &fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            latency_work,
            "--creator",
            super::definition_qualification::AUTHOR,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );
    let mut full = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let run = stores
            .data
            .run_query(&latency_member, APP, latency_work, "total", &window())
            .await
            .unwrap();
        let result = engine.execute(&run, DEADLINE).await.unwrap();
        stores
            .data
            .release_result(&latency_member, &run)
            .await
            .unwrap();
        receipt::build(&run, &result).unwrap();
        full.push(started.elapsed());
    }
    full.sort();
    let ms = |d: Duration| (d.as_secs_f64() * 10_000.0).round() / 10.0;
    let run = stores
        .data
        .run_query(&latency_member, APP, latency_work, "total", &window())
        .await
        .unwrap();
    let child = measure_latency(&engine, &run, 40).await;
    let latency = json!({
        "child": child,
        "authorized_path_wall_ms": {
            "samples": full.len(),
            "p50": ms(crate::query::tests::percentile(&full, 50.0)),
            "p95": ms(crate::query::tests::percentile(&full, 95.0)),
            "max": ms(*full.last().unwrap()),
        },
    });
    println!("APP012 latency: {latency}");
    check(
        &mut checks,
        "per-request latency measured: run_query + engine child + release_result + receipt, and the child alone sequentially and in waves of two",
    );
    assert!(matches!(
        engine.execute(&run, Duration::from_millis(1)).await,
        Err(QueryError::Failed(Failure::Deadline))
    ));

    // The pools hold process-wide role capacity (2 per role, 8 in all, as
    // one task would): the storage checks' stores close before the route
    // checks open their own.
    stores.close().await.unwrap();
    // --- The JSON route and the work page over real APP-009 sessions ---
    let (returned, routes) = route_campaign(fixture, engine.clone(), &mut checks).await;
    let mut fixture = returned;
    let stats = engine.stats();

    docker(
        &["rm", "--force", "--volumes", &fixture.name],
        b"",
        "remove isolated database",
    );
    fixture.removed = true;
    std::fs::remove_dir_all(&fixture.directory.0).unwrap();
    check(
        &mut checks,
        "local database and private fixture files removed",
    );
    if let Some(path) = std::env::var_os("GENUI_QUERY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "scope": "APP-012 isolated native PostgreSQL TLS query authorization, confined DuckDB child and receipt qualification",
            "managed_acceptance": false,
            "server_version": fixture.server_version,
            "revision": {"organization_id": ORG, "source_id": SOURCE, "revision_id": REVISION,
                         "content_digest": CONTENT, "normalized_digest": NORMALIZED, "contract_digest": CONTRACT},
            "bound_work": BOUND,
            "receipts": {"total": total.document, "destination": destination_receipt.document, "timeline": timeline_receipt.document},
            "latency": latency,
            "routes": routes,
            "admission": {"peak_task_running": stats.peak_task_running, "peak_organization_running": stats.peak_organization_running,
                          "rejected": stats.rejected, "launched": stats.launched},
            "checks": checks,
            "passed": true,
            "limits": [
                "Synthetic local password authentication is not RDS IAM. The storage checks use synthetic registry-bound requests; the route and page checks use real APP-009 sessions minted through the product routes against the cfg(test) synthetic loopback issuer (SIMULATED provider), rendered by a recording page renderer (the SSR page is the binary's test).",
                "The revision cache is seeded locally with the bytes APP-077 uploads (same SHA-256); the credentialed S3 fetch is unit-tested against botocore's Stubber, not S3.",
                "The engine child runs on macOS arm64 with the locked venv, not in the linux image under Fargate; its latency is re-measured in APP-078's window.",
                "The registered record is written by the test with the APP-077 digests and the content-derived revision id; upload-sources itself (merged separately) is not run here."
            ]
        })).unwrap()).unwrap();
    }
}

// --- APP-012 second half: the JSON route and the work page --------------------

/// The saved work the route checks create for their signed-in member.
const ROUTE_WORK: &str = "00000000-0000-4000-8000-0000000000f6";
const INVENTORY: &str = "00000000-0000-4000-8000-000000000070";

type Raw = (axum::http::StatusCode, axum::http::HeaderMap, Vec<u8>);

/// A request outside the cookie-jar browser (concurrent or paused), with
/// the browser's cookies and same-origin fetch metadata.
fn raw_request(
    method: &str,
    uri: &str,
    cookies: &str,
    body: Vec<u8>,
) -> axum::http::Request<axum::body::Body> {
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("sec-fetch-site", "same-origin")
        .header(axum::http::header::COOKIE, cookies)
        .body(axum::body::Body::from(body))
        .unwrap();
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "192.0.2.30:5000".parse::<std::net::SocketAddr>().unwrap(),
    ));
    request
}
async fn send_raw(router: axum::Router, request: axum::http::Request<axum::body::Body>) -> Raw {
    use tower::ServiceExt as _;
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 1 << 22)
        .await
        .unwrap();
    (status, headers, body.to_vec())
}
fn jar(browser: &crate::auth::tests::Browser) -> String {
    browser
        .jar
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
fn query_body(parameters: &Value) -> String {
    json!({"parameters": parameters}).to_string()
}
fn raw_404(reply: &Raw) -> bool {
    reply.0 == axum::http::StatusCode::NOT_FOUND
        && reply.2 == b"{\"code\":\"not_found\",\"status\":404}"
        && reply.1[axum::http::header::CACHE_CONTROL] == "no-store"
}
/// A JSON 200's receipt: contract-valid, its digest recomputed, and the
/// exact rows it covers.
fn receipted(body: &str) -> (Value, Value) {
    let value: Value = serde_json::from_str(body).unwrap();
    let document = value["receipt"].clone();
    assert!(contract_valid(&document), "{document}");
    assert_eq!(
        receipt::digest(&document).as_deref(),
        document["digest"].as_str()
    );
    assert_eq!(
        crate::query::worker::result_digest(
            &serde_json::from_value::<Vec<Vec<Value>>>(value["rows"].clone()).unwrap()
        )
        .as_deref(),
        document["result_digest"].as_str()
    );
    (value["rows"].clone(), document)
}

/// The route checks: members signed in through the APP-009 routes against
/// the synthetic loopback issuer, the APP-012 routes behind a dispatch
/// counter wrapping the real confined engine. Returns the fixture and the
/// report section.
async fn route_campaign(
    fixture: Fixture,
    engine: Arc<crate::query::Engine>,
    checks: &mut Vec<Value>,
) -> (Fixture, Value) {
    use crate::{
        auth::{
            invite::Invitations,
            synthetic_issuer::{Issuer, ISSUER as GATEWASM},
            tests::{self as harness, Campaign, CANARY_HANDLE, NORTHSTAR_HANDLE},
            Auth,
        },
        query::{
            page::WorkProps,
            routes::tests::{compose, Counting, RecordingPages},
            Executor, Work,
        },
        storage::admission_qualification::FakeProvider,
    };
    use axum::http::{header, StatusCode};

    for (org, handle) in [(ORG, NORTHSTAR_HANDLE), (OTHER, CANARY_HANDLE)] {
        operator(
            &fixture,
            &["issue-handle", "--organization", org, "--handle", handle],
        );
    }
    fixture.sql(&format!("INSERT INTO genui.provider_tenant_bindings VALUES ('{ORG}','{GATEWASM}','{}',true),('{OTHER}','{GATEWASM}','{}',true)", harness::NORTHSTAR_TENANT, harness::CANARY_TENANT), "genui");
    fixture.write_registry(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000a12","bindings":[
        {"issuer":GATEWASM,"provider_tenant_id":harness::NORTHSTAR_TENANT,"organization_id":ORG},
        {"issuer":GATEWASM,"provider_tenant_id":harness::CANARY_TENANT,"organization_id":OTHER}]}));
    let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
    stores.readiness().await.unwrap();
    let issuer = Issuer::start(harness::tenants()).await;
    // APP-074: invited signups go through the real `POST /invite/validate`;
    // one loopback management stand-in per tenant, one capability secret
    // (ADR-003 amendment 2026-10-30) and one service view of it.
    let provider = FakeProvider::start_at(
        &fixture,
        harness::NORTHSTAR_TENANT,
        "route-northstar",
        None,
        None,
    );
    let canary = FakeProvider::start_at(
        &fixture,
        harness::CANARY_TENANT,
        "route-canary",
        Some(&provider.capabilities),
        Some(&provider.secret),
    );
    let auth = Arc::new(
        Auth::loopback(
            harness::config(&issuer.origin),
            Some(stores.clone()),
            Arc::new(harness::TestPages),
        )
        .with_invitations(Some(Invitations::open(&provider.capabilities).unwrap())),
    );
    let counting = Arc::new(Counting {
        engine: Some(engine.clone() as Arc<dyn Executor>),
        ..Counting::default()
    });
    let pages = Arc::new(RecordingPages::default());
    let routes = compose(Arc::new(Work::new(
        auth.clone(),
        Some(counting.clone() as Arc<dyn Executor>),
        pages.clone(),
    )));
    let c = Campaign {
        app: harness::app(auth.clone()),
        fixture,
        stores: stores.clone(),
        issuer,
        auth: auth.clone(),
        log: Arc::default(),
        checks: Vec::new(),
        provider,
        canary: Some(canary),
        codes: Default::default(),
    };
    let last_page = || -> WorkProps { pages.last.lock().unwrap().clone().expect("a page") };
    let query = |view: &str| format!("/v1/work/{APP}/{ROUTE_WORK}/queries/{view}");
    let page = format!("/w/{APP}/{ROUTE_WORK}");
    let mut filtered = window();
    filtered["destination"] = json!("198.51.100.7");
    filtered["filter_epoch"] = json!(1);

    // The member: invited signup, verification, sign-in (APP-009 routes),
    // then source_read on both bound sources and a new saved work bound to
    // the uploaded revision with her as its creator (create-bound-work).
    let (maya_subject, mut maya) = c.member("0000000012a1").await;
    let member = c.principal(&maya_subject).await;
    for source in [SOURCE, INVENTORY] {
        grant(&c.fixture, &member, "source_read", &["--source", source]);
    }
    operator(
        &c.fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            ROUTE_WORK,
            "--creator",
            &member,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );

    // --- Authorized: exact results and receipts through the JSON route ---
    let total = maya
        .post(&routes, &query("total"), &query_body(&window()))
        .await;
    assert_eq!(total.status, StatusCode::OK, "{}", total.body);
    assert_eq!(total.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(total.headers[header::CACHE_CONTROL], "no-store");
    let (rows, total_receipt) = receipted(&total.body);
    assert_eq!(rows, json!([["1140000"]]));
    for (key, value) in [
        ("organization_id", json!(ORG)),
        ("app_id", json!(APP)),
        ("work_id", json!(ROUTE_WORK)),
        ("view_id", json!("total")),
        ("principal_id", json!(member)),
        ("mode", json!("published")),
        ("filter_epoch", json!(0)),
        ("partial", json!(false)),
    ] {
        assert_eq!(total_receipt[key], value, "{key}");
    }
    assert_eq!(
        total_receipt["source_revisions"],
        json!([{"input_id": "events", "source_id": SOURCE, "source_revision_id": REVISION, "source_digest": NORMALIZED}])
    );
    assert_eq!(
        total_receipt["parameters"],
        json!([
            {"field_id": "window_end", "value": {"type": "timestamp", "value": "2026-10-22T10:10:00Z"}},
            {"field_id": "window_start", "value": {"type": "timestamp", "value": "2026-10-22T10:00:00Z"}}
        ])
    );
    let destination = maya
        .post(&routes, &query("total"), &query_body(&filtered))
        .await;
    assert_eq!(destination.status, StatusCode::OK, "{}", destination.body);
    let (rows, destination_receipt) = receipted(&destination.body);
    assert_eq!(rows, json!([["900000"]]));
    assert_eq!(destination_receipt["filter_epoch"], 1);
    let timeline = maya
        .post(&routes, &query("timeline"), &query_body(&window()))
        .await;
    assert_eq!(timeline.status, StatusCode::OK, "{}", timeline.body);
    let (rows, timeline_receipt) = receipted(&timeline.body);
    let buckets: Vec<(String, String)> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r[0].as_str().unwrap().to_owned(),
                r[2].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        buckets,
        [
            ("2026-10-22T10:00:00Z", "60000"),
            ("2026-10-22T10:01:00Z", "120000"),
            ("2026-10-22T10:02:00Z", "300000"),
            ("2026-10-22T10:03:00Z", "600000"),
            ("2026-10-22T10:05:00Z", "60000"),
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    // The S02 number needs the window: the whole revision (E6 at 10:10
    // included) is 2,040,000, with a receipt that records no window.
    let whole = maya
        .post(&routes, &query("total"), &query_body(&json!({})))
        .await;
    assert_eq!(whole.status, StatusCode::OK);
    let (rows, whole_receipt) = receipted(&whole.body);
    assert_eq!(rows, json!([["2040000"]]));
    assert_eq!(whole_receipt["parameters"], json!([]));
    assert_eq!(counting.calls(), 4);
    check(
        checks,
        "route_authorized_member_gets_exact_result_and_receipt_digest: a member signed in through the APP-009 routes gets total 1,140,000 for [10:00, 10:10), 900,000 for destination 198.51.100.7 at filter epoch 1 and the five timeline buckets from POST /v1/work/{app}/{work}/queries/{view} (application/json, no-store); each receipt is contract-valid, its digest and result digest recompute, it pins her principal, the new work and the uploaded revision's real digest; without the window the same view is 2,040,000 with a receipt that records no window",
    );

    // --- The work page: idle views, then a run with every state's receipt ---
    let opened = maya.get(&routes, &page).await;
    assert_eq!(opened.status, StatusCode::OK);
    let props = last_page();
    assert_eq!(
        props
            .views
            .iter()
            .map(|v| (v.id.as_str(), v.state.as_str()))
            .collect::<Vec<_>>(),
        [("total", "idle"), ("timeline", "idle")]
    );
    assert!(props
        .views
        .iter()
        .all(|v| !v.has_summary && !v.has_table && !v.has_receipt));
    assert_eq!(props.action, page);
    let csrf = props.csrf.clone();
    assert_eq!(counting.calls(), 4, "opening the page runs nothing");
    let form = |start: &str, end: &str, destination: &str| {
        format!("csrf={csrf}&window_start={start}&window_end={end}&destination={destination}")
    };
    let ran = maya
        .post(
            &routes,
            &page,
            &form("2026-10-22T10:00", "2026-10-22T10:10", ""),
        )
        .await;
    assert_eq!(ran.status, StatusCode::OK);
    let props = last_page();
    let total_card = &props.views[0];
    assert_eq!(
        (
            total_card.state.as_str(),
            total_card.summary[0].value.as_str()
        ),
        ("ready", "1,140,000")
    );
    let card_receipt: Value = serde_json::from_str(&total_card.receipt_json).unwrap();
    assert!(contract_valid(&card_receipt));
    assert_eq!(
        card_receipt["digest"].as_str(),
        Some(total_card.receipt_digest.as_str())
    );
    assert_eq!(
        card_receipt["result_digest"],
        total_receipt["result_digest"]
    );
    let timeline_card = &props.views[1];
    assert_eq!(timeline_card.state, "ready");
    assert_eq!(timeline_card.rows.len(), 5);
    assert_eq!(timeline_card.rows[3].cells[2].text, "600,000");
    assert_eq!(props.window_start, "2026-10-22T10:00");
    let ran = maya
        .post(
            &routes,
            &page,
            &form("2026-10-22T10:00", "2026-10-22T10:10", "198.51.100.7"),
        )
        .await;
    assert_eq!(ran.status, StatusCode::OK);
    assert_eq!(last_page().views[0].summary[0].value, "900,000");
    // A destination with no event in the window: empty, never zero.
    let ran = maya
        .post(
            &routes,
            &page,
            &form("2026-10-22T10:00", "2026-10-22T10:10", "192.0.2.250"),
        )
        .await;
    assert_eq!(ran.status, StatusCode::OK);
    let props = last_page();
    assert_eq!(
        props
            .views
            .iter()
            .map(|v| v.state.as_str())
            .collect::<Vec<_>>(),
        ["empty", "empty"]
    );
    assert!(props
        .views
        .iter()
        .all(|v| v.summary.is_empty() && v.rows.is_empty() && v.has_receipt));
    let calls = counting.calls();
    // No window: 422, every view idle, nothing ran; a bad token: 403.
    let refused = maya
        .post(&routes, &page, &form("", "", "198.51.100.7"))
        .await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    let props = last_page();
    assert!(props.form_error && props.views.iter().all(|v| v.idle));
    let forged = maya
        .post(
            &routes,
            &page,
            &format!(
                "csrf={}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=",
                "A".repeat(43)
            ),
        )
        .await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    assert_eq!(counting.calls(), calls, "refused forms run nothing");
    check(
        checks,
        "work page: GET /w/{app}/{work} shows the definition's two runnable views idle and runs nothing; the run form (window required, destination optional, synchronizer token) shows total 1,140,000 and the five timeline buckets with contract-valid receipts, 900,000 for the destination, and empty (never zero) for a destination with no event; a missing window is 422 with every view idle and a forged token 403, neither reaching the engine",
    );

    // --- Filters travel in bodies only ---
    let calls = counting.calls();
    let in_url = maya
        .post(
            &routes,
            &format!("{}?destination=198.51.100.7", query("total")),
            &query_body(&window()),
        )
        .await;
    assert_eq!(in_url.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        in_url.body,
        "{\"code\":\"invalid_parameters\",\"status\":422}"
    );
    assert!(maya
        .get(&routes, &format!("{page}?destination=198.51.100.7"))
        .await
        .uniform_404());
    for body in [
        json!({"destination": "198.51.100.7"}).to_string(),
        json!({"parameters": {"sql": "SELECT 1"}}).to_string(),
        json!({"parameters": window(), "organization_id": OTHER}).to_string(),
        "not json".to_owned(),
    ] {
        let refused = maya.post(&routes, &query("total"), &body).await;
        assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let oversize = maya
        .post(
            &routes,
            &query("total"),
            &" ".repeat(crate::query::routes::QUERY_BODY_LIMIT + 1),
        )
        .await;
    assert_eq!(oversize.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(counting.calls(), calls);
    check(
        checks,
        "filters travel in bodies only: a query string on the JSON route is 422 invalid_parameters and on the page the uniform 404; unclosed, foreign-scoped or malformed bodies are 422 and a body over 64 KiB is 413; none reaches the engine",
    );

    // --- Foreign, revoked, pre-access and anonymous: uniform 404, no engine ---
    let calls = counting.calls();
    let mut anonymous = c.browser("198.51.100.20:1000");
    let pending_subject = "usr_00000000-0000-4000-8000-0000000012a2";
    c.issuer
        .user(pending_subject, "pending@example.test", false);
    let (_, digest) = c.issue_code();
    let mut pending = c.browser("192.0.2.31:5000");
    assert_eq!(
        c.invited_signup(&mut pending, &digest, pending_subject)
            .await
            .location(),
        "/auth/pending"
    );
    let pre_cookie = pending.cookie(crate::auth::cookies::PRE_ACCESS).unwrap();
    let mut swapped = c.browser("192.0.2.31:5001");
    swapped
        .jar
        .insert(crate::auth::cookies::SESSION.into(), pre_cookie);
    let (_, mut canary) = c.member_in(OTHER, "0000000012a3").await;
    let (_, mut outsider) = c.member("0000000012a4").await;
    let (revoked_subject, mut revoked) = c.member("0000000012a5").await;
    let revoked_principal = c.principal(&revoked_subject).await;
    operator(
        &c.fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            &revoked_principal,
        ],
    );
    let mut denials = 0;
    for (label, browser) in [
        ("anonymous", &mut anonymous),
        ("pre-access", &mut pending),
        ("pre-access cookie as a session", &mut swapped),
        ("canary member (foreign organization)", &mut canary),
        ("member without work or source access", &mut outsider),
        ("revoked membership", &mut revoked),
    ] {
        for uri in [
            query("total"),
            query("timeline"),
            format!("/v1/work/{APP}/{WORK}/queries/total"),
        ] {
            assert!(
                browser
                    .post(&routes, &uri, &query_body(&window()))
                    .await
                    .uniform_404(),
                "{label} {uri}"
            );
            denials += 1;
        }
        assert!(
            browser.get(&routes, &page).await.uniform_404(),
            "{label} page"
        );
        // A session's run form carries its own valid synchronizer token (the
        // token is tied to the cookie, not the work), so the refusal is the
        // work's authorization; without a session there is no token to check.
        let token = browser
            .cookie(crate::auth::cookies::SESSION)
            .map(|cookie| auth.csrf.token(crate::auth::csrf::Form::Query, &cookie))
            .unwrap_or_else(|| "x".into());
        assert!(
            browser
                .post(
                    &routes,
                    &page,
                    &format!("csrf={token}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=")
                )
                .await
                .uniform_404(),
            "{label} run form"
        );
        denials += 2;
    }
    // Source_read revoked for the member herself: the work is gone (404).
    operator(
        &c.fixture,
        &[
            "revoke-permission",
            "--organization",
            ORG,
            "--principal",
            &member,
            "--permission",
            "source_read",
            "--source",
            SOURCE,
        ],
    );
    assert!(maya
        .post(&routes, &query("total"), &query_body(&window()))
        .await
        .uniform_404());
    assert!(maya.get(&routes, &page).await.uniform_404());
    denials += 2;
    assert_eq!(
        counting.calls(),
        calls,
        "no denied request reached the engine"
    );
    grant(&c.fixture, &member, "source_read", &["--source", SOURCE]);
    check(
        checks,
        &format!("foreign, revoked, pre-access and anonymous requests get the uniform 404 and invoke no engine: {denials} requests (anonymous, a pre-access session, a pre-access cookie presented as a session, a canary member replaying the Northstar ids, a member without work or source access, a revoked membership, and the member after her own source_read revocation) on the JSON route, the page and the run form; the dispatch counter did not move"),
    );

    // --- Revocation between execution and delivery denies the release ---
    let calls = counting.calls();
    let (computed, resume) = counting.pause_next();
    let racing = tokio::spawn(send_raw(
        routes.clone(),
        raw_request(
            "POST",
            &query("total"),
            &jar(&maya),
            query_body(&window()).into_bytes(),
        ),
    ));
    computed.await.unwrap();
    operator(
        &c.fixture,
        &[
            "revoke-permission",
            "--organization",
            ORG,
            "--principal",
            &member,
            "--permission",
            "source_read",
            "--source",
            SOURCE,
        ],
    );
    resume.send(()).unwrap();
    let reply = racing.await.unwrap();
    assert!(raw_404(&reply), "{:?}", String::from_utf8_lossy(&reply.2));
    assert!(!String::from_utf8_lossy(&reply.2).contains("1140000"));
    assert_eq!(
        counting.calls(),
        calls + 1,
        "the engine computed; the release was denied"
    );
    grant(&c.fixture, &member, "source_read", &["--source", SOURCE]);
    // The page: both views computed or refused, then denied at release.
    let opened = maya.get(&routes, &page).await;
    assert_eq!(opened.status, StatusCode::OK);
    let csrf = last_page().csrf;
    let (computed, resume) = counting.pause_next();
    let racing = tokio::spawn(send_raw(
        routes.clone(),
        raw_request(
            "POST",
            &page,
            &jar(&maya),
            format!("csrf={csrf}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=").into_bytes(),
        ),
    ));
    computed.await.unwrap();
    operator(
        &c.fixture,
        &[
            "revoke-membership",
            "--organization",
            ORG,
            "--principal",
            &member,
        ],
    );
    resume.send(()).unwrap();
    let reply = racing.await.unwrap();
    assert_eq!(reply.0, StatusCode::OK);
    let props = last_page();
    assert_eq!(
        props
            .views
            .iter()
            .map(|v| v.state.as_str())
            .collect::<Vec<_>>(),
        ["denied", "denied"]
    );
    assert!(props
        .views
        .iter()
        .all(|v| v.summary.is_empty() && v.rows.is_empty() && !v.has_receipt));
    // And the revoked membership ends the session for the next request.
    assert!(maya.get(&routes, &page).await.uniform_404());
    check(
        checks,
        "revocation between execution and delivery denies release: with the engine paused after computing 1,140,000, a source_read revocation turns the JSON response into the uniform 404 with no number; on the page a membership revocation in the same gap renders both views denied with no value or receipt, and the next request finds no session",
    );

    // --- 429 under load: a full admission queue answers Retry-After: 2 ---
    let load_root = crate::query::tests::Directory::new("route-load");
    let cached = load_root.0.join("cache/o").join(ORG);
    std::fs::create_dir_all(&cached).unwrap();
    for path in [
        load_root.0.join("cache"),
        load_root.0.join("cache/o"),
        cached.clone(),
    ] {
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .unwrap();
    }
    let file = cached.join(format!("{SOURCE}.{REVISION}.parquet"));
    std::fs::write(&file, b"cached").unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o400)).unwrap();
    let line = r#"{"columns":["total_bytes"],"elapsed_ms":7,"outcome":"rows","partial":false,"rows":[["1140000"]],"total_scope":"full_authorized_filtered_input","truncated_page":false}"#;
    let slow = Arc::new(
        crate::query::Engine::new(crate::query::EngineSettings {
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                format!("cat >/dev/null; sleep 1.5; printf '%s\\n' '{line}'"),
            ],
            cache_root: load_root.0.join("cache"),
            scratch_root: load_root.0.join("scratch"),
            fetch: None,
            backstop: crate::query::worker::BACKSTOP,
            rss_limit_bytes: crate::query::worker::RSS_LIMIT_BYTES,
        })
        .unwrap(),
    );
    let loaded = compose(Arc::new(Work::new(
        auth.clone(),
        Some(slow.clone() as Arc<dyn Executor>),
        pages.clone(),
    )));
    let (load_subject, load_browser) = c.member("0000000012a6").await;
    let load_member = c.principal(&load_subject).await;
    for source in [SOURCE, INVENTORY] {
        grant(
            &c.fixture,
            &load_member,
            "source_read",
            &["--source", source],
        );
    }
    let load_work = "00000000-0000-4000-8000-0000000000f7";
    operator(
        &c.fixture,
        &[
            "create-bound-work",
            "--organization",
            ORG,
            "--app",
            APP,
            "--from-work",
            WORK,
            "--work",
            load_work,
            "--creator",
            &load_member,
            "--input",
            "events",
            "--source",
            SOURCE,
            "--revision",
            REVISION,
        ],
    );
    let cookies = jar(&load_browser);
    // Fill the engine's admission directly: 2 running (1.5 s children) and 20
    // waiting for the organization, as concurrent requests would. The next
    // authorized request through the route is then refused before any child.
    let filler = crate::storage::QueryRun::for_tests(
        ORG,
        &load_member,
        APP,
        load_work,
        super::qualification_cases::PUBLICATION,
        "sha256:filler",
        "total",
        seeded_query("total"),
        &window(),
        (SOURCE, REVISION, NORMALIZED),
    );
    let mut fill = tokio::task::JoinSet::new();
    for _ in 0..(crate::query::worker::ORGANIZATION_RUNNING + crate::query::worker::QUEUE) {
        let (slow, run) = (slow.clone(), filler.clone());
        fill.spawn(async move { slow.execute(&run, DEADLINE).await.map(|_| ()) });
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let busy = send_raw(
        loaded.clone(),
        raw_request(
            "POST",
            &format!("/v1/work/{APP}/{load_work}/queries/total"),
            &cookies,
            query_body(&window()).into_bytes(),
        ),
    )
    .await;
    assert_eq!(busy.0, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(busy.1[header::RETRY_AFTER], "2");
    assert_eq!(busy.1[header::CACHE_CONTROL], "no-store");
    assert_eq!(busy.2, b"{\"code\":\"too_many_requests\",\"status\":429}");
    // The page in the same state: every view is loading, none shows a value.
    let token = auth.csrf.token(
        crate::auth::csrf::Form::Query,
        &load_browser.cookie(crate::auth::cookies::SESSION).unwrap(),
    );
    let loading = send_raw(
        loaded.clone(),
        raw_request(
            "POST",
            &format!("/w/{APP}/{load_work}"),
            &cookies,
            format!("csrf={token}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=").into_bytes(),
        ),
    )
    .await;
    assert_eq!(loading.0, StatusCode::OK);
    let props = last_page();
    assert_eq!(
        props
            .views
            .iter()
            .map(|v| v.state.as_str())
            .collect::<Vec<_>>(),
        ["loading", "loading"]
    );
    assert!(props
        .views
        .iter()
        .all(|v| v.summary.is_empty() && v.rows.is_empty() && !v.has_receipt));
    let mut filled = (0, 0);
    while let Some(outcome) = fill.join_next().await {
        match outcome.unwrap() {
            Ok(()) => filled.0 += 1,
            Err(QueryError::Failed(Failure::Deadline)) => filled.1 += 1,
            Err(other) => panic!("filler {other:?}"),
        }
    }
    let load = slow.stats();
    assert_eq!(
        load.rejected, 3,
        "the JSON request and the page's two views"
    );
    assert!(load.peak_organization_running <= 2);
    // Drained, the same request is admitted again.
    let recovered = send_raw(
        loaded.clone(),
        raw_request(
            "POST",
            &format!("/v1/work/{APP}/{load_work}/queries/total"),
            &cookies,
            query_body(&window()).into_bytes(),
        ),
    )
    .await;
    assert_eq!(recovered.0, StatusCode::OK);
    check(
        checks,
        &format!("429 under load: with the organization's engine admission full (2 running, 20 waiting), an authorized POST /v1/work/{{app}}/{{work}}/queries/{{view}} is 429 too_many_requests with Retry-After: 2 and no-store, and the work page renders both views loading with no value or receipt; the engine rejected exactly those 3 and never ran more than {} at once; the fillers finished ({} completed, {} expired in the queue as failed); once drained the same request is 200", load.peak_organization_running, filled.0, filled.1),
    );

    let report = json!({
        "receipts": {"total": total_receipt, "destination": destination_receipt, "timeline": timeline_receipt, "whole_revision": whole_receipt},
        "dispatch_calls": counting.calls(),
        "denied_requests_without_dispatch": denials,
        "load": {"fillers": crate::query::worker::ORGANIZATION_RUNNING + crate::query::worker::QUEUE, "filler_child_ms": 1500,
                 "fillers_completed": filled.0, "fillers_expired": filled.1, "route_too_many_requests": 1, "page_views_loading": 2,
                 "peak_organization_running": load.peak_organization_running, "rejected": load.rejected},
    });
    stores.close().await.unwrap();

    // --- Store outage: 503 for everyone, the uniform 404 only without a session ---
    // A second Stores against a closed port (after the first closed: pool
    // capacity is process-wide). Every resolve_session fails at connect, so
    // GET /workspace, the JSON route, the page and its run form answer 503
    // `unavailable` for the member's live cookie and for a forged one alike
    // (no identity is judged), and the uniform 404 without a cookie.
    let down = Arc::new(Stores::new(c.fixture.unreachable_settings()).unwrap());
    let down_auth = Arc::new(Auth::loopback(
        harness::config(&c.issuer.origin),
        Some(down.clone()),
        Arc::new(harness::TestPages),
    ));
    let down_counting = Arc::new(Counting {
        engine: Some(engine.clone() as Arc<dyn Executor>),
        ..Counting::default()
    });
    let down_routes = compose(Arc::new(Work::new(
        down_auth.clone(),
        Some(down_counting.clone() as Arc<dyn Executor>),
        pages.clone(),
    )));
    let down_app = harness::app(down_auth.clone());
    // A well-formed session capability for Northstar that was never issued.
    let mut raw: Vec<u8> = (0..16)
        .map(|i| u8::from_str_radix(&ORG.replace('-', "")[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    raw.extend([0x5a; 32]);
    let forged = format!(
        "{}={}",
        crate::auth::cookies::SESSION,
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw)
    );
    assert!(
        crate::storage::capability_organization(&forged[forged.find('=').unwrap() + 1..]).is_some()
    );
    let started = std::time::Instant::now();
    let mut outage = Vec::new();
    for (label, cookie) in [("member", cookies.as_str()), ("forged", forged.as_str())] {
        for (router, method, uri, body) in [
            (down_app.clone(), "GET", "/workspace".to_owned(), Vec::new()),
            (
                down_routes.clone(),
                "POST",
                format!("/v1/work/{APP}/{load_work}/queries/total"),
                query_body(&window()).into_bytes(),
            ),
            (
                down_routes.clone(),
                "GET",
                format!("/w/{APP}/{load_work}"),
                Vec::new(),
            ),
            (
                down_routes.clone(),
                "POST",
                format!("/w/{APP}/{load_work}"),
                b"csrf=x&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination="
                    .to_vec(),
            ),
        ] {
            let reply = send_raw(router, raw_request(method, &uri, cookie, body)).await;
            assert_eq!(
                reply.0,
                StatusCode::SERVICE_UNAVAILABLE,
                "{label} {method} {uri}"
            );
            assert_eq!(
                reply.2, b"{\"code\":\"unavailable\",\"status\":503}",
                "{label} {method} {uri}"
            );
            assert_eq!(reply.1[header::CACHE_CONTROL], "no-store");
            outage.push(json!({"cookie": label, "method": method, "route": uri.replace(APP, "{app}").replace(load_work, "{work}"), "status": 503}));
        }
    }
    let outage_ms = started.elapsed().as_millis();
    for (router, method, uri) in [
        (down_app.clone(), "GET", "/workspace".to_owned()),
        (
            down_routes.clone(),
            "POST",
            format!("/v1/work/{APP}/{load_work}/queries/total"),
        ),
        (down_routes.clone(), "GET", format!("/w/{APP}/{load_work}")),
    ] {
        let reply = send_raw(router, raw_request(method, &uri, "", Vec::new())).await;
        assert_eq!(reply.0, StatusCode::NOT_FOUND, "anonymous {method} {uri}");
        assert_eq!(reply.2, b"{\"code\":\"not_found\",\"status\":404}");
    }
    assert_eq!(down_counting.calls(), 0);
    down.close().await.unwrap();
    check(
        checks,
        &format!("store outage maps alike on GET /workspace and the APP-012 routes: with the workspace store unreachable, the member's live cookie and a forged cookie both get 503 unavailable (no-store) on GET /workspace, the JSON route, the page and its run form ({} requests in {outage_ms} ms, inside the 5 s handler deadline, so the answer is the storage mapping), without a cookie the uniform 404; the engine is never reached", outage.len()),
    );
    let mut report = report;
    report["store_outage"] = json!({"requests": outage, "elapsed_ms": outage_ms, "dispatch_calls": down_counting.calls()});
    let Campaign { fixture, .. } = c;
    (fixture, report)
}
