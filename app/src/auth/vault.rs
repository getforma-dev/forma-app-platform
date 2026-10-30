//! D7.11 refresh-token custody: the fAuth refresh token (and the current
//! access token) live only here, sealed under the process-memory key and
//! keyed by a random reference that the session row stores. Nothing here is
//! written to disk, a row, a log or a response; a restart empties the vault
//! (D7.18). Refresh is serialized per entry, replaces both tokens atomically
//! and never presents a token again after any answer but success (ADR-006).
//! Every entry records its creation time; the expiry sweep removes entries
//! older than the workspace session's 12-hour absolute lifetime, which no
//! live row can reference (APP-009 review m5: an entry inserted by a callback
//! cancelled before its row committed is otherwise referenced by nothing).
use super::{
    capability::{random_uuid, Sealer},
    provider::{Client, RefreshFailure, Token},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

/// Bound on live entries (pre-access plus workspace sessions in one task).
const MAX_ENTRIES: usize = 10_000;
/// An access token this close to expiry is refreshed before use.
const ACCESS_MARGIN_SECONDS: u64 = 30;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct Tokens {
    refresh: Vec<u8>,
    access: Vec<u8>,
    access_expires_at: u64,
    /// Set after any refresh answer but success: never presented again.
    spent: bool,
}
struct Entry {
    slug: String,
    created_at: u64,
    tokens: tokio::sync::Mutex<Tokens>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessFailure {
    /// No entry: a restart, or the record was already destroyed.
    Missing,
    /// The provider refused the refresh (401).
    Unauthorized,
    /// Any other refresh failure; the entry is now spent.
    Failed,
}

pub struct Vault {
    sealer: Sealer,
    entries: Mutex<HashMap<String, Arc<Entry>>>,
}
impl Vault {
    pub fn new() -> Option<Self> {
        Some(Self {
            sealer: Sealer::new()?,
            entries: Mutex::new(HashMap::new()),
        })
    }
    fn context(reference: &str, kind: &str) -> Vec<u8> {
        format!("forma:vault:v1\0{reference}\0{kind}").into_bytes()
    }
    /// Stores an exchanged token pair; `None` when full or sealing fails.
    pub fn insert(&self, slug: &str, token: &Token) -> Option<String> {
        let reference = random_uuid()?;
        let tokens = Tokens {
            refresh: self.sealer.seal(
                &Self::context(&reference, "refresh"),
                token.refresh_token.as_bytes(),
            )?,
            access: self.sealer.seal(
                &Self::context(&reference, "access"),
                token.access_token.as_bytes(),
            )?,
            access_expires_at: token.expires_at,
            spent: false,
        };
        let mut entries = self.entries.lock().ok()?;
        if entries.len() >= MAX_ENTRIES {
            return None;
        }
        entries.insert(
            reference.clone(),
            Arc::new(Entry {
                slug: slug.to_owned(),
                created_at: now(),
                tokens: tokio::sync::Mutex::new(tokens),
            }),
        );
        Some(reference)
    }
    pub fn contains(&self, reference: &str) -> bool {
        self.entries
            .lock()
            .map(|e| e.contains_key(reference))
            .unwrap_or(false)
    }
    pub fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn entry(&self, reference: &str) -> Option<Arc<Entry>> {
        self.entries.lock().ok()?.get(reference).cloned()
    }
    /// A current access token, refreshing first when it is near expiry.
    pub async fn access(
        &self,
        reference: &str,
        provider: &Client,
    ) -> Result<String, AccessFailure> {
        let entry = self.entry(reference).ok_or(AccessFailure::Missing)?;
        let mut tokens = entry.tokens.lock().await;
        if tokens.spent {
            return Err(AccessFailure::Failed);
        }
        if tokens.access_expires_at > now() + ACCESS_MARGIN_SECONDS {
            return self
                .open(reference, "access", &tokens.access)
                .ok_or(AccessFailure::Failed);
        }
        let refresh = self
            .open(reference, "refresh", &tokens.refresh)
            .ok_or(AccessFailure::Failed)?;
        // Mark first: a cancelled or failed request may have consumed it.
        tokens.spent = true;
        let fresh =
            provider
                .refresh(&entry.slug, &refresh)
                .await
                .map_err(|failure| match failure {
                    RefreshFailure::Unauthorized => AccessFailure::Unauthorized,
                    RefreshFailure::Failed => AccessFailure::Failed,
                })?;
        if !super::token_shaped(&fresh) {
            return Err(AccessFailure::Failed);
        }
        *tokens = Tokens {
            refresh: self
                .sealer
                .seal(
                    &Self::context(reference, "refresh"),
                    fresh.refresh_token.as_bytes(),
                )
                .ok_or(AccessFailure::Failed)?,
            access: self
                .sealer
                .seal(
                    &Self::context(reference, "access"),
                    fresh.access_token.as_bytes(),
                )
                .ok_or(AccessFailure::Failed)?,
            access_expires_at: fresh.expires_at,
            spent: false,
        };
        Ok(fresh.access_token)
    }
    /// Removes an entry, returning its tenant slug and refresh token for the
    /// provider logout; `None` if absent or already spent.
    pub async fn take(&self, reference: &str) -> Option<(String, String)> {
        let entry = self.entries.lock().ok()?.remove(reference)?;
        let tokens = entry.tokens.lock().await;
        if tokens.spent {
            return None;
        }
        let refresh = self.open(reference, "refresh", &tokens.refresh)?;
        Some((entry.slug.clone(), refresh))
    }
    /// References of entries created more than `max_age_seconds` ago.
    pub fn stale(&self, max_age_seconds: u64) -> Vec<String> {
        let cutoff = now().saturating_sub(max_age_seconds);
        self.entries
            .lock()
            .map(|entries| {
                entries
                    .iter()
                    .filter(|(_, entry)| entry.created_at < cutoff)
                    .map(|(reference, _)| reference.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Test-only: move an entry's creation time into the past.
    #[cfg(test)]
    pub fn backdate(&self, reference: &str, seconds: u64) {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.remove(reference).unwrap();
        let entry = Arc::try_unwrap(entry).ok().unwrap();
        entries.insert(
            reference.to_owned(),
            Arc::new(Entry {
                created_at: entry.created_at - seconds,
                ..entry
            }),
        );
    }
    fn open(&self, reference: &str, kind: &str, sealed: &[u8]) -> Option<String> {
        String::from_utf8(self.sealer.open(&Self::context(reference, kind), sealed)?).ok()
    }
}
