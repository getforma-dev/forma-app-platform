//! APP-012 engine, admission and receipt checks. The fake engines are
//! `/bin/sh` scripts speaking the child protocol, so admission, deadlines,
//! confinement and parsing are tested without DuckDB; the real child runs in
//! `engine_child_campaign` (ignored: needs the locked Python environment with
//! DuckDB 2.0.0) and in `storage::query_qualification` (Docker).
use super::{
    receipt,
    worker::{
        integer_columns, result_digest, typed_integer_columns, Engine, EngineSettings, Failure,
        QueryError, QUEUE, RSS_LIMIT_BYTES,
    },
};
use crate::storage::QueryRun;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const ORG: &str = "00000000-0000-4000-8000-000000000001";
const OTHER_ORG: &str = "00000000-0000-4000-8000-000000000002";
const PRINCIPAL: &str = "00000000-0000-4000-8000-000000000066";
const APP: &str = "00000000-0000-4000-8000-000000000065";
const WORK: &str = "00000000-0000-4000-8000-000000000069";
const PUBLICATION: &str = "00000000-0000-4000-8000-000000000068";
const SOURCE: &str = "00000000-0000-4000-8000-00000000006e";
const REVISION: &str = "00000000-0000-4000-8000-00000000006f";
const DEFINITION: &str = "sha256:89424c7129b6d43c70c2a72f8eaa680504fe6081cdf563cb7b01a33fbf2d59b2";
/// APP-077's uploaded `part.parquet` of the Northstar events revision.
const NORMALIZED: &str = "sha256:0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082";
const ROWS_LINE: &str = r#"{"columns":["total_bytes"],"elapsed_ms":7,"outcome":"rows","partial":false,"rows":[["1140000"]],"total_scope":"full_authorized_filtered_input","truncated_page":false}"#;
/// The bytes the fake engines' cached revision holds.
const CACHED_BYTES: &[u8] = b"cached";

pub(crate) fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn read_json(relative: &str) -> Value {
    serde_json::from_slice(&fs::read(repository().join(relative)).unwrap()).unwrap()
}
/// The seeded (edition impl-2026-10-28.1) Northstar view `id`'s typed query.
pub(crate) fn seeded_query(view: &str) -> Value {
    let bundle = read_json("contracts/fixtures/impl-2026-10-28.1/northstar.json");
    bundle["definition"]["content"]["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == view)
        .unwrap()["query"]
        .clone()
}
pub(crate) fn window() -> Value {
    json!({"window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z"}})
}
/// `sha256:` of some bytes, as a revision's `normalized_digest`.
pub(crate) fn digest_of(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!(
        "sha256:{}",
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
/// The digest of the fake engines' cached bytes (the parent verifies the
/// cached file before every spawn, review m4).
pub(crate) fn cached_digest() -> String {
    digest_of(CACHED_BYTES)
}
fn run_for(organization: &str, view: &str, parameters: &Value, digest: &str) -> QueryRun {
    QueryRun::for_tests(
        organization,
        PRINCIPAL,
        APP,
        WORK,
        PUBLICATION,
        DEFINITION,
        view,
        seeded_query(view),
        parameters,
        (SOURCE, REVISION, digest),
    )
}
/// A run over the fake engines' cached revision.
fn fake_run(organization: &str) -> QueryRun {
    run_for(organization, "total", &window(), &cached_digest())
}

pub(crate) struct Directory(pub PathBuf);
impl Directory {
    pub fn new(label: &str) -> Self {
        let mut bytes = [0u8; 8];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes).unwrap();
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("genui-app012-{label}-{name}"));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn settings(root: &Path, program: &str, args: &[&str]) -> EngineSettings {
    EngineSettings {
        program: program.into(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        cache_root: root.join("cache"),
        scratch_root: root.join("scratch"),
        fetch: None,
        backstop: Duration::from_secs(5),
        rss_limit_bytes: RSS_LIMIT_BYTES,
    }
}
/// Writes `bytes` as the cached revision of `organization`, mode 0400.
pub(crate) fn cache_bytes(root: &Path, organization: &str, bytes: &[u8]) -> PathBuf {
    let directory = root.join("cache/o").join(organization);
    fs::create_dir_all(&directory).unwrap();
    for path in [root.join("cache"), root.join("cache/o"), directory.clone()] {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let file = directory.join(format!("{SOURCE}.{REVISION}.parquet"));
    let _ = fs::remove_file(&file);
    fs::write(&file, bytes).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o400)).unwrap();
    file
}
/// An engine whose child is `sh -c script`, with the run's revision cached.
fn fake(root: &Path, script: &str, organizations: &[&str]) -> Engine {
    let engine = Engine::new(settings(root, "/bin/sh", &["-c", script])).unwrap();
    for organization in organizations {
        cache_bytes(root, organization, CACHED_BYTES);
    }
    engine
}
fn answering(seconds: &str, line: &str) -> String {
    format!("cat >/dev/null; sleep {seconds}; printf '%s\\n' '{line}'")
}
fn scratch_is_empty(root: &Path) -> bool {
    fs::read_dir(root.join("scratch")).unwrap().next().is_none()
}
fn exact(columns: &[&str], rows: Vec<Vec<Value>>) -> super::WorkerResult {
    super::WorkerResult {
        columns: columns.iter().map(|c| (*c).to_owned()).collect(),
        integer_columns: BTreeSet::new(),
        column_types: Default::default(),
        result_digest: result_digest(&rows).unwrap(),
        rows,
        truncated_page: false,
        engine_elapsed_ms: 0,
        wall: Duration::ZERO,
        peak_rss_bytes: 0,
    }
}

#[test]
fn receipt_vectors_match_contracts() {
    // Every edition-1 receipt the contracts carry keeps its digest under the
    // Rust identity and passes the Rust contract check, and the seeded
    // `total` view's typed query is the receipt's `query_digest`.
    for bundle in ["northstar", "spend-review"] {
        let value = read_json(&format!(
            "contracts/fixtures/impl-2026-10-28.1/{bundle}.json"
        ));
        let receipt = &value["receipt"];
        assert_eq!(
            receipt::digest(receipt).as_deref(),
            receipt["digest"].as_str(),
            "{bundle}"
        );
        assert_eq!(receipt::conforms(receipt), Ok(()), "{bundle}");
    }
    let northstar = read_json("contracts/fixtures/impl-2026-10-28.1/northstar.json");
    let total = crate::admission::encode(&seeded_query("total")).unwrap();
    assert_eq!(
        crate::admission::digest::digest(crate::admission::digest::QUERY, &total),
        northstar["receipt"]["provenance"]["query_digest"]
            .as_str()
            .unwrap()
    );
    // A built receipt has exactly the edition-1 members, every required one.
    let schema = read_json("contracts/schema.json");
    let definition = &schema["$defs"]["query_receipt"];
    let run = run_for(ORG, "total", &window(), NORMALIZED);
    let result = exact(&["total_bytes"], vec![vec![json!("1140000")]]);
    let built = receipt::build(&run, &result).unwrap();
    let members: BTreeSet<&str> = built
        .document
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let properties: BTreeSet<&str> = definition["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let required: BTreeSet<&str> = definition["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(members, properties);
    assert!(required.is_subset(&members));
    assert_eq!(receipt::digest(&built.document), Some(built.digest.clone()));
    assert_eq!(built.document["digest"], json!(built.digest));
    assert_eq!(built.document["mode"], "published");
    // Every source dependency of the work (here its one), not only the read input.
    assert_eq!(
        built.document["source_revisions"],
        json!([{"input_id": "events", "source_id": SOURCE, "source_revision_id": REVISION, "source_digest": NORMALIZED}])
    );
    assert_eq!(
        built.document["parameters"],
        json!([
            {"field_id": "window_end", "value": {"type": "timestamp", "value": "2026-10-22T10:10:00Z"}},
            {"field_id": "window_start", "value": {"type": "timestamp", "value": "2026-10-22T10:00:00Z"}}
        ])
    );
    // The stored view and the effective (windowed) query have distinct identities.
    assert_ne!(
        built.document["provenance"]["query_digest"],
        built.document["provenance"]["predicate_digest"]
    );
    assert_eq!(
        built.document["provenance"]["query_digest"],
        northstar["receipt"]["provenance"]["query_digest"]
    );
    // Any changed member changes the identity.
    let mut changed = built.document.clone();
    changed["result_digest"] = json!(result_digest(&[vec![json!("0")]]).unwrap());
    assert_ne!(receipt::digest(&changed), Some(built.digest));
}

/// APP-012 review M2: the Rust receipt check refuses what
/// `contracts/validate.py` refuses, so such a receipt is never delivered.
#[test]
fn a_receipt_outside_the_contract_is_refused_and_never_built() {
    let run = run_for(ORG, "total", &window(), NORMALIZED);
    let result = exact(&["total_bytes"], vec![vec![json!("1140000")]]);
    let built = receipt::build(&run, &result).unwrap().document;
    assert_eq!(receipt::conforms(&built), Ok(()));
    let with = |edit: &dyn Fn(&mut Value)| {
        let mut document = built.clone();
        edit(&mut document);
        // Re-stamp the digest so only the edited member is judged.
        let id = receipt::digest(&document).unwrap();
        document["digest"] = json!(id);
        document
    };
    let impossible = with(&|d| {
        d["parameters"][1]["value"]["value"] = json!("2026-02-31T00:00:00Z");
    });
    let year_zero = with(&|d| {
        d["parameters"][0]["value"]["value"] = json!("0000-01-01T10:10:00Z");
    });
    let extra = with(&|d| d["note"] = json!("x"));
    let preview_with_publication = with(&|d| d["mode"] = json!("synthetic_preview"));
    let duplicate_source = with(&|d| {
        let first = d["source_revisions"][0].clone();
        d["source_revisions"].as_array_mut().unwrap().push(first);
    });
    let duplicate_parameter = with(&|d| {
        let first = d["parameters"][0].clone();
        d["parameters"].as_array_mut().unwrap().push(first);
    });
    let partial = with(&|d| d["partial"] = json!(true));
    let bad_epoch = with(&|d| d["membership_epoch"] = json!(0));
    let bad_view = with(&|d| d["view_id"] = json!("Total"));
    let money = with(&|d| {
        d["parameters"][0]["value"] = json!({"type": "money", "minor_units": "20000", "currency": "EUR", "minor_unit_exponent": 2});
    });
    let mut missing = built.clone();
    missing.as_object_mut().unwrap().remove("result_digest");
    let mut forged = built.clone();
    forged["result_digest"] = json!(result_digest(&[vec![json!("0")]]).unwrap());
    for (label, document) in [
        ("impossible date", impossible),
        ("year 0000", year_zero),
        ("extra member", extra),
        ("preview naming a publication", preview_with_publication),
        ("duplicate source", duplicate_source),
        ("duplicate parameter", duplicate_parameter),
        ("partial", partial),
        ("membership epoch 0", bad_epoch),
        ("view id outside the grammar", bad_view),
        ("unregistered currency", money),
        ("missing result digest", missing),
        ("forged result digest", forged),
    ] {
        assert!(receipt::conforms(&document).is_err(), "{label}");
    }
    // Not a receipt at all.
    assert!(receipt::conforms(&json!({"kind": "query_receipt"})).is_err());
    assert!(receipt::conforms(&json!([])).is_err());
}

/// Captured `tracing` output for one test thread (the service's `fmt`
/// subscriber, no colour); the guard restores the default on drop.
pub(crate) fn capture_log() -> (
    tracing::subscriber::DefaultGuard,
    Arc<std::sync::Mutex<Vec<u8>>>,
) {
    struct Sink(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buffer: Arc<std::sync::Mutex<Vec<u8>>> = Arc::default();
    let sink = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || Sink(sink.clone()))
        .with_ansi(false)
        .finish();
    (tracing::subscriber::set_default(subscriber), buffer)
}
/// The captured lines carrying `event="<event>"`.
pub(crate) fn log_lines(buffer: &Arc<std::sync::Mutex<Vec<u8>>>, event: &str) -> Vec<String> {
    let marker = format!("event=\"{event}\"");
    String::from_utf8_lossy(&buffer.lock().unwrap())
        .lines()
        .filter(|line| line.contains(&marker))
        .map(str::to_owned)
        .collect()
}

/// APP-076 G6: an authorized run logs exactly one `query.dispatch` line with
/// its identities and no parameter value; a run refused before any child
/// logs `query.refused_before_dispatch` with its code and no dispatch line.
#[tokio::test]
async fn the_dispatch_log_names_identities_never_parameters() {
    let (guard, log) = capture_log();
    let root = Directory::new("dispatch-log");
    let engine = fake(&root.0, &answering("0", ROWS_LINE), &[ORG]);
    let mut filtered = window();
    filtered["destination"] = json!("198.51.100.7");
    let run = run_for(ORG, "total", &filtered, &cached_digest());
    engine
        .execute(&run, Duration::from_millis(1800))
        .await
        .unwrap();
    let dispatched = log_lines(&log, "query.dispatch");
    assert_eq!(dispatched.len(), 1, "{dispatched:?}");
    for identity in [
        format!("organization_id=\"{ORG}\""),
        format!("app_id=\"{APP}\""),
        format!("work_id=\"{WORK}\""),
        "view_id=\"total\"".to_owned(),
        "duration_ms=".to_owned(),
    ] {
        assert!(
            dispatched[0].contains(&identity),
            "{identity}: {}",
            dispatched[0]
        );
    }
    assert!(log_lines(&log, "query.refused_before_dispatch").is_empty());
    // Nothing the member typed reaches the log.
    let text = String::from_utf8_lossy(&log.lock().unwrap()).into_owned();
    assert!(!text.contains("198.51.100.7") && !text.contains("2026-10-22"));
    // Refused before any child: the revision is not cached and cannot be fetched.
    let uncached = Engine::new(settings(&root.0, "/bin/sh", &["-c", "exit 9"])).unwrap();
    assert_eq!(
        uncached
            .execute(
                &run_for(OTHER_ORG, "total", &window(), NORMALIZED),
                Duration::from_millis(1800)
            )
            .await,
        Err(QueryError::SourceUnavailable)
    );
    assert_eq!(
        log_lines(&log, "query.dispatch").len(),
        1,
        "no new dispatch line"
    );
    let refused = log_lines(&log, "query.refused_before_dispatch");
    assert_eq!(refused.len(), 1);
    assert!(
        refused[0].contains("code=\"source_unavailable\"")
            && refused[0].contains(&format!("organization_id=\"{OTHER_ORG}\""))
    );
    drop(guard);
}

/// Review m11: the columns the typed query makes integers.
#[test]
fn integer_columns_follow_the_typed_query() {
    let names = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>();
    assert_eq!(
        names(integer_columns(&seeded_query("total"))),
        ["total_bytes"]
    );
    assert_eq!(names(integer_columns(&seeded_query("timeline"))), ["bytes"]);
    let project = json!({"operations": [{"type": "project", "fields": [{"input_id": "events", "field_id": "destination"}, {"input_id": "events", "field_id": "time"}]}]});
    assert!(integer_columns(&project).is_empty());
    let extremes = json!({"operations": [{"type": "aggregate", "group_by": [{"input_id": "events", "field_id": "destination"}], "measures": [
        {"id": "first", "operation": "min", "field": {"input_id": "events", "field_id": "time"}},
        {"id": "largest", "operation": "max", "field": {"input_id": "events", "field_id": "bytes"}},
        {"id": "events", "operation": "count", "field": {"input_id": "events", "field_id": "destination"}}]}]});
    assert_eq!(names(integer_columns(&extremes)), ["events", "largest"]);
    assert_eq!(names(integer_columns(&json!({}))), ["bytes"]);
    // ADR-011 library: money and integer fields of any input are exact
    // integers; text, enum, date, boolean and timestamp columns are not.
    let fields = vec![
        json!({"id": "expense_id", "value_type": {"type": "text"}, "nullable": false}),
        json!({"id": "time", "value_type": {"type": "timestamp"}, "nullable": false}),
        json!({"id": "category", "value_type": {"type": "enum", "values": ["travel"]}, "nullable": false}),
        json!({"id": "amount", "value_type": {"type": "money", "currency": "USD", "minor_unit_exponent": 2}, "nullable": false}),
        json!({"id": "paid", "value_type": {"type": "boolean"}, "nullable": false}),
        json!({"id": "due", "value_type": {"type": "date"}, "nullable": false}),
    ];
    assert_eq!(
        names(typed_integer_columns(&json!({}), &fields)),
        ["amount"]
    );
    let grouped = json!({"operations": [{"type": "aggregate", "group_by": [{"input_id": "x", "field_id": "category"}], "measures": [
        {"id": "claims", "operation": "count", "field": {"input_id": "x", "field_id": "expense_id"}},
        {"id": "spend", "operation": "sum", "field": {"input_id": "x", "field_id": "amount"}},
        {"id": "latest", "operation": "max", "field": {"input_id": "x", "field_id": "time"}},
        {"id": "largest", "operation": "max", "field": {"input_id": "x", "field_id": "amount"}}]}]});
    assert_eq!(
        names(typed_integer_columns(&grouped, &fields)),
        ["claims", "largest", "spend"]
    );
    let bucketed = json!({"operations": [{"type": "time_bucket", "field": {"input_id": "x", "field_id": "time"}, "seconds": 86400, "timezone": "UTC", "output_field": "time"},
        {"type": "aggregate", "group_by": [{"input_id": "x", "field_id": "time"}], "measures": [{"id": "total", "operation": "sum", "field": {"input_id": "x", "field_id": "amount"}}]}]});
    assert_eq!(names(typed_integer_columns(&bucketed, &fields)), ["total"]);
}

/// APP-093 review M4: every result column's value type follows the typed
/// query: a sum or extreme of money stays money in its currency and
/// exponent, a decimal keeps its scale, a count is an integer, a bucket is a
/// timestamp, text and enum are text; the query route returns them as
/// `column_types`.
#[test]
fn column_types_follow_the_typed_query() {
    use super::worker::{column_types_json, typed_column_types, ColumnType};
    let fields = vec![
        json!({"id": "time", "value_type": {"type": "timestamp"}, "nullable": false}),
        json!({"id": "category", "value_type": {"type": "enum", "values": ["travel"]}, "nullable": false}),
        json!({"id": "amount", "value_type": {"type": "money", "currency": "EUR", "minor_unit_exponent": 2}, "nullable": false}),
        json!({"id": "rate", "value_type": {"type": "decimal", "unit": "percent", "scale": 4}, "nullable": false}),
        json!({"id": "seats", "value_type": {"type": "integer", "unit": "each"}, "nullable": false}),
        json!({"id": "due", "value_type": {"type": "date"}, "nullable": false}),
        json!({"id": "paid", "value_type": {"type": "boolean"}, "nullable": false}),
    ];
    let query = json!({"operations": [
        {"type": "time_bucket", "field": {"input_id": "x", "field_id": "time"}, "seconds": 86400, "timezone": "UTC", "output_field": "day"},
        {"type": "aggregate", "group_by": [{"input_id": "x", "field_id": "day"}, {"input_id": "x", "field_id": "category"}], "measures": [
            {"id": "claims", "operation": "count", "field": {"input_id": "x", "field_id": "amount"}},
            {"id": "spend", "operation": "sum", "field": {"input_id": "x", "field_id": "amount"}},
            {"id": "top_rate", "operation": "max", "field": {"input_id": "x", "field_id": "rate"}},
            {"id": "rate_sum", "operation": "sum", "field": {"input_id": "x", "field_id": "rate"}},
            {"id": "seats", "operation": "sum", "field": {"input_id": "x", "field_id": "seats"}},
            {"id": "last_due", "operation": "max", "field": {"input_id": "x", "field_id": "due"}}]}]});
    let types = typed_column_types(&query, &fields);
    let eur = ColumnType::Money {
        currency: "EUR".into(),
        minor_unit_exponent: 2,
    };
    let expected: std::collections::BTreeMap<String, ColumnType> = [
        ("day", ColumnType::Timestamp),
        ("category", ColumnType::Text),
        ("claims", ColumnType::Integer),
        ("spend", eur),
        ("top_rate", ColumnType::Decimal { scale: 4 }),
        ("rate_sum", ColumnType::Decimal { scale: 4 }),
        ("seats", ColumnType::Integer),
        ("last_due", ColumnType::Date),
    ]
    .into_iter()
    .map(|(name, kind)| (name.to_owned(), kind))
    .collect();
    assert_eq!(types, expected);
    assert_eq!(
        column_types_json(&types)["spend"],
        json!({"type": "money", "currency": "EUR", "minor_unit_exponent": 2})
    );
    assert_eq!(
        column_types_json(&types)["top_rate"],
        json!({"type": "decimal", "scale": 4})
    );
    // A projection keeps only the named fields; a boolean stays verbatim.
    let project = json!({"operations": [{"type": "project", "fields": [{"input_id": "x", "field_id": "paid"}, {"input_id": "x", "field_id": "amount"}]}]});
    assert_eq!(
        typed_column_types(&project, &fields)
            .keys()
            .collect::<Vec<_>>(),
        ["amount", "paid"]
    );
    // An out-of-range exponent or a malformed currency types nothing.
    for bad in [
        json!({"type": "money", "currency": "usd", "minor_unit_exponent": 2}),
        json!({"type": "money", "currency": "USD", "minor_unit_exponent": 40}),
        json!({"type": "decimal", "scale": -1}),
        json!({"type": "reference"}),
    ] {
        assert_eq!(ColumnType::from_value_type(&bad), None, "{bad}");
    }
}

#[tokio::test]
async fn admission_bounds_two_per_organization_four_per_task_then_429() {
    let root = Directory::new("admission");
    let engine = Arc::new(fake(&root.0, &answering("0.2", ROWS_LINE), &[ORG]));
    let run = Arc::new(fake_run(ORG));
    let mut tasks = Vec::new();
    for _ in 0..(QUEUE + 2 + 8) {
        let (engine, run) = (engine.clone(), run.clone());
        tasks.push(tokio::spawn(async move {
            engine.execute(&run, Duration::from_millis(1800)).await
        }));
    }
    let mut busy = 0;
    let mut ok = 0;
    let mut expired = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(result) => {
                assert_eq!(result.rows, vec![vec![json!("1140000")]]);
                ok += 1;
            }
            Err(QueryError::Busy) => busy += 1,
            // Expired in the queue, or admitted so late that the child's
            // answer crossed the deadline: neither is a number.
            Err(QueryError::Failed(Failure::QueueExpired | Failure::Deadline)) => expired += 1,
            Err(other) => panic!("unexpected {other:?}"),
        }
    }
    let stats = engine.stats();
    // Two running and twenty waiting are admitted; the rest are 429 at once.
    assert_eq!(busy, 8, "{stats:?}");
    assert_eq!(ok + expired, QUEUE + 2);
    assert!(ok >= 8, "{ok} completed");
    assert_eq!(stats.peak_organization_running, 2);
    assert_eq!(stats.rejected, 8);
    // A request whose deadline passed in the queue never launched a child.
    assert!(stats.launched <= ok + expired && stats.launched >= ok);
    assert!(scratch_is_empty(&root.0));

    // Four per task across organizations, two per organization.
    let root = Directory::new("task");
    let organizations = [
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000002",
        "00000000-0000-4000-8000-000000000003",
    ];
    let engine = Arc::new(fake(&root.0, &answering("0.3", ROWS_LINE), &organizations));
    let mut tasks = Vec::new();
    for organization in organizations {
        for _ in 0..3 {
            let engine = engine.clone();
            let run = fake_run(organization);
            tasks.push(tokio::spawn(async move {
                engine.execute(&run, Duration::from_millis(1800)).await
            }));
        }
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let stats = engine.stats();
    assert_eq!(stats.peak_task_running, 4, "{stats:?}");
    assert_eq!(stats.peak_organization_running, 2);
    assert_eq!(stats.rejected, 0);
}

/// APP-012 review M1: the queue is per organization. With organization A at
/// 2 running and 20 waiting (1 s children), organization B's request is
/// admitted and completes, while A's next request is 429 at once.
#[tokio::test]
async fn one_organizations_full_queue_never_refuses_another_organization() {
    let root = Directory::new("per-organization");
    let engine = Arc::new(fake(&root.0, &answering("1", ROWS_LINE), &[ORG, OTHER_ORG]));
    let a = Arc::new(fake_run(ORG));
    let mut fill = tokio::task::JoinSet::new();
    for _ in 0..(2 + QUEUE) {
        let (engine, run) = (engine.clone(), a.clone());
        fill.spawn(async move {
            engine
                .execute(&run, Duration::from_millis(1800))
                .await
                .map(|_| ())
        });
    }
    // Let every filler take its place (2 running, 20 waiting).
    tokio::time::sleep(Duration::from_millis(300)).await;
    let refused = engine.execute(&a, Duration::from_millis(1800)).await;
    assert_eq!(refused, Err(QueryError::Busy), "A's 23rd request");
    let started = Instant::now();
    let b = engine
        .execute(&fake_run(OTHER_ORG), Duration::from_millis(1800))
        .await;
    assert_eq!(
        b.map(|r| r.rows),
        Ok(vec![vec![json!("1140000")]]),
        "B runs while A's queue is full"
    );
    assert!(started.elapsed() < Duration::from_millis(1800));
    // B holds one of the four task slots; A's two keep running.
    let mut outcomes = (0, 0);
    while let Some(outcome) = fill.join_next().await {
        match outcome.unwrap() {
            Ok(()) => outcomes.0 += 1,
            Err(QueryError::Failed(Failure::QueueExpired | Failure::Deadline)) => outcomes.1 += 1,
            Err(other) => panic!("filler {other:?}"),
        }
    }
    assert_eq!(outcomes.0 + outcomes.1, 2 + QUEUE);
    let stats = engine.stats();
    assert_eq!(stats.rejected, 1, "{stats:?}");
    assert_eq!(stats.peak_organization_running, 2);
    assert!(stats.peak_task_running <= 4);
    assert!(scratch_is_empty(&root.0));
}

#[tokio::test]
async fn worker_is_killed_at_deadline_and_reports_failed_not_zero() {
    let root = Directory::new("deadline");
    let mut settings = settings(&root.0, "/bin/sh", &["-c", "exec sleep 30"]);
    settings.backstop = Duration::from_millis(300);
    let engine = Engine::new(settings).unwrap();
    cache_bytes(&root.0, ORG, CACHED_BYTES);
    let run = fake_run(ORG);
    let started = Instant::now();
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::Failed(Failure::Deadline))
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(scratch_is_empty(&root.0), "scratch removed after the kill");

    // The child's own deadline report, an empty or extra line, a number
    // where the protocol carries strings, a missing scope or a nonzero exit
    // are failures: never rows, never zero. So are an extra or duplicated
    // member, a column outside the identifier grammar and a duplicate
    // column (review m3).
    for (script, expected) in [
        (
            answering(
                "0",
                r#"{"elapsed_ms":1800,"outcome":"failed","reason":"deadline"}"#,
            ),
            Failure::Deadline,
        ),
        (
            answering(
                "0",
                r#"{"elapsed_ms":3,"outcome":"failed","reason":"engine"}"#,
            ),
            Failure::Engine,
        ),
        ("cat >/dev/null".to_owned(), Failure::Engine),
        (
            format!("{}; echo extra", answering("0", ROWS_LINE)),
            Failure::Engine,
        ),
        (
            answering("0", &ROWS_LINE.replace(r#"[["1140000"]]"#, "[[0]]")),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                &ROWS_LINE.replace(r#""partial":false"#, r#""partial":true"#),
            ),
            Failure::Engine,
        ),
        (
            format!("{}; exit 3", answering("0", ROWS_LINE)),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                &ROWS_LINE.replace(r#""elapsed_ms":7"#, r#""elapsed_ms":7,"note":"x""#),
            ),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                r#"{"columns":["total_bytes"],"elapsed_ms":7,"outcome":"refused","outcome":"rows","partial":false,"rows":[["1140000"]],"total_scope":"full_authorized_filtered_input","truncated_page":false}"#,
            ),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                &ROWS_LINE.replace(r#"["total_bytes"]"#, r#"["Total Bytes"]"#),
            ),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                &ROWS_LINE
                    .replace(r#"["total_bytes"]"#, r#"["a","a"]"#)
                    .replace(r#"[["1140000"]]"#, r#"[["1","2"]]"#),
            ),
            Failure::Engine,
        ),
        (
            answering(
                "0",
                r#"{"elapsed_ms":2,"outcome":"refused","reason":"literal_not_qualified","rows":[]}"#,
            ),
            Failure::Engine,
        ),
    ] {
        let engine = fake(&root.0, &script, &[]);
        assert_eq!(
            engine.execute(&run, Duration::from_millis(1800)).await,
            Err(QueryError::Failed(expected)),
            "{script}"
        );
    }
    // Refusals keep their fixed code and never carry rows.
    let engine = fake(
        &root.0,
        &answering(
            "0",
            r#"{"elapsed_ms":2,"outcome":"refused","reason":"literal_not_qualified"}"#,
        ),
        &[],
    );
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::Refused("literal_not_qualified".into()))
    );
    let engine = fake(
        &root.0,
        &answering(
            "0",
            r#"{"elapsed_ms":2,"outcome":"refused","reason":"narrow_the_query"}"#,
        ),
        &[],
    );
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::NarrowTheQuery)
    );
    // A deadline already spent launches nothing: expired, not interrupted.
    let engine = fake(&root.0, &answering("0", ROWS_LINE), &[]);
    assert_eq!(
        engine.execute(&run, Duration::ZERO).await,
        Err(QueryError::Failed(Failure::QueueExpired))
    );
    assert_eq!(engine.stats().launched, 0);
}

/// APP-012 review m1 (probe P2): a valid answer that arrives after the
/// interactive deadline is withheld by the parent, whatever the child says.
#[tokio::test]
async fn a_late_answer_is_a_deadline_failure_not_rows() {
    let root = Directory::new("late");
    let engine = fake(&root.0, &answering("2.6", ROWS_LINE), &[ORG]);
    let started = Instant::now();
    assert_eq!(
        engine
            .execute(&fake_run(ORG), Duration::from_millis(1800))
            .await,
        Err(QueryError::Failed(Failure::Deadline))
    );
    assert!(started.elapsed() >= Duration::from_millis(2500));
    assert!(scratch_is_empty(&root.0));
    // An answer inside the deadline is rows.
    let engine = fake(&root.0, &answering("0.2", ROWS_LINE), &[ORG]);
    assert_eq!(
        engine
            .execute(&fake_run(ORG), Duration::from_millis(1800))
            .await
            .map(|r| r.rows),
        Ok(vec![vec![json!("1140000")]])
    );
}

#[tokio::test]
async fn engine_child_has_no_credentials_or_network_config() {
    let root = Directory::new("confinement");
    // The child reports its environment, its open descriptors and its
    // working directory; the parent holds credentials-shaped variables.
    let script = "cat >/dev/null; env; echo --; n=0; while [ $n -lt 256 ]; do [ -e /dev/fd/$n ] && echo fd$n; n=$((n+1)); done; echo --; pwd";
    let engine = fake(&root.0, script, &[ORG]);
    let scratch = root.0.join("scratch/probe");
    fs::create_dir(&scratch).unwrap();
    let output = engine.run_child(&scratch, b"{}".to_vec()).await.unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let parts: Vec<&str> = text.split("--\n").collect();
    assert_eq!(parts.len(), 3, "{text}");
    // `sh` may export PWD/SHLVL/_ itself; nothing the parent had arrives.
    for line in parts[0].lines() {
        let name = line.split('=').next().unwrap();
        assert!(
            matches!(name, "PWD" | "SHLVL" | "_" | "OLDPWD"),
            "inherited variable {name}"
        );
    }
    assert!(!parts[0].contains("AWS_") && !parts[0].contains("GENUI_") && !parts[0].contains("PG"));
    assert_eq!(parts[1].trim(), "fd0\nfd1\nfd2");
    assert_eq!(
        Path::new(parts[2].trim()).canonicalize().unwrap(),
        scratch.canonicalize().unwrap()
    );
    // The scratch lies outside the cache, by construction and by refusal.
    let mut nested = settings(&root.0, "/bin/sh", &[]);
    nested.scratch_root = root.0.join("cache/scratch");
    assert!(Engine::new(nested).is_err());
}

#[tokio::test]
async fn uncached_or_mismatched_revision_is_unavailable_and_never_executes() {
    let root = Directory::new("cache");
    let engine = Engine::new(settings(&root.0, "/bin/sh", &["-c", "exit 9"])).unwrap();
    let run = run_for(ORG, "total", &window(), NORMALIZED);
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::SourceUnavailable)
    );
    assert_eq!(engine.stats().launched, 0);
    // Review m4 (probe P5): wrong bytes already at the cache path, mode 0400,
    // are verified before any spawn, refused and removed, so the revision is
    // not unavailable forever; the engine never ran.
    let wrong = cache_bytes(&root.0, ORG, b"wrong bytes");
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::SourceUnavailable)
    );
    assert!(!wrong.exists(), "the mismatching file is evicted");
    assert_eq!(engine.stats().launched, 0);
    // A fetch child whose bytes do not match `normalized_digest` is refused
    // and its file removed; matching bytes are then served.
    let target = engine.cached_path(&run);
    let write = |bytes: &str| {
        format!(
            "cat >/dev/null; printf '{bytes}' > '{}'; chmod 0400 '{}'",
            target.display(),
            target.display()
        )
    };
    let mut fetching = settings(&root.0, "/bin/sh", &["-c", &answering("0", ROWS_LINE)]);
    fetching.fetch = Some(("/bin/sh".into(), vec!["-c".into(), write("wrong")]));
    let engine = Engine::new(fetching.clone()).unwrap();
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::SourceUnavailable)
    );
    assert!(!target.exists());
    let good = run_for(ORG, "total", &window(), &digest_of(b"right"));
    fetching.fetch = Some(("/bin/sh".into(), vec!["-c".into(), write("right")]));
    let engine = Engine::new(fetching.clone()).unwrap();
    let result = engine
        .execute(&good, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(result.rows, vec![vec![json!("1140000")]]);
    // Wrong bytes at the path of a fetchable revision are evicted and the
    // revision fetched again (served).
    cache_bytes(&root.0, ORG, b"stale");
    let result = engine
        .execute(&good, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(result.rows, vec![vec![json!("1140000")]]);
    assert_eq!(fs::read(&target).unwrap(), b"right");
}

/// APP-012 review m6: a killed fetch's `.fetch-*` temporary is removed at
/// start-up and, once stale, before the next fetch; a fresh one (another
/// fetch in flight) is kept.
#[tokio::test]
async fn stale_fetch_temporaries_are_removed() {
    let root = Directory::new("fetch-leftovers");
    let directory = root.0.join("cache/o").join(ORG);
    fs::create_dir_all(&directory).unwrap();
    for path in [
        root.0.join("cache"),
        root.0.join("cache/o"),
        directory.clone(),
    ] {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let leftover = directory.join(".fetch-abc123");
    fs::write(&leftover, b"partial").unwrap();
    let mut fetching = settings(&root.0, "/bin/sh", &["-c", &answering("0", ROWS_LINE)]);
    let target = directory.join(format!("{SOURCE}.{REVISION}.parquet"));
    fetching.fetch = Some((
        "/bin/sh".into(),
        vec![
            "-c".into(),
            format!(
                "cat >/dev/null; printf 'right' > '{}'; chmod 0400 '{}'",
                target.display(),
                target.display()
            ),
        ],
    ));
    let engine = Engine::new(fetching).unwrap();
    assert!(!leftover.exists(), "removed at start-up");
    // Before a fetch: a stale temporary goes, a fresh one stays.
    let stale = directory.join(".fetch-stale");
    fs::write(&stale, b"partial").unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(600);
    fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let fresh = directory.join(".fetch-fresh");
    fs::write(&fresh, b"partial").unwrap();
    let good = run_for(ORG, "total", &window(), &digest_of(b"right"));
    engine
        .execute(&good, Duration::from_millis(1800))
        .await
        .unwrap();
    assert!(!stale.exists() && fresh.exists());
}

/// The locked Python environment with DuckDB 2.0.0 (the operator lock, a
/// superset of the image's runtime lock), or `None` to skip.
pub(crate) fn engine_python() -> Option<PathBuf> {
    std::env::var_os("GENUI_QUERY_PYTHON").map(PathBuf::from)
}
/// The Northstar events revision exactly as APP-077 uploads it, written into
/// `<cache>/o/<organization>/` by the test module's adapter; returns its digest.
pub(crate) fn cache_northstar(
    python: &Path,
    cache: &Path,
    organization: &str,
    revision: &str,
) -> String {
    let directory = cache.join("o").join(organization);
    fs::create_dir_all(&directory).unwrap();
    for path in [cache.to_path_buf(), cache.join("o"), directory.clone()] {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let target = directory.join(format!("{SOURCE}.{revision}.parquet"));
    let runtime = repository().join("app/runtime");
    let output = std::process::Command::new(python)
        .current_dir(&runtime)
        .args([
            "-c",
            "import sys,test_query_worker as t; print(t.northstar_parquet(sys.argv[1]))",
            target.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::set_permissions(&target, fs::Permissions::from_mode(0o400)).unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
pub(crate) fn real_engine(python: &Path, root: &Path) -> Engine {
    Engine::new(EngineSettings {
        program: python.into(),
        args: vec![
            "-I".into(),
            repository()
                .join("app/runtime/runtime.py")
                .to_str()
                .unwrap()
                .into(),
            "query".into(),
        ],
        cache_root: root.join("cache"),
        scratch_root: root.join("scratch"),
        fetch: None,
        backstop: super::worker::BACKSTOP,
        rss_limit_bytes: RSS_LIMIT_BYTES,
    })
    .unwrap()
}

/// Nearest-rank percentile of sorted samples.
pub(crate) fn percentile(sorted: &[Duration], p: f64) -> Duration {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[test]
#[ignore = "requires GENUI_QUERY_PYTHON: the locked Python environment with DuckDB 2.0.0"]
fn engine_child_campaign() {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(child_campaign());
}

async fn child_campaign() {
    let python = engine_python().expect("GENUI_QUERY_PYTHON");
    let root = Directory::new("child");
    let digest = cache_northstar(&python, &root.0.join("cache"), ORG, REVISION);
    assert_eq!(
        digest, NORMALIZED,
        "the cached revision is the uploaded object"
    );
    let engine = real_engine(&python, &root.0);
    let deadline = Duration::from_millis(1800);
    let expected = read_json("fixtures/phase-one/v1/expected.json")["northstar"].clone();

    // test_query_worker_matches_app011_oracles, through the Rust parent.
    let total = engine
        .execute(&run_for(ORG, "total", &window(), NORMALIZED), deadline)
        .await
        .unwrap();
    assert_eq!(total.rows, vec![vec![expected["total_bytes"].clone()]]);
    assert_eq!(total.rows, vec![vec![json!("1140000")]]);
    assert_eq!(
        total.integer_columns.iter().collect::<Vec<_>>(),
        ["total_bytes"]
    );
    let mut filtered = window();
    filtered["destination"] = json!("198.51.100.7");
    let filtered = engine
        .execute(&run_for(ORG, "total", &filtered, NORMALIZED), deadline)
        .await
        .unwrap();
    assert_eq!(filtered.rows, vec![vec![json!("900000")]]);
    let timeline = engine
        .execute(&run_for(ORG, "timeline", &window(), NORMALIZED), deadline)
        .await
        .unwrap();
    assert_eq!(timeline.columns, ["minute", "destination", "bytes"]);
    assert_eq!(
        timeline.integer_columns.iter().collect::<Vec<_>>(),
        ["bytes"]
    );
    let mut minutes = std::collections::BTreeMap::new();
    for row in &timeline.rows {
        *minutes
            .entry(row[0].as_str().unwrap().to_owned())
            .or_insert(0u64) += row[2].as_str().unwrap().parse::<u64>().unwrap();
    }
    let oracle: std::collections::BTreeMap<String, u64> = expected["minute_bytes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, v)| *v != "0")
        .map(|(i, v)| {
            (
                format!("2026-10-22T10:{i:02}:00Z"),
                v.as_str().unwrap().parse().unwrap(),
            )
        })
        .collect();
    assert_eq!(minutes, oracle);
    // The lookup view stays refused; a deadline the engine cannot meet fails
    // (expired before launch, or interrupted by the child) and never shows rows.
    assert!(matches!(
        engine
            .execute(
                &run_for(ORG, "destinations", &json!({}), NORMALIZED),
                deadline
            )
            .await,
        Err(QueryError::Refused(_))
    ));
    assert!(matches!(
        engine
            .execute(
                &run_for(ORG, "total", &window(), NORMALIZED),
                Duration::from_millis(1)
            )
            .await,
        Err(QueryError::Failed(
            Failure::QueueExpired | Failure::Deadline
        ))
    ));
    // The other organization's cache is empty: nothing to read.
    assert_eq!(
        engine
            .execute(
                &run_for(
                    "00000000-0000-4000-8000-000000000002",
                    "total",
                    &window(),
                    NORMALIZED
                ),
                deadline
            )
            .await,
        Err(QueryError::SourceUnavailable)
    );
    assert!(scratch_is_empty(&root.0));

    // Per-request child latency (design §5.2): sequential, then waves of two.
    let measure = measure_latency(&engine, &run_for(ORG, "total", &window(), NORMALIZED), 40).await;
    println!("APP012 latency: {measure}");
    if let Some(path) = std::env::var_os("GENUI_QUERY_LATENCY_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&measure).unwrap()).unwrap();
    }
}

/// Sequential and paired (one wave of two) wall times of `samples` runs.
pub(crate) async fn measure_latency(engine: &Engine, run: &QueryRun, samples: usize) -> Value {
    let deadline = Duration::from_millis(1800);
    let mut walls = Vec::new();
    let mut engine_ms = Vec::new();
    let mut rss = 0u64;
    for _ in 0..samples {
        let result = engine.execute(run, deadline).await.unwrap();
        walls.push(result.wall);
        engine_ms.push(result.engine_elapsed_ms);
        rss = rss.max(result.peak_rss_bytes);
    }
    let mut waves = Vec::new();
    for _ in 0..(samples / 2) {
        let started = Instant::now();
        let (a, b) = tokio::join!(engine.execute(run, deadline), engine.execute(run, deadline));
        a.unwrap();
        b.unwrap();
        waves.push(started.elapsed());
    }
    walls.sort();
    waves.sort();
    engine_ms.sort();
    let ms = |d: Duration| (d.as_secs_f64() * 10_000.0).round() / 10.0;
    json!({
        "samples": samples,
        "sequential_wall_ms": {"p50": ms(percentile(&walls, 50.0)), "p95": ms(percentile(&walls, 95.0)),
            "p99": ms(percentile(&walls, 99.0)), "max": ms(*walls.last().unwrap())},
        "engine_reported_ms": {"p50": engine_ms[engine_ms.len() / 2], "max": engine_ms.last().unwrap()},
        "wave_of_two_wall_ms": {"p50": ms(percentile(&waves, 50.0)), "p99": ms(percentile(&waves, 99.0)),
            "max": ms(*waves.last().unwrap())},
        "peak_rss_bytes": rss,
    })
}
