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
//!
//! D10 (trial-25 plan): every call through [`Anthropic`] first takes one
//! slot of the process-wide [`DailyCap`] ([`daily_cap`]). It is one static
//! because the binary holds several clients (the composer, the folders, the
//! parser, the customizer, Slack's parser), and the cap is for the whole app.
//! The count starts again at 00:00 UTC; a call refused by the cap sends
//! nothing and is [`ModelError::DailyCap`], which every model route answers
//! as 429 `daily_limit` ([`daily_limit`]).
use axum::{
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{Mutex, OnceLock, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
/// D10: model calls allowed per UTC day for the whole app, unless
/// [`DAILY_CALLS_ENV`] says otherwise. 1,000 (trial decision of 2026-11-14,
/// not the plan's first 400): a composition is one router call plus up to
/// three generation attempts, so 400 was about six compositions a person on
/// a class day.
pub const DEFAULT_DAILY_CALLS: u32 = 1000;
/// The daily cap override (tests and dev), read once, at the first client.
/// Not set in the managed deployment: there the code's default holds.
pub const DAILY_CALLS_ENV: &str = "GENUI_MODEL_DAILY_CALLS";
const DAY_SECONDS: u64 = 86_400;

/// Model calls taken per UTC day, against a limit. A call is one request to
/// the client (its retries included), counted before anything is sent.
#[derive(Debug)]
pub struct DailyCap {
    limit: u32,
    /// The UTC day number (days since the UNIX epoch) and the calls taken in it.
    used: Mutex<(u64, u32)>,
}
impl DailyCap {
    pub const fn new(limit: u32) -> Self {
        Self {
            limit,
            used: Mutex::new((0, 0)),
        }
    }
    pub fn limit(&self) -> u32 {
        self.limit
    }
    /// Takes one call at `now` (seconds since the UNIX epoch; UNIX days are
    /// UTC days of exactly 86 400 s): `true` when the day's count was below
    /// the limit (it is now one higher), `false` when the limit is reached
    /// (nothing is taken). A later day starts at zero; a clock that steps
    /// back keeps counting in the latest day it saw.
    pub fn take(&self, now: u64) -> bool {
        let day = now / DAY_SECONDS;
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day > used.0 {
            *used = (day, 0);
        }
        if used.1 >= self.limit {
            return false;
        }
        used.1 += 1;
        true
    }
    /// Whether a call at `now` would be refused (the day's count is at the
    /// limit); takes nothing.
    pub fn reached(&self, now: u64) -> bool {
        let day = now / DAY_SECONDS;
        let used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day > used.0 {
            self.limit == 0
        } else {
            used.1 >= self.limit
        }
    }
}

/// The daily limit `value` names: a decimal count, else the default.
pub fn limit_from(value: Option<&str>) -> u32 {
    value
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_DAILY_CALLS)
}

/// The app's one cap, shared by every [`Anthropic`] client of the process.
pub fn daily_cap() -> &'static DailyCap {
    static CAP: OnceLock<DailyCap> = OnceLock::new();
    CAP.get_or_init(|| {
        let limit = limit_from(std::env::var(DAILY_CALLS_ENV).ok().as_deref());
        tracing::info!(
            event = "composer.model_daily_cap_set",
            limit,
            "daily model-call cap"
        );
        DailyCap::new(limit)
    })
}

/// Seconds until the next 00:00 UTC after `now` (1 to 86 400).
pub fn seconds_to_reset(now: u64) -> u64 {
    DAY_SECONDS - now % DAY_SECONDS
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// What a person reads when the cap is reached (the canvas has its own copy).
pub const DAILY_LIMIT_TEXT: &str = "Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.";

/// The answer of every model route to [`ModelError::DailyCap`]: 429
/// `daily_limit`, `Retry-After` the seconds until the count starts again.
pub fn daily_limit() -> Response {
    let mut response = crate::http::problem(StatusCode::TOO_MANY_REQUESTS, "daily_limit");
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from(seconds_to_reset(unix_now())),
    );
    response
}

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
    /// D10: the app's daily model-call cap is reached; nothing was sent.
    DailyCap,
}
impl ModelError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "model_unavailable",
            Self::Timeout => "model_timeout",
            Self::Refused => "model_refused",
            Self::Malformed => "model_malformed",
            Self::DailyCap => "model_daily_cap",
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

/// APP-108: a PDF sent as a Messages `document` block with citations
/// enabled (base64). `Debug` prints the title length and byte count only.
pub struct DocumentInput<'a> {
    pub title: &'a str,
    pub bytes: &'a [u8],
}
impl fmt::Debug for DocumentInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocumentInput({} bytes)", self.bytes.len())
    }
}
/// A `page_location` citation of a reply text block (1-indexed pages; the
/// end page is exclusive as the API reports it).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Citation {
    pub document_index: usize,
    pub start_page: u64,
    pub end_page: u64,
    pub cited_text: String,
}
/// A reply split into its text blocks, each with its citations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CitedReply {
    pub blocks: Vec<(String, Vec<Citation>)>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}
pub type CitedCompletion<'a> =
    Pin<Box<dyn Future<Output = Result<CitedReply, ModelError>> + Send + 'a>>;

/// What completes a request: the HTTPS client in the binary, stubs in tests.
pub trait Model: Send + Sync {
    /// `request` with `documents` (PDF `document` blocks, citations enabled)
    /// before the user text; a model without document support refuses.
    fn complete_with_documents<'a>(
        &'a self,
        request: &'a ModelRequest,
        documents: &'a [DocumentInput<'a>],
    ) -> CitedCompletion<'a> {
        let _ = (request, documents);
        Box::pin(async { Err(ModelError::Refused) })
    }
    fn complete<'a>(&'a self, request: &'a ModelRequest) -> Completion<'a>;
    /// D10: whether the next call would be refused by the daily cap (a stub
    /// has none). A caller that is about to write before a model call asks
    /// first, so the cap is refused before anything is placed.
    fn daily_cap_reached(&self) -> bool {
        false
    }
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
    /// [`daily_cap`] for every client the binary builds.
    cap: &'static DailyCap,
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
            cap: daily_cap(),
        })
    }
    /// Tests and campaigns: this client counts against `cap` instead of the
    /// process's, so a low cap can be reached without touching the others.
    #[cfg(test)]
    pub(crate) fn with_daily_cap(mut self, cap: &'static DailyCap) -> Self {
        self.cap = cap;
        self
    }
    /// One slot of the daily cap for `request`, before anything is sent.
    fn admit(&self, request: &ModelRequest) -> Result<(), ModelError> {
        if self.cap.take(unix_now()) {
            return Ok(());
        }
        tracing::warn!(
            event = "composer.model_daily_cap",
            code = ModelError::DailyCap.code(),
            limit = self.cap.limit(),
            requested_model = request.model,
            "daily model-call cap reached; nothing sent"
        );
        Err(ModelError::DailyCap)
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
        let bytes = capped_body(response, MAX_REPLY_BYTES)
            .await
            .map_err(|error| (error, error == ModelError::Unavailable))?;
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

#[derive(Deserialize)]
struct CitedBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    citations: Option<Vec<Value>>,
}
#[derive(Deserialize)]
struct CitedReplyWire {
    content: Vec<CitedBlock>,
    #[serde(default)]
    usage: Option<Usage>,
}

/// Every text block of a Messages reply with its `page_location`
/// citations (other citation kinds are ignored).
pub fn parse_cited(bytes: &[u8]) -> Option<CitedReply> {
    let reply: CitedReplyWire = serde_json::from_slice(bytes).ok()?;
    let mut blocks = Vec::new();
    for block in reply.content.iter().filter(|b| b.kind == "text") {
        let text = block.text.clone()?;
        let citations = block
            .citations
            .iter()
            .flatten()
            .filter(|c| c.get("type").and_then(Value::as_str) == Some("page_location"))
            .filter_map(|c| {
                Some(Citation {
                    document_index: usize::try_from(c.get("document_index")?.as_u64()?).ok()?,
                    start_page: c.get("start_page_number")?.as_u64()?,
                    end_page: c.get("end_page_number")?.as_u64()?,
                    cited_text: c.get("cited_text")?.as_str()?.chars().take(2000).collect(),
                })
            })
            .collect();
        blocks.push((text, citations));
    }
    (!blocks.is_empty()).then(|| CitedReply {
        blocks,
        input_tokens: reply.usage.as_ref().and_then(|u| u.input_tokens),
        output_tokens: reply.usage.as_ref().and_then(|u| u.output_tokens),
    })
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
    fn daily_cap_reached(&self) -> bool {
        self.cap.reached(unix_now())
    }
    fn complete_with_documents<'a>(
        &'a self,
        request: &'a ModelRequest,
        documents: &'a [DocumentInput<'a>],
    ) -> CitedCompletion<'a> {
        use base64::Engine as _;
        Box::pin(async move {
            self.admit(request)?;
            let started = Instant::now();
            let mut content: Vec<Value> = documents
                .iter()
                .map(|d| {
                    json!({"type": "document", "title": d.title,
                    "source": {"type": "base64", "media_type": "application/pdf",
                        "data": base64::engine::general_purpose::STANDARD.encode(d.bytes)},
                    "citations": {"enabled": true}})
                })
                .collect();
            content.push(json!({"type": "text", "text": request.user}));
            let body = json!({
                "model": request.model,
                "max_tokens": request.max_tokens,
                "temperature": 0,
                "system": request.system,
                "messages": [{"role": "user", "content": content}],
            });
            let deadline = started + request.budget;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(ModelError::Timeout)?;
            let response = self
                .http
                .post(&self.endpoint)
                .header("x-api-key", self.key.0.as_str())
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .timeout(remaining)
                .json(&body)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        ModelError::Timeout
                    } else {
                        ModelError::Unavailable
                    }
                })?;
            let status = response.status();
            if status.as_u16() == 429 || status.is_server_error() {
                return Err(ModelError::Unavailable);
            }
            if !status.is_success() {
                return Err(ModelError::Refused);
            }
            let bytes = capped_body(response, MAX_REPLY_BYTES).await?;
            let reply = parse_cited(&bytes).ok_or(ModelError::Malformed)?;
            tracing::info!(
                event = "composer.model_documents",
                requested_model = request.model,
                documents = documents.len(),
                input_tokens = reply.input_tokens,
                output_tokens = reply.output_tokens,
                duration_ms = elapsed_ms(started),
                "model replied with citations"
            );
            Ok(reply)
        })
    }
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
            self.admit(request)?;
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

/// The reply body, read chunk by chunk and refused (`Malformed`) as soon as
/// it passes `cap`, or before any read when its declared length does
/// (review: the whole reply used to be buffered before the size check); a
/// failed read is `Unavailable`.
async fn capped_body(mut response: reqwest::Response, cap: usize) -> Result<Vec<u8>, ModelError> {
    if response
        .content_length()
        .is_some_and(|length| length > cap as u64)
    {
        return Err(ModelError::Malformed);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ModelError::Unavailable)?
    {
        if body.len() + chunk.len() > cap {
            return Err(ModelError::Malformed);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
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

    #[tokio::test]
    async fn replies_are_capped_while_read() {
        let reply = |body: &'static str| {
            reqwest::Response::from(axum::http::Response::new(reqwest::Body::from(body)))
        };
        assert_eq!(capped_body(reply("12345"), 5).await.unwrap(), b"12345");
        assert_eq!(
            capped_body(reply("123456"), 5).await,
            Err(ModelError::Malformed)
        );
        assert_eq!(capped_body(reply(""), 5).await.unwrap(), b"");
    }

    /// 2026-11-30T00:00:00Z, a UTC midnight.
    const MIDNIGHT: u64 = 1_795_996_800;

    #[test]
    fn the_daily_cap_counts_calls_and_starts_again_at_utc_midnight() {
        assert_eq!(MIDNIGHT % DAY_SECONDS, 0);
        let cap = DailyCap::new(3);
        assert_eq!(cap.limit(), 3);
        let evening = MIDNIGHT - 120;
        // Three calls fit in the day; the fourth and later are refused, and
        // a refusal takes nothing.
        assert!(cap.take(evening));
        assert!(cap.take(evening + 1));
        assert!(cap.take(evening + 2));
        assert!(!cap.take(evening + 3));
        assert!(!cap.take(MIDNIGHT - 1));
        // 00:00:00 UTC: a new day, a new count of three.
        assert!(cap.take(MIDNIGHT));
        assert!(cap.take(MIDNIGHT + 1));
        assert!(cap.take(MIDNIGHT + 2));
        assert!(!cap.take(MIDNIGHT + 3));
        // A clock that steps back over midnight does not reopen the old day.
        assert!(!cap.take(MIDNIGHT - 10));
        // A day later the count is fresh again.
        assert!(cap.take(MIDNIGHT + DAY_SECONDS + 7));
        // A zero cap refuses everything.
        let closed = DailyCap::new(0);
        assert!(!closed.take(MIDNIGHT));
        assert!(closed.reached(MIDNIGHT) && closed.reached(MIDNIGHT + DAY_SECONDS));
        // `reached` peeks without taking: at the limit until midnight, then free.
        let peek = DailyCap::new(1);
        assert!(!peek.reached(MIDNIGHT));
        assert!(!peek.reached(MIDNIGHT), "peeking took nothing");
        assert!(peek.take(MIDNIGHT));
        assert!(peek.reached(MIDNIGHT + 1));
        assert!(!peek.reached(MIDNIGHT + DAY_SECONDS));
    }

    #[test]
    fn the_daily_limit_is_the_default_unless_a_count_is_named() {
        assert_eq!(DEFAULT_DAILY_CALLS, 1000);
        assert_eq!(limit_from(None), 1000);
        assert_eq!(limit_from(Some("3")), 3);
        assert_eq!(limit_from(Some(" 12 ")), 12);
        assert_eq!(limit_from(Some("0")), 0);
        assert_eq!(limit_from(Some("")), 1000);
        assert_eq!(limit_from(Some("-1")), 1000);
        assert_eq!(limit_from(Some("many")), 1000);
        assert_eq!(seconds_to_reset(MIDNIGHT), DAY_SECONDS);
        assert_eq!(seconds_to_reset(MIDNIGHT - 1), 1);
        assert_eq!(seconds_to_reset(MIDNIGHT + 3600), DAY_SECONDS - 3600);
    }

    #[tokio::test]
    async fn the_daily_limit_answer_is_429_with_the_seconds_to_midnight() {
        assert_eq!(ModelError::DailyCap.code(), "model_daily_cap");
        let response = daily_limit();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
        let wait: u64 = response.headers()[header::RETRY_AFTER]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=DAY_SECONDS).contains(&wait));
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], br#"{"code":"daily_limit","status":429}"#);
    }

    #[test]
    fn debug_output_never_carries_the_key() {
        let client = Anthropic::new("sk-ant-synthetic-secret", DEFAULT_ENDPOINT).unwrap();
        let rendered = format!("{client:?} {:?}", client.key);
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn cited_replies_keep_page_locations() {
        let reply = parse_cited(br#"{"content":[{"type":"text","text":"The brief says "},{"type":"text","text":"revenue rose","citations":[{"type":"page_location","cited_text":"Revenue rose 12%","document_index":0,"document_title":"q3.pdf","start_page_number":1,"end_page_number":2}]},{"type":"text","text":".","citations":[{"type":"char_location","cited_text":"x","document_index":0,"start_char_index":0,"end_char_index":1}]}],"usage":{"input_tokens":900,"output_tokens":40}}"#).unwrap();
        assert_eq!(reply.blocks.len(), 3);
        assert_eq!(
            reply.blocks[1].1,
            vec![Citation {
                document_index: 0,
                start_page: 1,
                end_page: 2,
                cited_text: "Revenue rose 12%".into()
            }]
        );
        assert!(reply.blocks[2].1.is_empty());
        assert_eq!(reply.input_tokens, Some(900));
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
