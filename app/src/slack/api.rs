//! The Slack Web API client: `reqwest` against `https://slack.com/api/*`
//! (or the loopback stub named by `GENUI_SLACK_API_BASE` in tests), the bot
//! token only in the `Authorization` header and never in argv, a URL or a
//! log line. Replies to an interaction go to its `response_url`, which must
//! be Slack's hooks host (or the stub's origin when the base is overridden):
//! a signed payload cannot point the server at another host.
use super::Secret;
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://slack.com/api";
const TIMEOUT: Duration = Duration::from_secs(5);
const HOOKS: &str = "https://hooks.slack.com/";

/// Why a Web API call did not succeed: a stable code for the log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiError {
    Transport,
    Status(u16),
    /// Slack answered `ok: false` with this (bounded, sanitized) error code.
    Slack(String),
    Refused,
}
impl ApiError {
    pub fn code(&self) -> String {
        match self {
            Self::Transport => "transport".into(),
            Self::Status(status) => format!("status_{status}"),
            Self::Slack(code) => code.clone(),
            Self::Refused => "refused_url".into(),
        }
    }
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    /// Allowed `response_url` prefixes.
    hooks: Vec<String>,
    token: Secret,
}

/// `scheme://host[:port]/` of a base URL.
fn origin_of(base: &str) -> Option<String> {
    let url = url::Url::parse(base).ok()?;
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}/", url.scheme()),
        None => format!("{}://{host}/", url.scheme()),
    })
}

impl Client {
    pub fn new(base: &str, token: Secret) -> Option<Self> {
        let base = base.trim_end_matches('/').to_owned();
        let parsed = url::Url::parse(&base).ok()?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return None;
        }
        let mut hooks = vec![HOOKS.to_owned()];
        if base != DEFAULT_BASE {
            hooks.push(origin_of(&base)?);
        }
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        Some(Self {
            http,
            base,
            hooks,
            token,
        })
    }

    /// `POST {base}/{method}` with a JSON body; `ok: true` or an error code.
    pub async fn call(&self, method: &str, body: &Value) -> Result<Value, ApiError> {
        let response = self
            .http
            .post(format!("{}/{method}", self.base))
            .bearer_auth(self.token.expose())
            .header("content-type", "application/json; charset=utf-8")
            .body(body.to_string())
            .send()
            .await
            .map_err(|_| ApiError::Transport)?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::Status(status.as_u16()));
        }
        let value: Value = response.json().await.map_err(|_| ApiError::Transport)?;
        if value.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(value)
        } else {
            let code: String = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .take(64)
                .collect();
            Err(ApiError::Slack(code))
        }
    }

    /// `conversations.open` with one user: the DM channel id.
    pub async fn open_dm(&self, user: &str) -> Result<String, ApiError> {
        let value = self
            .call("conversations.open", &json!({"users": user}))
            .await?;
        value
            .pointer("/channel/id")
            .and_then(Value::as_str)
            .filter(|id| crate::storage::slack_id(id))
            .map(str::to_owned)
            .ok_or_else(|| ApiError::Slack("no_channel".into()))
    }

    /// `chat.postMessage` to a channel.
    pub async fn post_message(
        &self,
        channel: &str,
        text: &str,
        blocks: Option<Vec<Value>>,
    ) -> Result<(), ApiError> {
        let mut body =
            json!({"channel": channel, "text": text, "unfurl_links": false, "unfurl_media": false});
        if let Some(blocks) = blocks {
            body["blocks"] = Value::Array(blocks);
        }
        self.call("chat.postMessage", &body).await.map(|_| ())
    }

    /// A DM to one Slack user.
    pub async fn direct(
        &self,
        user: &str,
        text: &str,
        blocks: Option<Vec<Value>>,
    ) -> Result<(), ApiError> {
        let channel = self.open_dm(user).await?;
        self.post_message(&channel, text, blocks).await
    }

    /// Whether `url` is a response URL this client may post to.
    pub fn allowed_response_url(&self, url: &str) -> bool {
        url.len() <= 512
            && !url.contains(['\r', '\n', ' '])
            && self.hooks.iter().any(|prefix| url.starts_with(prefix))
    }

    /// A message to an interaction's `response_url` (no bot token).
    pub async fn respond(&self, response_url: &str, message: &Value) -> Result<(), ApiError> {
        if !self.allowed_response_url(response_url) {
            return Err(ApiError::Refused);
        }
        let response = self
            .http
            .post(response_url)
            .header("content-type", "application/json; charset=utf-8")
            .body(message.to_string())
            .send()
            .await
            .map_err(|_| ApiError::Transport)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::Status(response.status().as_u16()))
        }
    }
}
