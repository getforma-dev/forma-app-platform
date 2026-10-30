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
    let mut fixture = Fixture::new();
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

    let stats = engine.stats();
    stores.close().await.unwrap();
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
            "admission": {"peak_task_running": stats.peak_task_running, "peak_organization_running": stats.peak_organization_running,
                          "rejected": stats.rejected, "launched": stats.launched},
            "checks": checks,
            "passed": true,
            "limits": [
                "Synthetic local password authentication is not RDS IAM; identities are synthetic registry-bound fixtures (no APP-009 session, route or page).",
                "The revision cache is seeded locally with the bytes APP-077 uploads (same SHA-256); the credentialed S3 fetch is unit-tested against botocore's Stubber, not S3.",
                "The engine child runs on macOS arm64 with the locked venv, not in the linux image under Fargate; its latency is re-measured in APP-078's window.",
                "The registered record is written by the test with the APP-077 digests and the content-derived revision id; upload-sources itself (merged separately) is not run here."
            ]
        })).unwrap()).unwrap();
    }
}
