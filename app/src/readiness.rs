use crate::config::{now, Config};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    generation: String,
    file_sha256: String,
    expires_at: u64,
    checked_at: u64,
}
struct Success {
    generation: String,
    file_sha256: String,
    expires_at: u64,
    received: Instant,
}
pub struct Readiness {
    path: PathBuf,
    stores: Option<Arc<crate::storage::Stores>>,
    success: Mutex<Option<Success>>,
}
impl Readiness {
    /// Isolated real-database qualification only; absent from production builds.
    #[cfg(test)]
    pub(crate) fn isolated(path: PathBuf, stores: Arc<crate::storage::Stores>) -> Self {
        Self {
            path,
            stores: Some(stores),
            success: Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) async fn qualify_once(&self, helper: &PathBuf, python: &str) -> bool {
        let success = self.check(helper, python).await;
        *self.success.lock().unwrap() = success;
        self.ready()
    }

    pub fn start(
        path: PathBuf,
        helper: PathBuf,
        python: String,
        stores: Option<Arc<crate::storage::Stores>>,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            path,
            stores,
            success: Mutex::new(None),
        });
        let background = state.clone();
        tokio::spawn(async move {
            loop {
                let success = background.check(&helper, &python).await;
                *background.success.lock().unwrap() = success;
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
        });
        state
    }
    async fn check(&self, helper: &PathBuf, python: &str) -> Option<Success> {
        let stores = self.stores.as_ref()?;
        let before = Config::load(&self.path).ok()?;
        let started = now();
        let mut command = tokio::process::Command::new(python);
        command
            .arg(helper)
            .args(["probe", "--generation", &before.generation])
            .env("GENUI_RUNTIME_CONFIG_FILE", &self.path)
            .kill_on_drop(true);
        // Both real native database pools and the managed S3/KMS probe must
        // succeed for the same credential generation. Missing native storage
        // cannot inherit the older helper-only readiness result.
        let (native, managed) = tokio::join!(
            stores.readiness(),
            tokio::time::timeout(Duration::from_secs(25), command.output())
        );
        native.ok()?;
        let output = managed.ok()?.ok()?;
        if !output.status.success() || output.stdout.len() > 4096 {
            return None;
        }
        let probe: Probe = serde_json::from_slice(&output.stdout).ok()?;
        let after = Config::load(&self.path).ok()?;
        if !valid_probe(&before, &after, &probe, started, now()) {
            return None;
        }
        Some(Success {
            generation: after.generation,
            file_sha256: after.file_sha256,
            expires_at: after.expires_at,
            received: Instant::now(),
        })
    }
    pub fn ready(&self) -> bool {
        let Ok(current) = Config::load(&self.path) else {
            return false;
        };
        let guard = self.success.lock().unwrap();
        guard.as_ref().is_some_and(|success| {
            success.generation == current.generation
                && success.file_sha256 == current.file_sha256
                && success.expires_at == current.expires_at
                && now() < success.expires_at
                && success.received.elapsed() < Duration::from_secs(45)
        })
    }
}
fn valid_probe(before: &Config, after: &Config, probe: &Probe, started: u64, current: u64) -> bool {
    before.generation == after.generation
        && before.file_sha256 == after.file_sha256
        && probe.file_sha256 == after.file_sha256
        && before.expires_at == after.expires_at
        && probe.generation == after.generation
        && probe.expires_at == after.expires_at
        && probe.checked_at >= started
        && probe.checked_at <= current
        && current < after.expires_at
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_or_other_generation_probe_cannot_extend_credentials() {
        let credential = |name: &str| crate::config::DatabaseCredential {
            username: format!("genui_{name}"),
            secret: serde_json::from_str("\"test-only\"").unwrap(),
            expires_at: 200,
        };
        let before = Config {
            generation: "first".into(),
            file_sha256: "fingerprint".into(),
            expires_at: 200,
            database_host: "synthetic.invalid".into(),
            database_name: "genui".into(),
            workspace: credential("workspace"),
            control: credential("control"),
            data: credential("data"),
            worker: credential("worker"),
        };
        let mut after = before.clone();
        let mut probe = Probe {
            generation: "first".into(),
            file_sha256: "fingerprint".into(),
            expires_at: 200,
            checked_at: 150,
        };
        assert!(valid_probe(&before, &after, &probe, 140, 160));
        assert!(!valid_probe(&before, &after, &probe, 140, 200));
        probe.expires_at = 900;
        assert!(!valid_probe(&before, &after, &probe, 140, 160));
        probe.expires_at = 200;
        after.generation = "second".into();
        assert!(!valid_probe(&before, &after, &probe, 140, 160));
    }
}
