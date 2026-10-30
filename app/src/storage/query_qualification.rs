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
//! validated against the contract schema by the contract validator, per
//! document (`validate`) and against the work it cites (`validate_bundle`,
//! APP-012 review M3).
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
/// The service's structured log (APP-076 G6), captured for the whole
/// campaign process: the dispatch and release lines are counted against the
/// dispatch counter and the engines' launch counts.
static LOG: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<u8>>>> = std::sync::OnceLock::new();
struct LogSink(Arc<std::sync::Mutex<Vec<u8>>>);
impl Write for LogSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn install_log() {
    let buffer = LOG.get_or_init(Default::default).clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || LogSink(buffer.clone()))
        .with_ansi(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
/// Log lines carrying `event="<event>"`.
fn log_lines(event: &str) -> usize {
    let text = String::from_utf8_lossy(&LOG.get().unwrap().lock().unwrap()).into_owned();
    let marker = format!("event=\"{event}\"");
    text.lines().filter(|line| line.contains(&marker)).count()
}
/// Whether any log line carries `needle` (a parameter value must never).
fn log_mentions(needle: &str) -> bool {
    String::from_utf8_lossy(&LOG.get().unwrap().lock().unwrap()).contains(needle)
}
const FIXTURE_BUNDLE: &str = "contracts/fixtures/impl-2026-10-28.1/northstar.json";
/// The seeded work's inventory dependency (`…0071`, placeholder digests),
/// which `create-bound-work` copies: a receipt of a bound work lists it beside
/// the events revision it read (APP-012 review M3).
fn inventory_dependency() -> Value {
    let bundle: Value =
        serde_json::from_slice(&std::fs::read(repository().join(FIXTURE_BUNDLE)).unwrap()).unwrap();
    bundle["work"]["source_dependencies"][1].clone()
}
/// The receipt's `source_revisions` for a work bound by `create-bound-work`.
fn bound_sources() -> Value {
    json!([
        {"input_id": "events", "source_id": SOURCE, "source_revision_id": REVISION, "source_digest": NORMALIZED},
        inventory_dependency()
    ])
}
/// `contracts/validate.py` `validate_bundle` over the Northstar fixture bundle
/// re-pointed at a bound work: its identity, its events binding and
/// dependency moved to the registered revision (whose canonical envelope is
/// `envelope`), the delivered `receipt`, every entity re-scoped to the
/// receipt's organization (the seeded database's, not the fixture's
/// synthetic one) and the command and outcome to that work and the receipt's
/// principal. `Ok` when the validator accepts the bundle, else its refusal
/// (APP-012 review M3, probe P17).
fn bundle_verdict(
    receipt: &Value,
    work_id: &str,
    envelope: &str,
) -> std::result::Result<(), String> {
    const SCRIPT: &str = "import json,sys
from validate import validate_bundle, payload_digest, ContractError
d = json.load(sys.stdin)
b = json.load(open(d['fixture']))
r = d['receipt']
b['work']['work_id'] = d['work_id']
for item in b['work']['bindings'] + b['work']['source_dependencies']:
    if item.get('input_id') == 'events':
        item['source_revision_id'] = d['revision_id']
        item['source_digest'] = d['normalized_digest']
b['sources'].append(json.loads(d['envelope']))
b['receipt'] = r
for k in ('command', 'outcome'):
    b[k]['work_id'] = d['work_id']
b['outcome']['principal_id'] = r['principal_id']
b['context']['principal_id'] = r['principal_id']
# The fixture names the contract's synthetic organization; the seeded database's is the receipt's.
b['context']['organization_id'] = r['organization_id']
for item in [b[k] for k in ('definition', 'publication', 'work', 'command', 'outcome')] + b['sources']:
    if 'organization_id' in item:
        item['organization_id'] = r['organization_id']
b['outcome']['payload_digest'] = payload_digest(b['command'])
try:
    validate_bundle(b)
except ContractError as e:
    print('refused: ' + str(e))
else:
    print('valid')
";
    let input = json!({
        "fixture": repository().join(FIXTURE_BUNDLE),
        "receipt": receipt,
        "work_id": work_id,
        "revision_id": REVISION,
        "normalized_digest": NORMALIZED,
        "envelope": envelope,
    });
    let mut child = Command::new(fixture_python())
        .current_dir(repository().join("contracts"))
        .args(["-c", SCRIPT])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&input).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() && stdout == "valid" {
        Ok(())
    } else {
        Err(format!(
            "{stdout} {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
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
    install_log();
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
    // Every source dependency of the work, as the contract's `validate_bundle`
    // requires (review M3): the events revision it read and the inventory
    // binding it did not.
    assert_eq!(total.document["source_revisions"], bound_sources());
    assert_eq!(run.dependencies().len(), 2);
    let envelope = rows(&fixture, &format!("SELECT convert_from(canonical_envelope,'UTF8') FROM genui.source_revisions WHERE organization_id='{ORG}' AND source_id='{SOURCE}' AND revision_id='{REVISION}'")).await;
    assert_eq!(envelope.len(), 1);
    assert_eq!(
        bundle_verdict(&total.document, BOUND, &envelope[0]),
        Ok(()),
        "validate_bundle accepts the receipt against its work"
    );
    let mut cut = total.document.clone();
    cut["source_revisions"] = json!([cut["source_revisions"][0].clone()]);
    cut["digest"] = json!(receipt::digest(&cut).unwrap());
    let refused = bundle_verdict(&cut, BOUND, &envelope[0]).expect_err("the read input alone");
    assert!(
        refused.contains("source evidence dependency mismatch"),
        "{refused}"
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
        "authorized_member_gets_exact_result_and_receipt_digest: total 1,140,000, destination 198.51.100.7 900,000 and the five timeline buckets of the APP-011 oracle, each released after re-authorization with a contract-valid published receipt pinning the new revision's real digest; the receipt lists every source dependency of the work (events and inventory) and passes contracts/validate.py validate_bundle against the work, while a receipt cut to the read input alone is refused (source evidence dependency mismatch)",
    );

    // --- ADR-011 prebuilt library (APP-094): seeded, cached, run, receipted ---
    let seeded = operator(&fixture, &["seed-library", "--organization", ORG]);
    assert_eq!(seeded["changed"], true);
    assert_eq!(seeded["apps"].as_array().unwrap().len(), 13);
    assert_eq!(seeded["audit_event"], Value::Null);
    let again = operator(&fixture, &["seed-library", "--organization", ORG]);
    assert_eq!(
        (again["changed"].clone(), again["rows_inserted"].clone()),
        (json!(false), json!(0))
    );
    let built = Command::new(&python)
        .current_dir(repository().join("app/runtime"))
        .args([
            "-c",
            "import json,sys,library_sources as l; print(json.dumps(l.build(sys.argv[1], sys.argv[2])))",
            ORG,
            root.0.join("cache").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let built: Value = serde_json::from_slice(&built.stdout).unwrap();
    assert_eq!(built.as_array().unwrap().len(), 13);
    let catalog: Value = serde_json::from_slice(
        &std::fs::read(repository().join("contracts/fixtures/library/catalog.json")).unwrap(),
    )
    .unwrap();
    let entry = |library_id: &str| -> Value {
        catalog["apps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["library_id"] == json!(library_id))
            .unwrap()
            .clone()
    };
    let ids = |library_id: &str| -> (String, String) {
        let e = entry(library_id);
        (
            e["app_id"].as_str().unwrap().to_owned(),
            e["work_id"].as_str().unwrap().to_owned(),
        )
    };
    // The catalog the composer reads: every library app the member may open,
    // with its router metadata; a member without the grants sees none.
    let listed = stores.data.library_catalog(&member).await.unwrap();
    assert_eq!(listed.len(), 13);
    for item in &listed {
        assert!(item.routing["intent_examples"].as_array().unwrap().len() >= 4);
        assert!(!item.purpose.is_empty() && !item.title.is_empty());
        assert_eq!(
            item.definition_digest,
            entry(&item.library_id)["definition_digest"]
        );
    }
    let reviewer = request(&stores, ORG, REVIEWER, 1);
    assert_eq!(
        stores.data.library_catalog(&reviewer).await.unwrap(),
        Vec::new()
    );
    check(
        &mut checks,
        "seed-library places all 13 library apps once (exact-match idempotent, no audit event), library_sources.build caches their revisions under the pinned digests, and library_catalog lists them with router metadata for the granted member only",
    );
    // APP-093 review M3: more compositions than the recency cap (120 saved
    // works pinning a newer publication of the authoring app) never hide a
    // library app: library entries are selected by their sidecar digest
    // apart from the capped listing, and the catalog keeps the seeded works.
    const COMPOSED: &str = "00000000-0000-4000-8000-00000000c0de";
    fixture.sql(&format!("INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope,publication_sequence) SELECT organization_id,app_id,'{COMPOSED}',definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,now()+interval '1 hour',canonical_envelope,publication_sequence+1000 FROM genui.publications WHERE organization_id='{ORG}' AND app_id='{APP}' ORDER BY publication_sequence DESC LIMIT 1; INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) SELECT '{ORG}','{APP}',gen_random_uuid(),'{COMPOSED}',1,'APP-093 M3 composition '||g,'{PRINCIPAL}' FROM generate_series(1,120) g;"), "genui");
    let saved: i64 = rows(
        &fixture,
        &format!("SELECT count(*)::text FROM genui.saved_work WHERE organization_id='{ORG}'"),
    )
    .await[0]
        .parse()
        .unwrap();
    assert!(saved > 120 + 13, "{saved} saved works");
    let crowded = stores.data.library_catalog(&member).await.unwrap();
    assert_eq!(crowded.len(), 13);
    for item in &crowded {
        let (app, work) = ids(&item.library_id);
        assert_eq!(
            (item.app.as_str(), item.work.as_str()),
            (app.as_str(), work.as_str()),
            "the seeded work"
        );
    }
    let workspace = stores.data.openable_works(&member).await.unwrap();
    for entry in catalog["apps"].as_array().unwrap() {
        let (app, work) = ids(entry["library_id"].as_str().unwrap());
        assert!(
            workspace.iter().any(|w| w.app == app && w.work == work),
            "{} is listed",
            entry["library_id"]
        );
    }
    assert!(workspace.len() <= 100 + 13);
    // Newest first: the listing opens with the latest compositions.
    assert_eq!(workspace[0].app, APP);
    assert_eq!(
        stores.data.library_catalog(&reviewer).await.unwrap(),
        Vec::new()
    );
    fixture.sql(&format!("DELETE FROM genui.saved_work WHERE organization_id='{ORG}' AND publication_id='{COMPOSED}'; DELETE FROM genui.publications WHERE organization_id='{ORG}' AND publication_id='{COMPOSED}';"), "genui");
    check(
        &mut checks,
        "APP-093 M3: with 120 more saved works than the 100-work recency cap (all newer than the library), openable_works still lists all 13 seeded library works after the newest compositions and library_catalog still lists the 13 seeded works (selected by sidecar definition digest, never through the capped listing); an ungranted member still sees none",
    );
    let (expense_app, expense_work) = ids("expense-review");
    let outline = stores
        .data
        .work_outline(&member, &expense_app, &expense_work)
        .await
        .unwrap();
    assert_eq!(
        outline
            .views
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>(),
        ["totals", "queue", "by_category", "by_status"]
    );
    let mut library_receipts = serde_json::Map::new();
    let run_view = |app: String, work: String, view: &'static str, parameters: Value| {
        let stores = stores.clone();
        let engine = engine.clone();
        let member = request(&stores, ORG, PRINCIPAL, 1);
        async move {
            let run = stores
                .data
                .run_query(&member, &app, &work, view, &parameters)
                .await
                .unwrap();
            let result = engine.execute(&run, DEADLINE).await.unwrap();
            stores.data.release_result(&member, &run).await.unwrap();
            let receipt = receipt::build(&run, &result).unwrap();
            assert!(contract_valid(&receipt.document), "{}", receipt.document);
            assert_eq!(
                receipt.document["result_digest"],
                json!(result.result_digest)
            );
            assert_eq!(receipt.document["view_id"], json!(view));
            (result, receipt)
        }
    };
    let (totals, totals_receipt) = run_view(
        expense_app.clone(),
        expense_work.clone(),
        "totals",
        json!({}),
    )
    .await;
    assert_eq!(totals.columns, ["claims", "total_amount", "largest_claim"]);
    assert_eq!(
        totals.rows,
        vec![vec![
            json!("240"),
            totals.rows[0][1].clone(),
            totals.rows[0][2].clone()
        ]]
    );
    assert!(
        totals.integer_columns.contains("total_amount")
            && totals.integer_columns.contains("largest_claim")
    );
    // APP-093 review M4: the money columns carry their type, and the page
    // shows sum(amount) = 26262004 minor units (the DuckDB oracle) as
    // currency, the exact value kept as the hover title and in the receipt's
    // result digest.
    let usd = crate::query::worker::ColumnType::Money {
        currency: "USD".into(),
        minor_unit_exponent: 2,
    };
    assert_eq!(totals.column_types["total_amount"], usd);
    assert_eq!(totals.column_types["largest_claim"], usd);
    assert_eq!(
        totals.column_types["claims"],
        crate::query::worker::ColumnType::Integer
    );
    assert_eq!(totals.rows[0][1], json!("26262004"));
    let totals_view = outline.views.iter().find(|v| v.id == "totals").unwrap();
    assert_eq!(totals_view.column_types, totals.column_types);
    let shown = crate::query::page::card(
        totals_view,
        crate::query::page::Outcome::Released(&totals, &totals_receipt),
    );
    assert_eq!(shown.summary[1].value, "$262,620.04");
    assert_eq!(shown.summary[1].raw, "26262004");
    check(
        &mut checks,
        "APP-093 M4: the expense library totals carry column_types (claims integer, total_amount and largest_claim money USD exponent 2) from the re-admitted input contract; the card shows sum(amount) 26262004 minor units as $262,620.04 with the exact value as its hover title",
    );
    library_receipts.insert("expense_totals".into(), totals_receipt.document.clone());
    let (queue, queue_receipt) = run_view(
        expense_app.clone(),
        expense_work.clone(),
        "queue",
        json!({}),
    )
    .await;
    assert_eq!(
        queue.columns,
        [
            "expense_id",
            "time",
            "employee",
            "vendor",
            "category",
            "amount"
        ]
    );
    assert_eq!((queue.rows.len(), queue.truncated_page), (50, true));
    assert_eq!(queue_receipt.document["truncated_page"], json!(true));
    let amounts: Vec<i64> = queue
        .rows
        .iter()
        .map(|r| r[5].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(
        amounts.windows(2).all(|w| w[0] >= w[1]),
        "ordered by amount desc"
    );
    library_receipts.insert("expense_queue".into(), queue_receipt.document.clone());
    let (grouped, grouped_receipt) = run_view(
        expense_app.clone(),
        expense_work.clone(),
        "by_category",
        json!({}),
    )
    .await;
    assert_eq!(grouped.columns, ["category", "claims", "amount"]);
    assert_eq!(
        grouped
            .rows
            .iter()
            .map(|r| r[1].as_str().unwrap().parse::<i64>().unwrap())
            .sum::<i64>(),
        240
    );
    library_receipts.insert(
        "expense_by_category".into(),
        grouped_receipt.document.clone(),
    );
    let (vendor_app, vendor_work) = ids("vendor-spend");
    let (unpaid, _) = run_view(vendor_app, vendor_work, "unpaid", json!({})).await;
    assert_eq!(unpaid.columns, ["invoice_id", "vendor", "amount", "due"]);
    assert!(!unpaid.rows.is_empty());
    let (tickets_app, tickets_work) = ids("support-queue");
    let (timeline, timeline_receipt) = run_view(
        tickets_app,
        tickets_work,
        "timeline",
        json!({"window": {"start": "2026-10-10T00:00:00Z", "end": "2026-10-13T00:00:00Z"}}),
    )
    .await;
    assert_eq!(timeline.columns, ["time", "tickets"]);
    assert!(timeline.rows.iter().all(|r| {
        let day = r[0].as_str().unwrap();
        day.ends_with("T00:00:00Z")
            && ("2026-10-10T00:00:00Z".."2026-10-13T00:00:00Z").contains(&day)
    }));
    assert_eq!(
        timeline_receipt.document["parameters"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    library_receipts.insert(
        "tickets_timeline_windowed".into(),
        timeline_receipt.document.clone(),
    );
    let (traffic_app, traffic_work) = ids("outbound-traffic");
    let (filtered_total, filtered_receipt) = run_view(
        traffic_app,
        traffic_work,
        "total",
        json!({"destination": "198.51.100.7", "filter_epoch": 2}),
    )
    .await;
    assert_eq!(
        filtered_total.columns,
        ["connections", "total_bytes", "largest_transfer"]
    );
    assert_eq!(
        filtered_receipt.document["parameters"][0]["field_id"],
        json!("destination")
    );
    assert_eq!(filtered_receipt.document["filter_epoch"], json!(2));
    library_receipts.insert(
        "traffic_total_filtered".into(),
        filtered_receipt.document.clone(),
    );
    // A window or destination parameter for an input without those fields is
    // refused before any engine, never recorded in a receipt it did not shape.
    let (inventory_app, inventory_work) = ids("inventory-comparison");
    assert_eq!(
        stores
            .data
            .run_query(
                &member,
                &inventory_app,
                &inventory_work,
                "totals",
                &window()
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        stores
            .data
            .run_query(
                &member,
                &inventory_app,
                &inventory_work,
                "totals",
                &json!({"destination": "x"})
            )
            .await
            .err(),
        Some(Error::Denied)
    );
    // The reviewer holds no grant on a library app: denied before any engine.
    assert_eq!(
        stores
            .data
            .run_query(&reviewer, &expense_app, &expense_work, "totals", &json!({}))
            .await
            .err(),
        Some(Error::Denied)
    );
    check(
        &mut checks,
        "library views run through run_query, the confined engine, release_result and a contract-valid receipt: a summary (count/sum/max over money), a paged table (50 rows, truncated), a grouped breakdown, a boolean filter, a windowed daily timeline and a destination-filtered total; a window or destination on an input without those fields and an ungranted member are Denied before any engine",
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
        Err(QueryError::Failed(
            Failure::QueueExpired | Failure::Deadline
        ))
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
            "library_receipts": library_receipts,
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
                "The registered record is written by the test with the APP-077 digests and the content-derived revision id; upload-sources itself (merged separately) is not run here.",
                "The library revisions (ADR-011) are normalized into the local cache by library_sources.build from the committed synthetic CSVs; their upload to the bucket is the deploy's (APP-096), under the B1 keys library_sources.py plan prints."
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
/// The `name=value` pair of the run cookie a raw reply set.
fn run_cookie_set(reply: &Raw) -> String {
    reply
        .1
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|c| c.starts_with(&format!("{}=", crate::auth::cookies::RUN)))
        .map(|c| c.split(';').next().unwrap().to_owned())
        .unwrap_or_else(|| panic!("no run cookie set: {:?}", reply.1))
}
/// The run form through the browser (POST/redirect/GET): the POST answers
/// `303` to the page and sets the run cookie (`Strict`, `HttpOnly`,
/// `Secure`, 10 minutes); the GET it redirects to runs the views.
async fn run_and_show(
    browser: &mut crate::auth::tests::Browser,
    routes: &axum::Router,
    page: &str,
    body: &str,
) -> crate::auth::tests::Reply {
    let posted = browser.post(routes, page, body).await;
    assert_eq!(
        posted.status,
        axum::http::StatusCode::SEE_OTHER,
        "{}",
        posted.body
    );
    assert_eq!(posted.location(), page);
    assert!(
        posted.body.is_empty(),
        "the POST carries no page and no number"
    );
    let set = posted
        .sets(crate::auth::cookies::RUN)
        .expect("the run cookie");
    assert!(
        set.contains("SameSite=Strict")
            && set.contains("HttpOnly")
            && set.contains("Secure")
            && set.contains("Path=/")
            && set.contains("Max-Age=600"),
        "{set}"
    );
    browser.get(routes, page).await
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
    // APP-093 review M4: the route carries each column's value type.
    assert_eq!(
        serde_json::from_str::<Value>(&total.body).unwrap()["column_types"],
        json!({"total_bytes": {"type": "integer"}})
    );
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
    assert_eq!(total_receipt["source_revisions"], bound_sources());
    let envelope = self::rows(&c.fixture, &format!("SELECT convert_from(canonical_envelope,'UTF8') FROM genui.source_revisions WHERE organization_id='{ORG}' AND source_id='{SOURCE}' AND revision_id='{REVISION}'")).await;
    assert_eq!(
        bundle_verdict(&total_receipt, ROUTE_WORK, &envelope[0]),
        Ok(()),
        "the delivered receipt passes validate_bundle against its work"
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
        "route_authorized_member_gets_exact_result_and_receipt_digest: a member signed in through the APP-009 routes gets total 1,140,000 for [10:00, 10:10), 900,000 for destination 198.51.100.7 at filter epoch 1 and the five timeline buckets from POST /v1/work/{app}/{work}/queries/{view} (application/json, no-store); each receipt is contract-valid (contracts/validate.py validate), its digest and result digest recompute, it pins her principal, the new work, every source dependency of the work and the uploaded revision's real digest, and the total receipt passes validate_bundle against its work; without the window the same view is 2,040,000 with a receipt that records no window",
    );

    // --- Review M2: an impossible window never runs, on either route ---
    let calls = counting.calls();
    for (start, end) in [
        ("2026-02-31T00:00:00Z", "2026-03-01T00:00:00Z"),
        ("0000-01-01T00:00:00Z", "0000-01-01T10:10:00Z"),
        ("2026-04-31T00:00:00Z", "2026-05-01T00:00:00Z"),
    ] {
        let refused = maya
            .post(
                &routes,
                &query("total"),
                &query_body(&json!({"window": {"start": start, "end": end}})),
            )
            .await;
        assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY, "{start}");
        assert_eq!(
            refused.body,
            "{\"code\":\"invalid_parameters\",\"status\":422}"
        );
    }
    assert_eq!(counting.calls(), calls, "impossible dates reach no engine");
    check(
        checks,
        "impossible window bounds (2026-02-31, 2026-04-31, year 0000) are 422 invalid_parameters on the JSON route before authorization or any engine (review M2, probe P6)",
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
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", ""),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
    let props = last_page();
    let total_card = &props.views[0];
    assert_eq!(
        (
            total_card.state.as_str(),
            total_card.badge.as_str(),
            total_card.summary[0].value.as_str()
        ),
        ("ready", "ready", "1,140,000")
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
    assert_eq!(card_receipt["source_revisions"], bound_sources());
    let timeline_card = &props.views[1];
    assert_eq!(timeline_card.state, "ready");
    assert_eq!(timeline_card.rows.len(), 5);
    assert_eq!(timeline_card.rows[3].cells[2].text, "600,000");
    // Timestamps and destinations are text: never digit-grouped (review m11).
    assert_eq!(timeline_card.rows[3].cells[0].text, "2026-10-22T10:03:00Z");
    // The restored form echoes the normalized window as the control spells it.
    assert_eq!(props.window_start, "2026-10-22T10:00:00");
    assert_eq!(props.window_end, "2026-10-22T10:10:00");
    let mut delivered_receipts = vec![card_receipt.clone()];
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", "198.51.100.7"),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
    let props = last_page();
    assert_eq!(props.views[0].summary[0].value, "900,000");
    assert_eq!(props.destination, "198.51.100.7");
    delivered_receipts.push(serde_json::from_str(&props.views[0].receipt_json).unwrap());
    // A destination with no event in the window: empty, never zero.
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", "192.0.2.250"),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
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
    for view in &props.views {
        delivered_receipts.push(serde_json::from_str(&view.receipt_json).unwrap());
    }
    let calls = counting.calls();
    // No window: 422 rendered directly (no redirect, the run cookie untouched),
    // every view idle, nothing ran; an impossible day likewise; a bad token: 403.
    let kept = maya.cookie(crate::auth::cookies::RUN).unwrap();
    for refused_form in [
        form("", "", "198.51.100.7"),
        form("2026-02-31T00:00", "2026-03-01T00:00", ""),
        form("0000-01-01T00:00", "0000-01-01T10:10", ""),
    ] {
        let refused = maya.post(&routes, &page, &refused_form).await;
        assert_eq!(
            refused.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{refused_form}"
        );
        assert!(refused.sets(crate::auth::cookies::RUN).is_none());
        let props = last_page();
        assert!(props.form_error && props.views.iter().all(|v| v.idle));
    }
    assert_eq!(
        maya.cookie(crate::auth::cookies::RUN).as_deref(),
        Some(kept.as_str())
    );
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
        "work page: GET /w/{app}/{work} shows the definition's two runnable views idle and runs nothing; the run form (window required, destination optional, synchronizer token) answers 303 to the page with the run cookie and the page then shows total 1,140,000 and the five timeline buckets with contract-valid receipts listing every source dependency, 900,000 for the destination, and empty (never zero) for a destination with no event; a missing window and an impossible day (2026-02-31, year 0000) are 422 with every view idle and the run cookie untouched, a forged token 403, none reaching the engine",
    );

    // --- Reload restores (ADR-010 §1 act 4): GET again, fresh run and release ---
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", ""),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
    let first = last_page();
    assert_eq!(first.views[0].summary[0].value, "1,140,000");
    let before = counting.calls();
    let mut reloads = Vec::new();
    for reload in 1..=2 {
        let again = maya.get(&routes, &page).await;
        assert_eq!(again.status, StatusCode::OK, "reload {reload}");
        assert!(again.headers.get(header::LOCATION).is_none());
        let refreshed = again.sets(crate::auth::cookies::RUN).expect("refreshed");
        assert!(refreshed.contains("Max-Age=600") && refreshed.contains("SameSite=Strict"));
        let props = last_page();
        assert_eq!(
            props.views[0].summary[0].value, "1,140,000",
            "reload {reload}"
        );
        assert_eq!(props.views[1].rows.len(), 5, "reload {reload}");
        assert_eq!(props.window_start, "2026-10-22T10:00:00");
        assert_eq!(
            counting.calls(),
            before + 2 * reload,
            "each reload runs both views again"
        );
        // The same pinned state and parameters reproduce the same receipt
        // identity: a fresh run, release and receipt, and the same number.
        assert_eq!(props.views[0].receipt_digest, first.views[0].receipt_digest);
        assert_eq!(props.views[1].receipt_digest, first.views[1].receipt_digest);
        delivered_receipts.push(serde_json::from_str(&props.views[0].receipt_json).unwrap());
        reloads.push(json!({"reload": reload, "total": props.views[0].summary[0].value, "timeline_rows": props.views[1].rows.len(), "receipt_digest": props.views[0].receipt_digest}));
    }
    // Revocation between execution and release on a restored run: denied cards.
    let (computed, resume) = counting.pause_next();
    let racing = tokio::spawn(send_raw(
        routes.clone(),
        raw_request("GET", &page, &jar(&maya), Vec::new()),
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
    // Authority already lost when the page reloads: the page itself fails
    // closed (uniform 404), so nothing is shown at all.
    assert!(maya.get(&routes, &page).await.uniform_404());
    grant(&c.fixture, &member, "source_read", &["--source", SOURCE]);
    assert_eq!(maya.get(&routes, &page).await.status, StatusCode::OK);
    assert_eq!(last_page().views[0].summary[0].value, "1,140,000");
    // Her run cookie presented on another work of hers: foreign, ignored,
    // cleared; the page is idle and nothing runs. A tampered cookie likewise.
    let second_work = "00000000-0000-4000-8000-0000000000f8";
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
            second_work,
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
    let calls = counting.calls();
    let other_page = maya.get(&routes, &format!("/w/{APP}/{second_work}")).await;
    assert_eq!(other_page.status, StatusCode::OK);
    assert!(other_page
        .sets(crate::auth::cookies::RUN)
        .is_some_and(|set| set.contains("Max-Age=0")));
    assert!(last_page().views.iter().all(|v| v.idle));
    assert_eq!(maya.cookie(crate::auth::cookies::RUN), None, "cleared");
    assert!(last_page().views.iter().all(|v| v.idle));
    let idle_again = maya.get(&routes, &page).await;
    assert_eq!(idle_again.status, StatusCode::OK);
    assert!(last_page().views.iter().all(|v| v.idle));
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", ""),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
    let calls = calls + 2;
    assert_eq!(counting.calls(), calls);
    let mut tampered = maya.cookie(crate::auth::cookies::RUN).unwrap();
    let last = tampered.pop().unwrap();
    tampered.push(if last == 'A' { 'B' } else { 'A' });
    maya.jar.insert(crate::auth::cookies::RUN.into(), tampered);
    let ignored = maya.get(&routes, &page).await;
    assert_eq!(ignored.status, StatusCode::OK);
    assert!(ignored
        .sets(crate::auth::cookies::RUN)
        .is_some_and(|set| set.contains("Max-Age=0")));
    assert!(last_page().views.iter().all(|v| v.idle));
    assert_eq!(counting.calls(), calls, "an ignored cookie runs nothing");
    // A query string on the page stays the uniform 404, cookie or not.
    assert!(maya
        .get(&routes, &format!("{page}?window_start=2026-10-22T10:00"))
        .await
        .uniform_404());
    check(
        checks,
        "reload restores (ADR-010 act 4): after the run form's 303, GET /w/{app}/{work} twice shows the same 1,140,000 and five buckets with a fresh run and release each time (dispatch +2 per reload, identical receipt identity for the same pinned state and parameters) and refreshes the Strict run cookie; a source_read revocation between execution and release of a restored run renders both views denied, a revocation before the reload makes the page the uniform 404; the cookie presented on another work of hers and a tampered cookie are ignored and cleared with every view idle and no dispatch; a query string stays the uniform 404",
    );

    // --- D7.9 (review M4): a lock wait on the member's own row during session
    // resolution is the uniform 404 (fails closed), never 503; no dispatch ---
    let calls = counting.calls();
    let holder = c.fixture.operator().await;
    holder.client.batch_execute(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{member}' FOR UPDATE;")).await.unwrap();
    let token = auth.csrf.token(
        crate::auth::csrf::Form::Query,
        &maya.cookie(crate::auth::cookies::SESSION).unwrap(),
    );
    let mut lock_wait = Vec::new();
    for (router, method, uri, body) in [
        (c.app.clone(), "GET", "/workspace".to_owned(), Vec::new()),
        (
            routes.clone(),
            "POST",
            query("total"),
            query_body(&window()).into_bytes(),
        ),
        (routes.clone(), "GET", page.clone(), Vec::new()),
        (
            routes.clone(),
            "POST",
            page.clone(),
            format!("csrf={token}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=").into_bytes(),
        ),
    ] {
        let started = Instant::now();
        let reply = send_raw(router, raw_request(method, &uri, &jar(&maya), body)).await;
        let elapsed = started.elapsed();
        assert!(
            raw_404(&reply),
            "{method} {uri}: {} {}",
            reply.0,
            String::from_utf8_lossy(&reply.2)
        );
        assert!(reply.1.get(header::SET_COOKIE).is_none());
        // The wait is the pool's lock_timeout (1,000 ms), inside the handler deadline.
        assert!(
            elapsed >= Duration::from_millis(900) && elapsed < Duration::from_secs(5),
            "{method} {uri}: {elapsed:?}"
        );
        lock_wait.push(json!({"method": method, "route": uri.replace(APP, "{app}").replace(ROUTE_WORK, "{work}"), "status": 404, "elapsed_ms": elapsed.as_millis()}));
    }
    holder.client.batch_execute("ROLLBACK").await.unwrap();
    drop(holder);
    assert_eq!(counting.calls(), calls, "no dispatch while the row is held");
    // Released, the same cookie is live again on every route.
    assert_eq!(
        send_raw(
            c.app.clone(),
            raw_request("GET", "/workspace", &jar(&maya), Vec::new())
        )
        .await
        .0,
        StatusCode::OK
    );
    let shown = run_and_show(
        &mut maya,
        &routes,
        &page,
        &form("2026-10-22T10:00", "2026-10-22T10:10", ""),
    )
    .await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(last_page().views[0].summary[0].value, "1,140,000");
    assert_eq!(counting.calls(), calls + 2);
    check(
        checks,
        "D7.9 lock wait during session resolution (review M4, probe P14): while an operator transaction holds the member's row FOR UPDATE, her live cookie on GET /workspace, the JSON route, the page and the run form answers the uniform 404 after lock_timeout (never 503), with no Set-Cookie and no dispatch; once released the same cookie is live again (200) and the run form shows 1,140,000",
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
    let dispatch_lines = log_lines("query.dispatch");
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
    // APP-076 G6: the service log shows the same: no `query.dispatch` line was
    // written for any of these requests.
    assert_eq!(log_lines("query.dispatch"), dispatch_lines);
    check(
        checks,
        &format!("foreign, revoked, pre-access and anonymous requests get the uniform 404 and invoke no engine: {denials} requests (anonymous, a pre-access session, a pre-access cookie presented as a session, a canary member replaying the Northstar ids, a member without work or source access, a revoked membership, and the member after her own source_read revocation) on the JSON route, the page and the run form; the dispatch counter did not move and the service log gained no query.dispatch line"),
    );

    // --- APP-093 review m5: GET /v1/library at the route ---
    // Without a live session (anonymous, pre-access, a pre-access cookie as a
    // session, a revoked membership) the uniform 404; a live member of either
    // organization gets 200 with only the library apps it may open now: none
    // for the canary member (its organization holds no Northstar app) and the
    // member without grants, exactly the one granted app for Maya.
    for (label, browser) in [
        ("anonymous", &mut anonymous),
        ("pre-access", &mut pending),
        ("pre-access cookie as a session", &mut swapped),
        ("revoked membership", &mut revoked),
    ] {
        assert!(
            browser.get(&routes, "/v1/library").await.uniform_404(),
            "{label} library"
        );
        assert!(
            browser
                .post(&routes, "/v1/library", "{}")
                .await
                .uniform_404(),
            "{label} library POST"
        );
    }
    let library = |reply: &crate::auth::tests::Reply| -> Vec<(String, String, String)> {
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        assert_eq!(reply.headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
        let body: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(
            (body["schema_version"].clone(), body["synthetic"].clone()),
            (json!(1), json!(true))
        );
        body["apps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["library_id"].as_str().unwrap().to_owned(),
                    a["app_id"].as_str().unwrap().to_owned(),
                    a["work_id"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    for (label, browser) in [
        ("canary member (foreign organization)", &mut canary),
        ("member without library grants", &mut outsider),
    ] {
        assert_eq!(
            library(&browser.get(&routes, "/v1/library").await),
            Vec::new(),
            "{label}"
        );
        assert!(
            browser
                .post(&routes, "/v1/library", "{}")
                .await
                .uniform_404(),
            "{label} library POST"
        );
    }
    assert_eq!(library(&maya.get(&routes, "/v1/library").await), Vec::new());
    let sidecar: Value = serde_json::from_slice(
        &std::fs::read(repository().join("contracts/fixtures/library/catalog.json")).unwrap(),
    )
    .unwrap();
    let expense = sidecar["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["library_id"] == "expense-review")
        .unwrap()
        .clone();
    let (expense_app, expense_work, expense_source) = (
        expense["app_id"].as_str().unwrap(),
        expense["work_id"].as_str().unwrap(),
        expense["sources"][0]["source_id"].as_str().unwrap(),
    );
    let library_grants: [(&str, Vec<&str>); 4] = [
        ("discover", vec!["--app", expense_app]),
        ("run", vec!["--app", expense_app]),
        (
            "record_read",
            vec!["--app", expense_app, "--work", expense_work],
        ),
        ("source_read", vec!["--source", expense_source]),
    ];
    for (permission, resource) in &library_grants {
        grant(&c.fixture, &member, permission, resource);
    }
    assert_eq!(
        library(&maya.get(&routes, "/v1/library").await),
        [(
            "expense-review".to_owned(),
            expense_app.to_owned(),
            expense_work.to_owned()
        )]
    );
    // M4 at the route: the expense totals answer carries the money type.
    let totals = maya
        .post(
            &routes,
            &format!("/v1/work/{expense_app}/{expense_work}/queries/totals"),
            &query_body(&json!({})),
        )
        .await;
    assert_eq!(totals.status, StatusCode::OK, "{}", totals.body);
    let (rows, _) = receipted(&totals.body);
    assert_eq!(rows[0][1], json!("26262004"));
    assert_eq!(
        serde_json::from_str::<Value>(&totals.body).unwrap()["column_types"],
        json!({"claims": {"type": "integer"},
            "largest_claim": {"type": "money", "currency": "USD", "minor_unit_exponent": 2},
            "total_amount": {"type": "money", "currency": "USD", "minor_unit_exponent": 2}})
    );
    // Without the work's record_read the app is not listed again.
    crate::storage::definition_qualification::revoke(
        &c.fixture,
        &member,
        "record_read",
        &["--app", expense_app, "--work", expense_work],
    );
    assert_eq!(library(&maya.get(&routes, "/v1/library").await), Vec::new());
    for (permission, resource) in &library_grants[..2] {
        crate::storage::definition_qualification::revoke(&c.fixture, &member, permission, resource);
    }
    crate::storage::definition_qualification::revoke(
        &c.fixture,
        &member,
        "source_read",
        &["--source", expense_source],
    );
    check(
        checks,
        "APP-093 m5, GET /v1/library at the route: anonymous, pre-access, a pre-access cookie as a session and a revoked membership get the uniform 404 (POST is the fallback's 404); a live canary member and a member without grants get 200 with no app; Maya gets exactly expense-review (its seeded app and work) once granted discover, run, record_read and source_read (and its totals route answer carries column_types: claims integer, total_amount 26262004 and largest_claim money USD exponent 2), and nothing once record_read is revoked",
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
    // The page: the form's 303 saves the run; the GET it redirects to computes
    // both views, then denies both at release.
    let opened = maya.get(&routes, &page).await;
    assert_eq!(opened.status, StatusCode::OK);
    let csrf = last_page().csrf;
    let posted = maya
        .post(
            &routes,
            &page,
            &format!("csrf={csrf}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination="),
        )
        .await;
    assert_eq!(posted.status, StatusCode::SEE_OTHER);
    let (computed, resume) = counting.pause_next();
    let racing = tokio::spawn(send_raw(
        routes.clone(),
        raw_request("GET", &page, &jar(&maya), Vec::new()),
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
        "revocation between execution and delivery denies release: with the engine paused after computing 1,140,000, a source_read revocation turns the JSON response into the uniform 404 with no number; on the page a membership revocation in the same gap (the run form's 303, then the restored GET) renders both views denied with no value or receipt, and the next request finds no session",
    );
    // Every receipt the page delivered validates under contracts/validate.py
    // (review M2: a receipt the validator refuses is never delivered).
    assert!(delivered_receipts.len() >= 6);
    for document in &delivered_receipts {
        assert!(contract_valid(document), "{document}");
    }
    check(
        checks,
        &format!("every receipt the work page delivered ({} receipts over the first run, the destination, the empty views and the reloads) validates under contracts/validate.py validate", delivered_receipts.len()),
    );

    // --- 429 under load: a full admission queue answers Retry-After: 2 ---
    // The slow engine's cache holds the real revision bytes (the parent
    // verifies the cached digest before every spawn, review m4).
    let load_root = crate::query::tests::Directory::new("route-load");
    assert_eq!(
        cache_northstar(&fixture_python(), &load_root.0.join("cache"), ORG, REVISION),
        NORMALIZED
    );
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
    // The page in the same state: the form's 303 saves the run, and the GET
    // it redirects to shows every view loading (labelled busy), none a value.
    let token = auth.csrf.token(
        crate::auth::csrf::Form::Query,
        &load_browser.cookie(crate::auth::cookies::SESSION).unwrap(),
    );
    let posted = send_raw(
        loaded.clone(),
        raw_request(
            "POST",
            &format!("/w/{APP}/{load_work}"),
            &cookies,
            format!("csrf={token}&window_start=2026-10-22T10:00&window_end=2026-10-22T10:10&destination=").into_bytes(),
        ),
    )
    .await;
    assert_eq!(posted.0, StatusCode::SEE_OTHER);
    let with_run = format!("{cookies}; {}", run_cookie_set(&posted));
    let loading = send_raw(
        loaded.clone(),
        raw_request(
            "GET",
            &format!("/w/{APP}/{load_work}"),
            &with_run,
            Vec::new(),
        ),
    )
    .await;
    assert_eq!(loading.0, StatusCode::OK);
    let props = last_page();
    assert_eq!(
        props
            .views
            .iter()
            .map(|v| (v.state.as_str(), v.badge.as_str()))
            .collect::<Vec<_>>(),
        [("loading", "busy"), ("loading", "busy")]
    );
    assert!(props
        .views
        .iter()
        .all(|v| v.summary.is_empty() && v.rows.is_empty() && !v.has_receipt));
    let mut filled = (0, 0);
    while let Some(outcome) = fill.join_next().await {
        match outcome.unwrap() {
            Ok(()) => filled.0 += 1,
            Err(QueryError::Failed(Failure::QueueExpired | Failure::Deadline)) => filled.1 += 1,
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
        &format!("429 under load: with the organization's engine admission full (2 running, 20 waiting), an authorized POST /v1/work/{{app}}/{{work}}/queries/{{view}} is 429 too_many_requests with Retry-After: 2 and no-store, and the work page (the form's 303, then the restored GET) renders both views loading, labelled busy, with no value or receipt; the engine rejected exactly those 3 and never ran more than {} at once; the fillers finished ({} completed, {} expired in the queue); once drained the same request is 200", load.peak_organization_running, filled.0, filled.1),
    );

    let report = json!({
        "receipts": {"total": total_receipt, "destination": destination_receipt, "timeline": timeline_receipt, "whole_revision": whole_receipt},
        "dispatch_calls": counting.calls(),
        "denied_requests_without_dispatch": denials,
        "reload_restores": {"mechanism": "POST 303 + __Host-genui_run (SameSite=Strict, HttpOnly, Secure, Max-Age 600, tag over work path, session cookie and canonical parameters) + GET", "reloads": reloads},
        "lock_wait": {"requests": lock_wait, "lock_timeout_ms": 1000},
        "delivered_page_receipts": delivered_receipts.len(),
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
                "GET",
                "/v1/library".to_owned(),
                Vec::new(),
            ),
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
        (down_routes.clone(), "GET", "/v1/library".to_owned()),
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
        &format!("store outage maps alike on GET /workspace and the APP-012 routes: with the workspace store unreachable, the member's live cookie and a forged cookie both get 503 unavailable (no-store) on GET /workspace, GET /v1/library, the JSON route, the page and its run form ({} requests in {outage_ms} ms, inside the 5 s handler deadline, so the answer is the storage mapping), without a cookie the uniform 404; the engine is never reached", outage.len()),
    );
    let mut report = report;
    report["store_outage"] = json!({"requests": outage, "elapsed_ms": outage_ms, "dispatch_calls": down_counting.calls()});

    // --- APP-076 G6: the structured query log accounts for every dispatch ---
    // One `query.dispatch` line per engine child launched (the real engine's
    // and the slow engine's), release decisions logged with receipt and
    // result identities, refusals before dispatch with their code; no window
    // bound or destination value ever appears in the log.
    let dispatched = log_lines("query.dispatch");
    assert_eq!(
        dispatched,
        engine.stats().launched + slow.stats().launched,
        "one dispatch line per child launched"
    );
    let released = log_lines("query.released");
    let denied_release = log_lines("query.release_denied");
    let refused_before = log_lines("query.refused_before_dispatch");
    // B1 review m4: a dispatched run that yields no result has its own
    // terminal line; every terminal line follows a dispatch (runs the
    // campaign drives through the engine directly have no release line).
    let failed_after = log_lines("query.failed_after_dispatch");
    assert!(
        released >= 10 && denied_release >= 2 && refused_before >= 3,
        "{released} {denied_release} {refused_before}"
    );
    assert!(
        released + denied_release + failed_after <= dispatched,
        "{released} + {denied_release} + {failed_after} terminal lines exceed {dispatched} dispatches"
    );
    assert!(log_mentions(&format!(
        "receipt_id=\"{}\"",
        total_receipt["digest"].as_str().unwrap()
    )));
    assert!(
        log_mentions("code=\"busy\"")
            && log_mentions("code=\"denied\"")
            && log_mentions("code=\"invalid_parameters\"")
    );
    for secret in [
        "2026-10-22T10:00:00Z",
        "198.51.100.7",
        "192.0.2.250",
        &cookies,
    ] {
        assert!(!log_mentions(secret), "{secret} in the log");
    }
    report["query_log"] = json!({"dispatch": dispatched, "released": released, "release_denied": denied_release, "failed_after_dispatch": failed_after, "refused_before_dispatch": refused_before});
    check(
        checks,
        &format!("structured query log (APP-076 G6): {dispatched} query.dispatch lines, one per engine child launched across both engines; {released} query.released and {denied_release} query.release_denied lines carrying receipt and result identities; {failed_after} query.failed_after_dispatch lines, and no more terminal lines than dispatches; {refused_before} query.refused_before_dispatch lines with their codes (busy, denied, invalid_parameters among them); no window bound, destination or cookie value appears in the log"),
    );
    let Campaign { fixture, .. } = c;
    (fixture, report)
}

/// ADR-011: the pitch canvas campaign extends this one (its fixture helpers).
#[path = "canvas_campaign.rs"]
mod canvas_campaign;

/// ADR-011: the pitch integration campaign (APP-096) extends this one too.
#[path = "pitch_qualification.rs"]
pub(crate) mod pitch_campaign;
