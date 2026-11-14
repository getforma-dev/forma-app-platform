//! Isolated test executable only: synthetic identities, local Docker and a
//! throwaway CA. This module is absent from production artifacts.
use super::{
    authorization::{Resource, Transaction},
    pool::{self, Lease, Settings},
    qualification_cases::{
        self as cases, check, pid, APP, COLLECTION, ORG, OTHER, PRINCIPAL, PUBLICATION, RECORD,
        WORK,
    },
    *,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant as WallInstant},
};
use tokio::time::{timeout, Instant};

pub(super) const IMAGE: &str =
    "postgres@sha256:5db6dac2ef5dcff512f4f00a009c3c332b9f2f28f24277175bf3e4a3d02ef83a";

fn command(mut command: Command, input: &[u8], label: &str) -> String {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("fixture command available");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input)
        .expect("fixture input accepted");
    let end = WallInstant::now() + Duration::from_secs(45);
    while child.try_wait().expect("fixture child status").is_none() {
        if WallInstant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("fixture command deadline: {label}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().expect("fixture output");
    assert!(output.status.success(), "fixture command refused: {label}");
    String::from_utf8(output.stdout).expect("fixture UTF-8 output")
}
pub(crate) fn docker(args: &[&str], input: &[u8], label: &str) -> String {
    let mut cmd = Command::new("docker");
    cmd.args(args);
    command(cmd, input, label)
}
fn private_write(path: &Path, bytes: &[u8]) {
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = path.with_extension("new");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    drop(file);
    fs::rename(temporary, path).unwrap();
}

/// The locked Python environment used for migrations, seed and the operator
/// command; overridable for a checkout whose venv lives elsewhere.
pub(crate) fn fixture_python() -> PathBuf {
    std::env::var_os("GENUI_NATIVE_FIXTURE_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime/.venv/bin/python")
        })
}

pub(crate) struct PrivateDirectory(pub PathBuf);
impl PrivateDirectory {
    pub fn new() -> Self {
        let nonce = ring::rand::SystemRandom::new();
        let mut bytes = [0u8; 12];
        ring::rand::SecureRandom::fill(&nonce, &mut bytes).unwrap();
        let suffix: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let base = std::env::temp_dir().canonicalize().unwrap();
        let path = base.join(format!("genui-native-{suffix}"));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) struct Fixture {
    pub directory: PrivateDirectory,
    pub name: String,
    pub port: u16,
    credentials: PathBuf,
    registry: PathBuf,
    pub ca: PathBuf,
    pub removed: bool,
    pub server_version: String,
}
pub(crate) struct Operator {
    pub client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<()>,
}
impl Drop for Operator {
    fn drop(&mut self) {
        self.driver.abort();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        if let Ok(mut child) = Command::new("docker")
            .args(["rm", "--force", "--volumes", &self.name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            let end = WallInstant::now() + Duration::from_secs(5);
            while matches!(child.try_wait(), Ok(None)) {
                if WallInstant::now() >= end {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
impl Fixture {
    pub fn new() -> Self {
        let directory = PrivateDirectory::new();
        let name = directory
            .0
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let mut fixture = Self {
            credentials: directory.0.join("credentials.json"),
            registry: directory.0.join("registry.json"),
            ca: directory.0.join("ca.pem"),
            directory,
            name,
            port: 0,
            removed: false,
            server_version: String::new(),
        };
        let openssl = |args: &[&str]| {
            let mut cmd = Command::new("openssl");
            cmd.current_dir(&fixture.directory.0).args(args);
            command(cmd, b"", "local TLS material");
        };
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-days",
            "1",
            "-subj",
            "/CN=APP008 synthetic CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ]);
        openssl(&[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
            "-subj",
            "/CN=localhost",
        ]);
        fs::write(fixture.directory.0.join("server.ext"),"subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
        openssl(&[
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "server.pem",
            "-days",
            "1",
            "-extfile",
            "server.ext",
        ]);
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "wrong.key",
            "-out",
            "wrong.pem",
            "-days",
            "1",
            "-subj",
            "/CN=APP008 wrong CA",
        ]);
        docker(
            &[
                "run",
                "--detach",
                "--rm",
                "--name",
                &fixture.name,
                "--publish",
                "127.0.0.1::5432",
                "--memory=512m",
                "--cpus=1",
                "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m",
                "--env=POSTGRES_HOST_AUTH_METHOD=trust",
                IMAGE,
            ],
            b"",
            "start local PostgreSQL",
        );
        let address = docker(&["port", &fixture.name, "5432/tcp"], b"", "local port");
        fixture.port = address
            .trim()
            .strip_prefix("127.0.0.1:")
            .unwrap()
            .parse()
            .unwrap();
        let ready = WallInstant::now() + Duration::from_secs(20);
        loop {
            let status = Command::new("docker")
                .args([
                    "exec",
                    &fixture.name,
                    "pg_isready",
                    "--host=127.0.0.1",
                    "--username=postgres",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            if status.success() {
                break;
            }
            assert!(
                WallInstant::now() < ready,
                "local PostgreSQL ready deadline"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        fixture.server_version = fixture
            .sql("SHOW server_version", "postgres")
            .trim()
            .to_owned();
        assert_eq!(
            fixture.server_version.split_whitespace().next(),
            Some("16.15")
        );
        for file in ["server.key", "server.pem"] {
            docker(
                &[
                    "cp",
                    fixture.directory.0.join(file).to_str().unwrap(),
                    &format!("{}:/var/lib/postgresql/{file}", fixture.name),
                ],
                b"",
                "copy synthetic TLS file",
            );
        }
        docker(
            &[
                "exec",
                "--user=0",
                &fixture.name,
                "chown",
                "postgres:postgres",
                "/var/lib/postgresql/server.key",
                "/var/lib/postgresql/server.pem",
            ],
            b"",
            "TLS file ownership",
        );
        docker(
            &[
                "exec",
                "--user=0",
                &fixture.name,
                "chmod",
                "600",
                "/var/lib/postgresql/server.key",
            ],
            b"",
            "TLS key permissions",
        );
        fixture.sql("ALTER SYSTEM SET ssl='on'; ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'; ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'; SELECT pg_reload_conf();", "postgres");
        fixture.sql("CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS; CREATE ROLE rds_iam NOLOGIN; GRANT rds_iam TO genui_migration WITH ADMIN TRUE;", "postgres");
        fixture.sql("CREATE DATABASE genui OWNER genui_migration;", "postgres");
        let app = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = app.parent().unwrap();
        let script = "import sys\nfrom pathlib import Path\nimport psycopg\nimport runtime\nfrom tenant_seed import make_bundles,seed\nwith psycopg.connect(host='127.0.0.1',port=int(sys.argv[1]),user='genui_migration',dbname='genui',connect_timeout=3) as c:\n runtime.apply_migrations(c,Path(sys.argv[2]))\n seed(c,make_bundles())\n";
        let mut cmd = Command::new(fixture_python());
        cmd.current_dir(app.join("runtime")).args([
            "-c",
            script,
            &fixture.port.to_string(),
            app.join("migrations").to_str().unwrap(),
        ]);
        let contract_lib = repo.join("contracts/.venv/lib");
        if let Ok(entries) = fs::read_dir(contract_lib) {
            for entry in entries.flatten() {
                let site = entry.path().join("site-packages");
                if site.is_dir() {
                    cmd.env("PYTHONPATH", site);
                    break;
                }
            }
        }
        command(cmd, b"", "canonical migration and validated seed");
        // Local password authentication exercises distinct current role material;
        // it is explicitly not AWS IAM authentication.
        fixture.sql("ALTER ROLE postgres PASSWORD 'synthetic-postgres'; ALTER ROLE genui_workspace PASSWORD 'synthetic-workspace'; ALTER ROLE genui_control PASSWORD 'synthetic-control'; ALTER ROLE genui_data PASSWORD 'synthetic-data'; ALTER ROLE genui_worker PASSWORD 'synthetic-worker';", "genui");
        docker(&["exec","--user=postgres",&fixture.name,"sh","-c","printf 'local all all trust\nhost all all all scram-sha-256\n' > /var/lib/postgresql/data/pg_hba.conf"],b"","fixed local authentication");
        fixture.sql("SELECT pg_reload_conf();", "genui");
        fixture.write_credentials("localhost", 1, 600);
        private_write(&fixture.registry,&serde_json::to_vec(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000aaa","bindings":[{"issuer":"app008-fixture-only","provider_tenant_id":format!("ten_{ORG}"),"organization_id":ORG},{"issuer":"app008-fixture-only","provider_tenant_id":format!("ten_{OTHER}"),"organization_id":OTHER}]})).unwrap());
        fixture
    }
    pub fn sql(&self, sql: &str, database: &str) -> String {
        docker(
            &[
                "exec",
                "-i",
                &self.name,
                "psql",
                "--no-psqlrc",
                "--set=ON_ERROR_STOP=1",
                "--tuples-only",
                "--no-align",
                "--username=postgres",
                "--dbname",
                database,
                "--file=-",
            ],
            sql.as_bytes(),
            "fixture operator SQL",
        )
    }
    fn write_credentials(&self, host: &str, generation: u64, lifetime: u64) {
        let now = crate::config::now();
        let mut roles = serde_json::Map::new();
        for role in ["workspace", "control", "data", "worker"] {
            roles.insert(role.into(),json!({"username":format!("genui_{role}"),"token":format!("synthetic-{role}"),"expires_at":now+lifetime}));
        }
        private_write(&self.credentials,&serde_json::to_vec(&json!({"schema_version":1,"generation":format!("00000000-0000-4000-8000-{generation:012x}"),"issued_at":now,"expires_at":now+lifetime,"database":{"host":host,"name":"genui","port":5432,"roles":roles},"storage":{}})).unwrap());
    }
    /// APP-009 session campaign: replaces the fixture's private registry
    /// (synthetic `gatewasm` bindings); new `Stores` read it.
    pub fn write_registry(&self, document: &Value) {
        let _ = fs::remove_file(&self.registry);
        private_write(&self.registry, &serde_json::to_vec(document).unwrap());
    }
    pub fn settings(&self) -> Settings {
        self.settings_with_registry(self.registry.clone())
    }
    /// APP-078 B1: the same settings reading the identity registry the
    /// supervisor materialized from the auth configuration secret.
    pub fn settings_with_registry(&self, registry: PathBuf) -> Settings {
        Settings::fixture(
            self.credentials.clone(),
            registry,
            self.ca.clone(),
            "localhost",
            self.port,
        )
    }
    /// APP-012: the same settings against a closed loopback port, so every
    /// acquisition fails at connect: a store outage that depends on no
    /// identity.
    pub fn unreachable_settings(&self) -> Settings {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .unwrap()
            .port();
        Settings::fixture(
            self.credentials.clone(),
            self.registry.clone(),
            self.ca.clone(),
            "127.0.0.1",
            port,
        )
    }
    fn request(
        &self,
        stores: &Stores,
        organization: &str,
        principal: &str,
        epoch: i64,
    ) -> TrustedRequest {
        TrustedRequest {
            organization: organization.into(),
            principal: principal.into(),
            epoch,
            issuer: "app008-fixture-only".into(),
            provider_tenant: format!("ten_{organization}"),
            provider_subject: format!("usr_{principal}"),
            registry_sha256: stores.registry_identity().sha256.clone(),
        }
    }
    pub async fn operator(&self) -> Operator {
        let mut config = tokio_postgres::Config::new();
        config
            .host("127.0.0.1")
            .port(self.port)
            .dbname("genui")
            .user("postgres")
            .password("synthetic-postgres")
            .connect_timeout(Duration::from_secs(3));
        // Operator transport belongs only to this local test fixture. Production
        // role pools always use the verified TLS connector above.
        let (client, connection) = config.connect(tokio_postgres::NoTls).await.unwrap();
        let driver = tokio::spawn(async move {
            let _ = connection.await;
        });
        Operator { client, driver }
    }
}

async fn empty_slots() {
    timeout(Duration::from_secs(2), async {
        while pool::slots().0 != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn refuse_all_roles(stores: &Stores) {
    for pool in [
        &stores.workspace.inner,
        &stores.control.inner,
        &stores.data.inner,
        &stores.worker.inner,
    ] {
        assert!(Lease::acquire(pool).await.is_err());
    }
    assert!(stores.readiness().await.is_err());
}

#[test]
#[ignore = "requires local Docker, OpenSSL and the locked migration/contract Python environments"]
fn native_postgres_campaign() {
    // The qualification coordinates many deliberately paused futures. Reserve
    // its test-thread stack explicitly; production request tasks are unchanged.
    std::thread::Builder::new()
        .name("app008-native-campaign".into())
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
    let manifest: Value =
        serde_json::from_str(include_str!("../../migrations/manifest.json")).unwrap();
    let schema_version = manifest["database_version"].as_u64().unwrap();
    let unsupported_version = schema_version.checked_add(1).unwrap();
    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let stores = Arc::new(Stores::new(settings).unwrap());
    let request = fixture.request(&stores, ORG, PRINCIPAL, 1);
    let mut checks = Vec::new();
    cases::readiness(&stores, &mut checks).await;
    let positive = fixture.directory.0.join("synthetic-positive-helper.py");
    let negative = fixture.directory.0.join("synthetic-negative-helper.py");
    private_write(&positive,b"import hashlib,json,os,pathlib,time\nb=pathlib.Path(os.environ['GENUI_RUNTIME_CONFIG_FILE']).read_bytes()\nd=json.loads(b)\nprint(json.dumps({'generation':d['generation'],'file_sha256':hashlib.sha256(b).hexdigest(),'expires_at':d['expires_at'],'checked_at':int(time.time())}))\n");
    private_write(&negative, b"raise SystemExit(1)\n");
    let composed =
        crate::readiness::Readiness::isolated(fixture.credentials.clone(), stores.clone());
    assert!(!composed.qualify_once(&negative, "python3").await);
    assert!(composed.qualify_once(&positive, "python3").await);
    fixture.sql("ALTER ROLE genui_worker CREATEDB", "genui");
    assert!(!composed.qualify_once(&positive, "python3").await);
    fixture.sql("ALTER ROLE genui_worker NOCREATEDB", "genui");
    assert!(composed.qualify_once(&positive, "python3").await);
    fixture.write_credentials("localhost", 100, 600);
    assert!(!composed.ready());
    assert!(composed.qualify_once(&positive, "python3").await);
    check(&mut checks,"composed readiness requires actual native success plus synthetic helper success and invalidates cache on generation change");
    cases::read_only_and_reuse(
        &stores,
        |organization, principal, epoch| fixture.request(&stores, organization, principal, epoch),
        &mut checks,
    )
    .await;
    for phase in ["recycle", "begin", "finish", "reset"] {
        let lease = Lease::acquire(&stores.data.inner).await.unwrap();
        let before = pid(&lease).await;
        lease
            .finish(false, Instant::now() + Duration::from_secs(3))
            .await
            .unwrap();
        hooks.arm(phase);
        {
            let operation = async {
                let tx = Transaction::begin(&stores.data.inner, &request)
                    .await
                    .unwrap();
                tx.finish(true).await.unwrap();
            };
            tokio::pin!(operation);
            tokio::select! {_ = &mut operation => panic!("test barrier not reached"), _ = hooks.reached.notified() => {}}
        } // Drop the whole in-flight future while the lifecycle barrier is held.
        let lease = Lease::acquire(&stores.data.inner).await.unwrap();
        assert_ne!(pid(&lease).await, before);
        lease
            .finish(false, Instant::now() + Duration::from_secs(3))
            .await
            .unwrap();
        check(
            &mut checks,
            &format!("cancellation at {phase} discards physical backend"),
        );
    }
    stores.data.inner.pool.resize(0);
    stores.data.inner.pool.resize(2);
    hooks.arm("connect");
    {
        let operation = Lease::acquire(&stores.data.inner);
        tokio::pin!(operation);
        tokio::select! {_=&mut operation=>panic!("connect barrier not reached"),_=hooks.reached.notified()=>{}}
        assert_eq!(pool::slots().1[2], 1);
    }
    timeout(Duration::from_secs(2), async {
        while pool::slots().1[2] != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    check(
        &mut checks,
        "cancelled connect retains capacity until owned transport destruction",
    );
    let tx = Transaction::begin(&stores.data.inner, &request)
        .await
        .unwrap();
    let before = pid(&tx.lease).await;
    assert!(timeout(
        Duration::from_millis(100),
        tx.query("SELECT pg_sleep(1)", &[])
    )
    .await
    .is_err());
    drop(tx);
    let lease = Lease::acquire(&stores.data.inner).await.unwrap();
    assert_ne!(pid(&lease).await, before);
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    check(
        &mut checks,
        "cancellation during real PostgreSQL query discards backend",
    );
    let tx = Transaction::begin(&stores.data.inner, &request)
        .await
        .unwrap();
    let before = pid(&tx.lease).await;
    assert_eq!(
        tx.lease
            .finish(true, Instant::now() - Duration::from_secs(1))
            .await,
        Err(Error::UnknownOutcome)
    );
    let lease = Lease::acquire(&stores.data.inner).await.unwrap();
    assert_ne!(pid(&lease).await, before);
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    check(
        &mut checks,
        "expired commit deadline gives unknown outcome and discards without retry",
    );
    hooks.arm("reset");
    let tx = Transaction::begin(&stores.data.inner, &request)
        .await
        .unwrap();
    let before = pid(&tx.lease).await;
    assert_eq!(tx.finish(true).await, Err(Error::Deadline));
    let lease = Lease::acquire(&stores.data.inner).await.unwrap();
    assert_ne!(pid(&lease).await, before);
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    check(
        &mut checks,
        "two-second cleanup deadline discards uncertain reset",
    );
    let lease = Lease::acquire(&stores.data.inner).await.unwrap();
    let before = pid(&lease).await;
    fixture.write_credentials("localhost", 2, 600);
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    let lease = Lease::acquire(&stores.data.inner).await.unwrap();
    assert_ne!(pid(&lease).await, before);
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    fixture.write_credentials("localhost", 3, 0);
    assert!(matches!(
        Lease::acquire(&stores.data.inner).await,
        Err(Error::Configuration)
    ));
    fixture.write_credentials("localhost", 4, 600);
    check(
        &mut checks,
        "generation rotation retires old leases and expired credentials refuse checkout",
    );
    // Permission changes follow the same authority fence as the application.
    let mutation=format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' FOR UPDATE; DELETE FROM genui.source_permissions WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'; UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'; COMMIT;");
    fixture.sql(&mutation, "genui");
    let epoch2 = fixture.request(&stores, ORG, PRINCIPAL, 2);
    assert_eq!(
        stores
            .data
            .read_record(&epoch2, APP, WORK, COLLECTION, RECORD)
            .await,
        Err(Error::Denied)
    );
    check(
        &mut checks,
        "current source visibility is required in addition to record and run permissions",
    );
    // Restoration is confined to this disposable fixture and preserves the
    // same writer ordering; the resulting epoch is a fresh trusted fixture.
    fixture.sql(&format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' FOR UPDATE; INSERT INTO genui.source_permissions SELECT organization_id,'{PRINCIPAL}'::uuid,source_id,'source_read' FROM genui.sources WHERE organization_id='{ORG}'; UPDATE genui.memberships SET membership_epoch=3 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'; COMMIT;"),"genui");
    let epoch3 = fixture.request(&stores, ORG, PRINCIPAL, 3);
    assert!(stores
        .data
        .read_record(&epoch3, APP, WORK, COLLECTION, RECORD)
        .await
        .is_ok());
    fixture.sql(
        &format!(
            "UPDATE genui.source_revisions SET deleted_at=now() WHERE organization_id='{ORG}'"
        ),
        "genui",
    );
    assert_eq!(
        stores
            .data
            .read_record(&epoch3, APP, WORK, COLLECTION, RECORD)
            .await,
        Err(Error::Denied)
    );
    fixture.sql(
        &format!("UPDATE genui.source_revisions SET deleted_at=NULL WHERE organization_id='{ORG}'"),
        "genui",
    );
    check(
        &mut checks,
        "tombstoned source dependencies deny bounded record reads",
    );
    let writer = fixture.operator().await;
    let writer_prefix=format!("BEGIN; SELECT 1 FROM genui.organizations WHERE organization_id='{ORG}' FOR SHARE; SELECT 1 FROM genui.memberships WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}' FOR UPDATE;");
    writer.client.batch_execute(&format!("{writer_prefix} UPDATE genui.memberships SET status='revoked',membership_epoch=4 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}';")).await.unwrap();
    let mut tx = Transaction::begin(&stores.data.inner, &epoch3)
        .await
        .unwrap();
    let pre = tx
        .precheck(
            Action::RecordRead,
            Resource::Work {
                app: APP,
                work: WORK,
            },
        )
        .await
        .unwrap();
    {
        let fence = tx.fence(pre);
        tokio::pin!(fence);
        tokio::select! {_=&mut fence=>panic!("membership fence bypassed writer"),_=tokio::time::sleep(Duration::from_millis(75))=>{}}
        writer.client.batch_execute("COMMIT").await.unwrap();
        assert!(matches!(fence.await, Err(Error::Denied)));
    }
    tx.finish(false).await.unwrap();
    check(
        &mut checks,
        "writer-first revocation blocks then invalidates the prior nonfinal precheck",
    );
    writer.client.batch_execute(&format!("{writer_prefix} UPDATE genui.memberships SET status='active',membership_epoch=5 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'; COMMIT;")).await.unwrap();
    let epoch5 = fixture.request(&stores, ORG, PRINCIPAL, 5);
    let mut tx = Transaction::begin(&stores.data.inner, &epoch5)
        .await
        .unwrap();
    let pre = tx
        .precheck(
            Action::RecordRead,
            Resource::Work {
                app: APP,
                work: WORK,
            },
        )
        .await
        .unwrap();
    let _permit = tx.fence(pre).await.unwrap();
    {
        let sql=format!("{writer_prefix} UPDATE genui.memberships SET status='revoked',membership_epoch=6 WHERE organization_id='{ORG}' AND principal_id='{PRINCIPAL}'; COMMIT;");
        let revoke = writer.client.batch_execute(&sql);
        tokio::pin!(revoke);
        tokio::select! {_=&mut revoke=>panic!("writer bypassed transaction permit"),_=tokio::time::sleep(Duration::from_millis(75))=>{}}
        tx.finish(true).await.unwrap();
        timeout(Duration::from_secs(1), revoke)
            .await
            .unwrap()
            .unwrap();
    }
    assert_eq!(
        stores
            .data
            .read_record(&epoch5, APP, WORK, COLLECTION, RECORD)
            .await,
        Err(Error::Denied)
    );
    drop(writer);
    check(
        &mut checks,
        "operation-first authority fence holds until commit then subsequent stale request denies",
    );
    // Connecting and live capacity is shared by all Stores and four roles.
    let mut held = Vec::new();
    let mut pids = std::collections::HashSet::new();
    for pool in [
        &stores.workspace.inner,
        &stores.control.inner,
        &stores.data.inner,
        &stores.worker.inner,
    ] {
        for _ in 0..2 {
            let lease = Lease::acquire(pool).await.unwrap();
            pids.insert(pid(&lease).await);
            held.push(lease);
        }
    }
    assert_eq!(pids.len(), 8);
    assert_eq!(pool::slots(), (8, [2, 2, 2, 2]));
    // APP-195: two running and thirty waiting are admitted; the 33rd caller
    // is refused at once, without waiting.
    let mut waiters = Vec::new();
    for _ in 0..30 {
        let pool = stores.data.inner.clone();
        waiters.push(tokio::spawn(async move { Lease::acquire(&pool).await }));
    }
    timeout(Duration::from_secs(1), async {
        while stores.data.inner.pool.status().waiting != 30 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let refused_at = WallInstant::now();
    assert!(matches!(
        Lease::acquire(&stores.data.inner).await,
        Err(Error::Unavailable)
    ));
    assert!(refused_at.elapsed() < Duration::from_millis(100));
    assert_eq!(pool::slots().0, 8);
    for waiter in waiters {
        waiter.abort();
        assert!(waiter.await.is_err());
    }
    let start = WallInstant::now();
    assert!(matches!(
        Lease::acquire(&stores.data.inner).await,
        Err(Error::Deadline)
    ));
    assert!(start.elapsed() < Duration::from_millis(5500));
    assert_eq!(pool::slots().0, 8);
    drop(held);
    check(&mut checks,"eight physical connections, two per role, thirty bounded waiters (the 33rd caller refused at once) and absolute acquisition deadline");
    stores.close().await.unwrap();
    empty_slots().await;
    check(
        &mut checks,
        "closed pools terminate all eight-capacity transport owners",
    );
    let settings = fixture.settings();
    let hooks = settings.hooks.clone();
    let retry = Stores::new(settings).unwrap();
    hooks.arm("connect");
    let start = WallInstant::now();
    {
        let acquire = Lease::acquire(&retry.workspace.inner);
        tokio::pin!(acquire);
        tokio::select! {_=&mut acquire=>panic!("connect pause missing"),_=hooks.reached.notified()=>{}}
        fixture.write_credentials("localhost", 8, 600);
        hooks.arm("connect");
        hooks.release.notify_one();
        tokio::select! {_=&mut acquire=>panic!("one changed-generation retry missing"),_=hooks.reached.notified()=>{}}
        assert!(matches!(acquire.await, Err(Error::Deadline)));
    }
    assert!(start.elapsed() < Duration::from_millis(5500));
    retry.close().await.unwrap();
    check(
        &mut checks,
        "changed-generation connection retry shares the original five-second deadline",
    );
    private_write(&fixture.registry,&serde_json::to_vec(&json!({"schema_version":1,"registry_version":"00000000-0000-4000-8000-000000000bbb","bindings":[]})).unwrap());
    let settings = fixture.settings();
    let readiness_hooks = settings.hooks.clone();
    let empty = Stores::new(settings).unwrap();
    empty.readiness().await.unwrap();
    let fabricated = fixture.request(&empty, ORG, PRINCIPAL, 6);
    assert_eq!(
        empty
            .workspace
            .read_publication(&fabricated, APP, PUBLICATION)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        empty
            .control
            .read_publication(&fabricated, APP, PUBLICATION)
            .await,
        Err(Error::Denied)
    );
    assert_eq!(
        empty
            .data
            .read_record(&fabricated, APP, WORK, COLLECTION, RECORD)
            .await,
        Err(Error::Denied)
    );
    fixture.write_credentials("localhost", 9, 0);
    assert_eq!(empty.readiness().await, Err(Error::Configuration));
    fixture.write_credentials("localhost", 10, 600);
    let saved = fixture.credentials.with_extension("saved");
    fs::rename(&fixture.credentials, &saved).unwrap();
    assert_eq!(empty.readiness().await, Err(Error::Configuration));
    fs::rename(saved, &fixture.credentials).unwrap();
    empty.readiness().await.unwrap();
    fixture.sql("ALTER ROLE genui_worker CREATEDB", "genui");
    assert!(empty.readiness().await.is_err());
    fixture.sql("ALTER ROLE genui_worker NOCREATEDB", "genui");
    empty.readiness().await.unwrap();
    for (mutate, restore) in [
        (
            "ALTER TABLE genui.internal_records DISABLE ROW LEVEL SECURITY",
            "ALTER TABLE genui.internal_records ENABLE ROW LEVEL SECURITY",
        ),
        // APP-013 (0007) grants genui_data UPDATE(current_version) on
        // internal_records, so the extra table-level UPDATE probe moves to a
        // relation the data role never writes.
        (
            "GRANT UPDATE ON genui.publications TO genui_data",
            "REVOKE UPDATE ON genui.publications FROM genui_data",
        ),
        (
            "ALTER TABLE genui.definition_inputs RENAME TO fixture_hidden_inputs",
            "ALTER TABLE genui.fixture_hidden_inputs RENAME TO definition_inputs",
        ),
    ] {
        fixture.sql(mutate, "genui");
        assert!(empty.readiness().await.is_err());
        fixture.sql(restore, "genui");
        empty.readiness().await.unwrap();
    }
    check(&mut checks,"canonical catalog comparison rejects disabled RLS, extra UPDATE and missing named relation before reuse");
    readiness_hooks.arm("recycle");
    let started = WallInstant::now();
    assert_eq!(empty.readiness().await, Err(Error::Deadline));
    assert!(started.elapsed() < Duration::from_millis(5500));
    empty.readiness().await.unwrap();
    empty.close().await.unwrap();
    check(&mut checks,"empty registry stays deny-all while fixed-role readiness detects expiry, role drift and aggregate timeout");
    let identity = Stores::new(fixture.settings()).unwrap();
    fixture.sql("ALTER ROLE genui_worker CREATEDB", "genui");
    assert!(Lease::acquire(&identity.worker.inner).await.is_err());
    fixture.sql("ALTER ROLE genui_worker NOCREATEDB", "genui");
    fixture.sql("GRANT genui_owner TO genui_worker", "genui");
    assert!(Lease::acquire(&identity.worker.inner).await.is_err());
    fixture.sql("REVOKE genui_owner FROM genui_worker", "genui");
    fixture.sql(&format!("ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check; UPDATE genui_meta.schema_version SET version={unsupported_version};"),"genui");
    assert!(Lease::acquire(&identity.worker.inner).await.is_err());
    fixture.sql(&format!("UPDATE genui_meta.schema_version SET version={schema_version}; ALTER TABLE genui_meta.schema_version ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND {schema_version});"),"genui");
    let lease = Lease::acquire(&identity.worker.inner).await.unwrap();
    drop(lease);
    identity.close().await.unwrap();
    check(
        &mut checks,
        "elevated flags, unexpected memberships and unsupported schema refuse new connections",
    );
    // R4: structural drift is refused before any app relation can execute.
    // These operator-installed traps are confined to this disposable fixture;
    // they do not assert that ordinary application roles have DDL authority.
    let metadata = Stores::new(fixture.settings()).unwrap();
    metadata.readiness().await.unwrap();
    fixture.sql(
        "CREATE SCHEMA reviewer_probe; CREATE TABLE reviewer_probe.marker (value integer); \
         CREATE FUNCTION reviewer_probe.mark() RETURNS integer LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN INSERT INTO reviewer_probe.marker VALUES (1); RETURN 2; END $$; \
         ALTER TABLE genui_meta.schema_version RENAME TO original_schema_version; \
         CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,reviewer_probe.mark() AS version; \
         GRANT SELECT ON genui_meta.schema_version TO genui_workspace,genui_control,genui_data,genui_worker; \
         SET ROLE genui_worker; SELECT version FROM genui_meta.schema_version WHERE singleton; RESET ROLE;",
        "genui",
    );
    assert_eq!(
        fixture
            .sql("SELECT count(*) FROM reviewer_probe.marker", "genui")
            .trim(),
        "1"
    );
    fixture.sql("TRUNCATE reviewer_probe.marker", "genui");
    refuse_all_roles(&metadata).await;
    metadata.close().await.unwrap();
    let metadata_fresh = Stores::new(fixture.settings()).unwrap();
    refuse_all_roles(&metadata_fresh).await;
    assert_eq!(
        fixture
            .sql("SELECT count(*) FROM reviewer_probe.marker", "genui")
            .trim(),
        "0"
    );
    fixture.sql(
        "DROP VIEW genui_meta.schema_version; ALTER TABLE genui_meta.original_schema_version RENAME TO schema_version; DROP SCHEMA reviewer_probe CASCADE",
        "genui",
    );
    metadata_fresh.readiness().await.unwrap();
    metadata_fresh.close().await.unwrap();
    check(&mut checks,"replaced metadata view is refused on new and reused TLS connections with zero marker execution and restored readiness");

    let shadow = Stores::new(fixture.settings()).unwrap();
    shadow.readiness().await.unwrap();
    // Hold one clean object only in this test to model a corrupted idle path.
    let poisoned = shadow.worker.inner.pool.get().await.unwrap();
    fixture.sql(
        "CREATE SCHEMA reviewer_probe; CREATE TABLE reviewer_probe.marker (value integer); \
         CREATE FUNCTION reviewer_probe.mark() RETURNS boolean LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path=pg_catalog AS $$ BEGIN INSERT INTO reviewer_probe.marker VALUES (1); RETURN true; END $$; \
         CREATE VIEW reviewer_probe.pg_roles AS SELECT r.* FROM pg_catalog.pg_roles r WHERE reviewer_probe.mark(); \
         GRANT USAGE ON SCHEMA reviewer_probe TO genui_workspace,genui_control,genui_data,genui_worker; \
         GRANT SELECT ON reviewer_probe.pg_roles TO genui_workspace,genui_control,genui_data,genui_worker; \
         SET ROLE genui_worker; SET search_path=reviewer_probe,pg_catalog; SELECT count(*) FROM pg_roles WHERE rolname=current_user; RESET ROLE; RESET search_path; \
         ALTER DATABASE genui SET search_path=reviewer_probe,pg_catalog;",
        "genui",
    );
    let control_count: u64 = fixture
        .sql("SELECT count(*) FROM reviewer_probe.marker", "genui")
        .trim()
        .parse()
        .unwrap();
    assert!(control_count > 0);
    fixture.sql("TRUNCATE reviewer_probe.marker", "genui");
    poisoned
        .client
        .batch_execute("SET search_path=reviewer_probe,pg_catalog")
        .await
        .unwrap();
    drop(poisoned);
    refuse_all_roles(&shadow).await;
    shadow.close().await.unwrap();
    let shadow_fresh = Stores::new(fixture.settings()).unwrap();
    refuse_all_roles(&shadow_fresh).await;
    assert_eq!(
        fixture
            .sql("SELECT count(*) FROM reviewer_probe.marker", "genui")
            .trim(),
        "0"
    );
    fixture.sql(
        "ALTER DATABASE genui RESET search_path; DROP SCHEMA reviewer_probe CASCADE",
        "genui",
    );
    shadow_fresh.readiness().await.unwrap();
    shadow_fresh.close().await.unwrap();
    check(&mut checks,"startup and idle search-path shadowing is refused on actual TLS connections with zero marker execution and restored readiness");

    let version = Stores::new(fixture.settings()).unwrap();
    version.readiness().await.unwrap();
    fixture.sql("UPDATE genui_meta.schema_version SET version=1", "genui");
    // The canonical catalog is structural; version 1 is legal row data under
    // unchanged current constraints, so this exercises the post-catalog read.
    let operator = fixture.operator().await;
    super::catalog::Catalog::embedded()
        .unwrap()
        .verify(&operator.client)
        .await
        .unwrap();
    refuse_all_roles(&version).await;
    version.close().await.unwrap();
    let version_fresh = Stores::new(fixture.settings()).unwrap();
    refuse_all_roles(&version_fresh).await;
    fixture.sql(
        &format!("UPDATE genui_meta.schema_version SET version={schema_version}"),
        "genui",
    );
    version_fresh.readiness().await.unwrap();
    version_fresh.close().await.unwrap();
    drop(operator);
    check(&mut checks,"legal version-one row with unchanged current catalog refuses new connections, recycle and readiness until current-version restoration");

    // pg_temp is implicitly searched before relations unless listed explicitly.
    // A temporary sequence can observe evaluation even in READ ONLY, so this
    // control proves the native catalog transaction alone is not the defense.
    let temporary = Stores::new(fixture.settings()).unwrap();
    temporary.readiness().await.unwrap();
    let idle = temporary.workspace.inner.pool.get().await.unwrap();
    assert!(idle
        .client
        .batch_execute("CREATE TEMP SEQUENCE forbidden_marker")
        .await
        .is_err());
    fixture.sql("GRANT TEMP ON DATABASE genui TO genui_workspace", "genui");
    idle.client.batch_execute(
        "CREATE TEMP SEQUENCE catalog_marker; CREATE TEMP VIEW pg_namespace AS \
         SELECT n.* FROM pg_catalog.pg_namespace n WHERE pg_catalog.nextval('pg_temp.catalog_marker') > 0"
    ).await.unwrap();
    fixture.sql(
        "REVOKE TEMP ON DATABASE genui FROM genui_workspace",
        "genui",
    );
    let query: Value =
        serde_json::from_str(include_str!("../../migrations/catalog-query.json")).unwrap();
    idle.client
        .batch_execute("BEGIN READ ONLY; SET LOCAL search_path=pg_catalog")
        .await
        .unwrap();
    idle.client
        .query_one(query["sql"].as_str().unwrap(), &[])
        .await
        .unwrap();
    idle.client.batch_execute("ROLLBACK").await.unwrap();
    assert!(idle
        .client
        .query_one("SELECT is_called FROM pg_temp.catalog_marker", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    idle.client
        .query_one(
            "SELECT pg_catalog.setval('pg_temp.catalog_marker',1,false)",
            &[],
        )
        .await
        .unwrap();
    super::catalog::Catalog::embedded()
        .unwrap()
        .verify(&idle.client)
        .await
        .unwrap();
    assert!(!idle
        .client
        .query_one("SELECT is_called FROM pg_temp.catalog_marker", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    let old_pid: i32 = idle
        .client
        .query_one("SELECT pg_catalog.pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    drop(idle);
    let lease = Lease::acquire(&temporary.workspace.inner).await.unwrap();
    assert_eq!(pid(&lease).await, old_pid);
    assert!(!lease
        .client()
        .query_one("SELECT is_called FROM pg_temp.catalog_marker", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    let lease = Lease::acquire(&temporary.workspace.inner).await.unwrap();
    assert_eq!(pid(&lease).await, old_pid);
    assert_eq!(
        lease
            .client()
            .query_one(
                "SELECT pg_catalog.to_regclass('pg_temp.catalog_marker')::pg_catalog.text",
                &[]
            )
            .await
            .unwrap()
            .get::<_, Option<String>>(0),
        None
    );
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    temporary.readiness().await.unwrap();
    temporary.close().await.unwrap();
    check(&mut checks,"temporary catalog shadow executes under old READ ONLY path but explicit pg_temp-last admission and recycle keep marker untouched before reset removes it");
    // R3: a committed all-role database default affects new physical sessions
    // without altering trigger catalog flags. Never repair it from the app.
    fixture.sql(
        "ALTER DATABASE genui SET session_replication_role=replica",
        "genui",
    );
    let replication = Stores::new(fixture.settings()).unwrap();
    for (role, pool) in [
        ("genui_workspace", &replication.workspace.inner),
        ("genui_control", &replication.control.inner),
        ("genui_data", &replication.data.inner),
        ("genui_worker", &replication.worker.inner),
    ] {
        let observed = docker(
            &[
                "exec",
                &fixture.name,
                "psql",
                "--no-psqlrc",
                "--tuples-only",
                "--no-align",
                "--username",
                role,
                "--dbname=genui",
                "--command=SHOW session_replication_role",
            ],
            b"",
            "observe inherited replication mode",
        );
        assert_eq!(observed.trim(), "replica");
        assert!(Lease::acquire(pool).await.is_err());
    }
    assert!(replication.readiness().await.is_err());
    fixture.sql(
        "ALTER DATABASE genui RESET session_replication_role",
        "genui",
    );
    replication.readiness().await.unwrap();
    check(&mut checks,"committed database replica default is observed by all four roles and refused until operator restoration");
    let lease = Lease::acquire(&replication.workspace.inner).await.unwrap();
    let poisoned_pid = pid(&lease).await;
    assert!(lease
        .client()
        .batch_execute("SET session_replication_role=replica")
        .await
        .is_err());
    fixture.sql(
        "GRANT SET ON PARAMETER session_replication_role TO genui_workspace",
        "genui",
    );
    lease
        .client()
        .batch_execute("SET session_replication_role=replica")
        .await
        .unwrap();
    assert_eq!(
        lease
            .client()
            .query_one("SHOW session_replication_role", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "replica"
    );
    fixture.sql(
        "REVOKE SET ON PARAMETER session_replication_role FROM genui_workspace",
        "genui",
    );
    assert_eq!(
        lease
            .finish(false, Instant::now() + Duration::from_secs(3))
            .await,
        Err(Error::Unavailable)
    );
    let lease = Lease::acquire(&replication.workspace.inner).await.unwrap();
    assert_ne!(pid(&lease).await, poisoned_pid);
    assert_eq!(
        lease
            .client()
            .query_one("SHOW session_replication_role", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "origin"
    );
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    check(&mut checks,"operator-enabled session replica drift is refused before reset and replaces the backend without app SET repair");
    // Test-only direct Deadpool access simulates idle corruption. Production
    // callers only hold the non-reusable Lease and cannot reach this seam.
    let idle = replication.workspace.inner.pool.get().await.unwrap();
    let idle_pid = idle
        .client
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get::<_, i32>(0);
    fixture.sql(
        "GRANT SET ON PARAMETER session_replication_role TO genui_workspace",
        "genui",
    );
    idle.client
        .batch_execute("SET session_replication_role=replica")
        .await
        .unwrap();
    fixture.sql(
        "REVOKE SET ON PARAMETER session_replication_role FROM genui_workspace",
        "genui",
    );
    drop(idle);
    let lease = Lease::acquire(&replication.workspace.inner).await.unwrap();
    assert_ne!(pid(&lease).await, idle_pid);
    assert_eq!(
        lease
            .client()
            .query_one("SHOW session_replication_role", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "origin"
    );
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    replication.close().await.unwrap();
    check(&mut checks,"replica-mode idle session is discarded during recycle before a new origin backend is admitted");
    let material = Stores::new(fixture.settings()).unwrap();
    let lease = Lease::acquire(&material.worker.inner).await.unwrap();
    let before = pid(&lease).await;
    fixture.sql(
        "ALTER ROLE genui_worker PASSWORD 'synthetic-worker-rotated'",
        "genui",
    );
    fixture.write_credentials("localhost", 11, 600);
    let synthetic = fs::read_to_string(&fixture.credentials)
        .unwrap()
        .replace("synthetic-worker", "synthetic-worker-rotated");
    private_write(&fixture.credentials, synthetic.as_bytes());
    lease
        .finish(false, Instant::now() + Duration::from_secs(3))
        .await
        .unwrap();
    let lease = Lease::acquire(&material.worker.inner).await.unwrap();
    assert_ne!(pid(&lease).await, before);
    drop(lease);
    material.close().await.unwrap();
    fixture.sql(
        "ALTER ROLE genui_worker PASSWORD 'synthetic-worker'",
        "genui",
    );
    fixture.write_credentials("localhost", 12, 600);
    check(
        &mut checks,
        "new physical connection authenticates with changed current role credential material",
    );
    fixture.write_credentials("127.0.0.1", 5, 600);
    let wrong_host = Stores::new(Settings::fixture(
        fixture.credentials.clone(),
        fixture.registry.clone(),
        fixture.ca.clone(),
        "127.0.0.1",
        fixture.port,
    ))
    .unwrap();
    assert!(Lease::acquire(&wrong_host.workspace.inner).await.is_err());
    wrong_host.close().await.unwrap();
    fixture.write_credentials("localhost", 6, 600);
    let wrong_ca = Stores::new(Settings::fixture(
        fixture.credentials.clone(),
        fixture.registry.clone(),
        fixture.directory.0.join("wrong.pem"),
        "localhost",
        fixture.port,
    ))
    .unwrap();
    assert!(Lease::acquire(&wrong_ca.workspace.inner).await.is_err());
    wrong_ca.close().await.unwrap();
    fixture.sql(
        "ALTER SYSTEM SET ssl='off'; SELECT pg_reload_conf();",
        "genui",
    );
    let no_tls = Stores::new(fixture.settings()).unwrap();
    assert!(Lease::acquire(&no_tls.workspace.inner).await.is_err());
    no_tls.close().await.unwrap();
    check(
        &mut checks,
        "wrong hostname, untrusted CA and plaintext PostgreSQL are refused",
    );
    docker(
        &["rm", "--force", "--volumes", &fixture.name],
        b"",
        "remove isolated database",
    );
    fixture.removed = true;
    fs::remove_dir_all(&fixture.directory.0).unwrap();
    assert!(!fixture.directory.0.exists());
    check(
        &mut checks,
        "local database and private fixture files removed",
    );
    if let Some(path) = std::env::var_os("GENUI_NATIVE_REPORT") {
        fs::write(path,serde_json::to_vec_pretty(&json!({"schema_version":1,"scope":"APP-008 isolated native PostgreSQL TLS qualification","managed_acceptance":false,"postgres_image":IMAGE,"server_version":fixture.server_version,"checks":checks,"passed":true,"limits":["Synthetic local password authentication is not RDS IAM.","Fixture identities do not establish APP-006/009/073/074 readiness.","Initial application access is read-only; business writes remain denied."]})).unwrap()).unwrap();
    }
}

// APP-195 `refresh-herd` campaign. It lives in this file because a new
// source file must be named in the qualification image's context, and that
// context is part of the deployment contract digest (`ops/promotion`).
pub(super) mod refresh_herd {
    //! APP-195 `refresh-herd` campaign. Isolated test executable only: local
    //! Docker PostgreSQL, OpenSSL, the locked runtime venv (the confined query
    //! child) and minted workspace sessions over the composed HTTP routes, as the
    //! APP-013 command campaign drives them (`oneshot` through the router).
    //!
    //! One owner and 25 members of one organization have one shared app open.
    //! The owner saves one change to its one-row shared list; every member's
    //! event stream gets the hint, and the campaign replays what each of the 26
    //! browsers then reads: the work document (`GET /w/{app}/{work}` as JSON),
    //! the record components batch and the typed view. Each read is the real
    //! route over the real pool, storage, engine child and release checks.
    //!
    //! - Run A (the APP-195 client): each page starts after a uniform random
    //!   0–3 s from its hint (seeded), the page that committed reads at once, a
    //!   503 is read once more after a random 1–3 s, and hints for a page while
    //!   one waits join it. Three herds 10 s apart, with the pass criteria below.
    //! - Run B (the client before APP-195): every page reads at one instant after
    //!   its hint, no retry. Measured only.
    //! - Run C: run B with every role admitting 8 callers (the pool before
    //!   APP-195). It must reproduce the APP-193 finding: at least ten 503s.
    //! - When 26 pages fail run A, run A with 20, 15, 10 and 5 pages, to the
    //!   first that passes; then run A's herds at shorter intervals for that
    //!   many pages (26 if run A passed), to the first cadence that fails.
    //! - Also measured alone: one page's three reads, one data-role
    //!   transaction, one catalog comparison, and the role transactions each
    //!   read takes.
    //!
    //! The share is the existing per-member share (`Workspace::share_work`, 25
    //! times, with record_write): an organization-wide share is not on this
    //! branch. The second list is shared too: per-person (`own`) lists are not on
    //! this branch either. Each page plays both the work page's read (the
    //! document) and the canvas board's (the batch and the view) at once, which
    //! is more than either page reads alone.
    use super::super::{
        admission_qualification::operator,
        catalog::Catalog,
        command_qualification::{json_headers, send, vault_token, Reply, TestPages},
        definition_qualification::{grant, request, POLICY},
        pool::{self, Role},
        qualification_cases::ORG,
        *,
    };
    use super::{fixture_python, Fixture, Operator};
    use crate::{
        admission::{self, digest},
        auth::{cookies, Auth, Pages},
        commands::{routes::Commands, RecordPage},
        events, http,
        query::{
            record_components::RecordExecution,
            routes::{tests::RecordingPages, Execution},
            tests::{real_engine, Directory},
            Engine, Executor, QueryError, Work,
        },
    };
    use axum::{
        body::{Body, BodyDataStream},
        extract::ConnectInfo,
        http::{header, Request, StatusCode},
        Router,
    };
    use serde_json::{json, Value};
    use std::{
        collections::BTreeMap,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };
    use tokio::time::{sleep, sleep_until, Instant};
    use tower::ServiceExt;

    /// tenant_seed.SPACE.
    const SPACE: &str = "00000000-0000-4000-8000-000000000384";
    const APP: &str = "00000000-0000-4000-8000-000001951000";
    const REVISION: &str = "00000000-0000-4000-8000-000001951001";
    const OWNER: &str = "00000000-0000-4000-8000-000001950000";
    const MEMBERS: usize = 25;
    /// The one shared row the owner changes.
    const ITEM: &str = "00000000-0000-4000-8000-000001952000";

    /// The APP-195 client (`app/admin/src/canvas/live-refresh.ts`).
    const SPREAD_MS: u64 = 3000;
    const RETRY_MIN_MS: u64 = 1000;
    const RETRY_MAX_MS: u64 = 3000;
    /// The canvas view's existing busy handling (`queries.ts`): a 429 waits its
    /// `Retry-After` and reads again, at most three times.
    const BUSY_RETRIES: usize = 3;
    /// Run A's pass criteria.
    const CURRENT_WITHIN: Duration = Duration::from_secs(6);
    const COMMAND_UNDER: Duration = Duration::from_millis(1500);
    const FIRST_TRY_SHARE: f64 = 0.90;

    fn member(index: usize) -> String {
        format!("00000000-0000-4000-8000-{:012x}", 0x195_0001 + index)
    }
    fn entry_id(n: usize) -> String {
        format!("00000000-0000-4000-8000-{:012x}", 0x195_3000 + n)
    }
    fn key(n: u64) -> String {
        format!("{:032x}", 0x195_0000_0000_u64 + n)
    }
    /// The shared row's text and number at herd `herd` (0 is the seed).
    fn label(herd: usize) -> String {
        format!("Item r{herd:03}")
    }
    fn number(herd: usize) -> i64 {
        7000 + herd as i64
    }

    fn definition() -> Value {
        let item_fields = json!([
            {"id": "label", "value_type": {"type": "text"}, "nullable": false},
            {"id": "number", "value_type": {"type": "integer", "unit": "count"}, "nullable": false},
            {"id": "state", "value_type": {"type": "enum", "values": ["open", "closed"]}, "nullable": false}
        ]);
        let entry_fields = json!([
            {"id": "item", "value_type": {"type": "integer", "unit": "count"}, "nullable": false},
            {"id": "choice", "value_type": {"type": "enum", "values": ["a", "b", "c", "d"]}, "nullable": false}
        ]);
        let content = json!({
            "title": "Shared item and entries",
            "description": "A shared current item the owner changes, the members' entries, their counts and a typed table.",
            "inputs": [
                {"id": "items", "contract_id": "item_v1", "contract_version": 1, "kind": "records", "fields": item_fields},
                {"id": "entries", "contract_id": "entry_v1", "contract_version": 1, "kind": "records", "fields": entry_fields}
            ],
            "views": [
                {"id": "current", "kind": "records_list", "renderer_version": 1, "title": "Current item", "records": {"version": 1, "input_id": "items", "schema_id": "item", "columns": ["number", "label", "state"]}},
                {"id": "total", "kind": "record_summary", "renderer_version": 1, "title": "Entries", "records": {"version": 1, "input_id": "entries", "schema_id": "entry", "measure": {"op": "count"}}},
                {"id": "by_choice", "kind": "record_ranking", "renderer_version": 1, "title": "By choice", "records": {"version": 1, "input_id": "entries", "schema_id": "entry", "measure": {"op": "count"}, "group_field": "choice", "limit": 4}},
                {"id": "by_item", "kind": "table", "renderer_version": 1, "title": "Items by number", "query": {"operator_version": 1, "inputs": ["items"], "operations": [{"type": "aggregate", "group_by": [{"input_id": "items", "field_id": "number"}, {"input_id": "items", "field_id": "state"}], "measures": [{"id": "n", "operation": "count", "field": {"input_id": "items", "field_id": "state"}}]}], "null_policy": "unknown"}},
                {"id": "answer_form", "kind": "form", "renderer_version": 1, "title": "Add an entry", "command_id": "answer"}
            ],
            "record_schemas": [
                {"id": "item", "version": 1, "fields": item_fields},
                {"id": "entry", "version": 1, "fields": entry_fields}
            ],
            "commands": [
                {"id": "post_item", "record_schema_id": "item", "operation": "create", "writable_fields": ["label", "number", "state"], "transitions": []},
                {"id": "advance", "record_schema_id": "item", "operation": "update", "writable_fields": ["label", "number", "state"], "transitions": []},
                {"id": "answer", "record_schema_id": "entry", "operation": "create", "writable_fields": ["item", "choice"], "transitions": []}
            ],
            "links": [],
            "default_layout": [
                {"view_id": "current", "column": 0, "row": 0, "width": 12},
                {"view_id": "answer_form", "column": 0, "row": 1, "width": 12},
                {"view_id": "total", "column": 0, "row": 2, "width": 4},
                {"view_id": "by_choice", "column": 4, "row": 2, "width": 8},
                {"view_id": "by_item", "column": 0, "row": 3, "width": 12}
            ]
        });
        let canonical = admission::encode(&content).unwrap();
        json!({
            "kind": "app_definition", "schema_version": 1, "organization_id": ORG, "app_id": APP,
            "revision_id": REVISION,
            "content": content,
            "digest": digest::digest(digest::APP_DEFINITION, &canonical),
        })
    }

    fn item_values(herd: usize) -> Value {
        json!([
            {"field_id": "label", "value": {"type": "text", "value": label(herd)}},
            {"field_id": "number", "value": {"type": "integer", "value": number(herd).to_string(), "unit": "count"}},
            {"field_id": "state", "value": {"type": "enum", "value": "open"}}
        ])
    }

    /// Fixture-only operator SQL (as tenant_seed writes accounts): an active
    /// member of the organization with a live entitlement.
    fn admit_member(fixture: &Fixture, principal: &str) {
        let tail = &principal[principal.len() - 12..];
        fixture.sql(&format!("BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','{ORG}',true); INSERT INTO genui.principals VALUES ('{ORG}','{principal}','app008-fixture-only','ten_{ORG}','usr_{principal}'); INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES ('{ORG}','{principal}','active',1); INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES ('{ORG}','{principal}','0000e000-0000-4000-8000-{tail}','active','member_invite',now()-interval '1 day',now()+interval '30 days'); COMMIT;"), "genui");
    }

    /// A deterministic uniform source (splitmix64), one per page and run.
    struct Seeded(u64);
    impl Seeded {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }
        /// Whole milliseconds in [from, to), as `live-refresh.ts` draws them.
        fn between(&mut self, from: u64, to: u64) -> Duration {
            Duration::from_millis(from + self.next() % (to - from))
        }
    }

    /// Counts the engine's answers per kind: the typed view (`execute`) and the
    /// record components (`execute_records`).
    #[derive(Default)]
    struct Tally {
        runs: AtomicUsize,
        ok: AtomicUsize,
        busy: AtomicUsize,
        expired: AtomicUsize,
        other: AtomicUsize,
    }
    impl Tally {
        fn note<T>(&self, result: &std::result::Result<T, QueryError>) {
            self.runs.fetch_add(1, Ordering::SeqCst);
            let counter = match result {
                Ok(_) => &self.ok,
                Err(QueryError::Busy) => &self.busy,
                Err(QueryError::Failed(crate::query::Failure::QueueExpired)) => &self.expired,
                Err(_) => &self.other,
            };
            counter.fetch_add(1, Ordering::SeqCst);
        }
        fn read(&self) -> [usize; 5] {
            [
                self.runs.load(Ordering::SeqCst),
                self.ok.load(Ordering::SeqCst),
                self.busy.load(Ordering::SeqCst),
                self.expired.load(Ordering::SeqCst),
                self.other.load(Ordering::SeqCst),
            ]
        }
    }
    fn tally_json(before: [usize; 5], after: [usize; 5]) -> Value {
        let d = |i: usize| after[i] - before[i];
        json!({"runs": d(0), "ok": d(1), "busy": d(2), "queue_expired": d(3), "other": d(4)})
    }
    struct Counted {
        engine: Engine,
        typed: Tally,
        records: Tally,
    }
    impl Executor for Counted {
        fn execute<'a>(&'a self, run: &'a QueryRun, deadline: Duration) -> Execution<'a> {
            Box::pin(async move {
                let result = Executor::execute(&self.engine, run, deadline).await;
                self.typed.note(&result);
                result
            })
        }
        fn execute_records<'a>(
            &'a self,
            run: &'a RecordComponentsRun,
            parameters: &'a Value,
            deadline: Duration,
        ) -> RecordExecution<'a> {
            Box::pin(async move {
                let result =
                    Executor::execute_records(&self.engine, run, parameters, deadline).await;
                self.records.note(&result);
                result
            })
        }
    }

    /// One page: a member's session, and what it saw.
    struct Page {
        index: usize,
        cookie: String,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
    enum Read {
        Document,
        Batch,
        View,
    }
    impl Read {
        fn name(self) -> &'static str {
            match self {
                Self::Document => "document",
                Self::Batch => "batch",
                Self::View => "view",
            }
        }
    }

    /// One read of one refresh: every attempt's status, and what the last
    /// answer showed.
    #[derive(Clone, Debug)]
    struct ReadResult {
        read: Read,
        statuses: Vec<u16>,
        /// The herd whose value the final answer showed (`None`: not shown).
        seen: Option<usize>,
        /// The document's own parts: views and record components not ready.
        degraded: Vec<String>,
    }
    impl ReadResult {
        fn first(&self) -> u16 {
            self.statuses[0]
        }
        fn last(&self) -> u16 {
            *self.statuses.last().unwrap()
        }
    }
    #[derive(Clone, Debug)]
    struct Refresh {
        page: usize,
        /// The herd whose hint (or commit) started it.
        herd: usize,
        committer: bool,
        started: Duration,
        ended: Duration,
        reads: Vec<ReadResult>,
    }
    impl Refresh {
        /// The herd this refresh shows on all three reads, if any.
        fn current(&self) -> Option<usize> {
            if self.reads.iter().any(|read| read.last() != 200) {
                return None;
            }
            self.reads.iter().map(|read| read.seen).min().flatten()
        }
    }

    /// What a run replays.
    #[derive(Clone, Copy)]
    struct Plan {
        name: &'static str,
        /// APP-195: each page starts 0–3 s after its hint; the committer at once.
        jitter: bool,
        /// APP-195: a 503 is read once more after 1–3 s.
        retry: bool,
        herds: usize,
        cadence: Duration,
        seed: u64,
        /// The first `pages` pages (the owner's first) have the work open.
        pages: usize,
    }

    /// The highest herd whose label or number `body` shows.
    fn seen_in(body: &str, herds: usize, read: Read) -> Option<usize> {
        (0..=herds).rev().find(|&herd| match read {
            Read::View => body.contains(&format!("\"{}\"", number(herd))),
            _ => body.contains(&label(herd)),
        })
    }

    /// The document's parts that are not ready: its views and record components.
    fn degraded(document: &Value) -> Vec<String> {
        let mut out = Vec::new();
        for view in document["views"].as_array().into_iter().flatten() {
            let state = view["state"].as_str().unwrap_or("");
            if state != "ready" && state != "empty" {
                out.push(format!(
                    "view:{}:{}:{}",
                    view["id"].as_str().unwrap_or(""),
                    state,
                    view["badge"].as_str().unwrap_or("")
                ));
            }
        }
        let mut stack = vec![document];
        while let Some(node) = stack.pop() {
            match node {
                Value::Object(map) => {
                    if map.contains_key("kind")
                        && map.contains_key("message")
                        && map.get("failed") == Some(&Value::Bool(true))
                    {
                        out.push(format!("component:{}", map["id"].as_str().unwrap_or("")));
                    }
                    stack.extend(map.values());
                }
                Value::Array(items) => stack.extend(items.iter()),
                _ => {}
            }
        }
        out.sort();
        out.dedup();
        out
    }

    struct Bench {
        app: Router,
        work: String,
        publication: String,
        pages: Vec<Page>,
        counted: Arc<Counted>,
    }

    impl Bench {
        async fn get_document(&self, page: &Page) -> Reply {
            send(
                &self.app,
                "GET",
                &format!("/w/{APP}/{}", self.work),
                Some(&page.cookie),
                "",
                &[("accept", "application/json")],
            )
            .await
        }
        async fn post_batch(&self, page: &Page) -> Reply {
            send(
                &self.app,
                "POST",
                &format!("/v1/work/{APP}/{}/record-components", self.work),
                Some(&page.cookie),
                "{\"filter_epoch\":0,\"filters\":[]}",
                &[
                    ("sec-fetch-site", "same-origin"),
                    ("content-type", "application/json"),
                ],
            )
            .await
        }
        async fn post_view(&self, page: &Page) -> Reply {
            send(
                &self.app,
                "POST",
                &format!("/v1/work/{APP}/{}/queries/by_item", self.work),
                Some(&page.cookie),
                "{\"parameters\":{}}",
                &[
                    ("sec-fetch-site", "same-origin"),
                    ("content-type", "application/json"),
                ],
            )
            .await
        }
        async fn attempt(&self, page: &Page, read: Read) -> Reply {
            match read {
                Read::Document => self.get_document(page).await,
                Read::Batch => self.post_batch(page).await,
                Read::View => self.post_view(page).await,
            }
        }
        /// One read as the page's client makes it: the view's existing 429
        /// handling, and with `retry` (APP-195) one more read after a 503.
        async fn read(
            &self,
            page: &Page,
            read: Read,
            retry: bool,
            random: &Mutex<Seeded>,
            herds: usize,
        ) -> ReadResult {
            let mut statuses = Vec::new();
            let mut busy = 0;
            let mut retried = false;
            let reply = loop {
                let reply = self.attempt(page, read).await;
                statuses.push(reply.status.as_u16());
                if read == Read::View
                    && reply.status == StatusCode::TOO_MANY_REQUESTS
                    && busy < BUSY_RETRIES
                {
                    busy += 1;
                    let wait = reply.headers[header::RETRY_AFTER]
                        .to_str()
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(2)
                        .clamp(1, 10);
                    sleep(Duration::from_secs(wait)).await;
                    continue;
                }
                if retry && !retried && reply.status == StatusCode::SERVICE_UNAVAILABLE {
                    retried = true;
                    let wait = random.lock().unwrap().between(RETRY_MIN_MS, RETRY_MAX_MS);
                    sleep(wait).await;
                    continue;
                }
                break reply;
            };
            let (seen, degraded) = if reply.status == StatusCode::OK {
                let parts = if read == Read::Document {
                    degraded(&reply.json())
                } else {
                    Vec::new()
                };
                (seen_in(&reply.body, herds, read), parts)
            } else {
                (None, Vec::new())
            };
            ReadResult {
                read,
                statuses,
                seen,
                degraded,
            }
        }
        /// One page's refresh: its three reads at once.
        async fn refresh(
            &self,
            page: &Page,
            retry: bool,
            random: &Mutex<Seeded>,
            herds: usize,
        ) -> Vec<ReadResult> {
            let (document, batch, view) = tokio::join!(
                self.read(page, Read::Document, retry, random, herds),
                self.read(page, Read::Batch, retry, random, herds),
                self.read(page, Read::View, retry, random, herds),
            );
            vec![document, batch, view]
        }
    }

    /// A page's event stream, read as a body.
    async fn open_events(app: &Router, cookie: &str) -> BodyDataStream {
        let mut request = Request::builder()
            .uri("/v1/events")
            .header(header::COOKIE, format!("{}={cookie}", cookies::SESSION))
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "192.0.2.40:4000".parse::<std::net::SocketAddr>().unwrap(),
        ));
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.into_body().into_data_stream()
    }
    /// The next non-comment frame, or `None` when the body ended.
    async fn frame(body: &mut BodyDataStream) -> Option<String> {
        loop {
            let next = std::future::poll_fn(|cx| {
                futures_core::Stream::poll_next(Pin::new(&mut *body), cx)
            })
            .await?;
            let text = String::from_utf8(next.ok()?.to_vec()).ok()?;
            if text.starts_with(':') {
                continue;
            }
            return Some(text);
        }
    }

    /// What one run observed.
    #[derive(Default)]
    struct Observed {
        /// Per herd: when the owner's command was sent, answered, and its status.
        commits: Vec<(Duration, Duration, u16)>,
        /// Per herd, per page: when its hint arrived.
        hints: Vec<BTreeMap<usize, Duration>>,
        refreshes: Vec<Refresh>,
    }

    fn millis(duration: Duration) -> f64 {
        (duration.as_secs_f64() * 10_000.0).round() / 10.0
    }
    fn spread(mut values: Vec<f64>) -> Value {
        if values.is_empty() {
            return json!({"n": 0});
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |p: f64| values[((values.len() as f64 - 1.0) * p).round() as usize];
        json!({"n": values.len(), "min_ms": values[0], "p50_ms": at(0.5), "p95_ms": at(0.95), "max_ms": values[values.len() - 1]})
    }

    /// The shared row's current version, read by the fixture superuser (no
    /// role pool), so a command whose answer was lost does not desynchronize.
    async fn item_version(operator: &Operator) -> i64 {
        operator
            .client
            .query_one(
                &format!("SELECT current_version::bigint FROM genui.internal_records WHERE organization_id='{ORG}' AND record_id='{ITEM}'"),
                &[],
            )
            .await
            .unwrap()
            .get(0)
    }

    async fn run(bench: &Arc<Bench>, plan: Plan, operator: &Operator) -> (Value, bool) {
        let saturated_before = pool::saturated();
        let typed_before = bench.counted.typed.read();
        let records_before = bench.counted.records.read();
        let origin = Instant::now() + Duration::from_millis(500);
        let observed = Arc::new(Mutex::new(Observed {
            hints: vec![BTreeMap::new(); plan.herds],
            ..Default::default()
        }));
        // Per herd: when the command was sent (a hint belongs to the latest herd
        // sent before it arrived) and when it was answered (the committer's echo).
        let sent: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
        let answered: Arc<Mutex<Vec<Option<Instant>>>> =
            Arc::new(Mutex::new(vec![None; plan.herds]));
        let mut tasks = tokio::task::JoinSet::new();
        for index in 0..plan.pages {
            let bench = bench.clone();
            let observed = observed.clone();
            let sent = sent.clone();
            let answered = answered.clone();
            let mut body = open_events(&bench.app, &bench.pages[index].cookie).await;
            tasks.spawn(async move {
                let random = Arc::new(Mutex::new(Seeded(
                    plan.seed.wrapping_mul(1000).wrapping_add(index as u64),
                )));
                // The page's coalescing (`hintSchedule` and the document's own):
                // one hint waits at a time, and refreshes run one after another.
                let waiting = Arc::new(Mutex::new(false));
                let serial = Arc::new(tokio::sync::Mutex::new(()));
                let mut refreshes = tokio::task::JoinSet::new();
                while let Some(text) = frame(&mut body).await {
                    if !text.contains("event: record.committed") || !text.contains(&bench.work) {
                        continue;
                    }
                    let now = Instant::now();
                    let Some(herd) = sent.lock().unwrap().iter().rposition(|at| *at <= now) else {
                        continue;
                    };
                    observed.lock().unwrap().hints[herd]
                        .entry(index)
                        .or_insert(now - origin);
                    let committer = index == 0;
                    let start = if plan.jitter {
                        // The committer's echo: a read that started after it
                        // arrived covers it (`hintSchedule.now`).
                        if committer && answered.lock().unwrap()[herd].is_none_or(|at| at >= now) {
                            continue;
                        }
                        if std::mem::replace(&mut *waiting.lock().unwrap(), true) {
                            continue;
                        }
                        now + random.lock().unwrap().between(0, SPREAD_MS)
                    } else {
                        // The client before APP-195: every page reads at once.
                        // A barrier: all at one instant after the hints arrive.
                        let barrier = sent.lock().unwrap()[herd] + Duration::from_millis(400);
                        barrier.max(now)
                    };
                    let (bench, observed, random, waiting, serial) = (
                        bench.clone(),
                        observed.clone(),
                        random.clone(),
                        waiting.clone(),
                        serial.clone(),
                    );
                    refreshes.spawn(async move {
                        sleep_until(start).await;
                        *waiting.lock().unwrap() = false;
                        let _turn = serial.lock().await;
                        let page = &bench.pages[index];
                        let started = Instant::now() - origin;
                        let reads = bench
                            .refresh(page, plan.retry, &random, global_herd(plan, plan.herds))
                            .await;
                        let ended = Instant::now() - origin;
                        observed.lock().unwrap().refreshes.push(Refresh {
                            page: index,
                            herd,
                            committer: false,
                            started,
                            ended,
                            reads,
                        });
                    });
                }
                while refreshes.join_next().await.is_some() {}
            });
        }
        // The owner commits each herd's change; with the APP-195 client its own
        // page reads at once after the answer.
        let mut committed = tokio::task::JoinSet::new();
        for herd in 0..plan.herds {
            sleep_until(origin + plan.cadence * herd as u32).await;
            let version = item_version(operator).await;
            let at = Instant::now();
            sent.lock().unwrap().push(at);
            let body = serde_json::to_string(&json!({
                "kind": "command_request", "schema_version": 1, "app_id": APP, "work_id": bench.work,
                "publication_id": bench.publication,
                "command_id": "advance", "schema_id": "item", "schema_version_expected": 1,
                "record_id": ITEM, "expected_record_version": version, "expected_work_version": 1,
                "values": item_values(global_herd(plan, herd)),
            }))
            .unwrap();
            let idempotency = key(plan.seed * 100 + herd as u64);
            let reply = send(
                &bench.app,
                "POST",
                &format!("/v1/work/{APP}/{}/commands/advance", bench.work),
                Some(&bench.pages[0].cookie),
                &body,
                &json_headers(&idempotency),
            )
            .await;
            let done = Instant::now();
            answered.lock().unwrap()[herd] = Some(done);
            observed.lock().unwrap().commits.push((
                at - origin,
                done - origin,
                reply.status.as_u16(),
            ));
            if plan.jitter && reply.status == StatusCode::OK {
                let (bench, observed) = (bench.clone(), observed.clone());
                committed.spawn(async move {
                    let random = Mutex::new(Seeded(plan.seed ^ 0xc0ff_ee00 ^ herd as u64));
                    let started = Instant::now() - origin;
                    let reads = bench
                        .refresh(
                            &bench.pages[0],
                            plan.retry,
                            &random,
                            global_herd(plan, plan.herds),
                        )
                        .await;
                    let ended = Instant::now() - origin;
                    observed.lock().unwrap().refreshes.push(Refresh {
                        page: 0,
                        herd,
                        committer: true,
                        started,
                        ended,
                        reads,
                    });
                });
            }
        }
        // Every page has had time to read, retry and settle.
        sleep(CURRENT_WITHIN + Duration::from_secs(6)).await;
        while committed.join_next().await.is_some() {}
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        let saturated = pool::saturated();
        let observed = std::mem::take(&mut *observed.lock().unwrap());
        let report = judge(plan, &observed, plan.pages);
        let passed = report["passed"] == true;
        let mut report = report;
        report["admission_saturated"] = json!({
            "workspace": saturated[0] - saturated_before[0],
            "control": saturated[1] - saturated_before[1],
            "data": saturated[2] - saturated_before[2],
            "worker": saturated[3] - saturated_before[3],
        });
        report["engine"] = json!({
            "typed_view": tally_json(typed_before, bench.counted.typed.read()),
            "record_components": tally_json(records_before, bench.counted.records.read()),
        });
        (report, passed)
    }

    /// Herd numbers are global across runs, so every commit writes a new value.
    fn global_herd(plan: Plan, herd: usize) -> usize {
        (plan.seed as usize) * 10 + herd + 1
    }

    fn judge(plan: Plan, observed: &Observed, pages: usize) -> Value {
        let mut herds = Vec::new();
        let mut passed = true;
        let first_herd = global_herd(plan, 0);
        for herd in 0..plan.herds {
            let (sent, answered, status) = observed.commits[herd];
            let command_ms = millis(answered - sent);
            let hints = &observed.hints[herd];
            let delivery: Vec<f64> = hints
                .values()
                .map(|at| millis(at.saturating_sub(sent)))
                .collect();
            let ours: Vec<&Refresh> = observed
                .refreshes
                .iter()
                .filter(|r| r.herd == herd)
                .collect();
            let reads: Vec<&ReadResult> = ours.iter().flat_map(|r| r.reads.iter()).collect();
            let first_ok = reads.iter().filter(|read| read.first() == 200).count();
            let mut first = BTreeMap::<String, usize>::new();
            let mut last = BTreeMap::<String, usize>::new();
            let mut by_read = BTreeMap::<&str, BTreeMap<String, usize>>::new();
            for read in &reads {
                *first.entry(read.first().to_string()).or_default() += 1;
                *last.entry(read.last().to_string()).or_default() += 1;
                *by_read
                    .entry(read.read.name())
                    .or_default()
                    .entry(
                        read.statuses
                            .iter()
                            .map(u16::to_string)
                            .collect::<Vec<_>>()
                            .join(">"),
                    )
                    .or_default() += 1;
            }
            let terminal = reads.iter().filter(|read| read.last() != 200).count();
            let retried_503 = reads
                .iter()
                .filter(|read| read.statuses.contains(&503))
                .count();
            let view_busy = reads
                .iter()
                .filter(|read| read.read == Read::View)
                .map(|read| read.statuses.iter().filter(|s| **s == 429).count())
                .sum::<usize>();
            let document_degraded: BTreeMap<String, usize> = reads
                .iter()
                .flat_map(|read| read.degraded.iter())
                .fold(BTreeMap::new(), |mut map, part| {
                    *map.entry(part.clone()).or_default() += 1;
                    map
                });
            // A page is current for this herd at the end of its first refresh
            // whose three reads all show this herd's value or a later one.
            let target = first_herd + herd;
            let mut lags = Vec::new();
            let mut stale = Vec::new();
            for page in 0..pages {
                let done = observed
                    .refreshes
                    .iter()
                    .filter(|r| r.page == page && r.current().is_some_and(|seen| seen >= target))
                    .map(|r| r.ended)
                    .min();
                match done {
                    Some(at) => lags.push(millis(at.saturating_sub(sent))),
                    None => stale.push(page),
                }
            }
            let late = lags
                .iter()
                .filter(|lag| **lag > millis(CURRENT_WITHIN))
                .count();
            let share = if reads.is_empty() {
                0.0
            } else {
                first_ok as f64 / reads.len() as f64
            };
            let criteria = json!({
                "command_200": status == 200,
                "command_under_1500_ms": command_ms < millis(COMMAND_UNDER),
                "delivered_to_all": hints.len() == pages,
                "no_terminal_failure": terminal == 0,
                "first_try_200_at_least_90_percent": share >= FIRST_TRY_SHARE,
                "every_page_current_within_6_s": stale.is_empty() && late == 0,
            });
            let herd_passed = criteria.as_object().unwrap().values().all(|v| *v == true);
            passed &= herd_passed;
            herds.push(json!({
                "herd": herd,
                "commit": {"status": status, "ms": command_ms},
                "delivered": hints.len(),
                "delivery_after_send": spread(delivery),
                "refreshes": ours.len(),
                "committer_refreshes": ours.iter().filter(|r| r.committer).count(),
                "refresh_start_after_send": spread(ours.iter().map(|r| millis(r.started.saturating_sub(sent))).collect()),
                "reads": reads.len(),
                "first_attempt": first,
                "final": last,
                "attempts_by_read": by_read,
                "first_try_200_share": (share * 1000.0).round() / 1000.0,
                "reads_with_a_503": retried_503,
                "terminal_failures": terminal,
                "view_429_answers": view_busy,
                "document_parts_not_ready": document_degraded,
                "current_after_send": spread(lags),
                "pages_not_current": stale,
                "pages_current_later_than_6_s": late,
                "criteria": criteria,
                "passed": herd_passed,
            }));
        }
        json!({
            "run": plan.name,
            "jitter": plan.jitter,
            "retry": plan.retry,
            "herds": plan.herds,
            "cadence_ms": plan.cadence.as_millis() as u64,
            "seed": plan.seed,
            "pages": plan.pages,
            "per_herd": herds,
            "passed": passed,
        })
    }

    /// What one data-role transaction (a checkout, its recycle checks and one
    /// statement) and one catalog comparison (part of every recycle) cost in
    /// this build on this host, one at a time.
    async fn costs(fixture: &Fixture, stores: &Stores, owner: &TrustedRequest) -> Value {
        let mut transaction = Vec::new();
        for _ in 0..20 {
            let started = Instant::now();
            stores.data.member_current(owner).await.unwrap();
            transaction.push(millis(started.elapsed()));
        }
        let operator = fixture.operator().await;
        let catalog = Catalog::embedded().unwrap();
        let mut verify = Vec::new();
        for _ in 0..20 {
            let started = Instant::now();
            catalog.verify(&operator.client).await.unwrap();
            verify.push(millis(started.elapsed()));
        }
        json!({"data_transaction": spread(transaction), "catalog_verify": spread(verify)})
    }

    /// The role transactions one read takes, alone (admission permits per
    /// role in `Role::index` order: workspace, control, data, worker).
    async fn transactions(bench: &Bench) -> Value {
        let page = &bench.pages[1];
        let mut out = serde_json::Map::new();
        for read in [Read::Document, Read::Batch, Read::View] {
            let before = pool::acquired();
            let reply = bench.attempt(page, read).await;
            assert_eq!(reply.status, StatusCode::OK, "{}", read.name());
            let after = pool::acquired();
            let d = |i: usize| after[i] - before[i];
            out.insert(
                read.name().into(),
                json!({"workspace": d(0), "control": d(1), "data": d(2), "worker": d(3)}),
            );
        }
        Value::Object(out)
    }

    #[test]
    #[ignore = "requires local Docker, OpenSSL and the locked runtime venv (the confined query child)"]
    fn native_refresh_herd_campaign() {
        std::thread::Builder::new()
            .name("app195-refresh-herd".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(4)
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
        operator(&fixture, &["backfill", "--organization", ORG]);
        let stores = Arc::new(Stores::new(fixture.settings()).unwrap());
        stores.readiness().await.unwrap();
        let mut report = serde_json::Map::new();

        // --- 1 owner and 25 members; one app, one work, shared with each ---
        admit_member(&fixture, OWNER);
        let members: Vec<String> = (0..MEMBERS).map(member).collect();
        for principal in &members {
            admit_member(&fixture, principal);
        }
        let owner = request(&stores, ORG, OWNER, 1);
        operator(
            &fixture,
            &[
                "create-app",
                "--organization",
                ORG,
                "--app",
                APP,
                "--space",
                SPACE,
                "--owner",
                OWNER,
            ],
        );
        for permission in ["discover", "run"] {
            grant(&fixture, OWNER, permission, &["--app", APP]);
        }
        let admitted = admission::admit_definition(&serde_json::to_vec(&definition()).unwrap())
            .expect("the campaign definition admits");
        let revision = stores
            .workspace
            .create_definition_revision(&owner, APP, &admitted)
            .await
            .unwrap();
        let PublishResult::Published(publication) = stores
            .control
            .publish(&owner, APP, &revision.revision, POLICY)
            .await
            .unwrap()
        else {
            panic!("a fresh revision publishes");
        };
        let saved = stores
            .workspace
            .create_work(
                &owner,
                APP,
                &publication.publication_id,
                "Shared item",
                &[
                    WorkBinding::Records {
                        input: "items".into(),
                        schema: "item".into(),
                    },
                    WorkBinding::Records {
                        input: "entries".into(),
                        schema: "entry".into(),
                    },
                ],
            )
            .await
            .unwrap();
        let work = saved.work.clone();
        let command =
            |command: &str, schema: &str, record: String, expected: i64, values: Value, n: u64| {
                CommandRequest {
                    app: APP.into(),
                    work: work.clone(),
                    publication: publication.publication_id.clone(),
                    command: command.into(),
                    schema: schema.into(),
                    schema_version_expected: 1,
                    record,
                    expected_record_version: expected,
                    expected_work_version: 1,
                    values,
                    idempotency_key: key(n),
                }
            };
        let posted = stores
            .data
            .commit_record(
                &owner,
                &command("post_item", "item", ITEM.into(), 0, item_values(0), 1),
            )
            .await
            .unwrap();
        assert!(matches!(posted, CommandResult::Committed(_)), "{posted:?}");
        for principal in &members {
            let shared = stores
                .workspace
                .share_work(&owner, APP, &work, principal, true)
                .await
                .unwrap();
            assert!(shared.changed());
        }
        for n in 0..8 {
            let writer = request(&stores, ORG, &members[n], 1);
            let choice = ["a", "b", "c", "d"][n % 4];
            let values = json!([
                {"field_id": "item", "value": {"type": "integer", "value": "1", "unit": "count"}},
                {"field_id": "choice", "value": {"type": "enum", "value": choice}}
            ]);
            let entry = stores
                .data
                .commit_record(
                    &writer,
                    &command("answer", "entry", entry_id(n), 0, values, 100 + n as u64),
                )
                .await
                .unwrap();
            assert!(matches!(entry, CommandResult::Committed(_)), "{entry:?}");
        }
        report.insert(
            "setup".into(),
            json!({
                "owner": 1, "members": MEMBERS,
                "share": "Workspace::share_work per member with record_write (25 shares); no organization-wide share on this branch",
                "lists": "items: one shared row the owner changes; entries: a second shared list (no per-person list on this branch), 8 entries",
                "views": ["current (records_list)", "total (record_summary)", "by_choice (record_ranking)", "by_item (typed table over items)", "answer_form (form)"],
            }),
        );

        // --- the composed routes with a minted session per page ---
        let pages: Arc<TestPages> = Arc::new(TestPages);
        let auth_pages: Arc<dyn Pages> = pages.clone();
        let auth = Arc::new(Auth::new(None, Some(stores.clone()), auth_pages).unwrap());
        let root = Directory::new("refresh-herd");
        let counted = Arc::new(Counted {
            engine: real_engine(&fixture_python(), &root.0),
            typed: Tally::default(),
            records: Tally::default(),
        });
        let executor: Arc<dyn Executor> = counted.clone();
        let work_routes = Arc::new(Work::new(
            auth.clone(),
            Some(executor),
            Arc::new(RecordingPages::default()),
        ));
        let record_page: Arc<dyn RecordPage> = pages;
        let app = http::uniform(
            Router::new()
                .merge(crate::commands::routes::router(Arc::new(Commands {
                    auth: auth.clone(),
                    page: record_page,
                })))
                .merge(crate::query::routes::router(work_routes))
                .merge(events::routes::router(auth.clone())),
        );
        let mut sessions = Vec::new();
        for (index, principal) in std::iter::once(OWNER.to_owned())
            .chain(members.iter().cloned())
            .enumerate()
        {
            let reference = auth.vault.insert("northstar", &vault_token()).unwrap();
            let cookie = stores
                .workspace
                .mint_session_for_tests(ORG, &principal, &reference)
                .await
                .unwrap();
            sessions.push(Page { index, cookie });
        }
        let bench = Arc::new(Bench {
            app,
            work: work.clone(),
            publication: publication.publication_id.clone(),
            pages: sessions,
            counted,
        });
        // Every page opens and shows the seeded item once, alone.
        let mut alone = Vec::new();
        for page in &bench.pages {
            let random = Mutex::new(Seeded(0));
            let started = Instant::now();
            let reads = bench.refresh(page, false, &random, 0).await;
            alone.push(millis(started.elapsed()));
            for read in &reads {
                assert_eq!(
                    read.statuses,
                    [200],
                    "page {} {}: {:?}",
                    page.index,
                    read.read.name(),
                    read
                );
                assert_eq!(
                    read.seen,
                    Some(0),
                    "page {} {} shows the seeded item",
                    page.index,
                    read.read.name()
                );
                assert!(read.degraded.is_empty(), "{:?}", read.degraded);
            }
        }
        report.insert("one_page_alone".into(), spread(alone));
        report.insert("costs_alone".into(), costs(&fixture, &stores, &owner).await);

        report.insert("transactions_per_read".into(), transactions(&bench).await);

        // --- the runs ---
        let operator = fixture.operator().await;
        let pages = bench.pages.len();
        let mut runs = Vec::new();
        let a = Plan {
            name: "A: jitter 0-3 s, one retry, admitted 32",
            jitter: true,
            retry: true,
            herds: 3,
            cadence: Duration::from_secs(10),
            seed: 1,
            pages,
        };
        let (run_a, passed_a) = run(&bench, a, &operator).await;
        println!("APP195 run A: {run_a}");
        runs.push(run_a);
        let b = Plan {
            name: "B: barrier, no jitter, no retry, admitted 32",
            jitter: false,
            retry: false,
            seed: 2,
            ..a
        };
        let (run_b, _) = run(&bench, b, &operator).await;
        println!("APP195 run B: {run_b}");
        runs.push(run_b);
        let c = Plan {
            name: "C: run B with admitted 8 per role (before APP-195)",
            seed: 3,
            ..b
        };
        let limits = [
            pool::limit_admitted(Role::Workspace, 8).await,
            pool::limit_admitted(Role::Control, 8).await,
            pool::limit_admitted(Role::Data, 8).await,
            pool::limit_admitted(Role::Worker, 8).await,
        ];
        let (run_c, _) = run(&bench, c, &operator).await;
        drop(limits);
        println!("APP195 run C: {run_c}");
        let c_503: usize = run_c["per_herd"]
            .as_array()
            .unwrap()
            .iter()
            .map(|herd| herd["first_attempt"]["503"].as_u64().unwrap_or(0) as usize)
            .sum();
        runs.push(run_c);
        report.insert("runs".into(), json!(runs));
        report.insert("run_a_passed".into(), json!(passed_a));
        report.insert("run_c_503".into(), json!(c_503));

        // --- when 26 pages fail run A: the most pages that pass it ---
        let mut seed = 4;
        let mut sizes = Vec::new();
        let mut largest = passed_a.then_some(pages);
        if !passed_a {
            for size in [20, 15, 10, 5] {
                let plan = Plan {
                    name: "A with fewer pages",
                    seed,
                    pages: size,
                    ..a
                };
                seed += 1;
                let (result, passed) = run(&bench, plan, &operator).await;
                println!("APP195 run A with {size} pages: passed={passed}");
                sizes.push(result);
                if passed {
                    largest = Some(size);
                    break;
                }
            }
        }
        report.insert("page_counts".into(), json!(sizes));
        report.insert("largest_passing_pages".into(), json!(largest));

        // --- the shortest commit cadence at which run A still passes (for
        // 26 pages, or else for the most pages that pass) ---
        let mut cadences = Vec::new();
        let mut sustainable = largest.map(|_| 10_000_u64);
        if let Some(size) = largest {
            for ms in [5000_u64, 3000, 2000, 1500, 1000] {
                let plan = Plan {
                    name: "A at a shorter cadence",
                    herds: 4,
                    cadence: Duration::from_millis(ms),
                    seed,
                    pages: size,
                    ..a
                };
                seed += 1;
                let (result, passed) = run(&bench, plan, &operator).await;
                println!("APP195 cadence {ms} ms with {size} pages: passed={passed}");
                cadences.push(result);
                if !passed {
                    break;
                }
                sustainable = Some(ms);
            }
        }
        report.insert("cadences".into(), json!(cadences));
        report.insert(
            "sustainable_cadence".into(),
            json!({"pages": largest, "cadence_ms": sustainable}),
        );
        report.insert(
            "limits".into(),
            json!("Local Docker PostgreSQL on a shared, loaded macOS host; HTTP through the composed router in process (no TLS to the client, no browser, no network); the replay is the APP-195 client's timing, not a browser. Each page plays both the work page's document read and the canvas board's batch and view reads at once. The test profile is unoptimized; the service image is a release build."),
        );

        println!("APP195 report: {}", serde_json::to_string(&report).unwrap());
        if let Some(path) = std::env::var_os("GENUI_REFRESH_HERD_REPORT") {
            std::fs::write(
                path,
                serde_json::to_vec_pretty(&Value::Object(report.clone())).unwrap(),
            )
            .unwrap();
        }
        assert!(
            c_503 >= 10,
            "run C reproduces the APP-193 finding: {c_503} answers 503"
        );
        assert!(
            passed_a,
            "run A meets the APP-195 criteria with 26 pages (largest passing: {largest:?})"
        );
    }
}
