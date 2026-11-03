//! A confirmed CSV/XLSX becomes an immutable source revision (APP-016 lite).
//!
//! In order: the canonical CSV under the confirmed columns (`inspect`), the
//! normalizer child (`runtime.py normalize`: no credential, empty
//! environment, confined like the query engine) writes the Parquet into a
//! private scratch directory and reports its digest, which is re-computed
//! here; the original (the box file's bytes) and `part.parquet` are written
//! once under `o/<org>/s/<source>/r/<revision>/` (ADR-009 B1); the Parquet is
//! placed read-only in the engine's revision cache under the name and digest
//! the engine verifies before every run; then the source revision envelope
//! (contract `source_revision`) is stored by the 0011 definer with the
//! owner's `source_read`, and the registration is audited.
use super::{
    inspect::Column,
    store::{revision_key, sha256, ObjectStore},
};
use crate::supervise::{run_confined, Confinement};
use serde_json::{json, Value};
use std::{
    fs,
    io::Cursor,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const NORMALIZE_DEADLINE: Duration = Duration::from_secs(60);
const NORMALIZE_RSS_BYTES: u64 = 1536 * 1024 * 1024;

/// Where the normalizer runs and where the engine's cache lives.
#[derive(Clone, Debug)]
pub struct Normalizer {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// The engine's revision cache root (`query::EngineSettings::cache_root`).
    pub cache_root: PathBuf,
    /// Private per-registration scratch directories are made under it.
    pub scratch_root: PathBuf,
}
impl Normalizer {
    /// The image's layout (`app/Dockerfile`, `query::EngineSettings::image`).
    pub fn image() -> Self {
        Self {
            program: PathBuf::from("/usr/local/bin/python3"),
            args: vec!["-I".into(), "/app/runtime.py".into(), "normalize".into()],
            cache_root: PathBuf::from("/run/genui/cache"),
            scratch_root: PathBuf::from("/run/genui/box-scratch"),
        }
    }
}

/// A normalized revision ready to be registered.
#[derive(Clone, Debug)]
pub struct Normalized {
    pub source_id: String,
    pub revision_id: String,
    pub contract: Value,
    pub contract_digest: String,
    pub normalized_digest: String,
    pub row_count: i64,
    pub envelope: Vec<u8>,
}

/// Why a registration stopped before storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegisterError {
    /// A cell does not fit its column's confirmed type.
    Column(String),
    /// The normalizer refused the data (its fixed code).
    Refused(String),
    Unavailable,
}

/// `YYYY-MM-DDTHH:MM:SSZ` now.
pub fn utc_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    utc(seconds)
}
/// `YYYY-MM-DDTHH:MM:SSZ` of a Unix time.
pub fn utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest / 60) % 60,
        rest % 60
    )
}

/// The input contract of a registration: input id from the file name, a
/// contract id unique to this source (so two files with the same columns
/// never share a contract digest), the confirmed fields.
pub fn contract(input_id: &str, source_id: &str, columns: &[Column]) -> Value {
    let suffix: String = source_id.chars().filter(|c| *c != '-').take(8).collect();
    let mut contract_id = format!("{input_id}_{suffix}");
    contract_id.truncate(64);
    json!({
        "id": input_id,
        "contract_id": contract_id,
        "contract_version": 1,
        "kind": "source",
        "fields": columns.iter().map(Column::field).collect::<Vec<_>>(),
    })
}

/// The input id of a file: its name's stem as a local id.
pub fn input_id(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let mut id = super::inspect::local_id(stem, "data");
    id.truncate(40);
    id.trim_end_matches('_').to_owned()
}

fn private_dir(path: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[allow(clippy::too_many_arguments)]
/// Normalizes and stores one revision of `original` (the file bytes) under
/// confirmed `columns`; the caller registers it in the database.
pub async fn normalize_and_store(
    normalizer: &Normalizer,
    store: &ObjectStore,
    organization: &str,
    file_name: &str,
    format: &str,
    original: &[u8],
    table: &super::inspect::Table,
    columns: &[Column],
) -> Result<Normalized, RegisterError> {
    let csv = super::inspect::canonical_csv(table, columns).map_err(RegisterError::Column)?;
    let source_id = crate::auth::capability::random_uuid().ok_or(RegisterError::Unavailable)?;
    let revision_id = crate::auth::capability::random_uuid().ok_or(RegisterError::Unavailable)?;
    let contract = contract(&input_id(file_name), &source_id, columns);
    let canonical = crate::admission::encode(&contract).map_err(|_| RegisterError::Unavailable)?;
    let contract_digest =
        crate::admission::digest::digest(crate::admission::digest::INPUT_CONTRACT, &canonical);

    private_dir(&normalizer.scratch_root).map_err(|_| RegisterError::Unavailable)?;
    let scratch = Scratch(normalizer.scratch_root.join(&revision_id));
    private_dir(&scratch.0).map_err(|_| RegisterError::Unavailable)?;
    let target = scratch.0.join("part.parquet");
    let request = json!({"contract": contract, "csv": String::from_utf8(csv).map_err(|_| RegisterError::Unavailable)?, "target": target});
    let mut command = tokio::process::Command::new(&normalizer.program);
    command.args(&normalizer.args);
    let confinement = Confinement {
        rss_limit_bytes: NORMALIZE_RSS_BYTES,
        deadline: NORMALIZE_DEADLINE,
        nofile: 64,
        core: 0,
    };
    let bytes = serde_json::to_vec(&request).map_err(|_| RegisterError::Unavailable)?;
    let output = run_confined(command, Cursor::new(bytes), &confinement)
        .await
        .map_err(|_| RegisterError::Unavailable)?;
    let answer: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| RegisterError::Unavailable)?;
    if let Some(code) = answer.get("refused").and_then(Value::as_str) {
        return Err(RegisterError::Refused(code.to_owned()));
    }
    if !output.status.success() {
        return Err(RegisterError::Unavailable);
    }
    let parquet = fs::read(&target).map_err(|_| RegisterError::Unavailable)?;
    let normalized_digest = sha256(&parquet);
    let row_count = answer
        .get("row_count")
        .and_then(Value::as_i64)
        .ok_or(RegisterError::Unavailable)?;
    // The child is not trusted with the digest.
    if answer.get("normalized_digest").and_then(Value::as_str) != Some(normalized_digest.as_str())
        || row_count as usize != table.rows.len()
    {
        return Err(RegisterError::Unavailable);
    }
    let content_digest = sha256(original);
    store
        .put(
            organization,
            &revision_key(organization, &source_id, &revision_id, "original"),
            original,
        )
        .await
        .map_err(|_| RegisterError::Unavailable)?;
    store
        .put(
            organization,
            &revision_key(organization, &source_id, &revision_id, "part.parquet"),
            &parquet,
        )
        .await
        .map_err(|_| RegisterError::Unavailable)?;
    // The engine's read-only cache, as the fetch child would place it.
    let directory = normalizer.cache_root.join("o").join(organization);
    private_dir(&normalizer.cache_root).map_err(|_| RegisterError::Unavailable)?;
    private_dir(&normalizer.cache_root.join("o")).map_err(|_| RegisterError::Unavailable)?;
    private_dir(&directory).map_err(|_| RegisterError::Unavailable)?;
    let cached = directory.join(format!("{source_id}.{revision_id}.parquet"));
    let staging = directory.join(format!(".fetch-box-{revision_id}"));
    fs::write(&staging, &parquet)
        .and_then(|_| fs::set_permissions(&staging, fs::Permissions::from_mode(0o400)))
        .and_then(|_| fs::rename(&staging, &cached))
        .map_err(|_| {
            let _ = fs::remove_file(&staging);
            RegisterError::Unavailable
        })?;
    let has_time = columns
        .iter()
        .any(|c| c.id == "time" && c.kind == super::inspect::ColumnType::Timestamp);
    let envelope = json!({
        "kind": "source_revision", "schema_version": 1,
        "organization_id": organization, "source_id": source_id, "revision_id": revision_id,
        "content_digest": content_digest, "normalized_digest": normalized_digest,
        "input_contract": contract, "parser_version": 1, "mapping_version": 1,
        "format": format, "formula_policy": "reject", "row_count": row_count,
        "timezone": "UTC", "time_semantics": if has_time { "event" } else { "none" },
        "units_confirmed": true, "created_at": utc_now(), "provenance_sample": [],
    });
    let envelope = crate::admission::encode(&envelope).map_err(|_| RegisterError::Unavailable)?;
    Ok(Normalized {
        source_id,
        revision_id,
        contract,
        contract_digest,
        normalized_digest,
        row_count,
        envelope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formats_known_instants() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(1_793_350_800), "2026-10-30T09:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_now().len(), 20);
    }

    #[test]
    fn input_ids_come_from_file_stems() {
        assert_eq!(input_id("q3_metrics.csv"), "q3_metrics");
        assert_eq!(input_id("Q3 Metrics (final).xlsx"), "q3_metrics_final");
        assert_eq!(input_id("2026.csv"), "c_2026");
        let c = contract("q3", "0a1b2c3d-0000-4000-8000-000000000000", &[]);
        assert_eq!(c["contract_id"], "q3_0a1b2c3d");
    }
}
