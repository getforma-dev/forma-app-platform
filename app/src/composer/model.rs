//! The composer's model client (APP-093; ADR-011 §4): direct HTTPS to the
//! Anthropic Messages API, as Forma Vector Studio's `claude_client.rs` does
//! (one `x-api-key` header, `anthropic-version: 2023-06-01`, temperature 0,
//! a bounded number of retries on transport failures, 429 and 5xx, usage
//! logged by count). The key comes from the process environment only
//! (`KEY_ENV`, populated in deployment by the ECS secret `ComposerModelKey`);
//! it is held in a type whose `Debug` redacts it and is written to no log,
//! argv or file. A reply is text: the composer parses it strictly and the
//! only thing that ever leaves this module is a `String` the caller treats
//! as data. Tests use a [`Model`] stub; nothing in the test executable calls
//! the real endpoint.
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

/// The environment variable the service reads the key from. In the managed
/// deployment ECS injects it from the `ComposerModelKey` secret; absent,
/// empty or placeholder (`{}`) the composer is disabled and the route
/// answers `composer_unavailable`.
pub const KEY_ENV: &str = "ANTHROPIC_API_KEY";
/// The router model (the fast library-first step), overridable.
pub const ROUTER_MODEL_ENV: &str = "GENUI_COMPOSER_ROUTER_MODEL";
pub const DEFAULT_ROUTER_MODEL: &str = "claude-haiku-4-5-20251001";
/// The generation model (only when the library has no fit), overridable
/// (dev and smoke use the Haiku model for both).
pub const GENERATION_MODEL_ENV: &str = "GENUI_COMPOSER_GENERATION_MODEL";
pub const DEFAULT_GENERATION_MODEL: &str = "claude-sonnet-4-6";
/// The Messages endpoint, overridable only for the local stub (tests, smoke).
pub const ENDPOINT_ENV: &str = "GENUI_COMPOSER_ENDPOINT";
pub const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
/// Attempts per model call: the first plus two retries on a retryable failure.
pub const MAX_ATTEMPTS: u32 = 3;
const BACKOFF: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(3)];
/// Reply bodies beyond this are a protocol failure (a definition is ≤ 256 KiB).
const MAX_REPLY_BYTES: usize = 1024 * 1024;

/// A key that is usable: non-empty, printable ASCII, not the secret's initial
/// placeholder. Anything else means "no key": disabled, never an attempt.
pub fn usable_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|b| b.is_ascii_graphic())
        && !value.starts_with('{')
}

/// One model call.
#[derive(Clone, Debug)]
pub struct ModelRequest {
    pub model: String,
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    /// The whole call, every attempt included.
    pub budget: Duration,
}

/// The text of a reply and its usage counts (counts only are logged).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelReply {
    pub text: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Why a call produced no reply. Nothing here carries request or reply text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    /// Transport, 429 or 5xx after every attempt.
    Unavailable,
    /// The budget ran out.
    Timeout,
    /// A 4xx other than 429 (a bad key is one): not retried.
    Refused,
    /// A reply the protocol does not describe.
    Malformed,
}
impl ModelError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "model_unavailable",
            Self::Timeout => "model_timeout",
            Self::Refused => "model_refused",
            Self::Malformed => "model_malformed",
        }
    }
}

pub type Completion<'a> = Pin<Box<dyn Future<Output = Result<ModelReply, ModelError>> + Send + 'a>>;

/// An image sent with a request (APP-102, the receipt): the sniffed media
/// type and the bytes, borrowed and held in memory only. `Debug` prints the
/// type and the length, never the bytes.
pub struct ImageInput<'a> {
    pub media_type: &'static str,
    pub bytes: &'a [u8],
}
impl fmt::Debug for ImageInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ImageInput({}, {} bytes)",
            self.media_type,
            self.bytes.len()
        )
    }
}

/// What completes a request: the HTTPS client in the binary, stubs in tests.
pub trait Model: Send + Sync {
    fn complete<'a>(&'a self, request: &'a ModelRequest) -> Completion<'a>;
    /// `request` with `image` before the user text (one Messages image
    /// content block, base64). A model without vision refuses.
    fn complete_with_image<'a>(
        &'a self,
        request: &'a ModelRequest,
        image: &'a ImageInput<'a>,
    ) -> Completion<'a> {
        let _ = (request, image);
        Box::pin(async { Err(ModelError::Refused) })
    }
}

/// The key, redacted everywhere it could be printed.
#[derive(Clone)]
pub struct ApiKey(String);
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(redacted)")
    }
}

/// The Messages API client.
pub struct Anthropic {
    key: ApiKey,
    endpoint: String,
    http: reqwest::Client,
}
impl fmt::Debug for Anthropic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Anthropic")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}
impl Anthropic {
    /// A client for `endpoint` with `key`; `None` when the key is not usable.
    pub fn new(key: &str, endpoint: &str) -> Option<Self> {
        if !usable_key(key) || !(endpoint.starts_with("https://") || loopback(endpoint)) {
            return None;
        }
        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        Some(Self {
            key: ApiKey(key.to_owned()),
            endpoint: endpoint.to_owned(),
            http,
        })
    }
    /// The client the environment configures: `KEY_ENV` (the only source of
    /// the key) and the optional endpoint override. `None`: disabled.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var(KEY_ENV).ok()?;
        let endpoint = std::env::var(ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
        Self::new(&key, &endpoint)
    }

    async fn attempt(
        &self,
        body: &Value,
        deadline: Instant,
    ) -> Result<ModelReply, (ModelError, bool)> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or((ModelError::Timeout, false))?;
        let response = self
            .http
            .post(&self.endpoint)
            .header("x-api-key", self.key.0.as_str())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .timeout(remaining)
            .json(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    (ModelError::Timeout, false)
                } else {
                    (ModelError::Unavailable, true)
                }
            })?;
        let status = response.status();
        if status.as_u16() == 429 || status.is_server_error() {
            return Err((ModelError::Unavailable, true));
        }
        if !status.is_success() {
            return Err((ModelError::Refused, false));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| (ModelError::Unavailable, true))?;
        if bytes.len() > MAX_REPLY_BYTES {
            return Err((ModelError::Malformed, false));
        }
        parse_reply(&bytes).ok_or((ModelError::Malformed, false))
    }
}

fn loopback(endpoint: &str) -> bool {
    endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:")
}

#[derive(Deserialize)]
struct Reply {
    content: Vec<Block>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
}
#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}
#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

/// The first text block of a Messages reply, with its usage.
fn parse_reply(bytes: &[u8]) -> Option<ModelReply> {
    let reply: Reply = serde_json::from_slice(bytes).ok()?;
    let text = reply
        .content
        .iter()
        .find(|block| block.kind == "text")
        .and_then(|block| block.text.clone())?;
    Some(ModelReply {
        text,
        model: reply.model.unwrap_or_default(),
        input_tokens: reply.usage.as_ref().and_then(|usage| usage.input_tokens),
        output_tokens: reply.usage.as_ref().and_then(|usage| usage.output_tokens),
    })
}

impl Model for Anthropic {
    fn complete<'a>(&'a self, request: &'a ModelRequest) -> Completion<'a> {
        self.send(request, Value::String(request.user.clone()))
    }
    fn complete_with_image<'a>(
        &'a self,
        request: &'a ModelRequest,
        image: &'a ImageInput<'a>,
    ) -> Completion<'a> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(image.bytes);
        self.send(
            request,
            json!([
                {"type": "image", "source": {"type": "base64", "media_type": image.media_type, "data": data}},
                {"type": "text", "text": request.user},
            ]),
        )
    }
}

impl Anthropic {
    /// One call with `content` as the user message's content (text, or an
    /// image block and text); the body is never logged.
    fn send<'a>(&'a self, request: &'a ModelRequest, content: Value) -> Completion<'a> {
        Box::pin(async move {
            let started = Instant::now();
            let deadline = started + request.budget;
            let body = json!({
                "model": request.model,
                "max_tokens": request.max_tokens,
                "temperature": 0,
                "system": request.system,
                "messages": [{"role": "user", "content": content}],
            });
            let mut outcome = Err(ModelError::Unavailable);
            for attempt in 0..MAX_ATTEMPTS {
                if attempt > 0 {
                    let pause = BACKOFF[usize::from(attempt as u16 - 1).min(BACKOFF.len() - 1)];
                    if Instant::now() + pause >= deadline {
                        outcome = Err(ModelError::Timeout);
                        break;
                    }
                    tokio::time::sleep(pause).await;
                }
                match self.attempt(&body, deadline).await {
                    Ok(reply) => {
                        tracing::info!(
                            event = "composer.model",
                            model = reply.model,
                            requested_model = request.model,
                            attempt,
                            input_tokens = reply.input_tokens,
                            output_tokens = reply.output_tokens,
                            duration_ms = elapsed_ms(started),
                            "model replied"
                        );
                        return Ok(reply);
                    }
                    Err((error, retryable)) => {
                        tracing::warn!(
                            event = "composer.model_failed",
                            code = error.code(),
                            requested_model = request.model,
                            attempt,
                            retryable,
                            duration_ms = elapsed_ms(started),
                            "model call failed"
                        );
                        outcome = Err(error);
                        if !retryable {
                            break;
                        }
                    }
                }
            }
            outcome
        })
    }
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_and_malformed_keys_disable_the_composer() {
        assert!(!usable_key(""));
        assert!(!usable_key("{}"));
        assert!(!usable_key("sk ant with space"));
        assert!(!usable_key(&"k".repeat(513)));
        assert!(usable_key("sk-ant-api03-synthetic-not-a-real-key"));
        assert!(Anthropic::new("{}", DEFAULT_ENDPOINT).is_none());
        assert!(Anthropic::new("sk-ant-synthetic", "http://example.com/v1").is_none());
        assert!(Anthropic::new("sk-ant-synthetic", DEFAULT_ENDPOINT).is_some());
        assert!(Anthropic::new("sk-ant-synthetic", "http://127.0.0.1:1/v1/messages").is_some());
    }

    #[test]
    fn debug_output_never_carries_the_key() {
        let client = Anthropic::new("sk-ant-synthetic-secret", DEFAULT_ENDPOINT).unwrap();
        let rendered = format!("{client:?} {:?}", client.key);
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn reply_parsing_takes_the_first_text_block_and_usage() {
        let reply = parse_reply(br#"{"id":"msg","model":"m","content":[{"type":"tool_use","id":"x","name":"n","input":{}},{"type":"text","text":"{\"a\":1}"}],"usage":{"input_tokens":12,"output_tokens":3}}"#).unwrap();
        assert_eq!(reply.text, "{\"a\":1}");
        assert_eq!(reply.model, "m");
        assert_eq!(reply.input_tokens, Some(12));
        assert_eq!(reply.output_tokens, Some(3));
        assert!(parse_reply(br#"{"content":[]}"#).is_none());
        assert!(parse_reply(b"not json").is_none());
    }
}
