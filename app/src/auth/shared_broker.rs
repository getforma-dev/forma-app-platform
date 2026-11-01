//! Narrow operator-owned invite broker. The ordinary service never receives a
//! provider management key. No arbitrary provider path, role or return URI.
use super::provider::{CONNECT_TIMEOUT, TOTAL_TIMEOUT};
use serde::Deserialize;
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
pub const CONFIG_ENV: &str = "GENUI_SHARED_INVITE_BROKER_FILE";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    origin: String,
    token: String,
}
pub struct Client {
    origin: String,
    token: String,
    http: reqwest::Client,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invite {
    invite_token: String,
    expires_at: u64,
}
impl Client {
    /// Same-UID private file; absent configuration disables only new signup.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let refused = "shared invitation broker configuration refused";
        if !path.is_absolute() {
            return Err(refused);
        }
        let parent = path
            .parent()
            .ok_or(refused)?
            .symlink_metadata()
            .map_err(|_| refused)?;
        let uid = unsafe { libc::geteuid() };
        if !parent.is_dir() || parent.uid() != uid || parent.mode() & 0o077 != 0 {
            return Err(refused);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| refused)?;
        let metadata = file.metadata().map_err(|_| refused)?;
        if !metadata.is_file()
            || metadata.uid() != uid
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
            || metadata.len() > 4096
        {
            return Err(refused);
        }
        let mut bytes = Vec::new();
        file.take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| refused)?;
        let config: Config = serde_json::from_slice(&bytes).map_err(|_| refused)?;
        Self::build(config, false).ok_or(refused)
    }
    fn build(config: Config, loopback: bool) -> Option<Self> {
        let url = url::Url::parse(&config.origin).ok()?;
        let local = loopback
            && url.scheme() == "http"
            && url
                .host_str()?
                .parse::<std::net::IpAddr>()
                .ok()?
                .is_loopback();
        if !(url.scheme() == "https" || local)
            || url.origin().ascii_serialization() != config.origin
            || !url.username().is_empty()
            || url.password().is_some()
            || !crate::storage::capability_shaped(&config.token)
        {
            return None;
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .referer(false)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .build()
            .ok()?;
        Some(Self {
            origin: config.origin,
            token: config.token,
            http,
        })
    }
    #[cfg(test)]
    pub(crate) fn loopback(origin: &str, token: &str) -> Option<Self> {
        Self::build(
            Config {
                origin: origin.into(),
                token: token.into(),
            },
            true,
        )
    }
    /// One request only. The broker owns durable idempotency; transport-unknown
    /// never causes a second provider invitation from this client.
    pub async fn invitation(
        &self,
        tenant_slug: &str,
        email: &str,
        idempotency_key: &str,
    ) -> Option<String> {
        if tenant_slug.is_empty()
            || tenant_slug.len() > 64
            || !tenant_slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || crate::storage::normalize_share_email(email).as_deref() != Some(email)
            || !crate::storage::valid_identifier(idempotency_key)
        {
            return None;
        }
        let mut response=self.http.post(format!("{}/v1/shared-invitations",self.origin)).bearer_auth(&self.token).json(&serde_json::json!({"tenant_slug":tenant_slug,"email":email,"role":"user","delivery_mode":"server_only","idempotency_key":idempotency_key})).send().await.ok()?;
        if response.status() != reqwest::StatusCode::CREATED
            || response.content_length().is_some_and(|n| n > 4096)
        {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 4096 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        let invite: Invite = serde_json::from_slice(&bytes).ok()?;
        let now = super::now();
        (invite
            .invite_token
            .strip_prefix("tinv_")
            .is_some_and(crate::storage::valid_identifier)
            && invite.expires_at > now
            && invite.expires_at <= now + 600)
            .then_some(invite.invite_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    struct TestState {
        calls: Arc<Mutex<Vec<serde_json::Value>>>,
        response: Arc<Mutex<(StatusCode, serde_json::Value)>>,
        token: String,
    }
    async fn serve(
        State(state): State<TestState>,
        headers: HeaderMap,
        Json(body): Json<serde_json::Value>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        assert_eq!(
            headers.get("authorization").unwrap().to_str().unwrap(),
            format!("Bearer {}", state.token)
        );
        state.calls.lock().unwrap().push(body);
        let reply = state.response.lock().unwrap().clone();
        (reply.0, Json(reply.1))
    }
    #[tokio::test]
    async fn broker_contract_is_closed_bounded_and_never_blindly_retries() {
        let token = crate::auth::capability::random_capability().unwrap();
        let expected = "tinv_00000000-0000-4000-8000-000000000001";
        let state = TestState {
            calls: Arc::default(),
            response: Arc::new(Mutex::new((
                StatusCode::CREATED,
                serde_json::json!({"invite_token":expected,"expires_at":crate::auth::now()+300}),
            ))),
            token: token.clone(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/v1/shared-invitations", post(serve))
            .with_state(state.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::loopback(&origin, &token).unwrap();
        let key = "00000000-0000-4000-8000-000000000099";
        assert_eq!(
            client
                .invitation("northstar", "recipient@example.test", key)
                .await
                .as_deref(),
            Some(expected)
        );
        let body = state.calls.lock().unwrap()[0].clone();
        assert_eq!(
            body,
            serde_json::json!({"tenant_slug":"northstar","email":"recipient@example.test","role":"user","delivery_mode":"server_only","idempotency_key":key})
        );
        for response in [
            serde_json::json!({"invite_token":expected,"expires_at":crate::auth::now()+601}),
            serde_json::json!({"invite_token":"secret","expires_at":crate::auth::now()+300}),
            serde_json::json!({"invite_token":expected,"expires_at":crate::auth::now()+300,"invite_url":"https://example.test"}),
        ] {
            *state.response.lock().unwrap() = (StatusCode::CREATED, response);
            assert!(client
                .invitation("northstar", "recipient@example.test", key)
                .await
                .is_none());
        }
        *state.response.lock().unwrap() = (StatusCode::SERVICE_UNAVAILABLE, serde_json::json!({}));
        let before = state.calls.lock().unwrap().len();
        assert!(client
            .invitation("northstar", "recipient@example.test", key)
            .await
            .is_none());
        assert_eq!(state.calls.lock().unwrap().len(), before + 1);
        assert!(client
            .invitation("../manage", "recipient@example.test", key)
            .await
            .is_none());
        assert_eq!(state.calls.lock().unwrap().len(), before + 1);
        server.abort();
    }
}
