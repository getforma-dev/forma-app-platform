//! Held MCP drafts (APP-099; APP-098 DS-49: in process memory, a restart or
//! a second task loses them). A draft is the action path's resolved,
//! storage-dry-run command ([`crate::actions::resolve::Draft`]: typed values,
//! the server-minted idempotency key, the resolved approver), held for its
//! member and agent token for `DRAFT_TTL` under an unguessable id. The agent
//! commits a draft only by that id: it cannot commit values it did not
//! draft, and the commit re-judges everything in storage.
//!
//! A draft files at most one record: the first `forma_commit` marks it
//! spent; a repeated `forma_commit` of the same id within the TTL resubmits
//! the same command with the same key, which storage answers with the stored
//! outcome (`replayed`), never a second record. An expired id, an unknown
//! id and another member's or another token's id are the same refusal.
use crate::{actions::resolve::Draft, auth::capability, storage::AgentCaller};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// How long a draft may be committed.
pub const DRAFT_TTL: Duration = Duration::from_secs(600);
/// At most this many live drafts per member, and in the process.
const MAX_PER_MEMBER: usize = 20;
const MAX_TOTAL: usize = 2000;

#[derive(Clone, Debug)]
pub struct Held {
    pub draft: Draft,
    /// The first commit happened (later commits replay it).
    pub spent: bool,
}

struct Entry {
    organization: String,
    principal: String,
    token_id: String,
    created: Instant,
    held: Held,
}

#[derive(Default)]
pub struct Drafts {
    entries: Mutex<HashMap<String, Entry>>,
}

/// A fresh draft id: 128 random bits as 32 lowercase hex digits.
fn fresh_id() -> Option<String> {
    let bytes = capability::random_bytes::<16>()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
/// The exact shape of a draft id.
pub fn draft_id_shaped(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Drafts {
    fn prune(entries: &mut HashMap<String, Entry>, now: Instant) {
        entries.retain(|_, entry| now.duration_since(entry.created) < DRAFT_TTL);
    }

    /// Holds `draft` for the caller; `None` when the member or the process
    /// holds too many live drafts, or no random id could be drawn.
    pub fn hold(&self, caller: &AgentCaller, draft: Draft) -> Option<String> {
        self.hold_at(caller, draft, Instant::now())
    }
    pub(crate) fn hold_at(
        &self,
        caller: &AgentCaller,
        draft: Draft,
        now: Instant,
    ) -> Option<String> {
        let mut entries = self.entries.lock().ok()?;
        Self::prune(&mut entries, now);
        let organization = caller.request.organization();
        let principal = caller.request.principal();
        let mine = entries
            .values()
            .filter(|e| e.organization == organization && e.principal == principal)
            .count();
        if mine >= MAX_PER_MEMBER || entries.len() >= MAX_TOTAL {
            return None;
        }
        let id = fresh_id()?;
        entries.insert(
            id.clone(),
            Entry {
                organization: organization.to_owned(),
                principal: principal.to_owned(),
                token_id: caller.token_id.clone(),
                created: now,
                held: Held {
                    draft,
                    spent: false,
                },
            },
        );
        Some(id)
    }

    /// The caller's live draft `id`, or nothing (expired, unknown, another
    /// member's or another token's alike).
    pub fn get(&self, caller: &AgentCaller, id: &str) -> Option<Held> {
        self.get_at(caller, id, Instant::now())
    }
    pub(crate) fn get_at(&self, caller: &AgentCaller, id: &str, now: Instant) -> Option<Held> {
        if !draft_id_shaped(id) {
            return None;
        }
        let mut entries = self.entries.lock().ok()?;
        Self::prune(&mut entries, now);
        let entry = entries.get(id)?;
        (entry.organization == caller.request.organization()
            && entry.principal == caller.request.principal()
            && entry.token_id == caller.token_id)
            .then(|| entry.held.clone())
    }

    /// Marks the caller's draft `id` spent (its first commit happened).
    pub fn spend(&self, caller: &AgentCaller, id: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            if let Some(entry) = entries.get_mut(id) {
                if entry.principal == caller.request.principal()
                    && entry.token_id == caller.token_id
                {
                    entry.held.spent = true;
                }
            }
        }
    }

    /// Live drafts in the process (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.lock().map(|e| e.len()).unwrap_or(0)
    }
}
