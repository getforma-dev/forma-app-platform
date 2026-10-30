//! The per-request DuckDB engine child (ADR-007 D7.10).
//!
//! **Named deviation (D7.10):** the engine is a child process of the same UID
//! inside the service task, not APP-005's separate container. The mandatory
//! compensating controls live here and in `app/runtime/query_engine.py`:
//!
//! 1. APP-005's `POLICY` verbatim in the child (`threads=2`, `memory_limit`
//!    1 GiB, a 2 GiB temp limit, an exact one-file allowlist, then
//!    `enable_external_access=false` and `lock_configuration=true`, extension
//!    autoloading off, no httpfs, a read-only database).
//! 2. Fetch separated from execution: [`Engine`] makes sure the revision is in
//!    the read-only cache (`<cache>/o/<organization>/<source>.<revision>.parquet`)
//!    before any child starts. A missing revision is fetched by a separate,
//!    credentialed `runtime.py cache` child (the binary has no AWS SDK; see
//!    `app/README.md`), and its SHA-256 is verified against the revision's
//!    `normalized_digest` here before it is used. The engine child itself is
//!    spawned by `supervise::run_confined`: `env_clear()`, only its three
//!    pipes, `RLIMIT_NOFILE` 64 and no core, its resident set watched, and a
//!    private scratch directory outside the cache that is removed when the
//!    child exits or is killed.
//! 3. Admission: at most 2 running per organization and 4 per task, at most 20
//!    waiting; beyond that [`QueryError::Busy`] (429 with `Retry-After`).
//!    Queue time counts against the caller's deadline; a request whose
//!    deadline passes in the queue never launches a child.
//! 4. The child interrupts DuckDB at the caller's deadline (at most 1.8 s);
//!    the parent kills and reaps it at the 5 s backstop.
//!
//! A failed or killed execution is [`QueryError::Failed`]: it carries no row
//! and no number, so no caller can render a fabricated zero.
use crate::{
    admission::{self, digest},
    storage::QueryRun,
    supervise::{run_confined, Confinement, Killed},
};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    process::Command,
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{timeout_at, Instant},
};

/// APP-005's interactive deadline: the child interrupts DuckDB by then.
pub const INTERACTIVE_DEADLINE: Duration = Duration::from_millis(1800);
/// APP-005's backstop: the parent kills and reaps the child by then.
pub const BACKSTOP: Duration = Duration::from_secs(5);
pub const ORGANIZATION_RUNNING: usize = 2;
pub const TASK_RUNNING: usize = 4;
pub const QUEUE: usize = 20;
/// `Retry-After` for a 429 (seconds).
pub const RETRY_AFTER_SECONDS: u64 = 2;
/// The qualified page bound (APP-005).
pub const MAX_ROWS: usize = 500;
/// DuckDB's 1 GiB `memory_limit` plus the interpreter.
pub const RSS_LIMIT_BYTES: u64 = 1536 * 1024 * 1024;
/// A result line is at most 500 rows of 32 values of 2,000 characters.
const RESULT_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const MAX_REVISION_BYTES: u64 = 256 * 1024 * 1024;
const FETCH_DEADLINE: Duration = Duration::from_secs(10);

/// Why an execution produced no result. None of these is a number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// Interrupted at the deadline, expired in the queue, or killed at the backstop.
    Deadline,
    /// Killed at the resident-set line or an output bound, or crashed.
    Killed,
    /// The engine reported an internal error or answered outside the protocol.
    Engine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    /// The admission queue is full: 429 with `Retry-After`.
    Busy,
    /// More than one page without pagination: 422 `narrow_the_query`.
    NarrowTheQuery,
    /// The typed query is outside the qualified operator subset (a fixed code).
    Refused(String),
    /// The bound revision is not in the cache and could not be fetched and
    /// verified against its `normalized_digest`.
    SourceUnavailable,
    /// The execution failed; nothing is shown but the failed state.
    Failed(Failure),
}

/// Where the engine and the fetch run and what they may use.
#[derive(Clone, Debug)]
pub struct EngineSettings {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The read-only revision cache root; children only ever read it.
    pub cache_root: PathBuf,
    /// Private per-request scratch directories are created under it.
    pub scratch_root: PathBuf,
    /// The credentialed fetch child; `None` serves the cache only.
    pub fetch: Option<(PathBuf, Vec<String>)>,
    pub backstop: Duration,
    pub rss_limit_bytes: u64,
}
impl EngineSettings {
    /// The service image's layout (`app/Dockerfile`): `/run/genui` is the only
    /// writable volume; the cache and the scratch are separate directories in it.
    pub fn image() -> Self {
        let python = PathBuf::from("/usr/local/bin/python3");
        let mode = |name: &str| {
            vec![
                "-I".to_owned(),
                "/app/runtime.py".to_owned(),
                name.to_owned(),
            ]
        };
        Self {
            program: python.clone(),
            args: mode("query"),
            cache_root: PathBuf::from("/run/genui/cache"),
            scratch_root: PathBuf::from("/run/genui/query-scratch"),
            fetch: Some((python, mode("cache"))),
            backstop: BACKSTOP,
            rss_limit_bytes: RSS_LIMIT_BYTES,
        }
    }
}

/// One released-or-not result: rows exactly as the engine computed them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub truncated_page: bool,
    /// `forma:result:v1` over `{"columns_are_query_defined": true, "rows": rows}`.
    pub result_digest: String,
    /// The child's own measurement, request read to result written.
    pub engine_elapsed_ms: u64,
    /// Admission, cache check, spawn, execution and parsing, as the parent saw it.
    pub wall: Duration,
    pub peak_rss_bytes: u64,
}

/// Admission counters, for evidence and the bounds tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdmissionStats {
    pub peak_task_running: usize,
    pub peak_organization_running: usize,
    pub rejected: usize,
    pub launched: usize,
}

#[derive(Default)]
struct Running {
    task: usize,
    organizations: HashMap<String, usize>,
    stats: AdmissionStats,
}

pub struct Engine {
    settings: EngineSettings,
    task: Arc<Semaphore>,
    organizations: Mutex<HashMap<String, Arc<Semaphore>>>,
    waiting: AtomicUsize,
    running: Arc<Mutex<Running>>,
}

/// Holds both permits and the running count while a child may exist.
struct Slot {
    _organization: OwnedSemaphorePermit,
    _task: OwnedSemaphorePermit,
    organization: String,
    running: Arc<Mutex<Running>>,
}
impl Drop for Slot {
    fn drop(&mut self) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        running.task -= 1;
        if let Some(count) = running.organizations.get_mut(&self.organization) {
            *count -= 1;
        }
    }
}
struct Waiting<'a>(&'a AtomicUsize);
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A private scratch directory, removed with everything in it on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn new(root: &Path) -> std::io::Result<Self> {
        let mut bytes = [0u8; 16];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
            .map_err(|_| std::io::Error::other("random"))?;
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = root.join(format!("q-{name}"));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn private_directory(path: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(std::io::Error::other("directory refused"));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!(
        "sha256:{}",
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

impl Engine {
    pub fn new(settings: EngineSettings) -> std::io::Result<Self> {
        private_directory(&settings.cache_root)?;
        private_directory(&settings.scratch_root)?;
        if settings.cache_root.starts_with(&settings.scratch_root)
            || settings.scratch_root.starts_with(&settings.cache_root)
        {
            return Err(std::io::Error::other("scratch must lie outside the cache"));
        }
        Ok(Self {
            settings,
            task: Arc::new(Semaphore::new(TASK_RUNNING)),
            organizations: Mutex::new(HashMap::new()),
            waiting: AtomicUsize::new(0),
            running: Arc::new(Mutex::new(Running::default())),
        })
    }

    pub fn stats(&self) -> AdmissionStats {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).stats
    }

    /// The cached path of a run's revision (inside its organization's cache).
    pub fn cached_path(&self, run: &QueryRun) -> PathBuf {
        let source = run.source();
        self.settings
            .cache_root
            .join("o")
            .join(run.organization())
            .join(format!("{}.{}.parquet", source.source(), source.revision()))
    }

    async fn admit(&self, organization: &str, until: Instant) -> Result<Slot, QueryError> {
        if self.waiting.fetch_add(1, Ordering::SeqCst) >= QUEUE {
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            self.running
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .stats
                .rejected += 1;
            return Err(QueryError::Busy);
        }
        let waiting = Waiting(&self.waiting);
        let semaphore = self
            .organizations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(organization.to_owned())
            .or_insert_with(|| Arc::new(Semaphore::new(ORGANIZATION_RUNNING)))
            .clone();
        // The organization's own slot first, so one organization's waiters
        // never hold task slots another organization could use.
        let organization_permit = timeout_at(until, semaphore.acquire_owned())
            .await
            .map_err(|_| QueryError::Failed(Failure::Deadline))?
            .map_err(|_| QueryError::Failed(Failure::Engine))?;
        let task_permit = timeout_at(until, self.task.clone().acquire_owned())
            .await
            .map_err(|_| QueryError::Failed(Failure::Deadline))?
            .map_err(|_| QueryError::Failed(Failure::Engine))?;
        drop(waiting);
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        running.task += 1;
        let count = {
            let count = running
                .organizations
                .entry(organization.to_owned())
                .or_default();
            *count += 1;
            *count
        };
        running.stats.peak_task_running = running.stats.peak_task_running.max(running.task);
        running.stats.peak_organization_running =
            running.stats.peak_organization_running.max(count);
        Ok(Slot {
            _organization: organization_permit,
            _task: task_permit,
            organization: organization.to_owned(),
            running: self.running.clone(),
        })
    }

    /// The revision must be a regular, private file in its organization's
    /// cache; a missing one is fetched by the credentialed child and verified
    /// against `normalized_digest` before any engine reads it.
    async fn ensure_cached(&self, run: &QueryRun) -> Result<PathBuf, QueryError> {
        let path = self.cached_path(run);
        let present = |path: &Path| {
            fs::symlink_metadata(path).is_ok_and(|m| {
                m.is_file() && m.len() <= MAX_REVISION_BYTES && m.mode() & 0o077 == 0
            })
        };
        if present(&path) {
            return Ok(path);
        }
        let Some((program, args)) = &self.settings.fetch else {
            return Err(QueryError::SourceUnavailable);
        };
        let directory = path.parent().ok_or(QueryError::SourceUnavailable)?;
        private_directory(directory).map_err(|_| QueryError::SourceUnavailable)?;
        let source = run.source();
        let request = json!({
            "organization_id": run.organization(),
            "source_id": source.source(),
            "revision_id": source.revision(),
            "normalized_digest": source.normalized_digest(),
            "cache_root": self.settings.cache_root,
        });
        let mut command = Command::new(program);
        command.args(args);
        let confinement = Confinement {
            rss_limit_bytes: 512 * 1024 * 1024,
            deadline: FETCH_DEADLINE,
            nofile: 64,
            core: 0,
        };
        let bytes = serde_json::to_vec(&request).map_err(|_| QueryError::SourceUnavailable)?;
        let output = run_confined(command, Cursor::new(bytes), &confinement)
            .await
            .map_err(|_| QueryError::SourceUnavailable)?;
        if !output.status.success() || !present(&path) {
            return Err(QueryError::SourceUnavailable);
        }
        // The fetch child is not trusted with the comparison either.
        let actual = sha256_file(&path).map_err(|_| QueryError::SourceUnavailable)?;
        if actual != source.normalized_digest() {
            let _ = fs::remove_file(&path);
            return Err(QueryError::SourceUnavailable);
        }
        Ok(path)
    }

    /// Executes `run` within `deadline` (capped at [`INTERACTIVE_DEADLINE`]).
    pub async fn execute(
        &self,
        run: &QueryRun,
        deadline: Duration,
    ) -> Result<WorkerResult, QueryError> {
        let started = Instant::now();
        let until = started + deadline.min(INTERACTIVE_DEADLINE);
        let slot = self.admit(run.organization(), until).await?;
        self.ensure_cached(run).await?;
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining < Duration::from_millis(1) {
            return Err(QueryError::Failed(Failure::Deadline));
        }
        let scratch = Scratch::new(&self.settings.scratch_root)
            .map_err(|_| QueryError::Failed(Failure::Engine))?;
        let source = run.source();
        let request = json!({
            "organization_id": run.organization(),
            "source": {
                "source_id": source.source(),
                "revision_id": source.revision(),
                "normalized_digest": source.normalized_digest(),
                "s3_key": source.object_key(run.organization()),
            },
            "query": run.query(),
            "deadline_ms": u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX).min(1800),
            "cache_root": self.settings.cache_root,
            "scratch": scratch.0,
        });
        let bytes =
            serde_json::to_vec(&request).map_err(|_| QueryError::Failed(Failure::Engine))?;
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stats
            .launched += 1;
        let outcome = self.run_child(&scratch.0, bytes).await;
        drop(scratch);
        drop(slot);
        let output = outcome.map_err(|killed| {
            QueryError::Failed(match killed {
                Killed::Deadline => Failure::Deadline,
                _ => Failure::Killed,
            })
        })?;
        if !output.status.success() {
            return Err(QueryError::Failed(Failure::Engine));
        }
        let mut result = parse_result(&output.stdout)?;
        result.wall = started.elapsed();
        result.peak_rss_bytes = output.peak_rss_bytes;
        Ok(result)
    }

    /// The confined engine child with one request; its raw output.
    pub(crate) async fn run_child(
        &self,
        scratch: &Path,
        request: Vec<u8>,
    ) -> Result<crate::supervise::ConfinedOutput, Killed> {
        let mut command = Command::new(&self.settings.program);
        command.args(&self.settings.args).current_dir(scratch);
        let confinement = Confinement {
            rss_limit_bytes: self.settings.rss_limit_bytes,
            deadline: self.settings.backstop,
            nofile: 64,
            core: 0,
        };
        run_confined(command, Cursor::new(request), &confinement).await
    }
}

fn cell(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(_) => true,
        Value::String(s) => s.chars().count() <= 2000,
        _ => false,
    }
}

/// Exactly one JSON line in the engine protocol; anything else is a failure.
fn parse_result(stdout: &[u8]) -> Result<WorkerResult, QueryError> {
    let failed = QueryError::Failed(Failure::Engine);
    if stdout.len() > RESULT_LIMIT_BYTES || stdout.last() != Some(&b'\n') {
        return Err(failed);
    }
    let line = &stdout[..stdout.len() - 1];
    if line.contains(&b'\n') {
        return Err(failed);
    }
    let value: Value = serde_json::from_slice(line).map_err(|_| failed.clone())?;
    let elapsed = value["elapsed_ms"].as_u64().ok_or(failed.clone())?;
    match value["outcome"].as_str() {
        Some("rows") => {}
        Some("refused") => {
            let reason = value["reason"].as_str().ok_or(failed.clone())?;
            if !(1..=64).contains(&reason.len())
                || !reason.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            {
                return Err(failed);
            }
            return Err(match reason {
                "narrow_the_query" => QueryError::NarrowTheQuery,
                "source_unavailable" | "source_digest_mismatch" => QueryError::SourceUnavailable,
                other => QueryError::Refused(other.to_owned()),
            });
        }
        Some("failed") => {
            return Err(QueryError::Failed(
                if value["reason"].as_str() == Some("deadline") {
                    Failure::Deadline
                } else {
                    Failure::Engine
                },
            ))
        }
        _ => return Err(failed),
    }
    if value["partial"] != json!(false)
        || value["total_scope"] != json!("full_authorized_filtered_input")
    {
        return Err(failed);
    }
    let columns: Vec<String> = value["columns"]
        .as_array()
        .filter(|c| (1..=32).contains(&c.len()))
        .ok_or(failed.clone())?
        .iter()
        .map(|c| c.as_str().map(str::to_owned))
        .collect::<Option<_>>()
        .ok_or(failed.clone())?;
    let rows: Vec<Vec<Value>> = value["rows"]
        .as_array()
        .filter(|r| r.len() <= MAX_ROWS)
        .ok_or(failed.clone())?
        .iter()
        .map(|row| {
            row.as_array()
                .filter(|row| row.len() == columns.len() && row.iter().all(cell))
                .cloned()
        })
        .collect::<Option<_>>()
        .ok_or(failed.clone())?;
    let truncated_page = value["truncated_page"].as_bool().ok_or(failed.clone())?;
    Ok(WorkerResult {
        result_digest: result_digest(&rows).ok_or(failed)?,
        columns,
        rows,
        truncated_page,
        engine_elapsed_ms: elapsed,
        wall: Duration::ZERO,
        peak_rss_bytes: 0,
    })
}

/// `forma:result:v1` of APP-005's result document.
pub fn result_digest(rows: &[Vec<Value>]) -> Option<String> {
    let document = json!({"columns_are_query_defined": true, "rows": rows});
    let canonical = admission::encode(&document).ok()?;
    Some(digest::digest(digest::RESULT, &canonical))
}
