//! APP-075 entry tests. Unit tests use private temporary inputs owned by the
//! current UID; process tests re-run this executable to observe the real
//! output boundary; the ignored Docker campaign proves cleanup on real
//! PostgreSQL 16.13/TLS transports.
use super::*;
use crate::storage::{
    authorization::Transaction,
    pool::Lease,
    qualification::Fixture,
    qualification_cases::{APP, ORG, PRINCIPAL, PUBLICATION},
};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    time::Instant as WallInstant,
};

const RDS_CA: &[u8] = include_bytes!("../../../certs/rds-global-bundle.pem");
const CHILD: &str = "GENUI_APP075_CHILD";
const CHILD_ENTRY: &str = "storage::managed_qualification::tests::child_entry";

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn uid() -> u32 {
    unsafe { libc::geteuid() }
}
fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A private 0700 directory under the canonical temporary directory, so no
/// ancestor is a symbolic link (macOS /var is one).
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = ring::rand::SystemRandom::new();
        let mut bytes = [0u8; 12];
        ring::rand::SecureRandom::fill(&nonce, &mut bytes).unwrap();
        let suffix: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("genui-app075-{suffix}"));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn write(path: &Path, bytes: &[u8], mode: u32) {
    let _ = fs::remove_file(path);
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

const RUN: &str = "00000000-0000-4000-8000-000000000aaa";

fn credentials(host: &str) -> Vec<u8> {
    let now = now_seconds();
    let mut roles = serde_json::Map::new();
    for role in ["workspace", "control", "data", "worker"] {
        roles.insert(
            role.into(),
            json!({"username":format!("genui_{role}"),"token":format!("unit-{role}"),"expires_at":now+600}),
        );
    }
    serde_json::to_vec(&json!({"schema_version":1,"generation":"00000000-0000-4000-8000-000000000001",
        "issued_at":now,"expires_at":now+600,"database":{"host":host,"name":"genui","port":5432,"roles":roles},"storage":{}}))
    .unwrap()
}
fn manifest_value(issued_at: u64, expires_at: u64) -> Value {
    json!({"schema_version":1,"scope":SCOPE,"armed":true,"run_id":RUN,"issued_at":issued_at,"expires_at":expires_at,
        "database_host":HOST,"database_port":5432,"database_name":"genui","declared_rds_resource_id":RESOURCE_ID,
        "organizations":[cases::ORG,cases::OTHER],"registry_sha256":digest(&registry_bytes(RUN)),
        "ca_sha256":digest(RDS_CA),"pins":expected_pins()})
}

/// A complete, valid private input set for the current UID.
struct Inputs {
    scratch: Scratch,
    paths: Paths,
}
impl Inputs {
    fn new() -> Self {
        let scratch = Scratch::new();
        let run = scratch.0.join("run");
        let certs = scratch.0.join("certs");
        fs::create_dir(&run).unwrap();
        fs::set_permissions(&run, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(&certs).unwrap();
        fs::set_permissions(&certs, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = Paths {
            manifest: run.join("app008-managed-run.json"),
            credentials: run.join("credentials.json"),
            registry: run.join("app008-managed-registry.json"),
            ca: certs.join("rds-global-bundle.pem"),
        };
        let now = now_seconds();
        write(
            &paths.manifest,
            &serde_json::to_vec(&manifest_value(now, now + 600)).unwrap(),
            0o600,
        );
        write(&paths.registry, &registry_bytes(RUN), 0o600);
        write(&paths.credentials, &credentials(HOST), 0o600);
        write(&paths.ca, RDS_CA, 0o644);
        Self { scratch, paths }
    }
    fn load(&self) -> Result<Validated> {
        load_as(&environment(), &self.paths, uid())
    }
    fn settings(&self) -> std::result::Result<Settings, crate::storage::Error> {
        Settings::new(
            self.paths.credentials.clone(),
            self.paths.registry.clone(),
            self.paths.ca.clone(),
            HOST.into(),
        )
    }
}
fn environment() -> Environment {
    Environment {
        args: ARGUMENTS.iter().map(|s| s.to_string()).collect(),
        euid: uid(),
        arm: Some(ARM_VALUE.into()),
        now: SystemTime::now(),
        monotonic: Instant::now(),
    }
}
fn parse_now(raw: &[u8]) -> Result<Validated> {
    parse(raw, SystemTime::now(), Instant::now())
}
fn parse_value(value: &Value) -> Result<Validated> {
    parse_now(&serde_json::to_vec(value).unwrap())
}
fn valid() -> Value {
    let now = now_seconds();
    manifest_value(now, now + 600)
}
/// The six named shared-case checks of a passing run.
fn six() -> Vec<Value> {
    CHECKS
        .iter()
        .map(|name| json!({"name":name,"passed":true}))
        .collect()
}

#[test]
fn valid_inputs_load_and_bind_the_original_deadline() {
    let inputs = Inputs::new();
    let sampled = Instant::now();
    let validated = inputs.load().unwrap();
    assert_eq!(validated.manifest.run_id, RUN);
    let remaining = validated.deadline.saturating_duration_since(sampled);
    assert!(remaining <= Duration::from_secs(600) && remaining > Duration::from_secs(590));
    // The deadline is derived from the original expiry and an earlier
    // monotonic sample, never from the time validation finished.
    let now = SystemTime::now();
    let early = Instant::now() - Duration::from_secs(30);
    let validated = parse(&serde_json::to_vec(&valid()).unwrap(), now, early).unwrap();
    assert!(
        validated.deadline.saturating_duration_since(Instant::now()) <= Duration::from_secs(571)
    );
}

#[test]
fn manifest_is_closed_and_refuses_duplicate_or_unknown_keys() {
    assert!(parse_value(&valid()).is_ok());
    let raw = serde_json::to_string(&valid()).unwrap();
    let mut refused: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"null".to_vec(),
        b"[]".to_vec(),
        format!("{raw} {{}}").into_bytes(),
        format!("{{\"armed\":true,{}", &raw[1..]).into_bytes(),
        raw.replacen("\"pins\":{", "\"pins\":{\"database_version\":4,", 1)
            .into_bytes(),
        format!("{raw}{}", " ".repeat(8192)).into_bytes(),
        vec![0xff, 0xfe],
    ];
    let mut unknown = valid();
    unknown["extra"] = json!(true);
    refused.push(serde_json::to_vec(&unknown).unwrap());
    let mut unknown_pin = valid();
    unknown_pin["pins"]["image_sha256"] = json!("a".repeat(64));
    refused.push(serde_json::to_vec(&unknown_pin).unwrap());
    for field in ["armed", "run_id", "pins", "ca_sha256", "organizations"] {
        let mut missing = valid();
        missing.as_object_mut().unwrap().remove(field);
        refused.push(serde_json::to_vec(&missing).unwrap());
    }
    let mut missing_pin = valid();
    missing_pin["pins"]
        .as_object_mut()
        .unwrap()
        .remove("seed_spend_review_sha256");
    refused.push(serde_json::to_vec(&missing_pin).unwrap());
    let mut wrong_type = valid();
    wrong_type["database_port"] = json!("5432");
    refused.push(serde_json::to_vec(&wrong_type).unwrap());
    let mut three = valid();
    three["organizations"] = json!([cases::ORG, cases::OTHER, cases::OTHER]);
    refused.push(serde_json::to_vec(&three).unwrap());
    for raw in refused {
        assert_eq!(
            parse_now(&raw).err(),
            Some(Failure::Configuration),
            "{}",
            String::from_utf8_lossy(&raw[..raw.len().min(120)])
        );
    }
}

#[test]
fn selector_arming_scope_and_uid_are_exact() {
    let exact: Vec<String> = ARGUMENTS.iter().map(|s| s.to_string()).collect();
    let mut variants = vec![
        Vec::new(),
        exact[..4].to_vec(),
        [exact.clone(), vec!["--include-ignored".into()]].concat(),
        vec![
            exact[1].clone(),
            exact[0].clone(),
            exact[2].clone(),
            exact[3].clone(),
            exact[4].clone(),
        ],
    ];
    let mut renamed = exact.clone();
    renamed[1] = "storage::managed_qualification::managed_readonly_entr".into();
    variants.push(renamed);
    let mut threads = exact.clone();
    threads[4] = "--test-threads=2".into();
    variants.push(threads);
    for args in variants {
        let mut environment = environment();
        environment.args = args;
        let deadline = OnceLock::new();
        let (result, watchdog) = run(environment, &deadline);
        assert!(deadline.get().is_none());
        assert!(watchdog.is_none());
        assert_eq!(result["error"], "configuration_refused");
        assert_eq!(result["cleanup_applicable"], false);
        assert_eq!(result["cleanup_confirmed"], false);
        assert_eq!(result["run_id"], Value::Null);
    }
    let inputs = Inputs::new();
    for arm in [
        None,
        Some("app-008"),
        Some("APP-008 "),
        Some("APP-075"),
        Some(""),
    ] {
        let mut environment = environment();
        environment.arm = arm.map(Into::into);
        assert_eq!(
            load_as(&environment, &inputs.paths, uid()).err(),
            Some(Failure::Configuration)
        );
    }
    for euid in [0, uid() + 1, UID] {
        let mut environment = environment();
        environment.euid = euid;
        assert_eq!(
            load_as(&environment, &inputs.paths, uid()).err(),
            Some(Failure::Configuration)
        );
    }
    // The fixed entry insists on UID 65532 and the fixed /run/genui paths.
    if uid() != UID {
        assert_eq!(load(&environment()).err(), Some(Failure::Configuration));
    }
    for (field, value) in [
        ("schema_version", json!(2)),
        ("scope", json!("APP-008 managed qualification")),
        ("armed", json!(false)),
        ("run_id", json!("00000000-0000-7000-8000-000000000aaa")),
        ("run_id", json!("00000000-0000-4000-8000-000000000AAA")),
        ("run_id", json!("not-a-run")),
    ] {
        let mut manifest = valid();
        manifest[field] = value;
        assert_eq!(
            parse_value(&manifest).err(),
            Some(Failure::Configuration),
            "{field}"
        );
    }
}

#[test]
fn target_organizations_and_every_pin_are_fixed() {
    let other_host =
        "genuisyntheticfoundation-databaseb269d8bb-xxxxxxxxxxxx.cg58i06y2r4e.us-east-1.rds.amazonaws.com";
    for (field, value) in [
        ("database_host", json!(other_host)),
        ("database_host", json!("localhost")),
        ("database_host", json!(HOST.to_uppercase())),
        ("database_port", json!(5433)),
        ("database_name", json!("postgres")),
        ("declared_rds_resource_id", json!("db-OTHER")),
        ("organizations", json!([cases::OTHER, cases::ORG])),
        ("organizations", json!([cases::ORG, cases::ORG])),
        (
            "registry_sha256",
            json!(digest(&registry_bytes(
                "00000000-0000-4000-8000-000000000bbb"
            ))),
        ),
        ("ca_sha256", json!(digest(RDS_CA).to_uppercase())),
        ("ca_sha256", json!(digest(RDS_CA)[..63])),
    ] {
        let mut manifest = valid();
        manifest[field] = value;
        assert_eq!(
            parse_value(&manifest).err(),
            Some(Failure::Configuration),
            "{field}"
        );
    }
    let pins = serde_json::to_value(expected_pins()).unwrap();
    for (field, value) in pins.as_object().unwrap() {
        let mut manifest = valid();
        manifest["pins"][field] = match value {
            Value::String(text) => {
                let mut changed = text.clone();
                let last = changed.pop().unwrap();
                changed.push(if last == '0' { '1' } else { '0' });
                json!(changed)
            }
            Value::Number(number) => json!(number.as_u64().unwrap() + 1),
            _ => unreachable!(),
        };
        assert_eq!(
            parse_value(&manifest).err(),
            Some(Failure::Configuration),
            "{field}"
        );
    }
}

#[test]
fn registry_ca_and_credential_files_are_bound() {
    let inputs = Inputs::new();
    assert!(inputs.load().is_ok());
    let mut pretty =
        serde_json::to_vec_pretty(&serde_json::from_slice::<Value>(&registry_bytes(RUN)).unwrap())
            .unwrap();
    write(&inputs.paths.registry, &pretty, 0o600);
    assert!(inputs.load().is_err());
    pretty = registry_bytes(RUN);
    pretty.push(b'\n');
    write(&inputs.paths.registry, &pretty, 0o600);
    assert!(inputs.load().is_err());
    write(&inputs.paths.registry, &registry_bytes(RUN), 0o600);
    assert!(inputs.load().is_ok());
    // CA bytes must match the manifest; the public CA must not be writable by
    // group or others or be a link.
    let mut other = RDS_CA.to_vec();
    other.push(b'\n');
    write(&inputs.paths.ca, &other, 0o644);
    assert!(inputs.load().is_err());
    write(&inputs.paths.ca, RDS_CA, 0o664);
    assert!(inputs.load().is_err());
    write(&inputs.paths.ca, RDS_CA, 0o646);
    assert!(inputs.load().is_err());
    let real = inputs.scratch.0.join("real-ca.pem");
    write(&real, RDS_CA, 0o644);
    fs::remove_file(&inputs.paths.ca).unwrap();
    std::os::unix::fs::symlink(&real, &inputs.paths.ca).unwrap();
    assert!(inputs.load().is_err());
    fs::remove_file(&inputs.paths.ca).unwrap();
    write(&inputs.paths.ca, RDS_CA, 0o444);
    assert!(inputs.load().is_ok());
    // Credentials must name the fixed host and be a valid private envelope.
    write(&inputs.paths.credentials, &credentials("localhost"), 0o600);
    assert!(inputs.load().is_err());
    write(&inputs.paths.credentials, b"{}", 0o600);
    assert!(inputs.load().is_err());
    fs::remove_file(&inputs.paths.credentials).unwrap();
    assert!(inputs.load().is_err());
    write(&inputs.paths.credentials, &credentials(HOST), 0o600);
    assert!(inputs.load().is_ok());
}

#[test]
fn private_inputs_refuse_links_modes_owners_fifos_directories_and_size() {
    let inputs = Inputs::new();
    let manifest = fs::read(&inputs.paths.manifest).unwrap();
    let path = &inputs.paths.manifest;
    let reset = || write(path, &manifest, 0o600);
    assert!(bytes(path, MANIFEST_LIMIT, true, uid()).is_ok());
    // Other owner: the same file checked for a different expected UID.
    assert!(bytes(path, MANIFEST_LIMIT, true, uid() + 1).is_err());
    for mode in [0o640, 0o604, 0o400, 0o700] {
        write(path, &manifest, mode);
        assert!(
            bytes(path, MANIFEST_LIMIT, true, uid()).is_err(),
            "{mode:o}"
        );
    }
    reset();
    let run = path.parent().unwrap();
    for mode in [0o750, 0o755, 0o500] {
        fs::set_permissions(run, fs::Permissions::from_mode(mode)).unwrap();
        assert!(
            bytes(path, MANIFEST_LIMIT, true, uid()).is_err(),
            "{mode:o}"
        );
    }
    fs::set_permissions(run, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(inputs.load().is_ok());
    // Hard link (nlink 2) and symbolic link.
    let twin = run.join("twin.json");
    fs::hard_link(path, &twin).unwrap();
    assert!(bytes(path, MANIFEST_LIMIT, true, uid()).is_err());
    fs::remove_file(&twin).unwrap();
    let real = run.join("real.json");
    fs::rename(path, &real).unwrap();
    std::os::unix::fs::symlink(&real, path).unwrap();
    assert!(bytes(path, MANIFEST_LIMIT, true, uid()).is_err());
    fs::remove_file(path).unwrap();
    fs::remove_file(&real).unwrap();
    // A directory and an oversized file.
    fs::create_dir(path).unwrap();
    assert!(bytes(path, MANIFEST_LIMIT, true, uid()).is_err());
    fs::remove_dir(path).unwrap();
    write(path, &vec![b' '; MANIFEST_LIMIT as usize + 1], 0o600);
    assert!(bytes(path, MANIFEST_LIMIT, true, uid()).is_err());
    reset();
    // A FIFO is refused without blocking.
    let fifo = run.join("fifo.json");
    let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let (sender, receiver) = mpsc::channel();
    let probe = fifo.clone();
    std::thread::spawn(move || {
        sender
            .send(bytes(&probe, MANIFEST_LIMIT, true, uid()).is_err())
            .unwrap()
    });
    assert!(receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("FIFO read blocked"));
    // Relative, dot-dot and symlinked-ancestor paths.
    assert!(bytes(
        Path::new("run/app008-managed-run.json"),
        MANIFEST_LIMIT,
        true,
        uid()
    )
    .is_err());
    let dotted = run.join("..").join("run").join("app008-managed-run.json");
    assert!(bytes(&dotted, MANIFEST_LIMIT, true, uid()).is_err());
    let alias = inputs.scratch.0.join("alias");
    std::os::unix::fs::symlink(run, &alias).unwrap();
    assert!(bytes(
        &alias.join("app008-managed-run.json"),
        MANIFEST_LIMIT,
        true,
        uid()
    )
    .is_err());
    // Registry and credentials obey the same private rules.
    write(&inputs.paths.registry, &registry_bytes(RUN), 0o640);
    assert!(inputs.load().is_err());
    write(&inputs.paths.registry, &registry_bytes(RUN), 0o600);
    write(&inputs.paths.credentials, &credentials(HOST), 0o644);
    assert!(inputs.load().is_err());
    write(&inputs.paths.credentials, &credentials(HOST), 0o600);
    assert!(inputs.load().is_ok());
}

#[test]
fn original_deadline_bounds_span_remaining_time_and_issue_skew() {
    // Offsets from a clock sample taken immediately before each parse; the
    // pins are computed once beforehand. Accepted rows keep at least one
    // second of margin, refused rows sit at least one second past the bound.
    let mut manifest = valid();
    for (issued, expires, accepted) in [
        (0, 600, true),
        (-1, 600, false), // span 601
        (-538, 62, true),
        (-541, 59, false), // remaining under 60 s
        (-300, 59, false),
        (4, 64, true),
        (7, 67, false), // issued beyond the 5 s tolerance
        (-600, 0, false),
        (-600, -1, false),
        (0, 0, false),
        (10, 5, false),
    ] {
        let now = now_seconds() as i64;
        manifest["issued_at"] = json!(now + issued);
        manifest["expires_at"] = json!(now + expires);
        assert_eq!(
            parse_value(&manifest).is_ok(),
            accepted,
            "issued {issued:+} expires {expires:+}"
        );
    }
}

#[test]
fn reports_are_sanitized_closed_and_bounded() {
    let keys: BTreeSet<&str> = [
        "schema_version",
        "scope",
        "passed",
        "error",
        "refusal",
        "checks",
        "cleanup_applicable",
        "cleanup_confirmed",
        "run_id",
        "original_issued_at",
        "original_expires_at",
        "manifest_sha256",
        "resource_pins",
        "declared_control_plane_resource_id",
        "ca_sha256_self_consistency_only",
        "limits",
    ]
    .into_iter()
    .collect();
    let check = six;
    for (result, cleanup, passed, error, applicable, confirmed) in [
        (Ok(check()), Cleanup::Confirmed, true, None, true, true),
        (
            Ok(check()),
            Cleanup::Unconfirmed,
            false,
            Some("cleanup_unconfirmed"),
            true,
            false,
        ),
        (
            Err(Failure::Configuration),
            Cleanup::NotApplicable,
            false,
            Some("configuration_refused"),
            false,
            false,
        ),
        (
            Err(Failure::Case),
            Cleanup::Confirmed,
            false,
            Some("case_refused"),
            true,
            true,
        ),
        (
            Err(Failure::Refused(Refusal::Panic(3))),
            Cleanup::Confirmed,
            false,
            Some("case_refused"),
            true,
            true,
        ),
        (
            Err(Failure::Deadline),
            Cleanup::Confirmed,
            false,
            Some("original_deadline_reached"),
            true,
            true,
        ),
        (
            Err(Failure::Cleanup),
            Cleanup::Unconfirmed,
            false,
            Some("cleanup_unconfirmed"),
            true,
            false,
        ),
        (
            Err(Failure::EntryPanic),
            Cleanup::Unconfirmed,
            false,
            Some("entry_panic"),
            true,
            false,
        ),
    ] {
        let value = report(None, result, cleanup);
        let found: BTreeSet<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(found, keys);
        assert_eq!(value["passed"], passed);
        assert_eq!(value["error"], json!(error));
        assert_eq!(value["cleanup_applicable"], applicable);
        assert_eq!(value["cleanup_confirmed"], confirmed);
        assert_eq!(value["checks"].as_array().unwrap().is_empty(), !passed);
        // APP-086: a closed refusal exactly when a case refused.
        assert_eq!(value["refusal"].is_null(), error != Some("case_refused"));
        if !value["refusal"].is_null() {
            assert_closed_refusal(&value["refusal"], &[]);
        }
        let mut out = Vec::new();
        assert_eq!(emit(value, None, &mut out), passed);
        let line = String::from_utf8(out).unwrap();
        assert!(line.starts_with("\nAPP008_MANAGED_RESULT {") && line.ends_with("}\n"));
        assert_eq!(line.lines().filter(|l| !l.is_empty()).count(), 1);
        assert!(line.len() <= RESULT_LIMIT + RESULT_PREFIX.len() + 2);
    }
    // An oversized report is replaced by a bounded refusal, never truncated;
    // it keeps the report's boolean cleanup state and nothing else.
    for (applicable, confirmed, expected) in [
        (json!(false), json!(false), (false, false)),
        (json!(true), json!(false), (true, false)),
        (json!(true), json!(true), (true, true)),
        (json!("false"), json!(1), (true, false)),
        (Value::Null, Value::Null, (true, false)),
    ] {
        let huge = json!({"passed":true,"cleanup_applicable":applicable,"cleanup_confirmed":confirmed,
            "run_id":RUN,"checks":[{"name":"x".repeat(RESULT_LIMIT),"passed":true}]});
        let mut out = Vec::new();
        assert!(!emit(huge, None, &mut out));
        let line = String::from_utf8(out).unwrap();
        let body: Value =
            serde_json::from_str(line.trim().strip_prefix("APP008_MANAGED_RESULT ").unwrap())
                .unwrap();
        assert_eq!(body["error"], "report_refused");
        assert_eq!(body["passed"], false);
        assert_eq!(body["checks"], json!([]));
        assert_eq!(body["cleanup_applicable"], expected.0);
        assert_eq!(body["cleanup_confirmed"], expected.1);
        assert_eq!(body.as_object().unwrap().len(), 7);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervise_proves_cleanup_on_success_panic_and_cooperative_timeout() {
    let inputs = Inputs::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    let (result, cleanup) = supervise(
        inputs.settings(),
        deadline,
        |stores| async move {
            assert_eq!(stores.registry_identity().version, RUN);
            Ok(vec![json!({"name":"synthetic","passed":true})])
        },
        || Ok(()),
    )
    .await;
    assert_eq!(result.unwrap().len(), 1);
    assert_eq!(cleanup, Cleanup::Confirmed);
    let (result, cleanup) = supervise(
        inputs.settings(),
        deadline,
        |_| async move { panic!("synthetic case panic") },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Case));
    assert_eq!(cleanup, Cleanup::Confirmed);
    let started = Instant::now();
    let (result, cleanup) = supervise(
        inputs.settings(),
        Instant::now() + Duration::from_secs(6),
        |_| async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(Vec::new())
        },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Deadline));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert!(started.elapsed() < Duration::from_secs(5));
    // A replaced input after the cases voids an otherwise passing result.
    let (result, cleanup) = supervise(
        inputs.settings(),
        deadline,
        |_| async move { Ok(Vec::new()) },
        || Err(Failure::Configuration),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Configuration));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert_eq!(pool::slots().0, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervise_never_confirms_cleanup_for_an_unacknowledged_abort() {
    let inputs = Inputs::new();
    // A case that blocks its worker cannot observe the abort request.
    let (result, cleanup) = supervise(
        inputs.settings(),
        Instant::now() + Duration::from_millis(5500),
        |_| async move {
            std::thread::sleep(Duration::from_secs(4));
            Ok(Vec::new())
        },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Cleanup));
    assert_eq!(cleanup, Cleanup::Unconfirmed);
    let value = report(None, Err(Failure::Cleanup), cleanup);
    assert_eq!(value["cleanup_confirmed"], false);
    assert_eq!(value["passed"], false);
}

#[tokio::test]
async fn refusals_before_store_construction_are_not_applicable() {
    let inputs = Inputs::new();
    let (result, cleanup) = supervise(
        Err(crate::storage::Error::Configuration),
        Instant::now() + Duration::from_secs(60),
        |_| async move { unreachable!() },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Configuration));
    assert_eq!(cleanup, Cleanup::NotApplicable);
    fs::remove_file(&inputs.paths.registry).unwrap();
    let (result, cleanup) = supervise(
        inputs.settings(),
        Instant::now() + Duration::from_secs(60),
        |_| async move { unreachable!() },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Configuration));
    assert_eq!(cleanup, Cleanup::NotApplicable);
}

// ---------------------------------------------------------------------------
// APP-086 refusal diagnostics.

/// Every member is present, every string is a closed lowercase code, a
/// storage error variant or a five-character SQLSTATE, and no `forbidden`
/// text appears anywhere in the encoded refusal.
fn assert_closed_refusal(refusal: &Value, forbidden: &[&str]) {
    fn strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
        match value {
            Value::String(text) => out.push(text),
            Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
            Value::Object(members) => members.values().for_each(|item| strings(item, out)),
            _ => (),
        }
    }
    let keys: BTreeSet<&str> = refusal
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, REFUSAL_KEYS.into_iter().collect());
    let pool_keys: BTreeSet<&str> = [
        "pool",
        "stage",
        "step",
        "error",
        "sqlstate",
        "cause",
        "elapsed_ms",
    ]
    .into_iter()
    .collect();
    for pool in refusal["pools"].as_array().into_iter().flatten() {
        let keys: BTreeSet<&str> = pool
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, pool_keys);
    }
    let mut found = Vec::new();
    strings(refusal, &mut found);
    for text in found {
        let code = !text.is_empty()
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        let variant = [
            "Configuration",
            "Denied",
            "Unavailable",
            "Deadline",
            "UnknownOutcome",
        ]
        .contains(&text);
        let sqlstate = text.len() == 5
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase());
        assert!(
            code || variant || sqlstate,
            "unexpected refusal text {text}"
        );
    }
    let encoded = refusal.to_string();
    for text in forbidden {
        assert!(!encoded.contains(text), "refusal contains {text}");
    }
}

/// A loopback port with no listener (bound, then released).
fn closed_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn loopback_settings(inputs: &Inputs, port: u16) -> Settings {
    Settings::fixture(
        inputs.paths.credentials.clone(),
        inputs.paths.registry.clone(),
        inputs.paths.ca.clone(),
        "127.0.0.1",
        port,
    )
}
const SECRETS: [&str; 4] = ["unit-workspace", "unit-control", "unit-data", "unit-worker"];

/// Child mode `readiness_refusal`: the production cases against a loopback
/// port that refuses every transport. A child process, so the transport
/// permits it briefly holds never meet another test's cleanup proof.
fn readiness_refusal_child() -> (Value, Option<Watchdog>) {
    let inputs = Inputs::new();
    // The credential host matches the loopback target, whose port refuses
    // every transport: all four pools fail inside acquire's connect step.
    write(&inputs.paths.credentials, &credentials("127.0.0.1"), 0o600);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (result, cleanup) = runtime.block_on(supervise(
        Ok(loopback_settings(&inputs, closed_port())),
        Instant::now() + Duration::from_secs(30),
        production_cases,
        || Ok(()),
    ));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert_eq!(pool::slots().0, 0);
    assert!(matches!(
        result,
        Err(Failure::Refused(Refusal::Readiness(_)))
    ));
    (report(None, result, cleanup), None)
}

#[test]
fn a_readiness_refusal_names_the_pool_stage_and_closed_cause() {
    let output = child("readiness_refusal");
    assert_ne!(output.code, Some(0));
    let value = result_line(&output.stdout).expect("result line");
    assert_eq!(value["error"], "case_refused");
    assert_eq!(value["passed"], false);
    assert_eq!(value["cleanup_confirmed"], true);
    assert_eq!(value["checks"], json!([]));
    let refusal = &value["refusal"];
    for (key, expected) in [
        ("case", json!("readiness")),
        ("reason", json!("stage_failed")),
        ("pool", json!("workspace")),
        ("stage", json!("acquire")),
        ("step", json!("connect")),
        ("error", json!("Unavailable")),
        ("cause", json!("io_connection_refused")),
        ("sqlstate", Value::Null),
        ("checks_passed", json!(0)),
    ] {
        assert_eq!(refusal[key], expected, "{key}");
    }
    let pools = refusal["pools"].as_array().unwrap();
    assert_eq!(pools.len(), 4);
    for (probe, name) in pools.iter().zip(POOLS) {
        assert_eq!(probe["pool"], name);
        assert_eq!(probe["stage"], "acquire");
        assert_eq!(probe["error"], "Unavailable");
        assert_eq!(probe["cause"], "io_connection_refused");
    }
    let mut forbidden = SECRETS.to_vec();
    forbidden.extend([
        "127.0.0.1",
        HOST,
        "genui_",
        "SELECT",
        "password",
        "error connecting",
        "refused (os error",
    ]);
    assert_closed_refusal(refusal, &forbidden);
    // Nothing forbidden on either stream of the real output boundary.
    for text in &forbidden {
        assert!(
            !output.stdout.contains(text) && !output.stderr.contains(text),
            "{text}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configuration_stage_refusals_name_their_closed_cause() {
    // No transport is ever opened: each refusal precedes `connect`.
    let inputs = Inputs::new();
    let port = closed_port();
    // The credential file names the RDS host, the settings the loopback one.
    let stores = Stores::new(loopback_settings(&inputs, port)).unwrap();
    let probes = stores.readiness_diagnosis().await;
    stores.close().await.unwrap();
    for probe in probes {
        assert_eq!(probe.stage, "acquire");
        assert_eq!(probe.step, Some("configuration"));
        assert_eq!(probe.error, Some(crate::storage::Error::Configuration));
        assert_eq!(probe.cause, Some("credentials_host_mismatch"));
    }
    // Credentials that expire within the transaction margin.
    let mut expiring: Value = serde_json::from_slice(&credentials("127.0.0.1")).unwrap();
    let soon = now_seconds() + 5;
    expiring["expires_at"] = json!(soon);
    for role in ["workspace", "control", "data", "worker"] {
        expiring["database"]["roles"][role]["expires_at"] = json!(soon);
    }
    write(
        &inputs.paths.credentials,
        &serde_json::to_vec(&expiring).unwrap(),
        0o600,
    );
    let stores = Stores::new(loopback_settings(&inputs, port)).unwrap();
    let probes = stores.readiness_diagnosis().await;
    stores.close().await.unwrap();
    assert!(probes
        .iter()
        .all(|probe| probe.cause == Some("credentials_expiring")
            && probe.error == Some(crate::storage::Error::Configuration)));
    // An unreadable credential file (mode 0644 refused by Config::load).
    write(&inputs.paths.credentials, &credentials("127.0.0.1"), 0o644);
    let stores = Stores::new(loopback_settings(&inputs, port)).unwrap();
    let probes = stores.readiness_diagnosis().await;
    stores.close().await.unwrap();
    assert!(probes
        .iter()
        .all(|probe| probe.cause == Some("credentials_file")));
    let value = Refusal::Readiness(Box::new(probes)).value();
    assert_eq!(value["error"], "Configuration");
    assert_closed_refusal(&value, &SECRETS);
    assert_eq!(pool::slots().0, 0);
}

#[tokio::test]
async fn a_later_case_panic_names_the_first_case_without_a_passed_check() {
    let mut checks = Vec::new();
    let outcome = Unwind(Box::pin(async {
        for name in &CHECKS[..2] {
            cases::check(&mut checks, name);
        }
        panic!("{}", sentinel());
    }))
    .await;
    assert!(outcome.is_err());
    assert_eq!(checks.len(), 2);
    let value = report(
        None,
        Err(Failure::Refused(Refusal::Panic(checks.len()))),
        Cleanup::Confirmed,
    );
    assert_eq!(value["error"], "case_refused");
    let refusal = &value["refusal"];
    assert_eq!(refusal["case"], "rls_and_write_refusal");
    assert_eq!(refusal["reason"], "panic");
    assert_eq!(refusal["checks_passed"], 2);
    assert_closed_refusal(refusal, &[&sentinel()]);
    for (passed, case) in CASES.iter().enumerate() {
        assert_eq!(Refusal::Panic(passed).value()["case"], *case);
    }
    assert_eq!(Refusal::Panic(6).value()["case"], "after_last_check");
    // A completed future passes its value through.
    assert_eq!(Unwind(Box::pin(async { 7 })).await, Ok(7));
}

fn sentinel() -> String {
    // Built at run time so the expected bytes never appear as one literal.
    ["APP075", "SENTINEL", "d1c3a9"].join("-")
}

/// Child-process helper for the process-boundary tests below. It does
/// nothing unless the parent test sets `GENUI_APP075_CHILD`.
#[test]
#[ignore = "child process helper for APP-075 output-boundary tests"]
fn child_entry() -> std::result::Result<(), &'static str> {
    let mode = match std::env::var(CHILD) {
        Ok(mode) => mode,
        Err(_) => return Ok(()),
    };
    let (result, watchdog) = match mode.as_str() {
        "entry_panic" => guarded(|_| panic!("{}", sentinel())),
        "entry_panic_after_load" => {
            let (result, watchdog) = guarded(|deadline| {
                let _ = deadline.set(Instant::now() + Duration::from_secs(1));
                panic!("{}", sentinel())
            });
            // Stands in for a blocked stdout: only the re-armed watchdog
            // can end the process at the original deadline.
            std::thread::sleep(Duration::from_secs(20));
            (result, watchdog)
        }
        "case_panic" => guarded(|_| {
            let inputs = Inputs::new();
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            let (outcome, cleanup) = runtime.block_on(supervise(
                inputs.settings(),
                Instant::now() + Duration::from_secs(30),
                |_| async move { panic!("{}", sentinel()) },
                || Ok(()),
            ));
            (report(None, outcome, cleanup), None)
        }),
        "readiness_refusal" => guarded(|_| readiness_refusal_child()),
        "watchdog" => {
            let watchdog = Watchdog::start(Instant::now() + Duration::from_secs(1)).unwrap();
            std::thread::sleep(Duration::from_secs(20));
            (
                report(None, Ok(Vec::new()), Cleanup::Confirmed),
                Some(watchdog),
            )
        }
        _ => return Err("unknown child mode"),
    };
    if emit(result, watchdog, &mut std::io::stdout().lock()) {
        Ok(())
    } else {
        Err("managed qualification refused")
    }
}

struct ChildOutput {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    elapsed: Duration,
}
fn child(mode: &str) -> ChildOutput {
    let started = WallInstant::now();
    let mut process = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD_ENTRY,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, mode)
        .env_remove("RUST_BACKTRACE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Drain both pipes while waiting (the stdout-drain contract).
    let mut out = process.stdout.take().unwrap();
    let mut err = process.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || {
        let mut text = String::new();
        out.read_to_string(&mut text).unwrap();
        text
    });
    let err_reader = std::thread::spawn(move || {
        let mut text = String::new();
        err.read_to_string(&mut text).unwrap();
        text
    });
    let end = WallInstant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = process.try_wait().unwrap() {
            break status;
        }
        if WallInstant::now() >= end {
            let _ = process.kill();
            panic!("child deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    ChildOutput {
        code: status.code(),
        stdout: out_reader.join().unwrap(),
        stderr: err_reader.join().unwrap(),
        elapsed: started.elapsed(),
    }
}
fn result_line(stdout: &str) -> Option<Value> {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("APP008_MANAGED_RESULT "))
        .collect();
    assert!(lines.len() <= 1);
    lines
        .first()
        .map(|line| serde_json::from_str(&line["APP008_MANAGED_RESULT ".len()..]).unwrap())
}

#[test]
fn case_panic_output_is_sanitized_and_cleanup_is_proven() {
    let output = child("case_panic");
    assert!(!output.stdout.contains(&sentinel()) && !output.stderr.contains(&sentinel()));
    assert!(!output.stderr.contains("panicked"));
    let result = result_line(&output.stdout).expect("result line");
    assert_eq!(result["error"], "case_refused");
    assert_eq!(result["refusal"]["reason"], "task_panic");
    assert_eq!(result["passed"], false);
    assert_eq!(result["cleanup_confirmed"], true);
    assert_ne!(output.code, Some(0));
}

#[test]
fn entry_panic_is_labeled_and_never_confirms_cleanup() {
    let output = child("entry_panic");
    assert!(!output.stdout.contains(&sentinel()) && !output.stderr.contains(&sentinel()));
    let result = result_line(&output.stdout).expect("result line");
    assert_eq!(result["error"], "entry_panic");
    assert_eq!(result["cleanup_applicable"], true);
    assert_eq!(result["cleanup_confirmed"], false);
    assert_ne!(output.code, Some(0));
}

#[test]
fn an_entry_panic_after_load_keeps_the_original_deadline_watchdog() {
    let output = child("entry_panic_after_load");
    assert_eq!(output.code, Some(124));
    assert!(result_line(&output.stdout).is_none());
    assert!(output.elapsed < Duration::from_secs(10));
    assert!(!output.stdout.contains(&sentinel()) && !output.stderr.contains(&sentinel()));
}

#[test]
fn watchdog_exits_at_the_original_deadline_without_a_result() {
    let output = child("watchdog");
    assert_eq!(output.code, Some(124));
    assert!(result_line(&output.stdout).is_none());
    assert!(output.elapsed < Duration::from_secs(10));
}

#[test]
fn source_bundle_covers_every_compiled_source() {
    let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = BTreeSet::new();
    let mut pending = vec![app.join("src")];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && path != app.join("src/main.rs")
            {
                found.insert(
                    path.strip_prefix(repository())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                );
            }
        }
    }
    for extra in [
        "app/Cargo.lock",
        "app/Cargo.toml",
        "spikes/APP-006/Cargo.toml",
        "spikes/APP-006/src/lib.rs",
    ] {
        found.insert(extra.into());
    }
    // Every file a bound source embeds with include_str!/include_bytes! is a
    // compile-time input; only the migration resources are pinned separately.
    let root = repository().canonicalize().unwrap();
    let rust: Vec<String> = found
        .iter()
        .filter(|n| n.ends_with(".rs"))
        .cloned()
        .collect();
    let mut embedded = BTreeSet::new();
    for name in &rust {
        let text = fs::read_to_string(repository().join(name)).unwrap();
        let directory = repository().join(name).parent().unwrap().to_path_buf();
        for macro_name in ["include_str!(\"", "include_bytes!(\""] {
            for piece in text.split(macro_name).skip(1) {
                let relative = &piece[..piece.find('"').unwrap()];
                let target = directory.join(relative).canonicalize().unwrap();
                let target = target
                    .strip_prefix(&root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                if !target.starts_with("app/migrations/") {
                    embedded.insert(target);
                }
            }
        }
    }
    for name in [
        "app/certs/rds-global-bundle.pem",
        "contracts/implementation-lock.json",
        "contracts/fixtures/impl-2026-10-28.1/northstar.json",
    ] {
        assert!(embedded.contains(name), "{name}");
    }
    found.extend(embedded);
    let bound: BTreeSet<String> = SOURCES.iter().map(|(name, _)| name.to_string()).collect();
    assert_eq!(bound, found);
    let names: Vec<&str> = SOURCES.iter().map(|(name, _)| *name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "bundle order is lexicographic");
    for (name, raw) in SOURCES {
        assert_eq!(fs::read(repository().join(name)).unwrap(), raw, "{name}");
    }
}

#[test]
fn seed_catalog_and_migration_pins_match_repository_files() {
    let repo = repository();
    let sha = |path: &str| digest(&fs::read(repo.join(path)).unwrap());
    let pins = expected_pins();
    let reported = serde_json::to_value(&pins).unwrap();
    // SEED_INPUTS is the one path list: local_harness.py and check_image.py
    // read the same table from this source.
    for (field, path, declared) in SEED_INPUTS {
        assert_eq!(sha(path), declared, "{path}");
        assert_eq!(reported[field], declared, "{field}");
    }
    let fields: BTreeSet<&str> = SEED_INPUTS.iter().map(|(field, _, _)| *field).collect();
    assert_eq!(fields.len(), SEED_INPUTS.len());
    assert_eq!(
        pins.catalog_query_sha256,
        sha("app/migrations/catalog-query.json")
    );
    assert_eq!(
        pins.migrations_manifest_sha256,
        sha("app/migrations/manifest.json")
    );
    let version = pins.database_version;
    assert!(version >= 4);
    assert_eq!(
        pins.catalog_sha256,
        sha(&format!("app/migrations/catalog-v{version}.json"))
    );
    // The seed reads exactly these two contract fixtures, from the pinned
    // edition's folder; an edition switch in the seed fails here.
    let editions: BTreeSet<&str> = SEED_INPUTS
        .iter()
        .filter_map(|(_, path, _)| path.strip_prefix("contracts/fixtures/"))
        .map(|rest| rest.split('/').next().unwrap())
        .collect();
    assert_eq!(editions.len(), 1);
    let edition = editions.into_iter().next().unwrap();
    let seed = fs::read_to_string(repo.join("app/runtime/tenant_seed.py")).unwrap();
    assert!(seed.contains(&format!(
        "CONTRACTS / \"fixtures\" / \"{edition}\" / (name + \".json\")"
    )));
    assert!(seed.contains("((\"northstar\", ORGANIZATIONS[0]), (\"northstar\", ORGANIZATIONS[1]), (\"spend-review\", ORGANIZATIONS[1]))"));
    for name in ["northstar", "spend-review"] {
        assert!(SEED_INPUTS
            .iter()
            .any(|(_, path, _)| *path == format!("contracts/fixtures/{edition}/{name}.json")));
    }
}

#[test]
fn a_pass_requires_exactly_the_six_shared_checks() {
    let cases = SOURCES
        .iter()
        .find(|(name, _)| *name == "app/src/storage/qualification_cases.rs")
        .map(|(_, raw)| std::str::from_utf8(raw).unwrap())
        .unwrap();
    for name in CHECKS {
        assert_eq!(cases.matches(&format!("\"{name}\"")).count(), 1, "{name}");
    }
    assert!(complete(&six()));
    let mut variants = vec![
        Vec::new(),
        six()[..5].to_vec(),
        [six(), six()[..1].to_vec()].concat(),
    ];
    let mut renamed = six();
    renamed[2] = json!({"name":"synthetic","passed":true});
    variants.push(renamed);
    let mut failed = six();
    failed[5]["passed"] = json!(false);
    variants.push(failed);
    let mut extra = six();
    extra[0]["detail"] = json!("x");
    variants.push(extra);
    let mut reordered = six();
    reordered.swap(0, 1);
    variants.push(reordered);
    for checks in variants {
        assert!(!complete(&checks));
        let value = report(None, Ok(checks.clone()), Cleanup::Confirmed);
        assert_eq!(value["passed"], false);
        assert_eq!(value["error"], "case_refused");
        assert_eq!(value["checks"], json!([]));
        assert_eq!(value["cleanup_confirmed"], true);
        let value = report(None, Ok(checks), Cleanup::Unconfirmed);
        assert_eq!(value["error"], "cleanup_unconfirmed");
    }
}

#[test]
fn registry_bytes_are_the_canonical_cross_language_vector() {
    // app/runtime/test_qualification_supervisor.py asserts the same bytes.
    assert_eq!(
        String::from_utf8(registry_bytes(RUN)).unwrap(),
        concat!(
            r#"{"bindings":[{"issuer":"app008-fixture-only","organization_id":"00000000-0000-4000-8000-000000000001","provider_tenant_id":"ten_00000000-0000-4000-8000-000000000001"},"#,
            r#"{"issuer":"app008-fixture-only","organization_id":"00000000-0000-4000-8000-000000000002","provider_tenant_id":"ten_00000000-0000-4000-8000-000000000002"}],"#,
            r#""registry_version":"00000000-0000-4000-8000-000000000aaa","schema_version":1}"#
        )
    );
}

#[test]
fn entry_cannot_select_a_fixture_endpoint_or_insecure_transport() {
    let source = include_str!("../managed_qualification.rs");
    let production = source.split("\nmod tests;").next().unwrap();
    for forbidden in [
        "Settings::fixture",
        "::fixture(",
        "NoTls",
        "dangerous",
        "SslMode::Disable",
        "SslMode::Prefer",
        "set_var",
    ] {
        assert!(!production.contains(forbidden), "{forbidden}");
    }
    assert_eq!(production.matches("Settings::new(").count(), 1);
    assert!(production
        .contains("Settings::new(CREDENTIALS.into(), REGISTRY.into(), CA.into(), HOST.into())"));
    for host in [
        "localhost",
        "127.0.0.1",
        "genui.example.com",
        "db.us-east-1.rds.amazonaws.com.evil",
    ] {
        assert!(
            Settings::new(CREDENTIALS.into(), REGISTRY.into(), CA.into(), host.into()).is_err()
        );
    }
}

// ---------------------------------------------------------------------------
// Real-transport cleanup (local Docker PostgreSQL 16.13 with TLS).

fn request(stores: &Stores) -> TrustedRequest {
    TrustedRequest {
        organization: ORG.into(),
        principal: PRINCIPAL.into(),
        epoch: 1,
        issuer: ISSUER.into(),
        provider_tenant: format!("ten_{ORG}"),
        provider_subject: format!("usr_{PRINCIPAL}"),
        registry_sha256: stores.registry_identity().sha256.clone(),
    }
}
/// Server-side proof: no application-role backend survives the close.
fn server_sessions(fixture: &Fixture) -> i64 {
    let end = WallInstant::now() + Duration::from_secs(3);
    loop {
        let count: i64 = fixture
            .sql("SELECT count(*) FROM pg_stat_activity WHERE usename IN ('genui_workspace','genui_control','genui_data','genui_worker')", "genui")
            .trim()
            .parse()
            .unwrap();
        if count == 0 || WallInstant::now() >= end {
            return count;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn managed_cleanup_with_real_transports() {
    std::thread::Builder::new()
        .name("app075-cleanup-campaign".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
                .block_on(cleanup_campaign());
        })
        .unwrap()
        .join()
        .unwrap();
}
async fn cleanup_campaign() {
    let fixture = Fixture::new();
    let mut receipt = Vec::new();
    // Success: the six production checks over real TLS transports.
    let (result, cleanup) = supervise(
        Ok(fixture.settings()),
        Instant::now() + Duration::from_secs(60),
        production_cases,
        || Ok(()),
    )
    .await;
    let checks = result.expect("production cases pass on the local fixture");
    assert_eq!(checks, six());
    assert!(complete(&checks));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert_eq!(pool::slots().0, 0);
    assert_eq!(server_sessions(&fixture), 0);
    receipt.push(json!({"case":"success","checks":checks.len(),"cleanup":"confirmed","slots":0,"server_sessions":0}));
    // A case panic while it owns an open transaction on a leased transport.
    let (result, cleanup) = supervise(
        Ok(fixture.settings()),
        Instant::now() + Duration::from_secs(60),
        |stores| async move {
            let request = request(&stores);
            let tx = Transaction::begin(&stores.data.inner, &request)
                .await
                .unwrap();
            tx.query("SELECT 1", &[]).await.unwrap();
            let _held = Lease::acquire(&stores.workspace.inner).await.unwrap();
            assert!(pool::slots().0 >= 2);
            panic!("synthetic case panic while holding transports");
        },
        || Ok(()),
    )
    .await;
    assert_eq!(result.err(), Some(Failure::Case));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert_eq!(pool::slots().0, 0);
    assert_eq!(server_sessions(&fixture), 0);
    receipt.push(json!({"case":"case_panic_holding_transaction_and_lease","error":"case_refused","cleanup":"confirmed","slots":0,"server_sessions":0}));
    // A timeout while a TestHooks barrier holds an open transaction.
    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    hooks.arm("begin");
    let started = Instant::now();
    let (result, cleanup) = supervise(
        Ok(settings),
        Instant::now() + Duration::from_secs(8),
        |stores| async move {
            let request = request(&stores);
            let _tx = Transaction::begin(&stores.workspace.inner, &request).await;
            unreachable!("the begin barrier is never released")
        },
        || Ok(()),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(result.err(), Some(Failure::Deadline));
    assert_eq!(cleanup, Cleanup::Confirmed);
    assert!(elapsed >= Duration::from_secs(4) && elapsed < Duration::from_secs(8));
    assert_eq!(pool::slots().0, 0);
    assert_eq!(server_sessions(&fixture), 0);
    receipt.push(json!({"case":"timeout_with_begin_barrier_holding_transaction","error":"original_deadline_reached","cleanup":"confirmed","elapsed_ms":elapsed.as_millis() as u64,"slots":0,"server_sessions":0}));
    // The fixed entry's read path still works after all three closures.
    let stores = Stores::new(fixture.settings()).unwrap();
    assert!(stores
        .workspace
        .read_publication(&request(&stores), APP, PUBLICATION)
        .await
        .is_ok());
    stores.close().await.unwrap();
    assert_eq!(pool::slots().0, 0);
    println!(
        "APP075_CLEANUP_RECEIPT {}",
        json!({"server_version":fixture.server_version,"cases":receipt})
    );
}
