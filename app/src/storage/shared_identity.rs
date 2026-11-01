//! Account-only identity sessions. These never construct a tenant request or
//! create membership, entitlement, principal or organization permission rows.
use super::{
    admission::domain_digest,
    authorization::Transaction,
    pool::Workspace,
    require,
    sessions::{capability, capability_organization, capability_shaped},
    shared_access::{normalize_share_email, share_locator},
    valid_id, Error, Result,
};
use genui_auth_verifier::VerifiedIdentity;
use sha2::{Digest, Sha256};

const SESSION_DOMAIN: &[u8] = b"forma:shared-session:v1";
const FLOW_DOMAIN: &[u8] = b"forma:shared-flow:v1";
const STATE_DOMAIN: &[u8] = b"forma:shared-state:v1";
fn digest(domain: &[u8], value: &str) -> String {
    domain_digest(domain, value.as_bytes())
}

/// Only a successfully verified, email-verified provider exchange can construct
/// this proof. Email is metadata from that same authenticated token response.
#[derive(Clone)]
pub struct ShareIdentityProof {
    issuer: String,
    tenant: String,
    subject: String,
    email: String,
}
impl ShareIdentityProof {
    pub(crate) fn verified(
        identity: &VerifiedIdentity,
        email: &str,
        email_verified: bool,
    ) -> Option<Self> {
        if !email_verified {
            return None;
        }
        Some(Self {
            issuer: identity.issuer.clone(),
            tenant: identity.tenant_id.clone(),
            subject: identity.subject.clone(),
            email: normalize_share_email(email)?,
        })
    }
    fn account_id(&self) -> String {
        let bytes = Sha256::digest(format!(
            "forma:shared-account:v1\0{}\0{}\0{}",
            self.issuer, self.tenant, self.subject
        ));
        // Internal content-derived identifier in the catalog's UUID shape;
        // the UUID is only an RLS selector, never an authentication capability.
        let mut id = bytes[..16].to_vec();
        id[6] = (id[6] & 15) | 0x40;
        id[8] = (id[8] & 63) | 0x80;
        let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..]
        )
    }
}
impl std::fmt::Debug for ShareIdentityProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShareIdentityProof([redacted])")
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct ShareAccount {
    pub(super) account_id: String,
    pub(super) session_id: String,
    pub(super) account_epoch: i64,
    pub(super) refresh_ref: String,
    pub(super) verified_email: String,
}
impl ShareAccount {
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn account_epoch(&self) -> i64 {
        self.account_epoch
    }
    pub fn refresh_ref(&self) -> &str {
        &self.refresh_ref
    }
    pub fn verified_email(&self) -> &str {
        &self.verified_email
    }
}
impl std::fmt::Debug for ShareAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShareAccount([redacted])")
    }
}
pub struct SharedFlowStart {
    pub cookie: String,
    pub state: String,
}
pub struct SharedFlow {
    pub(crate) key: String,
    pub(crate) provider_organization: String,
    pub(crate) sealed_verifier: Vec<u8>,
}

impl Workspace {
    /// Bounded operator-owned cleanup; the definer reveals only vault references.
    pub async fn sweep_shared_identity(&self) -> Result<Vec<String>> {
        let tx = Transaction::begin_shared(&self.inner, None, None).await?;
        let result: Result<Vec<String>> = async {
            let rows = tx
                .query(
                    "SELECT refresh_token_ref::text FROM genui_private.expire_shared_identity()",
                    &[],
                )
                .await?;
            rows.iter()
                .map(|row| row.try_get(0).map_err(|_| Error::Unavailable))
                .collect()
        }
        .await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn begin_shared_flow(
        &self,
        key: &str,
        provider_organization: &str,
        sealed_verifier: &[u8],
    ) -> Result<SharedFlowStart> {
        let (owner, share) = share_locator(key)?;
        require(
            valid_id(provider_organization)
                && !sealed_verifier.is_empty()
                && sealed_verifier.len() <= 512,
        )?;
        let flow_id = super::sessions::fresh_uuid()?;
        let cookie = capability(&flow_id)?;
        let state = super::sessions::fresh_state()?;
        let tx = Transaction::begin_shared(&self.inner, None, Some(&flow_id)).await?;
        let result=tx.query("INSERT INTO genui_identity.shared_auth_flows(flow_id,owner_organization_id,share_id,provider_organization_id,cookie_digest,state_digest,sealed_verifier) VALUES($1::text::uuid,$2::text::uuid,$3::text::uuid,$4::text::uuid,$5::text,$6::text,$7)", &[&flow_id,&owner,&share,&provider_organization,&digest(FLOW_DOMAIN,&cookie),&digest(STATE_DOMAIN,&state),&sealed_verifier]).await;
        tx.finish(result.is_ok()).await?;
        result?;
        Ok(SharedFlowStart { cookie, state })
    }
    pub async fn complete_shared_flow(&self, cookie: &str, state: &str) -> Result<SharedFlow> {
        let flow_id = capability_organization(cookie).ok_or(Error::Denied)?;
        require(capability_shaped(state))?;
        let tx = Transaction::begin_shared(&self.inner, None, Some(&flow_id)).await?;
        let result=async {
            let rows=tx.query("DELETE FROM genui_identity.shared_auth_flows WHERE flow_id=$1::text::uuid AND cookie_digest=$2 AND state_digest=$3 AND consumed_at IS NULL AND expires_at>clock_timestamp() RETURNING owner_organization_id::text,share_id::text,provider_organization_id::text,sealed_verifier", &[&flow_id,&digest(FLOW_DOMAIN,cookie),&digest(STATE_DOMAIN,state)]).await?;
            let row=rows.first().ok_or(Error::Denied)?;
            let owner:String=row.try_get(0).map_err(|_|Error::Unavailable)?;
            let share:String=row.try_get(1).map_err(|_|Error::Unavailable)?;
            Ok(SharedFlow { key:format!("{owner}.{share}"),provider_organization:row.try_get(2).map_err(|_|Error::Unavailable)?,sealed_verifier:row.try_get(3).map_err(|_|Error::Unavailable)? })
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn discard_shared_flow(&self, cookie: &str) -> Result<()> {
        let flow_id = capability_organization(cookie).ok_or(Error::Denied)?;
        let tx = Transaction::begin_shared(&self.inner, None, Some(&flow_id)).await?;
        let result=tx.query("DELETE FROM genui_identity.shared_auth_flows WHERE flow_id=$1::text::uuid AND cookie_digest=$2", &[&flow_id,&digest(FLOW_DOMAIN,cookie)]).await;
        tx.finish(result.is_ok()).await?;
        result.map(|_| ())
    }
    #[cfg(test)]
    pub async fn mint_shared_session(
        &self,
        proof: &ShareIdentityProof,
        refresh_ref: &str,
    ) -> Result<String> {
        self.mint_shared_session_live(proof, refresh_ref, &|_| true)
            .await
    }
    pub async fn mint_shared_session_live(
        &self,
        proof: &ShareIdentityProof,
        refresh_ref: &str,
        refresh_live: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<String> {
        require(valid_id(refresh_ref))?;
        let account = proof.account_id();
        let cookie = capability(&account)?;
        let tx = Transaction::begin_shared(&self.inner, None, Some(&account)).await?;
        let result=async {
            let rows=tx.query("INSERT INTO genui_identity.accounts(account_id,issuer,provider_tenant_id,provider_subject,verified_email) VALUES($1::text::uuid,$2,$3,$4,$5) ON CONFLICT(account_id) DO UPDATE SET account_epoch=accounts.account_epoch+CASE WHEN accounts.verified_email IS DISTINCT FROM EXCLUDED.verified_email THEN 1 ELSE 0 END,verified_email=EXCLUDED.verified_email,updated_at=clock_timestamp() WHERE accounts.issuer=EXCLUDED.issuer AND accounts.provider_tenant_id=EXCLUDED.provider_tenant_id AND accounts.provider_subject=EXCLUDED.provider_subject AND accounts.status='active' AND (accounts.verified_email IS NOT DISTINCT FROM EXCLUDED.verified_email OR accounts.account_epoch<9007199254740991) RETURNING account_epoch::bigint", &[&account,&proof.issuer,&proof.tenant,&proof.subject,&proof.email]).await?;
            let epoch:i64=rows.first().ok_or(Error::Denied)?.try_get(0).map_err(|_|Error::Unavailable)?;
            // The account UPSERT above holds its row lock through this count and
            // insertion, serializing concurrent mint attempts for this identity.
            let sessions=tx.query("SELECT session_id::text,refresh_token_ref::text FROM genui_identity.shared_sessions WHERE account_id=$1::text::uuid AND account_epoch=$2::bigint AND revoked_at IS NULL AND expires_at>clock_timestamp() AND last_seen_at>clock_timestamp()-interval '1 hour'", &[&account,&epoch]).await?;
            let mut active=0;
            for session in &sessions {
                let id:String=session.try_get(0).map_err(|_|Error::Unavailable)?;
                let reference:String=session.try_get(1).map_err(|_|Error::Unavailable)?;
                if refresh_live(&reference) {active+=1;} else {
                    tx.query("DELETE FROM genui_identity.shared_sessions WHERE account_id=$1::text::uuid AND session_id=$2::text::uuid", &[&account,&id]).await?;
                }
            }
            require(active<8)?;
            tx.query("INSERT INTO genui_identity.shared_sessions(account_id,session_id,cookie_digest,refresh_token_ref,account_epoch) VALUES($1::text::uuid,gen_random_uuid(),$2::text,$3::text::uuid,$4::bigint)", &[&account,&digest(SESSION_DOMAIN,&cookie),&refresh_ref,&epoch]).await?;
            Ok(())
        }.await;
        tx.finish(result.is_ok()).await?;
        result?;
        Ok(cookie)
    }
    pub async fn resolve_shared_session(
        &self,
        cookie: &str,
        refresh_live: &(dyn Fn(&str) -> bool + Sync),
    ) -> Result<Option<ShareAccount>> {
        let Some(account) = capability_organization(cookie) else {
            return Ok(None);
        };
        let tx = Transaction::begin_shared(&self.inner, None, Some(&account)).await?;
        let result=async {
            let rows=tx.query("SELECT s.session_id::text,s.account_epoch::bigint,s.refresh_token_ref::text,a.verified_email FROM genui_identity.shared_sessions s JOIN genui_identity.accounts a USING(account_id) WHERE s.account_id=$1::text::uuid AND s.cookie_digest=$2 AND a.status='active' AND a.account_epoch=s.account_epoch AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '1 hour' FOR SHARE OF a", &[&account,&digest(SESSION_DOMAIN,cookie)]).await?;
            let Some(row)=rows.first() else {return Ok(None)};
            let resolved=ShareAccount{account_id:account.clone(),session_id:row.try_get(0).map_err(|_|Error::Unavailable)?,account_epoch:row.try_get(1).map_err(|_|Error::Unavailable)?,refresh_ref:row.try_get(2).map_err(|_|Error::Unavailable)?,verified_email:row.try_get(3).map_err(|_|Error::Unavailable)?};
            if !refresh_live(&resolved.refresh_ref) { return Ok(None); }
            tx.query("UPDATE genui_identity.shared_sessions SET last_seen_at=clock_timestamp() WHERE account_id=$1::text::uuid AND session_id=$2::text::uuid AND last_seen_at<clock_timestamp()-interval '60 seconds'", &[&account,&resolved.session_id]).await?;
            Ok(Some(resolved))
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
    pub async fn destroy_shared_session(&self, cookie: &str) -> Result<Option<String>> {
        let Some(account) = capability_organization(cookie) else {
            return Ok(None);
        };
        let tx = Transaction::begin_shared(&self.inner, None, Some(&account)).await?;
        let result=async {
            let rows=tx.query("DELETE FROM genui_identity.shared_sessions WHERE account_id=$1::text::uuid AND cookie_digest=$2 RETURNING refresh_token_ref::text", &[&account,&digest(SESSION_DOMAIN,cookie)]).await?;
            rows.first().map(|r|r.try_get(0).map_err(|_|Error::Unavailable)).transpose()
        }.await;
        tx.finish(result.is_ok()).await?;
        result
    }
}
