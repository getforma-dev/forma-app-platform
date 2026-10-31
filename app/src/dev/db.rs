//! The dev binary's database (ADR-012 D12.1): this instance's Compose
//! PostgreSQL (`compose.dev.yaml`), made into the shape the product's role
//! pools require (verified TLS, the four roles, the canonical migrations and
//! the validated seed) by idempotent steps at every start, so a restart
//! keeps the data and a new volume is ready in seconds.
//!
//! The local superuser is reached only through `docker exec` (socket trust
//! inside the container); TCP needs a password once the first start has
//! written `pg_hba.conf`. Every password is a fixed synthetic dev value for a
//! loopback-only database, never a real secret. The credentials file is
//! written here with the feature's long expiry (`config::DEV_LIFETIME`), so
//! nothing rewrites it while the server runs.
use super::{private_dir, repository, Env, Profile, SPACE};
use crate::storage::Settings;
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MIGRATION_PASSWORD: &str = "synthetic-migration";
const GENERATION: &str = "00000000-0000-4000-8000-00000000de01";
const ISSUER: &str = "app008-fixture-only";

pub(crate) struct Database {
    container: String,
    port: u16,
    state: PathBuf,
    python: PathBuf,
}

fn run(mut command: Command, input: &[u8], label: &str) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{label}: {e}"))?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .map_err(|e| format!("{label}: {e}"))?;
    let output = child
        .wait_with_output()
        .map_err(|e| format!("{label}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{label} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("{label}: output is not UTF-8"))
}

/// Writes `bytes` to `path` atomically, readable by its owner only.
fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = path.with_extension("new");
    let _ = std::fs::remove_file(&temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| format!("{}: {e}", temporary.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", temporary.display()))?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
}

impl Database {
    pub fn new(env: &Env) -> Result<Self, String> {
        private_dir(&env.state)?;
        Ok(Self {
            container: env.db_container.clone(),
            port: env.db_port,
            state: env.state.clone(),
            python: env.python.clone(),
        })
    }

    fn tls(&self) -> PathBuf {
        self.state.join("tls")
    }
    fn ca(&self) -> PathBuf {
        self.tls().join("ca.pem")
    }

    fn docker(&self, args: &[&str], input: &[u8], label: &str) -> Result<String, String> {
        let mut command = Command::new("docker");
        command.args(args);
        run(command, input, label)
    }

    /// SQL as the container's local superuser (socket trust), one statement
    /// at a time in autocommit, tuples only.
    pub fn sql(&self, sql: &str, database: &str) -> Result<String, String> {
        self.docker(
            &[
                "exec",
                "-i",
                &self.container,
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
            "dev database SQL",
        )
    }

    /// A shell command in the container as the `postgres` user.
    fn shell(&self, script: &str, input: &[u8], label: &str) -> Result<(), String> {
        self.docker(
            &[
                "exec",
                "-i",
                "--user=postgres",
                &self.container,
                "sh",
                "-c",
                script,
            ],
            input,
            label,
        )
        .map(drop)
    }

    fn wait_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let ready = Command::new("docker")
                .args([
                    "exec",
                    &self.container,
                    "pg_isready",
                    "--host=127.0.0.1",
                    "--username=postgres",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ready {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "the database container {} is not ready (is the db process up?)",
                    self.container
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// A local CA and a `localhost` server certificate, made once per state
    /// directory (the server copy is refreshed into the volume every start).
    fn ensure_tls(&self) -> Result<(), String> {
        let dir = self.tls();
        if ["ca.pem", "server.pem", "server.key"]
            .iter()
            .all(|f| dir.join(f).is_file())
        {
            return Ok(());
        }
        private_dir(&dir)?;
        let openssl = |args: &[&str]| {
            let mut command = Command::new("openssl");
            command.current_dir(&dir).args(args);
            run(command, b"", "local TLS material").map(drop)
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
            "3650",
            "-subj",
            "/CN=genui-dev local CA",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ])?;
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
        ])?;
        std::fs::write(
            dir.join("server.ext"),
            "subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
        )
        .map_err(|e| format!("server.ext: {e}"))?;
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
            "3650",
            "-extfile",
            "server.ext",
        ])
    }

    /// The files the role pools and the operator read: the CA, the
    /// credentials (long-lived, loopback only) and the identity registry.
    pub fn prepare_files(&self) -> Result<(), String> {
        self.ensure_tls()?;
        let now = crate::config::now();
        let expires = now + crate::config::DEV_LIFETIME;
        let mut roles = serde_json::Map::new();
        for role in ["workspace", "control", "data", "worker"] {
            roles.insert(
                role.into(),
                json!({"username": format!("genui_{role}"), "token": format!("synthetic-{role}"), "expires_at": expires}),
            );
        }
        let credentials = json!({"schema_version": 1, "generation": GENERATION, "issued_at": now, "expires_at": expires,
            "database": {"host": "localhost", "name": "genui", "port": 5432, "roles": roles}, "storage": {}});
        private_write(
            &self.state.join("credentials.json"),
            &serde_json::to_vec(&credentials).expect("JSON"),
        )?;
        let bindings: Vec<Value> = [super::ORG, super::OTHER]
            .iter()
            .map(|org| json!({"issuer": ISSUER, "provider_tenant_id": format!("ten_{org}"), "organization_id": org}))
            .collect();
        private_write(
            &self.state.join("registry.json"),
            &serde_json::to_vec(&json!({"schema_version": 1, "registry_version": "00000000-0000-4000-8000-000000000aaa", "bindings": bindings}))
                .expect("JSON"),
        )
    }

    /// Everything the product needs from the database, idempotently: TLS on,
    /// the migration role, the `genui` database, migrations and the
    /// validated seed (both no-ops when current), role passwords and
    /// password-only TCP authentication.
    pub fn bootstrap(&self) -> Result<(), String> {
        let started = Instant::now();
        self.wait_ready()?;
        self.prepare_files()?;
        let tls = self.tls();
        for file in ["server.key", "server.pem"] {
            let bytes = std::fs::read(tls.join(file)).map_err(|e| format!("{file}: {e}"))?;
            self.shell(
                &format!(
                    "umask 077 && mkdir -p \"$PGDATA/dev-tls\" && cat > \"$PGDATA/dev-tls/{file}\""
                ),
                &bytes,
                "copy the server TLS material",
            )?;
        }
        self.sql(
            "ALTER SYSTEM SET ssl=on; \
             ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/data/dev-tls/server.pem'; \
             ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/data/dev-tls/server.key'; \
             SELECT pg_reload_conf(); \
             CREATE TABLE IF NOT EXISTS genui_dev_marks(name text PRIMARY KEY, value text NOT NULL, at timestamptz NOT NULL DEFAULT now()); \
             DO $$BEGIN \
               IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='genui_migration') THEN \
                 CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS; END IF; \
               IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='rds_iam') THEN CREATE ROLE rds_iam NOLOGIN; END IF; \
               IF NOT EXISTS (SELECT 1 FROM pg_auth_members m JOIN pg_roles r ON r.oid=m.roleid JOIN pg_roles u ON u.oid=m.member \
                              WHERE r.rolname='rds_iam' AND u.rolname='genui_migration') THEN \
                 GRANT rds_iam TO genui_migration WITH ADMIN TRUE; END IF; \
             END$$; \
             ALTER ROLE genui_migration PASSWORD 'synthetic-migration';",
            "postgres",
        )?;
        if self
            .sql(
                "SELECT 1 FROM pg_database WHERE datname='genui'",
                "postgres",
            )?
            .trim()
            .is_empty()
        {
            self.sql("CREATE DATABASE genui OWNER genui_migration;", "postgres")?;
            println!("genui-dev: new database (first start of this volume)");
        }
        let script = "import sys\nfrom pathlib import Path\nimport psycopg\nimport runtime\nfrom tenant_seed import make_bundles,seed\nwith psycopg.connect(host='127.0.0.1',port=int(sys.argv[1]),user='genui_migration',password=sys.argv[3],dbname='genui',sslmode='require',connect_timeout=5) as c:\n runtime.apply_migrations(c,Path(sys.argv[2]))\n seed(c,make_bundles())\n";
        let app = repository().join("app");
        let mut command = Command::new(&self.python);
        command.current_dir(app.join("runtime")).args([
            "-c",
            script,
            &self.port.to_string(),
            app.join("migrations").to_str().expect("UTF-8 path"),
            MIGRATION_PASSWORD,
        ]);
        run(command, b"", "migrations and the validated seed")?;
        self.sql(
            "ALTER ROLE postgres PASSWORD 'synthetic-postgres'; \
             ALTER ROLE genui_workspace PASSWORD 'synthetic-workspace'; \
             ALTER ROLE genui_control PASSWORD 'synthetic-control'; \
             ALTER ROLE genui_data PASSWORD 'synthetic-data'; \
             ALTER ROLE genui_worker PASSWORD 'synthetic-worker';",
            "genui",
        )?;
        self.shell(
            "printf 'local all all trust\\nhost all all all scram-sha-256\\n' > \"$PGDATA/pg_hba.conf\"",
            b"",
            "password-only TCP authentication",
        )?;
        self.sql("SELECT pg_reload_conf();", "postgres")?;
        if self.marked("epoch")?.is_none() {
            let mut bytes = [0u8; 8];
            ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
                .map_err(|_| "random epoch")?;
            let epoch: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            self.mark("epoch", &epoch)?;
        }
        println!(
            "genui-dev: database ready on 127.0.0.1:{} ({} ms)",
            self.port,
            started.elapsed().as_millis()
        );
        Ok(())
    }

    /// This database's identity (new after `reset`): the clean profile's
    /// listener clears a browser's storage once per epoch.
    pub fn epoch(&self) -> Result<String, String> {
        self.marked("epoch")?
            .ok_or_else(|| "the database has no epoch".into())
    }

    /// The profile's operator setup, once per database volume (marker
    /// `operator`); the library sources are rebuilt when the state
    /// directory's cache lost them.
    pub fn profile_setup(&self, profile: Profile) -> Result<(), String> {
        let org = profile.organization();
        if self.marked("operator")?.is_none() {
            match profile {
                Profile::Pitch => {
                    for org in [super::ORG, super::OTHER] {
                        self.operator(&["backfill", "--organization", org])?;
                    }
                    let seeded = self.operator(&["seed-library", "--organization", org])?;
                    let apps = seeded["apps"].as_array().map(Vec::len).unwrap_or(0);
                    if apps != 20 {
                        return Err(format!("the library seeded {apps} apps, not 20"));
                    }
                    for persona in profile.personas() {
                        self.operator(&[
                            "grant-library-access",
                            "--organization",
                            org,
                            "--principal",
                            persona.principal,
                            "--record-write",
                            "--authoring-app",
                            persona.authoring,
                        ])?;
                    }
                    self.mark("library", "seeded")?;
                }
                Profile::Clean => {
                    self.operator(&["backfill", "--organization", org])?;
                    // No library: only the customer's own authoring app, the
                    // app the composer drafts a generated definition on.
                    for persona in profile.personas() {
                        self.operator(&[
                            "create-app",
                            "--organization",
                            org,
                            "--app",
                            persona.authoring,
                            "--space",
                            SPACE,
                            "--owner",
                            persona.principal,
                        ])?;
                        for permission in ["discover", "run"] {
                            self.operator(&[
                                "grant-permission",
                                "--organization",
                                org,
                                "--principal",
                                persona.principal,
                                "--permission",
                                permission,
                                "--app",
                                persona.authoring,
                            ])?;
                        }
                    }
                }
            }
            self.mark("operator", "done")?;
            println!("genui-dev: operator setup done ({})", profile.name());
        }
        if self.marked("library")?.is_some() {
            self.library_sources(org)?;
        }
        Ok(())
    }

    /// The library's source revisions, normalized into this instance's
    /// revision cache (`library_sources.py build`); skipped when present.
    pub fn library_sources(&self, org: &str) -> Result<(), String> {
        let cache = self.state.join("cache");
        let done = cache.join(format!(".library-{org}"));
        if done.exists() {
            return Ok(());
        }
        private_dir(&cache)?;
        let mut command = Command::new(&self.python);
        command.current_dir(repository().join("app/runtime")).args([
            "-c",
            "import json,sys,library_sources as l; print(json.dumps(l.build(sys.argv[1], sys.argv[2])))",
            org,
            cache.to_str().expect("UTF-8 path"),
        ]);
        run(command, b"", "library sources")?;
        std::fs::write(&done, b"").map_err(|e| format!("{}: {e}", done.display()))
    }

    /// The real operator command (`admission_operator.py`) as genui_migration
    /// over verify-full TLS, exactly as an operator runs it; one JSON line.
    pub fn operator(&self, args: &[&str]) -> Result<Value, String> {
        let mut command = Command::new(&self.python);
        command
            .current_dir(repository().join("app/runtime"))
            .arg("admission_operator.py")
            .args(args)
            .env("PGHOST", "localhost")
            .env("PGPORT", self.port.to_string())
            .env("PGUSER", "genui_migration")
            .env("PGPASSWORD", MIGRATION_PASSWORD)
            .env("PGDATABASE", "genui")
            .env("PGSSLROOTCERT", self.ca());
        for name in ["AWS_PROFILE", "AWS_SESSION_TOKEN", "AWS_DEFAULT_PROFILE"] {
            command.env_remove(name);
        }
        let output = run(command, b"", &format!("operator {}", args[0]))?;
        serde_json::from_str(output.trim()).map_err(|_| format!("operator {}: not JSON", args[0]))
    }

    pub fn marked(&self, name: &str) -> Result<Option<String>, String> {
        debug_assert!(name.bytes().all(|b| b.is_ascii_lowercase()));
        let value = self.sql(
            &format!("SELECT value FROM genui_dev_marks WHERE name='{name}'"),
            "postgres",
        )?;
        let value = value.trim();
        Ok((!value.is_empty()).then(|| value.to_owned()))
    }

    pub fn mark(&self, name: &str, value: &str) -> Result<(), String> {
        debug_assert!(name.bytes().all(|b| b.is_ascii_lowercase()));
        debug_assert!(value.bytes().all(|b| b.is_ascii_alphanumeric()));
        self.sql(
            &format!("INSERT INTO genui_dev_marks(name,value) VALUES ('{name}','{value}') ON CONFLICT (name) DO UPDATE SET value=EXCLUDED.value, at=now();"),
            "postgres",
        )
        .map(drop)
    }

    /// The product's role pools against this database (verified TLS).
    pub fn settings(&self) -> Settings {
        Settings::fixture(
            self.state.join("credentials.json"),
            self.state.join("registry.json"),
            self.ca(),
            "localhost",
            self.port,
        )
    }
}
