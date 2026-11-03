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
