//! Box objects: immutable, under the organization's prefix (ADR-009 B1
//! layout: a box file at `o/<org>/b/<box>/f/<file>`, a registered revision
//! at `o/<org>/s/<source>/r/<revision>/{original,part.parquet}`), each
//! written once and read back only when its SHA-256 matches the recorded
//! digest.
//!
//! The service binary has no AWS SDK (as for the revision cache, `query::
//! worker`): in the image, [`ObjectStore::Runtime`] spawns the credentialed
//! `runtime.py object` child per operation, which uses the organization's
//! scoped tenant credentials, writes with `ServerSideEncryption=aws:kms`
//! under the tenant key, `If-None-Match: *` and a SHA-256 checksum, and
//! reads one object back. Locally (the campaigns and the stand)
//! [`ObjectStore::Local`] keeps the same keys in a private directory, each
//! file created exclusively and made read-only.
use crate::supervise::{run_confined, Confinement};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

/// The credentialed child's bound per operation.
const OBJECT_DEADLINE: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// The object could not be written or read (or its digest differs).
    Unavailable,
    /// A different object already exists at the key.
    Exists,
}

#[derive(Clone, Debug)]
pub enum ObjectStore {
    /// A private local directory (campaigns, the local stand).
    Local { root: PathBuf },
    /// The image: `program args… object` children, staging under `scratch`.
    Runtime {
        program: PathBuf,
        args: Vec<String>,
        scratch: PathBuf,
    },
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
/// `sha256:<hex>`.
pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

/// A key under `o/<organization>/` made of UUID and fixed segments only.
pub fn key_shaped(organization: &str, key: &str) -> bool {
    let Some(rest) = key.strip_prefix(&format!("o/{organization}/")) else {
        return false;
    };
    crate::storage::valid_identifier(organization)
        && rest.split('/').all(|segment| {
            crate::storage::valid_identifier(segment)
                || matches!(segment, "b" | "f" | "s" | "r" | "original" | "part.parquet")
        })
}

pub fn file_key(organization: &str, box_id: &str, file_id: &str) -> String {
    format!("o/{organization}/b/{box_id}/f/{file_id}")
}
pub fn revision_key(organization: &str, source: &str, revision: &str, name: &str) -> String {
    format!("o/{organization}/s/{source}/r/{revision}/{name}")
}

fn private_dirs(path: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

/// A private staging file, removed on drop.
struct Staged(PathBuf);
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn staging(scratch: &Path) -> Result<Staged, StoreError> {
    private_dirs(scratch).map_err(|_| StoreError::Unavailable)?;
    let name = crate::auth::capability::random_uuid().ok_or(StoreError::Unavailable)?;
    Ok(Staged(scratch.join(format!(".object-{name}"))))
}

impl ObjectStore {
    /// The image's store (`app/Dockerfile`): `/run/genui/object-scratch`.
    pub fn image() -> Self {
        Self::Runtime {
            program: PathBuf::from("/usr/local/bin/python3"),
            args: vec!["-I".into(), "/app/runtime.py".into(), "object".into()],
            scratch: PathBuf::from("/run/genui/object-scratch"),
        }
    }

    /// Writes `bytes` once at `key` (an identical existing object is fine).
    pub async fn put(&self, organization: &str, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        if !key_shaped(organization, key) {
            return Err(StoreError::Unavailable);
        }
        match self {
            Self::Local { root } => {
                let path = root.join(key);
                let parent = path.parent().ok_or(StoreError::Unavailable)?;
                private_dirs(parent).map_err(|_| StoreError::Unavailable)?;
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)
                {
                    Ok(mut file) => {
                        file.write_all(bytes)
                            .and_then(|_| file.sync_all())
                            .map_err(|_| StoreError::Unavailable)?;
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
                            .map_err(|_| StoreError::Unavailable)?;
                        Ok(())
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let existing = fs::read(&path).map_err(|_| StoreError::Unavailable)?;
                        if sha256_hex(&existing) == sha256_hex(bytes) {
                            Ok(())
                        } else {
                            Err(StoreError::Exists)
                        }
                    }
                    Err(_) => Err(StoreError::Unavailable),
                }
            }
            Self::Runtime {
                program,
                args,
                scratch,
            } => {
                let staged = staging(scratch)?;
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&staged.0)
                    .and_then(|mut file| file.write_all(bytes))
                    .map_err(|_| StoreError::Unavailable)?;
                let request = json!({"operation": "put", "organization_id": organization, "key": key,
                    "path": staged.0, "sha256": sha256_hex(bytes)});
                self.child(program, args, &request).await
            }
        }
    }

    /// Reads the object at `key`; its SHA-256 must be `digest` (`sha256:…`).
    pub async fn get(
        &self,
        organization: &str,
        key: &str,
        digest: &str,
    ) -> Result<Vec<u8>, StoreError> {
        if !key_shaped(organization, key) {
            return Err(StoreError::Unavailable);
        }
        let bytes = match self {
            Self::Local { root } => {
                let path = root.join(key);
                let metadata = fs::symlink_metadata(&path).map_err(|_| StoreError::Unavailable)?;
                if !metadata.is_file() || metadata.len() > crate::boxes::MAX_FILE_BYTES as u64 * 2 {
                    return Err(StoreError::Unavailable);
                }
                fs::read(&path).map_err(|_| StoreError::Unavailable)?
            }
            Self::Runtime {
                program,
                args,
                scratch,
            } => {
                let staged = staging(scratch)?;
                let request = json!({"operation": "get", "organization_id": organization, "key": key,
                    "path": staged.0, "sha256": digest.trim_start_matches("sha256:")});
                self.child(program, args, &request).await?;
                fs::read(&staged.0).map_err(|_| StoreError::Unavailable)?
            }
        };
        if sha256(&bytes) != digest {
            return Err(StoreError::Unavailable);
        }
        Ok(bytes)
    }

    async fn child(
        &self,
        program: &Path,
        args: &[String],
        request: &serde_json::Value,
    ) -> Result<(), StoreError> {
        let mut command = tokio::process::Command::new(program);
        command.args(args);
        let confinement = Confinement {
            rss_limit_bytes: 512 * 1024 * 1024,
            deadline: OBJECT_DEADLINE,
            nofile: 64,
            core: 0,
        };
        let bytes = serde_json::to_vec(request).map_err(|_| StoreError::Unavailable)?;
        let output = run_confined(command, Cursor::new(bytes), &confinement)
            .await
            .map_err(|_| StoreError::Unavailable)?;
        match output.status.code() {
            Some(0) => Ok(()),
            Some(3) => Err(StoreError::Exists),
            _ => Err(StoreError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORG: &str = "00000000-0000-4000-8000-000000000001";
    const ID: &str = "00000000-0000-4000-8000-0000000000aa";

    #[test]
    fn keys_stay_under_the_organization() {
        assert!(key_shaped(ORG, &file_key(ORG, ID, ID)));
        assert!(key_shaped(ORG, &revision_key(ORG, ID, ID, "part.parquet")));
        assert!(!key_shaped(
            ORG,
            "o/00000000-0000-4000-8000-000000000002/b/x"
        ));
        assert!(!key_shaped(ORG, &format!("o/{ORG}/b/../../etc")));
        assert!(!key_shaped(ORG, &format!("o/{ORG}/b/{ID}/f/{ID}/x")));
    }

    #[tokio::test]
    async fn local_objects_are_write_once_and_digest_checked() {
        let root = std::env::temp_dir().join(format!("genui-box-store-{}", std::process::id()));
        let store = ObjectStore::Local { root: root.clone() };
        let key = file_key(ORG, ID, ID);
        store.put(ORG, &key, b"hello").await.unwrap();
        store.put(ORG, &key, b"hello").await.unwrap();
        assert_eq!(
            store.put(ORG, &key, b"other").await,
            Err(StoreError::Exists)
        );
        assert_eq!(
            store.get(ORG, &key, &sha256(b"hello")).await.unwrap(),
            b"hello"
        );
        assert_eq!(
            store.get(ORG, &key, &sha256(b"other")).await,
            Err(StoreError::Unavailable)
        );
        let _ = fs::set_permissions(root.join(&key), fs::Permissions::from_mode(0o600));
        let _ = fs::remove_dir_all(&root);
    }
}
