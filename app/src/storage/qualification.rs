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
    "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59";

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
            Some("16.13")
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
    let mut waiters = Vec::new();
    for _ in 0..6 {
        let pool = stores.data.inner.clone();
        waiters.push(tokio::spawn(async move { Lease::acquire(&pool).await }));
    }
    timeout(Duration::from_secs(1), async {
        while stores.data.inner.pool.status().waiting != 6 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        Lease::acquire(&stores.data.inner).await,
        Err(Error::Unavailable)
    ));
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
    check(&mut checks,"eight physical connections, two per role, bounded waiters and absolute acquisition deadline");
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
    fixture.sql("ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check; UPDATE genui_meta.schema_version SET version=16;","genui");
    assert!(Lease::acquire(&identity.worker.inner).await.is_err());
    fixture.sql("UPDATE genui_meta.schema_version SET version=15; ALTER TABLE genui_meta.schema_version ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND 15);","genui");
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
    // unchanged v5 constraints, so this exercises the post-catalog read.
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
    fixture.sql("UPDATE genui_meta.schema_version SET version=15", "genui");
    version_fresh.readiness().await.unwrap();
    version_fresh.close().await.unwrap();
    drop(operator);
    check(&mut checks,"legal version-one row with unchanged v4 catalog refuses new connections, recycle and readiness until version-four restoration");

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
