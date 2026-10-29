use super::{valid_id, Error, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryIdentity {
    pub version: String,
    pub sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    registry_version: String,
    bindings: Vec<Binding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    issuer: String,
    provider_tenant_id: String,
    organization_id: String,
}

pub(super) struct Registry {
    pub identity: RegistryIdentity,
    bindings: Vec<Binding>,
}

fn label(value: &str, bound: usize) -> bool {
    !value.is_empty() && value.len() <= bound && value.bytes().all(|b| b.is_ascii_graphic())
}

impl Registry {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = private_bytes(path, 16 * 1024)?;
        let document: Document = serde_json::from_slice(&raw).map_err(|_| Error::Configuration)?;
        let mut pairs = HashSet::new();
        if document.schema_version != 1
            || !valid_id(&document.registry_version)
            || document.bindings.len() > 32
            || document.bindings.iter().any(|binding| {
                !label(&binding.issuer, 256)
                    || !label(&binding.provider_tenant_id, 128)
                    || !valid_id(&binding.organization_id)
                    || !pairs.insert((&binding.issuer, &binding.provider_tenant_id))
            })
        {
            return Err(Error::Configuration);
        }
        Ok(Self {
            identity: RegistryIdentity {
                version: document.registry_version,
                sha256: Sha256::digest(&raw)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            },
            bindings: document.bindings,
        })
    }

    pub fn agrees(&self, issuer: &str, tenant: &str, organization: &str) -> bool {
        self.bindings.iter().any(|binding| {
            binding.issuer == issuer
                && binding.provider_tenant_id == tenant
                && binding.organization_id == organization
        })
    }
}

fn private_bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let refused = |_| Error::Configuration;
    if !path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(Error::Configuration);
    }
    for ancestor in path.ancestors().skip(1) {
        if !ancestor.symlink_metadata().map_err(refused)?.is_dir() {
            return Err(Error::Configuration);
        }
    }
    let parent = path
        .parent()
        .ok_or(Error::Configuration)?
        .symlink_metadata()
        .map_err(refused)?;
    // Same-UID private state is trusted. No link or public directory is accepted.
    let uid = unsafe { libc::geteuid() };
    if parent.uid() != uid || parent.mode() & 0o777 != 0o700 {
        return Err(Error::Configuration);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(refused)?;
    let metadata = file.metadata().map_err(refused)?;
    if !metadata.is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() > limit
    {
        return Err(Error::Configuration);
    }
    let mut raw = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut raw)
        .map_err(refused)?;
    if raw.len() as u64 > limit {
        return Err(Error::Configuration);
    }
    Ok(raw)
}
