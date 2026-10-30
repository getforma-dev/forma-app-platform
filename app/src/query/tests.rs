//! APP-012 engine, admission and receipt checks. The fake engines are
//! `/bin/sh` scripts speaking the child protocol, so admission, deadlines,
//! confinement and parsing are tested without DuckDB; the real child runs in
//! `engine_child_campaign` (ignored: needs the locked Python environment with
//! DuckDB 2.0.0) and in `storage::query_qualification` (Docker).
use super::{
    receipt,
    worker::{result_digest, Engine, EngineSettings, Failure, QueryError, QUEUE, RSS_LIMIT_BYTES},
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
/// An engine whose child is `sh -c script`, with the run's revision cached.
fn fake(root: &Path, script: &str, organizations: &[&str]) -> Engine {
    let engine = Engine::new(settings(root, "/bin/sh", &["-c", script])).unwrap();
    for organization in organizations {
        let directory = root.join("cache/o").join(organization);
        fs::create_dir_all(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let file = directory.join(format!("{SOURCE}.{REVISION}.parquet"));
        fs::write(&file, b"cached").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o400)).unwrap();
    }
    engine
}
fn answering(seconds: &str, line: &str) -> String {
    format!("cat >/dev/null; sleep {seconds}; printf '%s\\n' '{line}'")
}
fn scratch_is_empty(root: &Path) -> bool {
    fs::read_dir(root.join("scratch")).unwrap().next().is_none()
}

#[test]
fn receipt_vectors_match_contracts() {
    // Every edition-1 receipt the contracts carry keeps its digest under the
    // Rust identity, and the seeded `total` view's typed query is the
    // receipt's `query_digest`.
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
    let rows = vec![vec![json!("1140000")]];
    let result = super::WorkerResult {
        columns: vec!["total_bytes".into()],
        result_digest: result_digest(&rows).unwrap(),
        rows,
        truncated_page: false,
        engine_elapsed_ms: 0,
        wall: Duration::ZERO,
        peak_rss_bytes: 0,
    };
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

#[tokio::test]
async fn admission_bounds_two_per_organization_four_per_task_then_429() {
    let root = Directory::new("admission");
    let engine = Arc::new(fake(&root.0, &answering("0.2", ROWS_LINE), &[ORG]));
    let run = Arc::new(run_for(ORG, "total", &window(), NORMALIZED));
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
            Err(QueryError::Failed(Failure::Deadline)) => expired += 1,
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
            let run = run_for(organization, "total", &window(), NORMALIZED);
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

#[tokio::test]
async fn worker_is_killed_at_deadline_and_reports_failed_not_zero() {
    let root = Directory::new("deadline");
    let mut settings = settings(&root.0, "/bin/sh", &["-c", "exec sleep 30"]);
    settings.backstop = Duration::from_millis(300);
    let engine = Engine::new(settings).unwrap();
    let cached = root.0.join("cache/o").join(ORG);
    fs::create_dir_all(&cached).unwrap();
    fs::set_permissions(&cached, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(cached.join(format!("{SOURCE}.{REVISION}.parquet")), b"x").unwrap();
    fs::set_permissions(
        cached.join(format!("{SOURCE}.{REVISION}.parquet")),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    let run = run_for(ORG, "total", &window(), NORMALIZED);
    let started = Instant::now();
    assert_eq!(
        engine.execute(&run, Duration::from_millis(1800)).await,
        Err(QueryError::Failed(Failure::Deadline))
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(scratch_is_empty(&root.0), "scratch removed after the kill");

    // The child's own deadline report, an empty or extra line, a number
    // where the protocol carries strings, a missing scope or a nonzero exit
    // are failures: never rows, never zero.
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
    // A deadline already spent launches nothing.
    let engine = fake(&root.0, &answering("0", ROWS_LINE), &[]);
    assert_eq!(
        engine.execute(&run, Duration::ZERO).await,
        Err(QueryError::Failed(Failure::Deadline))
    );
    assert_eq!(engine.stats().launched, 0);
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
    let good = run_for(
        ORG,
        "total",
        &window(),
        &format!(
            "sha256:{}",
            sha2::Sha256::digest(b"right")
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
    );
    fetching.fetch = Some(("/bin/sh".into(), vec!["-c".into(), write("right")]));
    let engine = Engine::new(fetching).unwrap();
    let result = engine
        .execute(&good, Duration::from_millis(1800))
        .await
        .unwrap();
    assert_eq!(result.rows, vec![vec![json!("1140000")]]);
    use sha2::Digest as _;
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
    // The lookup view stays refused; a deadline the engine cannot meet fails.
    assert!(matches!(
        engine
            .execute(
                &run_for(ORG, "destinations", &json!({}), NORMALIZED),
                deadline
            )
            .await,
        Err(QueryError::Refused(_))
    ));
    assert_eq!(
        engine
            .execute(
                &run_for(ORG, "total", &window(), NORMALIZED),
                Duration::from_millis(1)
            )
            .await,
        Err(QueryError::Failed(Failure::Deadline))
    );
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
