//! Isolated, explicitly armed test executable only. Never a service entry.
use super::{pool, qualification_cases as cases, valid_id, Settings, Stores, TrustedRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path},
    sync::{mpsc, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{timeout_at, Instant};

const ENTRY: &str = "storage::managed_qualification::managed_readonly_entry";
const SCOPE: &str = "APP-008 isolated application-data-read-only qualification";
const ARM: &str = "GENUI_APP008_MANAGED_QUALIFICATION";
const HOST: &str = "genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com";
const RESOURCE_ID: &str = "db-KW4LRMTPI5YXHZXF4LELQZKTAU";
const MANIFEST: &str = "/run/genui/app008-managed-run.json";
const CREDENTIALS: &str = "/run/genui/credentials.json";
const REGISTRY: &str = "/run/genui/app008-managed-registry.json";
const CA: &str = "/etc/ssl/certs/rds-global-bundle.pem";
const FIXTURE_SHA: &str = "29221b7a63b8f19786fbac0a2443543d9245186333be928ba4917b70bc387459";
const QUERY: &[u8] = include_bytes!("../../migrations/catalog-query.json");
const CATALOG: &[u8] = include_bytes!("../../migrations/catalog-v2.json");
const MIGRATIONS: &[u8] = include_bytes!("../../migrations/manifest.json");
const RESERVE: Duration = Duration::from_secs(4);

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    source_bundle_sha256: String,
    fixture_setup_sha256: String,
    catalog_query_sha256: String,
    catalog_v2_sha256: String,
    migrations_manifest_sha256: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    scope: String,
    armed: bool,
    run_id: String,
    issued_at: u64,
    expires_at: u64,
    database_host: String,
    database_port: u16,
    database_name: String,
    declared_rds_resource_id: String,
    organizations: [String; 2],
    registry_sha256: String,
    ca_sha256: String,
    pins: Pins,
}

struct Validated {
    manifest: Manifest,
    manifest_sha256: String,
    deadline: Instant,
}

#[derive(Clone, Copy)]
enum Failure {
    Configuration,
    Case,
    Deadline,
    Cleanup,
}
impl Failure {
    fn code(self) -> &'static str {
        match self {
            Self::Configuration => "configuration_refused",
            Self::Case => "case_refused",
            Self::Deadline => "original_deadline_reached",
            Self::Cleanup => "cleanup_unconfirmed",
        }
    }
}
type Result<T> = std::result::Result<T, Failure>;

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Failure::Configuration)
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn source_bundle_sha256() -> String {
    // Compiled storage/entry inputs, not a claim about the complete image or
    // an externally supplied Git revision. Name + NUL + file SHA + newline.
    let files: [(&str, &[u8]); 11] = [
        ("app/Cargo.lock", include_bytes!("../../Cargo.lock")),
        ("app/Cargo.toml", include_bytes!("../../Cargo.toml")),
        ("app/src/config.rs", include_bytes!("../config.rs")),
        ("app/src/lib.rs", include_bytes!("../lib.rs")),
        (
            "app/src/storage/authorization.rs",
            include_bytes!("authorization.rs"),
        ),
        ("app/src/storage/catalog.rs", include_bytes!("catalog.rs")),
        (
            "app/src/storage/managed_qualification.rs",
            include_bytes!("managed_qualification.rs"),
        ),
        ("app/src/storage/mod.rs", include_bytes!("mod.rs")),
        ("app/src/storage/pool.rs", include_bytes!("pool.rs")),
        (
            "app/src/storage/qualification_cases.rs",
            include_bytes!("qualification_cases.rs"),
        ),
        ("app/src/storage/registry.rs", include_bytes!("registry.rs")),
    ];
    let mut binding = Vec::new();
    for (name, raw) in files {
        binding.extend_from_slice(name.as_bytes());
        binding.push(0);
        binding.extend_from_slice(digest(raw).as_bytes());
        binding.push(b'\n');
    }
    digest(&binding)
}
fn expected_pins() -> Pins {
    Pins {
        source_bundle_sha256: source_bundle_sha256(),
        fixture_setup_sha256: FIXTURE_SHA.into(),
        catalog_query_sha256: digest(QUERY),
        catalog_v2_sha256: digest(CATALOG),
        migrations_manifest_sha256: digest(MIGRATIONS),
    }
}
fn registry_bytes(run: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":1,"registry_version":run,"bindings":[
        {"issuer":"app008-fixture-only","provider_tenant_id":format!("ten_{}",cases::ORG),"organization_id":cases::ORG},
        {"issuer":"app008-fixture-only","provider_tenant_id":format!("ten_{}",cases::OTHER),"organization_id":cases::OTHER}
    ]})).expect("fixed registry serialization")
}

fn bytes(path: &Path, limit: u64, private: bool) -> Result<Vec<u8>> {
    require(
        path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
    )?;
    for parent in path.ancestors().skip(1) {
        require(
            parent
                .symlink_metadata()
                .map_err(|_| Failure::Configuration)?
                .is_dir(),
        )?;
    }
    let uid = unsafe { libc::geteuid() };
    let parent = path
        .parent()
        .ok_or(Failure::Configuration)?
        .symlink_metadata()
        .map_err(|_| Failure::Configuration)?;
    if private {
        require(parent.uid() == uid && parent.mode() & 0o777 == 0o700)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| Failure::Configuration)?;
    let metadata = file.metadata().map_err(|_| Failure::Configuration)?;
    require(metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit)?;
    if private {
        require(metadata.uid() == uid && metadata.mode() & 0o777 == 0o600)?;
    } else {
        require(metadata.mode() & 0o022 == 0)?;
    }
    let mut raw = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut raw)
        .map_err(|_| Failure::Configuration)?;
    require(raw.len() as u64 <= limit)?;
    Ok(raw)
}

fn parse(raw: &[u8], now: SystemTime) -> Result<Validated> {
    // Pair the monotonic origin with the caller's fresh wall-clock sample
    // before parsing/hashing, so validation time cannot renew the deadline.
    let monotonic_sample = Instant::now();
    require(!raw.is_empty() && raw.len() <= 8192)?;
    let manifest: Manifest = serde_json::from_slice(raw).map_err(|_| Failure::Configuration)?;
    require(manifest.schema_version == 1 && manifest.scope == SCOPE && manifest.armed)?;
    require(valid_id(&manifest.run_id) && manifest.run_id.as_bytes()[14] == b'4')?;
    let now_seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::Configuration)?
        .as_secs();
    require(
        manifest.issued_at <= now_seconds
            && manifest.expires_at > manifest.issued_at
            && manifest.expires_at - manifest.issued_at <= 600,
    )?;
    let expiry = UNIX_EPOCH
        .checked_add(Duration::from_secs(manifest.expires_at))
        .ok_or(Failure::Configuration)?;
    let remaining = expiry
        .duration_since(now)
        .map_err(|_| Failure::Configuration)?;
    require(remaining > RESERVE && remaining <= Duration::from_secs(600))?;
    require(
        manifest.database_host == HOST
            && manifest.database_port == 5432
            && manifest.database_name == "genui"
            && manifest.declared_rds_resource_id == RESOURCE_ID
            && manifest.organizations == [cases::ORG, cases::OTHER],
    )?;
    let pins = expected_pins();
    require(
        manifest.pins.source_bundle_sha256 == pins.source_bundle_sha256
            && manifest.pins.fixture_setup_sha256 == pins.fixture_setup_sha256
            && manifest.pins.catalog_query_sha256 == pins.catalog_query_sha256
            && manifest.pins.catalog_v2_sha256 == pins.catalog_v2_sha256
            && manifest.pins.migrations_manifest_sha256 == pins.migrations_manifest_sha256,
    )?;
    require(manifest.registry_sha256 == digest(&registry_bytes(&manifest.run_id)))?;
    require(
        manifest.ca_sha256.len() == 64
            && manifest
                .ca_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
    )?;
    Ok(Validated {
        manifest,
        manifest_sha256: digest(raw),
        deadline: monotonic_sample + remaining,
    })
}

fn load() -> Result<Validated> {
    require(unsafe { libc::geteuid() } == 65532)?;
    require(std::env::var(ARM).ok().as_deref() == Some("APP-008"))?;
    let raw = bytes(Path::new(MANIFEST), 8192, true)?;
    let validated = parse(&raw, SystemTime::now())?;
    validate_files(&validated)?;
    Ok(validated)
}
fn validate_files(validated: &Validated) -> Result<()> {
    require(digest(&bytes(Path::new(MANIFEST), 8192, true)?) == validated.manifest_sha256)?;
    require(
        bytes(Path::new(REGISTRY), 16384, true)? == registry_bytes(&validated.manifest.run_id),
    )?;
    require(digest(&bytes(Path::new(CA), 1024 * 1024, false)?) == validated.manifest.ca_sha256)?;
    bytes(Path::new(CREDENTIALS), 65536, true)?;
    let config =
        crate::config::Config::load(Path::new(CREDENTIALS)).map_err(|_| Failure::Configuration)?;
    require(config.database_host == HOST && config.database_name == "genui")?;
    require(
        SystemTime::now() < UNIX_EPOCH + Duration::from_secs(validated.manifest.expires_at)
            && Instant::now() < validated.deadline,
    )?;
    Ok(())
}

struct Watchdog(
    Option<mpsc::SyncSender<()>>,
    Option<std::thread::JoinHandle<()>>,
);
impl Watchdog {
    fn start(deadline: Instant) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let thread = std::thread::Builder::new()
            .name("app008-original-deadline".into())
            .spawn(move || {
                if receiver.recv_timeout(remaining).is_err() {
                    std::process::exit(124);
                }
            })
            .map_err(|_| Failure::Configuration)?;
        Ok(Self(Some(sender), Some(thread)))
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.try_send(());
        }
        if let Some(thread) = self.1.take() {
            let _ = thread.join();
        }
    }
}

async fn execute(validated: &Validated) -> (Result<Vec<Value>>, bool) {
    let settings = match Settings::new(CREDENTIALS.into(), REGISTRY.into(), CA.into(), HOST.into())
    {
        Ok(settings) => settings,
        Err(_) => return (Err(Failure::Configuration), true),
    };
    let stores = match Stores::new(settings) {
        Ok(stores) => Arc::new(stores),
        Err(_) => return (Err(Failure::Configuration), true),
    };
    let case_stores = stores.clone();
    let mut task = tokio::spawn(async move {
        let mut checks = Vec::new();
        cases::readiness(&case_stores, &mut checks).await;
        cases::read_only_and_reuse(
            &case_stores,
            |organization, principal, epoch| TrustedRequest {
                organization: organization.into(),
                principal: principal.into(),
                epoch,
                issuer: "app008-fixture-only".into(),
                provider_tenant: format!("ten_{organization}"),
                provider_subject: format!("usr_{principal}"),
                registry_sha256: case_stores.registry_identity().sha256.clone(),
            },
            &mut checks,
        )
        .await;
        checks
    });
    let (result, task_finished) = match timeout_at(validated.deadline - RESERVE, &mut task).await {
        Ok(Ok(checks)) => (Ok(checks), true),
        Ok(Err(_)) => (Err(Failure::Case), true),
        Err(_) => {
            task.abort();
            // Await cancellation before closing pools; no detached owner may
            // survive while a successful cleanup receipt is emitted.
            let finished = timeout_at(validated.deadline - Duration::from_secs(3), &mut task)
                .await
                .is_ok();
            (Err(Failure::Deadline), finished)
        }
    };
    let cleanup = matches!(
        timeout_at(validated.deadline, stores.close()).await,
        Ok(Ok(()))
    ) && pool::slots().0 == 0
        && task_finished;
    let result = if !cleanup {
        Err(Failure::Cleanup)
    } else if validate_files(validated).is_err() {
        Err(Failure::Configuration)
    } else {
        result
    };
    (result, cleanup)
}

fn report(validated: Option<&Validated>, result: Result<Vec<Value>>, cleanup: bool) -> Value {
    let (passed, checks, error) = match result {
        Ok(checks) => (true, checks, None),
        Err(error) => (false, Vec::new(), Some(error.code())),
    };
    json!({"schema_version":1,"scope":SCOPE,"passed":passed,"error":error,"checks":checks,
        "cleanup_confirmed":cleanup,"run_id":validated.map(|v|&v.manifest.run_id),
        "original_issued_at":validated.map(|v|v.manifest.issued_at),"original_expires_at":validated.map(|v|v.manifest.expires_at),
        "manifest_sha256":validated.map(|v|&v.manifest_sha256),"resource_pins":expected_pins(),
        "declared_control_plane_resource_id":RESOURCE_ID,
        "ca_sha256_self_consistency_only":validated.map(|v|&v.manifest.ca_sha256),
        "limits":["Fixture identities are synthetic; no hosted identity or application admission is established.",
        "RDS resource ID and CA provenance require separate reviewed control-plane and immutable-image receipts.",
        "This subset does not pass managed races, drift, HTTP, recovery or complete APP-008 acceptance."]})
}

#[test]
#[ignore = "operator-only exact selector, fixed arming and private immutable run manifest required"]
fn managed_readonly_entry() -> std::result::Result<(), &'static str> {
    // No assertion or driver panic payload may reach the test harness output.
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args
            != [
                "--exact",
                ENTRY,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ]
        {
            return (report(None, Err(Failure::Configuration), true), None);
        }
        let validated = match load() {
            Ok(validated) => validated,
            Err(error) => return (report(None, Err(error), true), None),
        };
        let watchdog = match Watchdog::start(validated.deadline) {
            Ok(watchdog) => watchdog,
            Err(error) => return (report(Some(&validated), Err(error), true), None),
        };
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(_) => {
                return (
                    report(Some(&validated), Err(Failure::Configuration), true),
                    Some(watchdog),
                )
            }
        };
        let (outcome, cleanup) = runtime.block_on(execute(&validated));
        runtime.shutdown_timeout(Duration::ZERO);
        (report(Some(&validated), outcome, cleanup), Some(watchdog))
    });
    let (result, watchdog) =
        result.unwrap_or_else(|_| (report(None, Err(Failure::Case), false), None));
    let passed = result["passed"] == true;
    let encoded = serde_json::to_string(&result).map_err(|_| "managed qualification refused")?;
    if encoded.len() > 8192 {
        return Err("managed qualification refused");
    }
    println!("APP008_MANAGED_RESULT {encoded}");
    drop(watchdog);
    if passed {
        Ok(())
    } else {
        Err("managed qualification refused")
    }
}
