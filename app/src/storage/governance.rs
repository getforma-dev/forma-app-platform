//! APP-115 quick wins (ADR-011 pitch, `pivot/governance`, migration 0013):
//! agent access governance in storage.
//!
//! - [`AgentScope`]: the scope a member chooses when minting an agent token:
//!   every space and app (`all`, today's behavior and the default) or the
//!   selected spaces (the library pane's categories, plus `mine` for the
//!   member's own and composed apps and `folders`) and apps, and the
//!   actions the token may take. Stored on the token row; read by
//!   `sessions::resolve_agent_token` into the `AgentCaller`. It grants
//!   nothing: the MCP layer (`mcp::scope`) only refuses.
//! - The agent access log: one append-only row per MCP `tools/call`, hash
//!   chained per organization (the audit chains' construction under its own
//!   domain), written as the token's member under a per-organization
//!   transaction advisory lock.
//! - The member's governance reads: their agents with scope, use and refusal
//!   counts, and one agent's timeline, newest first. Member-scoped only: no
//!   organization admin role exists yet (APP-115 full).
use super::{
    authorization::Transaction, pool::Workspace, require, sessions::AgentCaller, valid_id, Error,
    Result, TrustedRequest,
};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

const GOVERNANCE_DEADLINE: Duration = Duration::from_secs(5);
/// The spaces a scope may name (the 0013 CHECK): the library pane's
/// categories, the member's own apps (`mine`) and folders.
pub const SCOPE_SPACES: [&str; 9] = [
    "finance",
    "security",
    "operations",
    "sales",
    "support",
    "people",
    "utilities",
    "mine",
    "folders",
];
/// The actions a scope may allow (the 0013 CHECK).
pub const SCOPE_ACTIONS: [&str; 6] = ["read", "draft", "commit", "decide", "share", "build"];
/// At most this many apps in one scope (the 0013 CHECK).
pub const MAX_SCOPE_APPS: usize = 20;
/// At most this many ids of one kind recorded per call (the 0013 CHECK).
pub const MAX_TOUCHED: usize = 20;
/// Timeline rows answered per request.
pub const ACTIVITY_LIMIT: i64 = 100;
/// Agents listed per member (live and ended).
const AGENTS_LIMIT: i64 = 50;
const ACCESS_DOMAIN: &[u8] = b"forma:agent-access:v1\x00";
const GENESIS: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const UTC: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'";

/// A token's scope. `all` with every action is today's behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentScope {
    /// Every space and app the member may use.
    pub all: bool,
    /// The selected spaces (`SCOPE_SPACES`), when not `all`.
    pub spaces: Vec<String>,
    /// The selected apps, when not `all`.
    pub apps: Vec<String>,
    /// The allowed actions (`SCOPE_ACTIONS`), never empty.
    pub actions: Vec<String>,
}
impl Default for AgentScope {
    fn default() -> Self {
        Self {
            all: true,
            spaces: Vec::new(),
            apps: Vec::new(),
            actions: SCOPE_ACTIONS.iter().map(|a| (*a).to_owned()).collect(),
        }
    }
}
impl AgentScope {
    /// The 0013 CHECKs, judged before any write.
    pub fn shaped(&self) -> bool {
        let distinct = |items: &[String]| {
            let mut sorted = items.to_vec();
            sorted.sort();
            sorted.dedup();
            sorted.len() == items.len()
        };
        let spaces_ok = self
            .spaces
            .iter()
            .all(|s| SCOPE_SPACES.contains(&s.as_str()));
        let apps_ok = self.apps.len() <= MAX_SCOPE_APPS && self.apps.iter().all(|a| valid_id(a));
        let actions_ok = !self.actions.is_empty()
            && self
                .actions
                .iter()
                .all(|a| SCOPE_ACTIONS.contains(&a.as_str()));
        let kind_ok = if self.all {
            self.spaces.is_empty() && self.apps.is_empty()
        } else {
            !self.spaces.is_empty() || !self.apps.is_empty()
        };
        spaces_ok
            && apps_ok
            && actions_ok
            && kind_ok
            && distinct(&self.spaces)
            && distinct(&self.apps)
            && distinct(&self.actions)
    }
    pub fn allows_action(&self, action: &str) -> bool {
        self.actions.iter().any(|a| a == action)
    }
    /// Whether an app in `space` (its id `app`, when known) is in scope.
    pub fn allows(&self, space: &str, app: Option<&str>) -> bool {
        self.all
            || self.spaces.iter().any(|s| s == space)
            || app.is_some_and(|app| self.apps.iter().any(|a| a == app))
    }
    pub fn document(&self) -> Value {
        json!({
            "kind": if self.all { "all" } else { "selected" },
            "spaces": self.spaces, "apps": self.apps, "actions": self.actions,
        })
    }
    pub(super) fn kind(&self) -> &'static str {
        if self.all {
            "all"
        } else {
            "selected"
        }
    }
    /// From the token row's four scope columns (as `sessions` reads them).
    pub(super) fn from_row(row: &Row, first: usize) -> Result<Self> {
        let kind: String = row.try_get(first).map_err(|_| Error::Unavailable)?;
        let get = |i: usize| -> Result<Vec<String>> {
            row.try_get::<_, Vec<String>>(first + i)
                .map_err(|_| Error::Unavailable)
        };
        let scope = Self {
            all: kind == "all",
            spaces: get(1)?,
            apps: get(2)?,
            actions: get(3)?,
        };
        require(scope.shaped()).map_err(|_| Error::Unavailable)?;
        Ok(scope)
    }
}
/// The token row's scope columns, in [`AgentScope::from_row`]'s order.
pub(super) const SCOPE_COLUMNS: &str =
    "t.scope_kind,t.scope_spaces,array(SELECT a::text FROM unnest(t.scope_apps) a),t.scope_actions";

/// One MCP `tools/call` as the log records it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccessEntry {
    pub tool: String,
    pub apps: Vec<String>,
    pub works: Vec<String>,
    pub records: Vec<String>,
    pub folders: Vec<String>,
    pub files: Vec<String>,
    /// `None` when allowed; the refusal's reason code otherwise.
    pub refused: Option<String>,
    /// The space of the resource judged (`SCOPE_SPACES`), when known.
    pub space: Option<String>,
    /// A display label of the resource (an app or work title), when known.
    pub resource_label: Option<String>,
    pub duration_ms: i32,
}
fn code_shaped(code: &str) -> bool {
    let b = code.as_bytes();
    (1..=40).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || *c == b'_')
}
fn bounded(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if valid_id(id) && !out.contains(id) && out.len() < MAX_TOUCHED {
            out.push(id.clone());
        }
    }
    out
}
fn label_of(text: &Option<String>) -> Option<String> {
    let text = text.as_deref()?.trim();
    let clean: String = text.chars().filter(|c| !c.is_control()).take(120).collect();
    (!clean.is_empty()).then_some(clean)
}

/// The hash of one log row: SHA-256 under the access domain over the
/// previous hash and the canonical row body.
pub fn access_hash(previous: &str, body: &Value) -> Result<String> {
    let canonical = crate::admission::encode(body).map_err(|_| Error::Unavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(ACCESS_DOMAIN);
    hasher.update(previous.as_bytes());
    hasher.update(&canonical);
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for byte in hasher.finalize().iter() {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

/// The canonical body of one log row (every column but `hash`).
#[allow(clippy::too_many_arguments)]
pub fn access_body(
    organization: &str,
    entry_id: &str,
    sequence: i64,
    token: &str,
    principal: &str,
    label: &str,
    entry: &AccessEntry,
    occurred: &str,
    previous: &str,
) -> Value {
    json!({
        "agent_label": label,
        "chain_sequence": sequence,
        "duration_ms": entry.duration_ms,
        "entry_id": entry_id,
        "occurred_at": occurred,
        "organization_id": organization,
        "outcome": if entry.refused.is_some() { "refused" } else { "allowed" },
        "previous_hash": previous,
        "principal_id": principal,
        "reason": entry.refused,
        "resource_label": entry.resource_label,
        "space": entry.space,
        "token_id": token,
        "tool": entry.tool,
        "touched_apps": entry.apps,
        "touched_files": entry.files,
        "touched_folders": entry.folders,
        "touched_records": entry.records,
        "touched_works": entry.works,
    })
}

/// One of the member's agents, as the governance page lists it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSummary {
    pub token_id: String,
    pub label: String,
    pub scope: AgentScope,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    /// `live`, `revoked`, `expired` or `ended` (a sign-out or a registry
    /// change ended it).
    pub status: String,
    pub calls: i64,
    pub refused: i64,
    pub last_call_at: Option<String>,
}

/// One timeline row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessRow {
    pub entry_id: String,
    pub sequence: i64,
    pub tool: String,
    pub apps: Vec<String>,
    pub works: Vec<String>,
    pub records: Vec<String>,
    pub folders: Vec<String>,
    pub files: Vec<String>,
    pub outcome: String,
    pub reason: Option<String>,
    pub space: Option<String>,
    pub resource_label: Option<String>,
    pub duration_ms: i32,
    pub occurred_at: String,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn ids(row: &Row, index: usize) -> Result<Vec<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

const CURRENT: &str = "SELECT 1 FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid AND o.status='active' AND m.status='active' AND m.membership_epoch=$3::bigint AND EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=m.organization_id AND e.principal_id=m.principal_id AND e.status='active' AND e.expires_at>clock_timestamp())";

async fn current(transaction: &Transaction<'_>, request: &TrustedRequest) -> Result<()> {
    let rows = transaction
        .query(
            CURRENT,
            &[&request.organization, &request.principal, &request.epoch],
        )
        .await?;
    require(!rows.is_empty())
}

impl Workspace {
    /// Appends one `tools/call` to the organization's agent access chain,
    /// as the caller's member, bound to the caller's own token row (another
    /// member's token writes nothing: `Denied`).
    pub async fn record_agent_access(
        &self,
        caller: &AgentCaller,
        entry: &AccessEntry,
    ) -> Result<()> {
        let request = &caller.request;
        require(
            valid_id(&caller.token_id)
                && entry.tool.starts_with("forma_")
                && code_shaped(entry.tool.trim_start_matches("forma_"))
                && entry.refused.as_deref().is_none_or(code_shaped)
                && entry.space.as_deref().is_none_or(code_shaped)
                && entry.duration_ms >= 0,
        )?;
        let entry = AccessEntry {
            apps: bounded(&entry.apps),
            works: bounded(&entry.works),
            records: bounded(&entry.records),
            folders: bounded(&entry.folders),
            files: bounded(&entry.files),
            resource_label: label_of(&entry.resource_label),
            ..entry.clone()
        };
        let cap = Instant::now() + GOVERNANCE_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let key = format!("genui:agent_access_log:{}", request.organization);
            transaction
                .query(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))::text",
                    &[&key],
                )
                .await?;
            let token = transaction.query("SELECT t.label,gen_random_uuid()::text,to_char(date_trunc('second',now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.agent_tokens t WHERE t.organization_id=$1::text::uuid AND t.token_id=$2::text::uuid AND t.principal_id=$3::text::uuid", &[&request.organization,&caller.token_id,&request.principal]).await?;
            let token = token.first().ok_or(Error::Denied)?;
            let label = text(token, 0)?;
            let entry_id = text(token, 1)?;
            let occurred = text(token, 2)?;
            let head = transaction.query("SELECT chain_sequence::bigint,hash FROM genui.agent_access_log WHERE organization_id=$1::text::uuid ORDER BY chain_sequence DESC LIMIT 1", &[&request.organization]).await?;
            let (sequence, previous) = match head.first() {
                Some(row) => {
                    let sequence: i64 = row.try_get(0).map_err(|_| Error::Unavailable)?;
                    (
                        sequence.checked_add(1).ok_or(Error::Unavailable)?,
                        text(row, 1)?,
                    )
                }
                None => (1, GENESIS.to_owned()),
            };
            let body = access_body(
                &request.organization,
                &entry_id,
                sequence,
                &caller.token_id,
                &request.principal,
                &label,
                &entry,
                &occurred,
                &previous,
            );
            let hash = access_hash(&previous, &body)?;
            let outcome = if entry.refused.is_some() { "refused" } else { "allowed" };
            transaction.query("INSERT INTO genui.agent_access_log(organization_id,entry_id,chain_sequence,token_id,principal_id,agent_label,tool,touched_apps,touched_works,touched_records,touched_folders,touched_files,outcome,reason,space,resource_label,duration_ms,occurred_at,previous_hash,hash) VALUES ($1::text::uuid,$2::text::uuid,$3::bigint,$4::text::uuid,$5::text::uuid,$6::text,$7::text,$8::text[]::uuid[],$9::text[]::uuid[],$10::text[]::uuid[],$11::text[]::uuid[],$12::text[]::uuid[],$13::text,$14::text,$15::text,$16::text,$17::int,$18::text::timestamptz,$19::text,$20::text)",
                &[&request.organization,&entry_id,&sequence,&caller.token_id,&request.principal,&label,&entry.tool,&entry.apps,&entry.works,&entry.records,&entry.folders,&entry.files,&outcome,&entry.refused,&entry.space,&entry.resource_label,&entry.duration_ms,&occurred,&previous,&hash]).await?;
            Ok(())
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The member's agents (`GET /v1/governance/agents`), newest first:
    /// live and ended tokens alike, with their scope, use and the log's
    /// call and refusal counts.
    pub async fn agent_governance(&self, request: &TrustedRequest) -> Result<Vec<AgentSummary>> {
        let cap = Instant::now() + GOVERNANCE_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            let rows = transaction.query(&format!("SELECT t.token_id::text,t.label,{SCOPE_COLUMNS},to_char(t.created_at AT TIME ZONE 'UTC',{UTC}),to_char(t.expires_at AT TIME ZONE 'UTC',{UTC}),to_char(t.last_used_at AT TIME ZONE 'UTC',{UTC}),CASE WHEN t.revoked_at IS NOT NULL THEN 'revoked' WHEN t.expires_at<=now() THEN 'expired' WHEN t.membership_epoch_at_mint<>$3::bigint OR t.registry_sha256<>$4::text THEN 'ended' ELSE 'live' END,(SELECT count(*) FROM genui.agent_access_log l WHERE l.organization_id=t.organization_id AND l.token_id=t.token_id),(SELECT count(*) FROM genui.agent_access_log l WHERE l.organization_id=t.organization_id AND l.token_id=t.token_id AND l.outcome='refused'),(SELECT to_char(max(l.occurred_at) AT TIME ZONE 'UTC',{UTC}) FROM genui.agent_access_log l WHERE l.organization_id=t.organization_id AND l.token_id=t.token_id) FROM genui.agent_tokens t WHERE t.organization_id=$1::text::uuid AND t.principal_id=$2::text::uuid ORDER BY t.created_at DESC,t.token_id LIMIT $5"), &[&request.organization,&request.principal,&request.epoch,&request.registry_sha256,&AGENTS_LIMIT]).await?;
            rows.iter()
                .map(|row| {
                    Ok(AgentSummary {
                        token_id: text(row, 0)?,
                        label: text(row, 1)?,
                        scope: AgentScope::from_row(row, 2)?,
                        created_at: text(row, 6)?,
                        expires_at: text(row, 7)?,
                        last_used_at: optional(row, 8)?,
                        status: text(row, 9)?,
                        calls: row.try_get(10).map_err(|_| Error::Unavailable)?,
                        refused: row.try_get(11).map_err(|_| Error::Unavailable)?,
                        last_call_at: optional(row, 12)?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// One of the member's agents' timeline, newest first
    /// (`GET /v1/governance/agents/{token}/activity`). Another member's, a
    /// foreign and a nonexistent token id are all `Denied`.
    pub async fn agent_activity(
        &self,
        request: &TrustedRequest,
        token_id: &str,
    ) -> Result<(AgentSummary, Vec<AccessRow>)> {
        require(valid_id(token_id))?;
        let agents = self.agent_governance(request).await?;
        let agent = agents
            .into_iter()
            .find(|a| a.token_id == token_id)
            .ok_or(Error::Denied)?;
        let cap = Instant::now() + GOVERNANCE_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            let rows = transaction.query(&format!("SELECT l.entry_id::text,l.chain_sequence::bigint,l.tool,array(SELECT a::text FROM unnest(l.touched_apps) a),array(SELECT a::text FROM unnest(l.touched_works) a),array(SELECT a::text FROM unnest(l.touched_records) a),array(SELECT a::text FROM unnest(l.touched_folders) a),array(SELECT a::text FROM unnest(l.touched_files) a),l.outcome,l.reason,l.space,l.resource_label,l.duration_ms,to_char(l.occurred_at AT TIME ZONE 'UTC',{UTC}) FROM genui.agent_access_log l WHERE l.organization_id=$1::text::uuid AND l.token_id=$2::text::uuid AND l.principal_id=$3::text::uuid ORDER BY l.chain_sequence DESC LIMIT $4"), &[&request.organization,&token_id,&request.principal,&ACTIVITY_LIMIT]).await?;
            rows.iter()
                .map(|row| {
                    Ok(AccessRow {
                        entry_id: text(row, 0)?,
                        sequence: row.try_get(1).map_err(|_| Error::Unavailable)?,
                        tool: text(row, 2)?,
                        apps: ids(row, 3)?,
                        works: ids(row, 4)?,
                        records: ids(row, 5)?,
                        folders: ids(row, 6)?,
                        files: ids(row, 7)?,
                        outcome: text(row, 8)?,
                        reason: optional(row, 9)?,
                        space: optional(row, 10)?,
                        resource_label: optional(row, 11)?,
                        duration_ms: row.try_get(12).map_err(|_| Error::Unavailable)?,
                        occurred_at: text(row, 13)?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        Ok((agent, result?))
    }

    /// Test executable only: whether the organization's whole agent access
    /// chain verifies (sequence, link and hash of every row).
    #[cfg(test)]
    pub(crate) async fn agent_access_chain_verifies(
        &self,
        request: &TrustedRequest,
    ) -> Result<(bool, i64)> {
        let cap = Instant::now() + GOVERNANCE_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let rows = transaction.query(&format!("SELECT l.entry_id::text,l.chain_sequence::bigint,l.token_id::text,l.principal_id::text,l.agent_label,l.tool,array(SELECT a::text FROM unnest(l.touched_apps) a),array(SELECT a::text FROM unnest(l.touched_works) a),array(SELECT a::text FROM unnest(l.touched_records) a),array(SELECT a::text FROM unnest(l.touched_folders) a),array(SELECT a::text FROM unnest(l.touched_files) a),l.reason,l.space,l.resource_label,l.duration_ms,to_char(l.occurred_at AT TIME ZONE 'UTC',{UTC}),l.previous_hash,l.hash,l.outcome FROM genui.agent_access_log l WHERE l.organization_id=$1::text::uuid ORDER BY l.chain_sequence"), &[&request.organization]).await?;
            let mut previous = GENESIS.to_owned();
            let mut expected = 1_i64;
            for row in &rows {
                let sequence: i64 = row.try_get(1).map_err(|_| Error::Unavailable)?;
                let entry = AccessEntry {
                    tool: text(row, 5)?,
                    apps: ids(row, 6)?,
                    works: ids(row, 7)?,
                    records: ids(row, 8)?,
                    folders: ids(row, 9)?,
                    files: ids(row, 10)?,
                    refused: optional(row, 11)?,
                    space: optional(row, 12)?,
                    resource_label: optional(row, 13)?,
                    duration_ms: row.try_get(14).map_err(|_| Error::Unavailable)?,
                };
                let outcome = text(row, 18)?;
                let body = access_body(
                    &request.organization,
                    &text(row, 0)?,
                    sequence,
                    &text(row, 2)?,
                    &text(row, 3)?,
                    &text(row, 4)?,
                    &entry,
                    &text(row, 15)?,
                    &previous,
                );
                let stored_previous = text(row, 16)?;
                let hash = text(row, 17)?;
                let consistent_outcome = (outcome == "refused") == entry.refused.is_some();
                if sequence != expected
                    || stored_previous != previous
                    || access_hash(&previous, &body)? != hash
                    || !consistent_outcome
                {
                    return Ok((false, sequence));
                }
                previous = hash;
                expected += 1;
            }
            Ok((true, expected - 1))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_shaped_like_the_migration() {
        assert!(AgentScope::default().shaped());
        let security = AgentScope {
            all: false,
            spaces: vec!["security".into()],
            apps: vec![],
            actions: vec!["read".into(), "draft".into()],
        };
        assert!(security.shaped());
        assert!(security.allows("security", None));
        assert!(!security.allows("people", Some("00000000-0000-4000-8000-000000001701")));
        assert!(security.allows_action("draft") && !security.allows_action("decide"));
        for bad in [
            AgentScope {
                all: false,
                ..AgentScope::default()
            },
            AgentScope {
                spaces: vec!["security".into()],
                ..AgentScope::default()
            },
            AgentScope {
                actions: vec![],
                ..AgentScope::default()
            },
            AgentScope {
                actions: vec!["delete".into()],
                ..AgentScope::default()
            },
            AgentScope {
                all: false,
                spaces: vec!["hr".into()],
                ..AgentScope::default()
            },
            AgentScope {
                all: false,
                spaces: vec!["people".into(), "people".into()],
                ..AgentScope::default()
            },
            AgentScope {
                all: false,
                apps: vec!["not-a-uuid".into()],
                ..AgentScope::default()
            },
        ] {
            assert!(!bad.shaped(), "{bad:?}");
        }
    }

    #[test]
    fn the_chain_hash_covers_every_column() {
        let entry = AccessEntry {
            tool: "forma_draft_record".into(),
            apps: vec!["00000000-0000-4000-8000-000000001701".into()],
            refused: Some("scope_space".into()),
            space: Some("people".into()),
            resource_label: Some("Time off".into()),
            duration_ms: 12,
            ..AccessEntry::default()
        };
        let body = |entry: &AccessEntry| {
            access_body(
                "o",
                "e",
                1,
                "t",
                "p",
                "Claude",
                entry,
                "2026-10-30T00:00:00Z",
                GENESIS,
            )
        };
        let hash = access_hash(GENESIS, &body(&entry)).unwrap();
        assert!(hash.starts_with("sha256:") && hash.len() == 71);
        let mut edited = entry.clone();
        edited.refused = None;
        assert_ne!(hash, access_hash(GENESIS, &body(&edited)).unwrap());
        let mut edited = entry.clone();
        edited.duration_ms = 13;
        assert_ne!(hash, access_hash(GENESIS, &body(&edited)).unwrap());
        assert_ne!(hash, access_hash(&hash, &body(&entry)).unwrap());
    }
}
