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
//!    before any child starts, and verifies the cached file's SHA-256 against
//!    the revision's `normalized_digest` before every spawn (a mismatching
//!    file is removed and fetched again; APP-012 review m4). A missing
//!    revision is fetched by a separate, credentialed `runtime.py cache` child
//!    (the binary has no AWS SDK; see `app/README.md`), bounded by the
//!    request's remaining budget, and its SHA-256 is verified here before it
//!    is used. The engine child itself is spawned by `supervise::run_confined`:
//!    `env_clear()`, only its three pipes, `RLIMIT_NOFILE` 64 and no core, its
//!    resident set watched, and a private scratch directory outside the cache
//!    that is removed when the child exits or is killed.
//! 3. Admission: at most 2 running per organization and 4 per task, and at
//!    most 20 waiting **per organization** (D7.10(3), design §6.3: "429 past
//!    20 an organization"); beyond that [`QueryError::Busy`] (429 with
//!    `Retry-After`). One organization's full queue never refuses another's
//!    request (APP-012 review M1). Queue time counts against the caller's
//!    deadline; a request whose deadline passes in the queue never launches a
//!    child and is [`Failure::QueueExpired`] (429 as well: nothing ran).
//! 4. The child interrupts DuckDB at the caller's deadline (at most 1.8 s,
//!    less a start-up allowance); the parent also refuses a result that
//!    arrives after the deadline (plus a 250 ms grace; review m1) and kills
//!    and reaps the child at the 5 s backstop.
//!
//! The result line is parsed strictly (review m3): exactly one forma-json-v1
//! line of at most 1 MiB with the exact member set of its outcome, unique
//! identifier column names, and no number where the protocol carries
//! strings. A failed or killed execution is [`QueryError::Failed`]: it carries
//! no row and no number, so no caller can render a fabricated zero.
use crate::{
    admission::{self, digest},
    storage::QueryRun,
    supervise::{run_confined, Confinement, Killed},
};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::{Cursor, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
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
/// Waiting requests admitted per organization (D7.10(3)).
pub const QUEUE: usize = 20;
/// `Retry-After` for a 429 (seconds).
pub const RETRY_AFTER_SECONDS: u64 = 2;
/// The qualified page bound (APP-005).
pub const MAX_ROWS: usize = 500;
/// DuckDB's 1 GiB `memory_limit` plus the interpreter.
pub const RSS_LIMIT_BYTES: u64 = 1536 * 1024 * 1024;
/// The result line is a forma-json-v1 document: the decoder's 1 MiB bound
/// (a Phase 1 line is a few hundred bytes; review m3).
const RESULT_LIMIT_BYTES: usize = admission::canonical::MAX_DOCUMENT_BYTES;
const MAX_REVISION_BYTES: u64 = 256 * 1024 * 1024;
/// The fetch child's own ceiling; the request's remaining budget bounds it
/// further (review m6).
const FETCH_DEADLINE: Duration = Duration::from_secs(10);
/// A `.fetch-*` temporary older than this is a killed fetch's leftover.
const STALE_FETCH: Duration = Duration::from_secs(60);
/// Interpreter start-up the child cannot count against its own deadline.
const STARTUP_ALLOWANCE: Duration = Duration::from_millis(100);
/// How late a child's answer may arrive after the deadline and still count.
const LATE_GRACE: Duration = Duration::from_millis(250);

/// Why an execution produced no result. None of these is a number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The deadline passed before any child ran (in the admission queue or
    /// before launch): nothing executed; the caller may retry (429).
    QueueExpired,
    /// Interrupted at the deadline, late past it, or killed at the backstop:
    /// the query is past its budget (422 `narrow_the_query`).
    Deadline,
    /// Killed at the resident-set line or an output bound, or crashed
    /// (422 `narrow_the_query`).
    Killed,
    /// The engine reported an internal error or answered outside the protocol
    /// (503 `query_failed`).
    Engine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryError {
    /// The organization's admission queue is full: 429 with `Retry-After`.
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
    /// The columns whose values are exact integer strings (an `integer`, a
    /// `money` column's minor units or a `decimal` column's coefficient),
    /// derived from the typed query the engine ran (`integer_columns`).
    /// Kept for existing consumers; [`WorkerResult::column_types`] says how
    /// each one is displayed (review m11, APP-093 review M4).
    pub integer_columns: BTreeSet<String>,
    /// Every result column's value type, from the re-admitted definition's
    /// input fields followed through the query's operators
    /// ([`typed_column_types`]): what the page and the canvas format by.
    pub column_types: BTreeMap<String, ColumnType>,
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

/// One organization's admission: its running permits and its waiting count.
struct Organization {
    semaphore: Arc<Semaphore>,
    waiting: usize,
}
type Organizations = Arc<Mutex<HashMap<String, Organization>>>;

pub struct Engine {
    settings: EngineSettings,
    task: Arc<Semaphore>,
    organizations: Organizations,
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
/// One waiter of an organization's queue; leaving the queue (admitted,
/// expired or cancelled) frees its place.
struct Waiting {
    organizations: Organizations,
    organization: String,
}
impl Drop for Waiting {
    fn drop(&mut self) {
        let mut organizations = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = organizations.get_mut(&self.organization) {
            entry.waiting -= 1;
        }
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

/// Removes a killed fetch's `.fetch-*` temporaries in one organization's
/// cache directory: every one at start-up, only those older than
/// [`STALE_FETCH`] before a fetch (a concurrent fetch of another revision
/// keeps its own file; review m6).
fn remove_stale_fetches(directory: &Path, older_than: Option<Duration>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with(".fetch-") {
            continue;
        }
        let stale = match older_than {
            None => true,
            Some(age) => entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|elapsed| elapsed >= age),
        };
        if stale {
            let _ = fs::remove_file(entry.path());
        }
    }
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
        // No fetch is in flight at start-up: every leftover temporary goes.
        if let Ok(organizations) = fs::read_dir(settings.cache_root.join("o")) {
            for entry in organizations.flatten() {
                remove_stale_fetches(&entry.path(), None);
            }
        }
        Ok(Self {
            settings,
            task: Arc::new(Semaphore::new(TASK_RUNNING)),
            organizations: Arc::new(Mutex::new(HashMap::new())),
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

    /// Admission for one organization: a place in its queue (at most
    /// [`QUEUE`] waiting; the 21st is refused at once with `Busy`, and no
    /// other organization's queue counts), then its running permit, then a
    /// task permit, each bounded by the caller's deadline.
    async fn admit(&self, organization: &str, until: Instant) -> Result<Slot, QueryError> {
        let semaphore = {
            let mut organizations = self.organizations.lock().unwrap_or_else(|e| e.into_inner());
            let entry = organizations
                .entry(organization.to_owned())
                .or_insert_with(|| Organization {
                    semaphore: Arc::new(Semaphore::new(ORGANIZATION_RUNNING)),
                    waiting: 0,
                });
            if entry.waiting >= QUEUE {
                drop(organizations);
                self.running
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .stats
                    .rejected += 1;
                return Err(QueryError::Busy);
            }
            entry.waiting += 1;
            entry.semaphore.clone()
        };
        let waiting = Waiting {
            organizations: self.organizations.clone(),
            organization: organization.to_owned(),
        };
        // The organization's own slot first, so one organization's waiters
        // never hold task slots another organization could use.
        let organization_permit = timeout_at(until, semaphore.acquire_owned())
            .await
            .map_err(|_| QueryError::Failed(Failure::QueueExpired))?
            .map_err(|_| QueryError::Failed(Failure::Engine))?;
        let task_permit = timeout_at(until, self.task.clone().acquire_owned())
            .await
            .map_err(|_| QueryError::Failed(Failure::QueueExpired))?
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
    /// cache whose SHA-256 is the revision's `normalized_digest` (verified
    /// here before every spawn; a mismatching file is removed, review m4). A
    /// missing one is fetched by the credentialed child within the request's
    /// remaining budget and verified again before any engine reads it.
    async fn ensure_cached(&self, run: &QueryRun, until: Instant) -> Result<PathBuf, QueryError> {
        let path = self.cached_path(run);
        let source = run.source();
        let present = |path: &Path| {
            fs::symlink_metadata(path).is_ok_and(|m| {
                m.is_file() && m.len() <= MAX_REVISION_BYTES && m.mode() & 0o077 == 0
            })
        };
        let verified = |path: &Path| -> bool {
            present(path)
                && sha256_file(path).is_ok_and(|actual| actual == source.normalized_digest())
        };
        if present(&path) {
            if verified(&path) {
                return Ok(path);
            }
            // The bytes are not the bound revision: never served, never kept.
            let _ = fs::remove_file(&path);
        }
        let Some((program, args)) = &self.settings.fetch else {
            return Err(QueryError::SourceUnavailable);
        };
        let directory = path.parent().ok_or(QueryError::SourceUnavailable)?;
        private_directory(directory).map_err(|_| QueryError::SourceUnavailable)?;
        remove_stale_fetches(directory, Some(STALE_FETCH));
        let request = json!({
            "organization_id": run.organization(),
            "source_id": source.source(),
            "revision_id": source.revision(),
            "normalized_digest": source.normalized_digest(),
            "cache_root": self.settings.cache_root,
        });
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining < Duration::from_millis(1) {
            return Err(QueryError::Failed(Failure::QueueExpired));
        }
        let mut command = Command::new(program);
        command.args(args);
        let confinement = Confinement {
            rss_limit_bytes: 512 * 1024 * 1024,
            deadline: FETCH_DEADLINE.min(remaining),
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
        if !verified(&path) {
            let _ = fs::remove_file(&path);
            return Err(QueryError::SourceUnavailable);
        }
        Ok(path)
    }

    /// Executes `run` within `deadline` (capped at [`INTERACTIVE_DEADLINE`]).
    /// Emits one `query.dispatch` log line per child launched, or one
    /// `query.refused_before_dispatch` line when no child runs (APP-076 G6);
    /// a launched child that yields no result adds one
    /// `query.failed_after_dispatch` line with its code (B1 review m4).
    pub async fn execute(
        &self,
        run: &QueryRun,
        deadline: Duration,
    ) -> Result<WorkerResult, QueryError> {
        let started = Instant::now();
        let until = started + deadline.min(INTERACTIVE_DEADLINE);
        let refused = |error: QueryError| {
            log_refused_before_dispatch(run, refusal_code(&error), started.elapsed());
            error
        };
        let slot = self
            .admit(run.organization(), until)
            .await
            .map_err(refused)?;
        self.ensure_cached(run, until).await.map_err(refused)?;
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining < Duration::from_millis(1) {
            return Err(refused(QueryError::Failed(Failure::QueueExpired)));
        }
        let scratch = Scratch::new(&self.settings.scratch_root)
            .map_err(|_| refused(QueryError::Failed(Failure::Engine)))?;
        let source = run.source();
        // The child measures its deadline from its own `main()`, after
        // interpreter start-up, so it is given the remaining budget less an
        // allowance for that start-up (at least 1 ms; review m1).
        let child_deadline = remaining
            .saturating_sub(STARTUP_ALLOWANCE)
            .max(Duration::from_millis(1));
        let request = json!({
            "organization_id": run.organization(),
            "source": {
                "source_id": source.source(),
                "revision_id": source.revision(),
                "normalized_digest": source.normalized_digest(),
                "s3_key": source.object_key(run.organization()),
                // ADR-011 library: the input and its contract fields, from the
                // re-admitted definition; the child types columns from them.
                "input_id": source.input(),
                "fields": run.fields(),
            },
            "query": run.query(),
            "deadline_ms": u64::try_from(child_deadline.as_millis()).unwrap_or(u64::MAX).min(1800),
            "cache_root": self.settings.cache_root,
            "scratch": scratch.0,
        });
        let bytes = serde_json::to_vec(&request)
            .map_err(|_| refused(QueryError::Failed(Failure::Engine)))?;
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stats
            .launched += 1;
        tracing::info!(
            event = "query.dispatch",
            organization_id = run.organization(),
            app_id = run.app(),
            work_id = run.work(),
            view_id = run.view(),
            duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "engine child dispatched"
        );
        let finished = async {
            let outcome = self.run_child(&scratch.0, bytes).await;
            drop(scratch);
            drop(slot);
            let output = outcome.map_err(|killed| {
                QueryError::Failed(match killed {
                    Killed::Deadline => Failure::Deadline,
                    _ => Failure::Killed,
                })
            })?;
            // The parent enforces the interactive deadline too: an answer that
            // arrives late is withheld, whatever the child reported (review m1).
            if Instant::now() > until + LATE_GRACE {
                return Err(QueryError::Failed(Failure::Deadline));
            }
            if !output.status.success() {
                return Err(QueryError::Failed(Failure::Engine));
            }
            let mut result = parse_result(&output.stdout)?;
            result.column_types = typed_column_types(run.query(), run.fields());
            result.integer_columns = integer_backed(&result.column_types);
            result.wall = started.elapsed();
            result.peak_rss_bytes = output.peak_rss_bytes;
            Ok(result)
        }
        .await;
        if let Err(error) = &finished {
            log_failed_after_dispatch(run, refusal_code(error), started.elapsed());
        }
        finished
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

/// The stable code of an engine refusal, for the dispatch log.
pub fn refusal_code(error: &QueryError) -> &'static str {
    match error {
        QueryError::Busy => "busy",
        QueryError::NarrowTheQuery => "narrow_the_query",
        QueryError::Refused(_) => "query_refused",
        QueryError::SourceUnavailable => "source_unavailable",
        QueryError::Failed(Failure::QueueExpired) => "queue_expired",
        QueryError::Failed(Failure::Deadline) => "deadline",
        QueryError::Failed(Failure::Killed) => "killed",
        QueryError::Failed(Failure::Engine) => "engine",
    }
}

/// B1 review m4: the terminal line of a dispatched run that produced no
/// result (deadline, killed, engine failure, protocol refusal), so every
/// `query.dispatch` line reconciles with exactly one terminal line.
pub fn log_failed_after_dispatch(run: &QueryRun, code: &str, elapsed: Duration) {
    tracing::info!(
        event = "query.failed_after_dispatch",
        code,
        organization_id = run.organization(),
        app_id = run.app(),
        work_id = run.work(),
        view_id = run.view(),
        duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        "query failed after dispatch"
    );
}

/// APP-076 G6: one structured line when an authorized run is refused before
/// any engine child runs (its identities and the refusal code; never a
/// parameter value or a secret).
pub fn log_refused_before_dispatch(run: &QueryRun, code: &str, elapsed: Duration) {
    tracing::info!(
        event = "query.refused_before_dispatch",
        code,
        organization_id = run.organization(),
        app_id = run.app(),
        work_id = run.work(),
        view_id = run.view(),
        duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        "query refused before dispatch"
    );
}

fn cell(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(_) => true,
        Value::String(s) => s.chars().count() <= 2000,
        _ => false,
    }
}

/// A column name: a local identifier `[a-z][a-z0-9_]{0,63}`.
fn column_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

/// The exact member set of one protocol outcome.
fn exact_keys(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

/// Exactly one JSON line in the engine protocol; anything else is a failure.
/// The line is decoded as forma-json-v1 (duplicate keys, floats and anything
/// over 1 MiB refused) and must carry exactly the members of its outcome.
fn parse_result(stdout: &[u8]) -> Result<WorkerResult, QueryError> {
    let failed = QueryError::Failed(Failure::Engine);
    if stdout.len() > RESULT_LIMIT_BYTES || stdout.last() != Some(&b'\n') {
        return Err(failed);
    }
    let line = &stdout[..stdout.len() - 1];
    if line.contains(&b'\n') {
        return Err(failed);
    }
    let value: Value = admission::decode(line).map_err(|_| failed.clone())?;
    let elapsed = value["elapsed_ms"].as_u64().ok_or(failed.clone())?;
    match value["outcome"].as_str() {
        Some("rows") => {
            if !exact_keys(
                &value,
                &[
                    "columns",
                    "elapsed_ms",
                    "outcome",
                    "partial",
                    "rows",
                    "total_scope",
                    "truncated_page",
                ],
            ) {
                return Err(failed);
            }
        }
        Some(outcome @ ("refused" | "failed")) => {
            if !exact_keys(&value, &["elapsed_ms", "outcome", "reason"]) {
                return Err(failed);
            }
            let reason = value["reason"].as_str().ok_or(failed.clone())?;
            if !(1..=64).contains(&reason.len())
                || !reason.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            {
                return Err(failed);
            }
            if outcome == "failed" {
                return Err(QueryError::Failed(if reason == "deadline" {
                    Failure::Deadline
                } else {
                    Failure::Engine
                }));
            }
            return Err(match reason {
                "narrow_the_query" => QueryError::NarrowTheQuery,
                "source_unavailable" | "source_digest_mismatch" => QueryError::SourceUnavailable,
                other => QueryError::Refused(other.to_owned()),
            });
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
        .map(|c| {
            c.as_str()
                .filter(|name| column_name(name))
                .map(str::to_owned)
        })
        .collect::<Option<_>>()
        .ok_or(failed.clone())?;
    if columns.iter().collect::<BTreeSet<_>>().len() != columns.len() {
        return Err(failed);
    }
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
        integer_columns: BTreeSet::new(),
        column_types: BTreeMap::new(),
        rows,
        truncated_page,
        engine_elapsed_ms: elapsed,
        wall: Duration::ZERO,
        peak_rss_bytes: 0,
    })
}

/// A result column's value type (APP-093 review M4): the contract value
/// type of the input field it comes from, followed through the operators, or
/// the type an aggregate produces. The engine's values stay exact strings;
/// this only says how they are displayed: a `money` column holds minor units
/// (`26262004` with exponent 2 is $262,620.04), a `decimal` column its
/// coefficient at `scale`, an `integer` column an exact integer; `text`
/// (and `enum`), `date`, `timestamp` and `boolean` are shown verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColumnType {
    Integer,
    Money {
        currency: String,
        minor_unit_exponent: u32,
    },
    Decimal {
        scale: u32,
    },
    Text,
    Date,
    Timestamp,
    Boolean,
}

/// The largest exponent or scale a column type may carry (the engine's
/// columns are signed 64-bit integers: at most 19 digits).
const MAX_SCALE: u64 = 18;

impl ColumnType {
    /// The column type of a contract `value_type`; `None` for a type no
    /// result column has (`reference`) or an out-of-range exponent or scale.
    pub fn from_value_type(value_type: &Value) -> Option<Self> {
        let bounded = |key: &str| {
            value_type
                .get(key)
                .and_then(Value::as_u64)
                .filter(|n| *n <= MAX_SCALE)
                .and_then(|n| u32::try_from(n).ok())
        };
        Some(match value_type.get("type").and_then(Value::as_str)? {
            "integer" => Self::Integer,
            "money" => Self::Money {
                currency: value_type
                    .get("currency")
                    .and_then(Value::as_str)
                    .filter(|c| c.len() == 3 && c.bytes().all(|b| b.is_ascii_uppercase()))?
                    .to_owned(),
                minor_unit_exponent: bounded("minor_unit_exponent")?,
            },
            "decimal" => Self::Decimal {
                scale: bounded("scale")?,
            },
            "text" | "enum" => Self::Text,
            "date" => Self::Date,
            "timestamp" => Self::Timestamp,
            "boolean" => Self::Boolean,
            _ => return None,
        })
    }
    /// Whether the column's values are exact integer strings (the
    /// `integer_columns` of earlier consumers).
    pub fn integer_backed(&self) -> bool {
        matches!(
            self,
            Self::Integer | Self::Money { .. } | Self::Decimal { .. }
        )
    }
    /// The data contract of the query route and the canvas group JSON:
    /// `{"type": "money", "currency": "USD", "minor_unit_exponent": 2}`,
    /// `{"type": "decimal", "scale": 2}`, or `{"type": <name>}`.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Integer => json!({"type": "integer"}),
            Self::Money {
                currency,
                minor_unit_exponent,
            } => {
                json!({"type": "money", "currency": currency, "minor_unit_exponent": minor_unit_exponent})
            }
            Self::Decimal { scale } => json!({"type": "decimal", "scale": scale}),
            Self::Text => json!({"type": "text"}),
            Self::Date => json!({"type": "date"}),
            Self::Timestamp => json!({"type": "timestamp"}),
            Self::Boolean => json!({"type": "boolean"}),
        }
    }
}

/// `column_types` as the query route returns it: one member per typed
/// result column, `{column: ColumnType::to_json}`.
pub fn column_types_json(types: &BTreeMap<String, ColumnType>) -> Value {
    Value::Object(
        types
            .iter()
            .map(|(name, kind)| (name.clone(), kind.to_json()))
            .collect(),
    )
}

/// The integer-backed columns of `types` (`integer_columns`).
pub fn integer_backed(types: &BTreeMap<String, ColumnType>) -> BTreeSet<String> {
    types
        .iter()
        .filter(|(_, kind)| kind.integer_backed())
        .map(|(name, _)| name.clone())
        .collect()
}

/// The columns of an effective typed query whose values are exact integers,
/// derived from the contract `events_v1` field types (`time` timestamp,
/// `destination` text, `bytes` integer) and the operators the engine
/// compiles: `count` and `sum` measures are integers, `min`/`max` keep their
/// field's type, a `time_bucket` output is a timestamp, `project` keeps the
/// named fields and `filter`/`sort`/`limit` change nothing (review m11).
pub fn integer_columns(query: &Value) -> BTreeSet<String> {
    typed_integer_columns(query, &crate::storage::events_fields())
}

/// [`integer_columns`] for any input contract (ADR-011 library): the
/// integer-backed columns (`integer`, `money` minor units, `decimal`
/// coefficient) of [`typed_column_types`].
pub fn typed_integer_columns(query: &Value, fields: &[Value]) -> BTreeSet<String> {
    integer_backed(&typed_column_types(query, fields))
}

/// Every result column's [`ColumnType`] for a typed query over an input
/// whose contract `fields` are given (from the re-admitted definition, never
/// the client): the fields' own types, followed through `project` (kept
/// fields), `aggregate` (group fields keep their type; `count` is an
/// integer, `sum` keeps its numeric field's type, so a sum of money is money
/// in the same currency and exponent, `min`/`max` keep their field's type),
/// `time_bucket` (its output is a timestamp) and `filter`/`sort`/`limit`
/// (nothing changes). A column whose type cannot be derived is absent, and
/// is shown verbatim.
pub fn typed_column_types(query: &Value, fields: &[Value]) -> BTreeMap<String, ColumnType> {
    let mut types: BTreeMap<String, ColumnType> = BTreeMap::new();
    let mut present: BTreeSet<String> = BTreeSet::new();
    for item in fields {
        let Some(name) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        present.insert(name.to_owned());
        if let Some(kind) = item.get("value_type").and_then(ColumnType::from_value_type) {
            types.insert(name.to_owned(), kind);
        }
    }
    let field = |reference: &Value| {
        reference
            .get("field_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    for operation in query
        .get("operations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match operation.get("type").and_then(Value::as_str) {
            Some("project") => {
                let kept: BTreeSet<String> = operation
                    .get("fields")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(field)
                    .collect();
                present.retain(|name| kept.contains(name));
                types.retain(|name, _| kept.contains(name));
            }
            Some("aggregate") => {
                let groups: Vec<String> = operation
                    .get("group_by")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(field)
                    .collect();
                let mut next_types = BTreeMap::new();
                let mut next_present: BTreeSet<String> = groups.iter().cloned().collect();
                for name in &groups {
                    if let Some(kind) = types.get(name) {
                        next_types.insert(name.clone(), kind.clone());
                    }
                }
                for measure in operation
                    .get("measures")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(id) = measure.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    next_present.insert(id.to_owned());
                    let source = measure
                        .get("field")
                        .and_then(field)
                        .and_then(|name| types.get(&name));
                    let kind = match measure.get("operation").and_then(Value::as_str) {
                        Some("count") => Some(ColumnType::Integer),
                        // The engine sums only numeric fields; the sum keeps
                        // the field's unit, currency, exponent or scale.
                        Some("sum") => source.filter(|kind| kind.integer_backed()).cloned(),
                        Some("min" | "max") => source.cloned(),
                        _ => None,
                    };
                    match kind {
                        Some(kind) => {
                            next_types.insert(id.to_owned(), kind);
                        }
                        None => {
                            next_types.remove(id);
                        }
                    }
                }
                types = next_types;
                present = next_present;
            }
            Some("time_bucket") => {
                if let (Some(source), Some(output)) = (
                    operation.get("field").and_then(field),
                    operation.get("output_field").and_then(Value::as_str),
                ) {
                    present.remove(&source);
                    types.remove(&source);
                    present.insert(output.to_owned());
                    types.insert(output.to_owned(), ColumnType::Timestamp);
                }
            }
            _ => {}
        }
    }
    types.retain(|name, _| present.contains(name));
    types
}

/// `forma:result:v1` of APP-005's result document.
pub fn result_digest(rows: &[Vec<Value>]) -> Option<String> {
    let document = json!({"columns_are_query_defined": true, "rows": rows});
    let canonical = admission::encode(&document).ok()?;
    Some(digest::digest(digest::RESULT, &canonical))
}
