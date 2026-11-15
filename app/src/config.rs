use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct Credential(String);
impl Credential {
    pub fn expose_to_database_adapter(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseCredential {
    pub username: String,
    #[serde(rename = "token")]
    pub secret: Credential,
    pub expires_at: u64,
}
#[derive(Debug, Clone)]
pub struct Config {
    pub generation: String,
    pub file_sha256: String,
    pub expires_at: u64,
    pub database_host: String,
    pub database_name: String,
    pub workspace: DatabaseCredential,
    pub control: DatabaseCredential,
    pub data: DatabaseCredential,
    pub worker: DatabaseCredential,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    generation: String,
    issued_at: u64,
    expires_at: u64,
    database: Database,
    storage: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Database {
    host: String,
    name: String,
    port: u16,
    roles: BTreeMap<String, DatabaseCredential>,
}

/// Production credentials live at most 600 s (the sidecar rewrites them).
const MAX_LIFETIME: u64 = 600;
/// ADR-012 D12.1: the local dev binary (feature `dev-stand`, never in a
/// release build) writes its own credentials for its loopback Compose
/// database with a long expiry instead of rewriting a 600 s file.
#[cfg(feature = "dev-stand")]
pub const DEV_LIFETIME: u64 = 30 * 86_400;

fn max_lifetime(host: &str) -> u64 {
    #[cfg(feature = "dev-stand")]
    if host == "localhost" {
        return DEV_LIFETIME;
    }
    let _ = host;
    MAX_LIFETIME
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let parent = path
            .parent()
            .ok_or("credential path refused")?
            .symlink_metadata()
            .map_err(|_| "credential directory unavailable")?;
        // geteuid has no arguments or memory access; compare against opened file metadata.
        let uid = unsafe { libc::geteuid() };
        if !parent.is_dir() || parent.mode() & 0o077 != 0 || parent.uid() != uid {
            return Err("credential directory permissions refused");
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| "credential file unavailable")?;
        let metadata = file
            .metadata()
            .map_err(|_| "credential metadata unavailable")?;
        if !metadata.is_file()
            || metadata.mode() & 0o077 != 0
            || metadata.uid() != uid
            || metadata.len() > 65536
        {
            return Err("credential file permissions or size refused");
        }
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| "credential read refused")?;
        if bytes.len() > 65536 {
            return Err("credential size refused");
        }
        Self::decode(&bytes, now())
    }
    fn decode(bytes: &[u8], current: u64) -> Result<Self, &'static str> {
        let mut document: Document =
            serde_json::from_slice(bytes).map_err(|_| "credential document refused")?;
        if document.schema_version != 1
            || document.generation.len() != 36
            || !document
                .generation
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b'-')
            || document.issued_at > current.saturating_add(5)
            || document.expires_at <= current
            || document.expires_at
                > document
                    .issued_at
                    .saturating_add(max_lifetime(&document.database.host))
        {
            return Err("credential generation or expiry refused");
        }
        let db = &mut document.database;
        if db.port != 5432
            || db.name != "genui"
            || db.host.is_empty()
            || !db
                .host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            || db.roles.len() != 4
        {
            return Err("database configuration refused");
        }
        let roles = ["workspace", "control", "data", "worker"];
        for name in roles {
            let role = db.roles.get(name).ok_or("missing database role")?;
            if role.username != format!("genui_{name}")
                || role.secret.0.is_empty()
                || role.secret.0 == "REPLACE_ME"
                || role.expires_at < document.expires_at
            {
                return Err("database role credential refused");
            }
        }
        for (i, name) in roles.iter().enumerate() {
            if roles
                .iter()
                .skip(i + 1)
                .any(|other| db.roles[*name].secret.0 == db.roles[*other].secret.0)
            {
                return Err("logical roles share credentials");
            }
        }
        // Storage credentials remain in the private file and are consumed only by
        // the fixed helper; they are never rendered or included in Debug output.
        if !document.storage.is_object() {
            return Err("storage configuration refused");
        }
        Ok(Self {
            generation: document.generation,
            file_sha256: Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            expires_at: document.expires_at,
            database_host: db.host.clone(),
            database_name: db.name.clone(),
            workspace: db.roles.remove("workspace").unwrap(),
            control: db.roles.remove("control").unwrap(),
            data: db.roles.remove("data").unwrap(),
            worker: db.roles.remove("worker").unwrap(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn file_reader_rejects_fifo_symlink_and_oversized_input() {
        let path = std::env::temp_dir().join(format!(
            "genui-reader-unit-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = path.join("credentials.json");
        std::fs::write(&file, vec![b'x'; 65537]).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(Config::load(&file).is_err());
        let link = path.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(Config::load(&link).is_err());
        let fifo = path.join("fifo");
        let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            sender.send(Config::load(&fifo).is_err()).unwrap();
        });
        assert!(receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("FIFO reader blocked"));
        std::fs::remove_dir_all(path).unwrap();
    }
    pub(crate) fn fixture() -> serde_json::Value {
        serde_json::json!({"schema_version":1,"generation":"00000000-0000-4000-8000-000000000001","issued_at":100,"expires_at":200,
            "database":{"host":"synthetic.invalid","port":5432,"name":"genui","roles":{
                "workspace":{"username":"genui_workspace","token":"unit-workspace","expires_at":200},
                "control":{"username":"genui_control","token":"unit-control","expires_at":200},
                "data":{"username":"genui_data","token":"unit-data","expires_at":200},
                "worker":{"username":"genui_worker","token":"unit-worker","expires_at":200}}},"storage":{}})
    }
    #[test]
    fn refuse_expiry_shared_credentials_and_role_interchange_without_disclosure() {
        let mut value = fixture();
        let good = Config::decode(&serde_json::to_vec(&value).unwrap(), 150).unwrap();
        assert!(!format!("{good:?}").contains("unit-workspace"));
        assert!(Config::decode(&serde_json::to_vec(&value).unwrap(), 200).is_err());
        value["database"]["roles"]["workspace"]["token"] = "unit-data".into();
        assert!(Config::decode(&serde_json::to_vec(&value).unwrap(), 150).is_err());
        value = fixture();
        value["database"]["roles"]["data"]["username"] = "genui_migration".into();
        assert!(Config::decode(&serde_json::to_vec(&value).unwrap(), 150).is_err());
    }
}

/// APP-219: the process's logs. Production writes one JSON object per line,
/// `timestamp`, `level`, `target` and the event's own fields flattened beside
/// them (`event` is the stable name of an app event, such as `model.call`),
/// and never an ANSI escape. `GENUI_LOG_FORMAT=pretty` (the dev stand sets
/// it) keeps the human text, also without colour. The filter is `RUST_LOG`
/// when set (target directives, such as `info,genui=debug`), else
/// [`logs::DEFAULT_FILTER`], which keeps tokio_postgres's per-connection
/// notices out of the log. Both settings are read once, at startup, and a
/// value they do not describe stops the process.
///
/// What a line may carry is unchanged: identities (principal and
/// organization ids), codes, counts and durations; never a name, a note, a
/// prompt, record content, an email address or a raw request path.
pub mod logs {
    use tracing_subscriber::{
        filter::Targets, fmt::MakeWriter, layer::SubscriberExt, util::SubscriberInitExt,
    };

    pub const FORMAT_ENV: &str = "GENUI_LOG_FORMAT";
    pub const FILTER_ENV: &str = "RUST_LOG";
    pub const DEFAULT_FILTER: &str = "info,tokio_postgres=warn";

    /// How each line is written.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Format {
        /// One JSON object per line (production, and the default).
        Json,
        /// The human text, for a developer's terminal or the dev stand's files.
        Pretty,
    }
    impl Format {
        /// The format [`FORMAT_ENV`]'s `value` names; unset or empty is `json`.
        pub fn from_setting(value: Option<&str>) -> Result<Self, String> {
            match value.map(str::trim) {
                None | Some("" | "json") => Ok(Self::Json),
                Some("pretty") => Ok(Self::Pretty),
                Some(_) => Err(format!("{FORMAT_ENV} must be json or pretty")),
            }
        }
    }

    /// The filter [`FILTER_ENV`]'s `value` names; unset or empty is
    /// [`DEFAULT_FILTER`].
    pub fn filter(value: Option<&str>) -> Result<Targets, String> {
        let value = value.map(str::trim).filter(|value| !value.is_empty());
        value
            .unwrap_or(DEFAULT_FILTER)
            .parse()
            .map_err(|_| format!("{FILTER_ENV} must be target directives such as {DEFAULT_FILTER}"))
    }

    /// The process's subscriber: `format` lines through `filter` to `writer`
    /// (stdout in the binaries, a buffer in tests).
    pub fn subscriber<W>(
        format: Format,
        filter: Targets,
        writer: W,
    ) -> Box<dyn tracing::Subscriber + Send + Sync>
    where
        W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
    {
        let registry = tracing_subscriber::registry().with(filter);
        match format {
            Format::Json => Box::new(
                registry.with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_current_span(false)
                        .with_span_list(false)
                        .with_ansi(false)
                        .with_writer(writer),
                ),
            ),
            Format::Pretty => Box::new(
                registry.with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(writer),
                ),
            ),
        }
    }

    fn setting(name: &str) -> Result<Option<String>, String> {
        match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} must be text")),
        }
    }

    /// Reads both settings and installs the subscriber for the process, with
    /// `log` records (tokio_postgres's) routed through it. A subscriber
    /// already installed (a test, the dev stand's second start) is kept.
    pub fn init() -> Result<(), String> {
        let format = Format::from_setting(setting(FORMAT_ENV)?.as_deref())?;
        let filter = filter(setting(FILTER_ENV)?.as_deref())?;
        let _ = subscriber(format, filter, std::io::stdout).try_init();
        Ok(())
    }

    /// Tests: the production JSON subscriber writing into a buffer, and the
    /// lines it wrote.
    #[cfg(test)]
    pub(crate) mod testing {
        use std::sync::{Arc, Mutex, OnceLock};

        /// Keeps a second, inert dispatcher registered for the whole test
        /// process. With one dispatcher registered, tracing-core judges a new
        /// callsite by the registering thread's own default alone, so a
        /// callsite another test thread hits first would be cached as
        /// "never" and a scoped capture here would miss its lines.
        fn shared_interest() {
            static KEEP: OnceLock<tracing::Dispatch> = OnceLock::new();
            KEEP.get_or_init(|| {
                tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default())
            });
        }

        #[derive(Clone, Default)]
        pub(crate) struct Lines(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Lines {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Lines {
            type Writer = Lines;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }
        impl Lines {
            /// The production subscriber (JSON, the default filter) over this buffer.
            pub(crate) fn subscriber(&self) -> Box<dyn tracing::Subscriber + Send + Sync> {
                shared_interest();
                super::subscriber(
                    super::Format::Json,
                    super::filter(None).unwrap(),
                    self.clone(),
                )
            }
            /// A dispatcher of [`Self::subscriber`], for threads.
            pub(crate) fn dispatch(&self) -> tracing::Dispatch {
                tracing::Dispatch::new(self.subscriber())
            }
            /// Everything written.
            pub(crate) fn text(&self) -> String {
                String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
            }
            /// Every line, each parsed; a line that is not one JSON object fails.
            pub(crate) fn parsed(&self) -> Vec<serde_json::Value> {
                self.text()
                    .lines()
                    .map(|line| {
                        let value: serde_json::Value = serde_json::from_str(line)
                            .unwrap_or_else(|_| panic!("not one JSON object: {line}"));
                        assert!(value.is_object(), "not one JSON object: {line}");
                        value
                    })
                    .collect()
            }
            /// The lines whose `event` is `name`.
            pub(crate) fn events(&self, name: &str) -> Vec<serde_json::Value> {
                self.parsed()
                    .into_iter()
                    .filter(|line| line["event"] == name)
                    .collect()
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{testing::Lines, *};

        /// APP-219: every production line is one JSON object with the fixed
        /// keys and no ANSI escape, even when a field's own text holds one
        /// (it is escaped as `\u001b`); tokio_postgres's notices stay out
        /// unless they warn.
        #[test]
        fn every_production_line_is_one_json_object_without_ansi() {
            let lines = Lines::default();
            tracing::subscriber::with_default(lines.subscriber(), || {
                tracing::info!(event = "model.call", source = "canvas", "model replied");
                tracing::warn!(
                    event = "http.server_error",
                    status = 503u16,
                    route = "/v1/x/{id}"
                );
                tracing::error!(code = "\u{1b}[31mred\u{1b}[0m", "a field holding an escape");
                tracing::info!(target: "tokio_postgres::connection", "NOTICE: per-connection noise");
                tracing::warn!(target: "tokio_postgres::connection", "a warning that stays");
            });
            let text = lines.text();
            assert!(
                !text.as_bytes().contains(&0x1b),
                "no ANSI escape byte: {text}"
            );
            let parsed = lines.parsed();
            assert_eq!(
                parsed.len(),
                4,
                "the tokio_postgres INFO line is filtered: {text}"
            );
            for line in &parsed {
                for key in ["timestamp", "level", "target"] {
                    assert!(
                        line.get(key).is_some_and(|v| v.is_string()),
                        "{key}: {line}"
                    );
                }
            }
            assert_eq!(parsed[0]["event"], "model.call");
            assert_eq!(parsed[0]["source"], "canvas");
            assert_eq!(parsed[0]["level"], "INFO");
            assert_eq!(parsed[1]["event"], "http.server_error");
            assert_eq!(parsed[1]["status"], 503);
            assert_eq!(parsed[2]["code"], "\u{1b}[31mred\u{1b}[0m");
            assert_eq!(parsed[3]["target"], "tokio_postgres::connection");
            assert_eq!(parsed[3]["level"], "WARN");
            assert!(
                parsed.iter().all(|line| line.get("fields").is_none()),
                "flattened"
            );
        }

        #[test]
        fn the_settings_are_json_by_default_and_refuse_what_they_do_not_name() {
            assert_eq!(Format::from_setting(None), Ok(Format::Json));
            assert_eq!(Format::from_setting(Some("")), Ok(Format::Json));
            assert_eq!(Format::from_setting(Some("json")), Ok(Format::Json));
            assert_eq!(Format::from_setting(Some("pretty")), Ok(Format::Pretty));
            let refused = Format::from_setting(Some("text")).unwrap_err();
            assert!(refused.contains(FORMAT_ENV), "{refused}");
            assert!(filter(None).is_ok() && filter(Some("")).is_ok());
            assert!(filter(Some("debug")).is_ok());
            assert!(filter(Some("info,genui=debug,tokio_postgres=warn")).is_ok());
            let refused = filter(Some("genui=loud")).unwrap_err();
            assert!(refused.contains(FILTER_ENV), "{refused}");
            assert_eq!(DEFAULT_FILTER, "info,tokio_postgres=warn");
        }
    }
}
