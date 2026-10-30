//! APP-099 (ADR-011 pitch, `pivot/mcp`): the member's agent tokens and their
//! display attributions (migration 0010; `expires_at` since 0012). Resolving a token into a
//! `TrustedRequest` is `sessions` (the only constructor); this module mints,
//! lists and revokes a member's own tokens under the member's own trusted
//! request (the canvas session), and records and reads which token produced
//! a committed record version or a composed saved work, for the canvas's
//! "via <label>" chip.
//!
//! Every statement runs as `genui_workspace` under the request's tenant scope
//! after the currency read (an active organization and membership at the
//! request's epoch with a live entitlement), and names the member's own
//! principal: another member's token id, a foreign token id and a
//! nonexistent one are the same `Denied`. A token row is never deleted
//! (no DELETE grant): revocation sets `revoked_at`, and `sessions` refuses
//! it from then on.
use super::{
    authorization::Transaction,
    pool::Workspace,
    require,
    sessions::{agent_token_digest, fresh_agent_token, AgentCaller},
    valid_id, Error, Result, TrustedRequest,
};
use std::time::Duration;
use tokio::time::Instant;
use tokio_postgres::Row;

/// ADR-007 D7.14 handler deadline; each transaction here stays inside it.
const AGENT_HANDLER_DEADLINE: Duration = Duration::from_secs(5);
/// At most this many live tokens per member.
pub const MAX_LIVE_AGENT_TOKENS: i64 = 10;
/// A token lives this many days from minting (APP-113 review; 0012).
pub const AGENT_TOKEN_DAYS: i32 = 30;
/// A token that `sessions::resolve_agent_token` would still accept, as far
/// as its own row says: unrevoked, unexpired, minted at the member's current
/// epoch (`$3`) under this registry identity (`$4`). Counts and the canvas
/// list use it, so a token a sign-out or a registry change ended neither
/// fills the member's quota nor shows as connected (APP-113 review).
const LIVE: &str = "revoked_at IS NULL AND expires_at>now() AND membership_epoch_at_mint=$3::bigint AND registry_sha256=$4::text";
/// At most this many attributions are read for one work.
const ATTRIBUTION_LIMIT: i64 = 500;

const CURRENT: &str = "SELECT 1 FROM genui.organizations o JOIN genui.memberships m USING(organization_id) WHERE o.organization_id=$1::text::uuid AND m.principal_id=$2::text::uuid AND o.status='active' AND m.status='active' AND m.membership_epoch=$3::bigint AND EXISTS(SELECT 1 FROM genui.entitlements e WHERE e.organization_id=m.organization_id AND e.principal_id=m.principal_id AND e.status='active' AND e.expires_at>clock_timestamp())";

/// A label the member gives a token: 1 to 40 characters, a letter or digit
/// first, then letters, digits, spaces, `.`, `_` or `-` (the 0010 CHECK).
pub fn agent_label_shaped(label: &str) -> bool {
    let bytes = label.as_bytes();
    (1..=40).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'.' | b'_' | b'-'))
}

/// A freshly minted token: the raw value is returned once, never stored.
pub struct MintedAgentToken {
    pub token_id: String,
    pub label: String,
    pub created_at: String,
    pub expires_at: String,
    pub token: String,
}
impl std::fmt::Debug for MintedAgentToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MintedAgentToken")
            .field("token_id", &self.token_id)
            .finish_non_exhaustive()
    }
}

/// One of the member's live tokens, as the canvas lists it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentToken {
    pub token_id: String,
    pub label: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
}

/// One display attribution of a work: the record version an agent token
/// committed (or the work itself, composed through MCP) and that token's
/// label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAttribution {
    pub record: Option<(String, i64)>,
    pub label: String,
}

fn text(row: &Row, index: usize) -> Result<String> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}
fn optional(row: &Row, index: usize) -> Result<Option<String>> {
    row.try_get(index).map_err(|_| Error::Unavailable)
}

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
    /// Mints an agent token for the member (`POST /v1/agent-tokens`): the
    /// currency read, the member's token writes serialized, at most
    /// `MAX_LIVE_AGENT_TOKENS` live tokens, then one row bound to the member,
    /// the request's epoch and registry identity, expiring in
    /// `AGENT_TOKEN_DAYS` days.
    pub async fn mint_agent_token(
        &self,
        request: &TrustedRequest,
        label: &str,
    ) -> Result<MintedAgentToken> {
        require(agent_label_shaped(label))?;
        let token = fresh_agent_token(&request.organization)?;
        let cap = Instant::now() + AGENT_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            super::boxes::serialize_member(&transaction, request, "agent_tokens").await?;
            let live = transaction.query(&format!("SELECT count(*) FROM genui.agent_tokens WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND {LIVE}"), &[&request.organization,&request.principal,&request.epoch,&request.registry_sha256]).await?;
            let live: i64 = live
                .first()
                .ok_or(Error::Unavailable)?
                .try_get(0)
                .map_err(|_| Error::Unavailable)?;
            require(live < MAX_LIVE_AGENT_TOKENS)?;
            let rows = transaction.query("INSERT INTO genui.agent_tokens(organization_id,token_id,token_digest,principal_id,label,membership_epoch_at_mint,registry_sha256,created_at,expires_at) VALUES ($1::text::uuid,gen_random_uuid(),$2::text,$3::text::uuid,$4::text,$5::bigint,$6::text,now(),now()+make_interval(days=>$7)) RETURNING token_id::text,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
                &[&request.organization,&agent_token_digest(&token),&request.principal,&label,&request.epoch,&request.registry_sha256,&AGENT_TOKEN_DAYS]).await?;
            let row = rows.first().ok_or(Error::Unavailable)?;
            Ok((text(row, 0)?, text(row, 1)?, text(row, 2)?))
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        let (token_id, created_at, expires_at) = result?;
        Ok(MintedAgentToken {
            token_id,
            label: label.to_owned(),
            created_at,
            expires_at,
            token,
        })
    }

    /// The member's live tokens, newest first (`GET /v1/agent-tokens`).
    pub async fn agent_tokens(&self, request: &TrustedRequest) -> Result<Vec<AgentToken>> {
        let cap = Instant::now() + AGENT_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            let rows = transaction.query(&format!("SELECT token_id::text,label,to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),to_char(last_used_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM genui.agent_tokens WHERE organization_id=$1::text::uuid AND principal_id=$2::text::uuid AND {LIVE} ORDER BY created_at DESC,token_id LIMIT $5"), &[&request.organization,&request.principal,&request.epoch,&request.registry_sha256,&MAX_LIVE_AGENT_TOKENS]).await?;
            rows.iter()
                .map(|row| {
                    Ok(AgentToken {
                        token_id: text(row, 0)?,
                        label: text(row, 1)?,
                        created_at: text(row, 2)?,
                        expires_at: text(row, 4)?,
                        last_used_at: optional(row, 3)?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Revokes one of the member's own live tokens
    /// (`DELETE /v1/agent-tokens/{id}`). Another member's, a revoked, a
    /// foreign and a nonexistent token id are all `Denied`.
    pub async fn revoke_agent_token(&self, request: &TrustedRequest, token_id: &str) -> Result<()> {
        require(valid_id(token_id))?;
        let cap = Instant::now() + AGENT_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            current(&transaction, request).await?;
            let rows = transaction.query("UPDATE genui.agent_tokens SET revoked_at=now() WHERE organization_id=$1::text::uuid AND token_id=$2::text::uuid AND principal_id=$3::text::uuid AND revoked_at IS NULL RETURNING 1", &[&request.organization,&token_id,&request.principal]).await?;
            require(rows.len() == 1)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// Records that the caller's token committed `record` at `version` on
    /// the work, or composed the work (`record` absent). Display provenance
    /// only: written after the audited commit, by the caller's own live
    /// token row; a revoked token writes nothing (`Denied`).
    pub async fn attribute_to_agent(
        &self,
        caller: &AgentCaller,
        app: &str,
        work: &str,
        record: Option<(&str, i64)>,
    ) -> Result<()> {
        let request = &caller.request;
        require(
            valid_id(app)
                && valid_id(work)
                && valid_id(&caller.token_id)
                && record.is_none_or(|(id, version)| valid_id(id) && version >= 1),
        )?;
        let (record_id, version) = match record {
            Some((id, version)) => (Some(id.to_owned()), Some(version)),
            None => (None, None),
        };
        let cap = Instant::now() + AGENT_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let rows = transaction.query("INSERT INTO genui.agent_attributions(organization_id,attribution_id,token_id,app_id,work_id,record_id,record_version,created_at) SELECT t.organization_id,gen_random_uuid(),t.token_id,$3::text::uuid,$4::text::uuid,$5::text::uuid,$6::bigint,now() FROM genui.agent_tokens t WHERE t.organization_id=$1::text::uuid AND t.token_id=$2::text::uuid AND t.principal_id=$7::text::uuid AND t.revoked_at IS NULL RETURNING 1",
                &[&request.organization,&caller.token_id,&app,&work,&record_id,&version,&request.principal]).await?;
            require(rows.len() == 1)
        }
        .await;
        transaction.finish(result.is_ok()).await?;
        result
    }

    /// The display attributions of one work, oldest first. The caller has
    /// already authorized the member's read of the work (the records route
    /// calls this only after `Data::list_records` answered); labels are
    /// display text the members chose, never authority.
    pub async fn agent_attributions(
        &self,
        request: &TrustedRequest,
        app: &str,
        work: &str,
    ) -> Result<Vec<AgentAttribution>> {
        require(valid_id(app) && valid_id(work))?;
        let cap = Instant::now() + AGENT_HANDLER_DEADLINE;
        let transaction = Transaction::begin_until(&self.inner, request, cap).await?;
        let result = async {
            let rows = transaction.query("SELECT a.record_id::text,a.record_version,t.label FROM genui.agent_attributions a JOIN genui.agent_tokens t ON t.organization_id=a.organization_id AND t.token_id=a.token_id WHERE a.organization_id=$1::text::uuid AND a.app_id=$2::text::uuid AND a.work_id=$3::text::uuid ORDER BY a.created_at,a.attribution_id LIMIT $4", &[&request.organization,&app,&work,&ATTRIBUTION_LIMIT]).await?;
            rows.iter()
                .map(|row| {
                    let record: Option<String> = optional(row, 0)?;
                    let version: Option<i64> = row.try_get(1).map_err(|_| Error::Unavailable)?;
                    Ok(AgentAttribution {
                        record: record.zip(version),
                        label: text(row, 2)?,
                    })
                })
                .collect::<Result<Vec<_>>>()
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
    fn labels_are_short_plain_names() {
        for good in ["Claude", "OpenAI", "Cursor 1.2", "my-agent_v2", "A"] {
            assert!(agent_label_shaped(good), "{good}");
        }
        for bad in [
            "",
            " Claude",
            "<script>",
            "Claude\n",
            "Clau'de",
            "x".repeat(41).as_str(),
            "Ünïcode",
        ] {
            assert!(!agent_label_shaped(bad), "{bad}");
        }
    }
}
