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
//!
//! APP-215 (content M3): beside the process cap, every call made for a
//! member first takes one slot of that member's own daily allowance
//! ([`MemberDailyCap`], [`member_daily_cap`], 80 a day by default), so one or
//! two people cannot spend the whole class's 1,000. The member travels in
//! [`ModelRequest::caller`] as a [`Caller::Member`], named by every route that
//! calls a model for a signed-in member (the composer, Ask, Shape, the
//! folders, the action path, the widget customizer, MCP and Slack, which act
//! as the member). A [`Caller::System`] call (none in product code: the demo
//! rehearsals call no model) counts against the process cap only. A member
//! whose allowance is reached gets the same 429 `daily_limit`, with
//! `"scope":"member"` so the canvas says it is theirs ([`daily_limit_for`]).
//!
//! APP-222 part A: the caller is required and typed. [`ModelRequest::caller`]
//! is a [`Caller`] and [`ModelRequest::source`] a [`Source`], with no default,
//! so a route that forgets who or what a call is for does not compile. Every
//! product call is a [`Caller::Member`] (a test keeps the list of
//! [`Caller::System`] reasons in product code, empty today). The usage is
//! logged here, at the one chokepoint, never in the routes: one `model.call`
//! line per call (source, outcome, latency, the reply's `usage` tokens, the
//! member's ids), one `model.call_failed` line per failed attempt (provider
//! status and error class), and `model.usage_threshold` once per UTC day per
//! scope when a count first reaches 80 % and 100 % of its limit
//! ([`USAGE_THRESHOLDS`]). No line carries a prompt, a reply, a name or a
//! note.
use crate::storage::TrustedRequest;
use axum::{
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
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
/// APP-215 (content M3): model calls one member may take per UTC day, unless
/// [`MEMBER_DAILY_CALLS_ENV`] says otherwise. 80: about twenty compositions,
/// or a long afternoon of Ask, for one person; twelve such people reach the
/// process cap of 1,000, which stays the ceiling.
pub const DEFAULT_MEMBER_DAILY_CALLS: u32 = 80;
/// The per-member allowance override, read like [`DAILY_CALLS_ENV`].
pub const MEMBER_DAILY_CALLS_ENV: &str = "GENUI_MODEL_DAILY_CALLS_PER_MEMBER";
/// Members the allowance table holds for one day before a new member is
/// refused (fail closed, like `http::limits::MAX_KEYS`); the table is emptied
/// at the reset. An organization of 25 never nears it.
pub const MAX_MEMBERS: usize = 20_000;
const DAY_SECONDS: u64 = 86_400;

/// APP-222: the percents of a limit at which `model.usage_threshold` fires,
/// each at most once per UTC day per scope (the process cap, each member).
pub const USAGE_THRESHOLDS: [u32; 2] = [80, 100];

/// A usage threshold one day's count has just reached: its percent and the
/// count that reached it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Threshold {
    pub percent: u32,
    pub count: u32,
}

/// The once-guard of [`USAGE_THRESHOLDS`] for one day's count: one bit per
/// threshold already reported. A new day starts with none.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Marks(u8);
impl Marks {
    const NONE: Self = Self(0);
    /// The thresholds `count` of `limit` has reached and that were not yet
    /// reported, now marked as reported: each is returned once.
    fn pass(&mut self, count: u32, limit: u32) -> Vec<Threshold> {
        let mut passed = Vec::new();
        for (bit, percent) in USAGE_THRESHOLDS.into_iter().enumerate() {
            let flag = 1u8 << bit;
            let reached =
                limit > 0 && u64::from(count) * 100 >= u64::from(limit) * u64::from(percent);
            if reached && self.0 & flag == 0 {
                self.0 |= flag;
                passed.push(Threshold { percent, count });
            }
        }
        passed
    }
}

/// One UTC day's calls (the day number is days since the UNIX epoch) and
/// the thresholds they have reported.
#[derive(Debug)]
struct Day {
    day: u64,
    calls: u32,
    marks: Marks,
}

/// Model calls taken per UTC day, against a limit. A call is one request to
/// the client (its retries included), counted before anything is sent.
#[derive(Debug)]
pub struct DailyCap {
    limit: u32,
    used: Mutex<Day>,
}
impl DailyCap {
    pub const fn new(limit: u32) -> Self {
        Self {
            limit,
            used: Mutex::new(Day {
                day: 0,
                calls: 0,
                marks: Marks::NONE,
            }),
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
        if day > used.day {
            *used = Day {
                day,
                calls: 0,
                marks: Marks::NONE,
            };
        }
        if used.calls >= self.limit {
            return false;
        }
        used.calls += 1;
        true
    }
    /// Whether a call at `now` would be refused (the day's count is at the
    /// limit); takes nothing.
    pub fn reached(&self, now: u64) -> bool {
        let day = now / DAY_SECONDS;
        let used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day > used.day {
            self.limit == 0
        } else {
            used.calls >= self.limit
        }
    }
    /// APP-222: the [`USAGE_THRESHOLDS`] the day's count at `now` has reached
    /// and not yet reported, now reported. Judged and marked under the lock,
    /// so of any number of concurrent calls exactly one reports each
    /// threshold, once per UTC day. A day with no count yet reports nothing.
    pub fn passed(&self, now: u64) -> Vec<Threshold> {
        let day = now / DAY_SECONDS;
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day > used.day {
            return Vec::new();
        }
        let calls = used.calls;
        used.marks.pass(calls, self.limit)
    }
}

/// APP-215 (content M3): model calls taken per UTC day per member, against
/// one limit. The key is [`member_key`] (`organization:principal`). Memory is
/// one entry per member who called today, at most [`MAX_MEMBERS`], emptied
/// when the day turns.
#[derive(Debug)]
pub struct MemberDailyCap {
    limit: u32,
    /// The UTC day number and, for that day only, each member's calls and
    /// the thresholds they have reported (APP-222).
    used: Mutex<(u64, Members)>,
}
/// One day's members: each member's calls and reported thresholds.
type Members = BTreeMap<String, (u32, Marks)>;
impl MemberDailyCap {
    pub const fn new(limit: u32) -> Self {
        Self {
            limit,
            used: Mutex::new((0, BTreeMap::new())),
        }
    }
    pub fn limit(&self) -> u32 {
        self.limit
    }
    /// Takes one call of `member` at `now`: `true` when their count was
    /// below the limit (it is now one higher), `false` when it is reached, or
    /// when the table is full and the member is new (nothing is taken). A
    /// later day starts every member at zero and drops yesterday's entries.
    pub fn take(&self, member: &str, now: u64) -> bool {
        let day = now / DAY_SECONDS;
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day > used.0 {
            *used = (day, BTreeMap::new());
        }
        match used.1.get_mut(member) {
            Some((count, _)) => {
                if *count >= self.limit {
                    return false;
                }
                *count += 1;
                true
            }
            None => {
                if self.limit == 0 || used.1.len() >= MAX_MEMBERS {
                    return false;
                }
                used.1.insert(member.to_owned(), (1, Marks::NONE));
                true
            }
        }
    }
    /// Gives back a call `take` admitted in the same day (the process cap
    /// refused it afterwards, so nothing was sent). A later day gives back
    /// nothing: the count it belonged to is gone.
    pub fn release(&self, member: &str, now: u64) {
        let day = now / DAY_SECONDS;
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day != used.0 {
            return;
        }
        if let Some((count, _)) = used.1.get_mut(member) {
            *count = count.saturating_sub(1);
        }
    }
    /// APP-222: [`DailyCap::passed`] for `member`'s count, with its own
    /// once-guard. The client asks only after both caps admitted the call,
    /// so a slot given back never reports.
    pub fn passed(&self, member: &str, now: u64) -> Vec<Threshold> {
        let day = now / DAY_SECONDS;
        let mut used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if day != used.0 {
            return Vec::new();
        }
        match used.1.get_mut(member) {
            Some((count, marks)) => {
                let count = *count;
                marks.pass(count, self.limit)
            }
            None => Vec::new(),
        }
    }
    /// Whether a call of `member` at `now` would be refused by their
    /// allowance; takes nothing.
    pub fn reached(&self, member: &str, now: u64) -> bool {
        let day = now / DAY_SECONDS;
        let used = self.used.lock().unwrap_or_else(PoisonError::into_inner);
        if self.limit == 0 {
            return true;
        }
        if day > used.0 {
            return false;
        }
        used.1
            .get(member)
            .is_some_and(|(count, _)| *count >= self.limit)
    }
    /// Members with a count today.
    pub fn members(&self) -> usize {
        self.used
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .1
            .len()
    }
}

/// The per-member key: a member's calls are counted within their
/// organization (`organization:principal`).
pub fn member_key(organization: &str, principal: &str) -> String {
    format!("{organization}:{principal}")
}

/// APP-222: who a model call is for. Every [`ModelRequest`] names one; there
/// is no default. A member's call takes a slot of their own allowance
/// ([`MemberDailyCap`]) before the process cap's; a system call takes the
/// process cap's only. Only identities travel here (organization and
/// principal ids), never a name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Caller {
    /// A signed-in member, acting on any surface (the canvas, MCP, Slack).
    Member {
        organization: String,
        principal: String,
    },
    /// The service itself, for a fixed reason. None in product code today:
    /// the composer test `no_product_model_call_is_a_system_call` keeps the
    /// closed list of reasons, and it is empty.
    System { reason: &'static str },
}
impl Caller {
    /// The member a workspace request is.
    pub fn member(request: &TrustedRequest) -> Self {
        Self::Member {
            organization: request.organization().to_owned(),
            principal: request.principal().to_owned(),
        }
    }
    /// Their allowance key ([`member_key`]); `None` for a system call.
    pub fn allowance_key(&self) -> Option<String> {
        match self {
            Self::Member {
                organization,
                principal,
            } => Some(member_key(organization, principal)),
            Self::System { .. } => None,
        }
    }
    pub fn organization(&self) -> Option<&str> {
        match self {
            Self::Member { organization, .. } => Some(organization),
            Self::System { .. } => None,
        }
    }
    pub fn principal(&self) -> Option<&str> {
        match self {
            Self::Member { principal, .. } => Some(principal),
            Self::System { .. } => None,
        }
    }
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Member { .. } => None,
            Self::System { reason } => Some(reason),
        }
    }
}

/// Tests: a synthetic member to make calls for (a test never names a
/// [`Caller::System`] outside this module; see the composer test
/// `no_product_model_call_is_a_system_call`).
#[cfg(test)]
pub(crate) fn test_caller() -> Caller {
    Caller::Member {
        organization: "00000000-0000-4000-8000-00000000a222".into(),
        principal: "00000000-0000-4000-8000-00000000b222".into(),
    }
}

/// APP-222: the product path a model call is made for, logged with every
/// call, so a spike says whether it is building in the canvas, an agent
/// looping through MCP, or Slack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// The canvas composer (`/v1/compose`) and its live transform.
    Canvas,
    /// Ask over records and widgets (`/v1/ask`).
    Ask,
    /// Shape (`/v1/shape`), the widget restyle it reaches included.
    Shape,
    /// The folder routes: build, brief, explain and the folder's Ask.
    Folder,
    /// The action path (`/v1/act`, the receipt, the assistant), and a folder
    /// request made through it.
    Action,
    /// The widget customizer route.
    Customizer,
    /// An agent over MCP (`forma_compose_app` and the folder tools).
    Mcp,
    /// Slack's slash command, and a folder request made through it.
    Slack,
}
impl Source {
    pub const ALL: [Self; 8] = [
        Self::Canvas,
        Self::Ask,
        Self::Shape,
        Self::Folder,
        Self::Action,
        Self::Customizer,
        Self::Mcp,
        Self::Slack,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Ask => "ask",
            Self::Shape => "shape",
            Self::Folder => "folder",
            Self::Action => "action",
            Self::Customizer => "customizer",
            Self::Mcp => "mcp",
            Self::Slack => "slack",
        }
    }
}

/// The daily limit `value` names: a decimal count, else the default.
pub fn limit_from(value: Option<&str>) -> u32 {
    value
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_DAILY_CALLS)
}
/// The per-member allowance `value` names: a decimal count, else the default.
pub fn member_limit_from(value: Option<&str>) -> u32 {
    value
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(DEFAULT_MEMBER_DAILY_CALLS)
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
/// The app's one per-member allowance table (APP-215), shared by every
/// [`Anthropic`] client of the process like [`daily_cap`].
pub fn member_daily_cap() -> &'static MemberDailyCap {
    static CAP: OnceLock<MemberDailyCap> = OnceLock::new();
    CAP.get_or_init(|| {
        let limit = member_limit_from(std::env::var(MEMBER_DAILY_CALLS_ENV).ok().as_deref());
        tracing::info!(
            event = "composer.model_member_daily_cap_set",
            limit,
            "daily model-call allowance per member"
        );
        MemberDailyCap::new(limit)
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
/// What a member reads when their own allowance is reached (APP-215; the
/// canvas has its own copy in `daily-limit.ts`).
pub const MEMBER_DAILY_LIMIT_TEXT: &str = "You've used your AI allowance for today. Building apps and Ask resume for you at 00:00 UTC; your apps and entries keep working.";

/// The text for a `daily_limit` refusal of `caller` (MCP and Slack): the
/// member's own words when their allowance is what is reached, else the
/// workspace's.
pub fn daily_limit_text_for(caller: &Caller) -> &'static str {
    if member_limit_reached(caller) {
        MEMBER_DAILY_LIMIT_TEXT
    } else {
        DAILY_LIMIT_TEXT
    }
}
fn member_limit_reached(caller: &Caller) -> bool {
    caller
        .allowance_key()
        .is_some_and(|key| member_daily_cap().reached(&key, unix_now()))
}

/// The answer of every model route to [`ModelError::DailyCap`]: 429
/// `daily_limit`, `Retry-After` the seconds until the count starts again.
pub fn daily_limit() -> Response {
    with_retry_after(crate::http::problem(
        StatusCode::TOO_MANY_REQUESTS,
        "daily_limit",
    ))
}
/// [`daily_limit`] for `caller` (APP-215): when a member's own allowance is
/// what is reached, the body carries `"scope":"member"` so the canvas says
/// so (`daily-limit.ts`); otherwise the body is the workspace's unchanged
/// `{"code":"daily_limit","status":429}`. Both are judged on the live
/// counts: a member whose allowance is reached is told so whatever the
/// process cap says, since nothing of theirs is sent today.
pub fn daily_limit_for(caller: &Caller) -> Response {
    if member_limit_reached(caller) {
        with_retry_after(crate::http::problem_with(
            StatusCode::TOO_MANY_REQUESTS,
            "daily_limit",
            json!({"scope": "member"}),
        ))
    } else {
        daily_limit()
    }
}
fn with_retry_after(mut response: Response) -> Response {
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
    /// APP-222 (was APP-215's optional member): who the call is for. A
    /// member takes one slot of their daily allowance before the process cap.
    pub caller: Caller,
    /// APP-222: the product path the call is made for, logged with it.
    pub source: Source,
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
    /// APP-215: whether the next call of `caller` would be refused by their
    /// own allowance (a stub has none; a system call has none).
    fn member_daily_cap_reached(&self, caller: &Caller) -> bool {
        let _ = caller;
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
    /// [`member_daily_cap`] for every client the binary builds (APP-215).
    member_cap: &'static MemberDailyCap,
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
            member_cap: member_daily_cap(),
        })
    }
    /// Tests and campaigns: this client counts against `cap` instead of the
    /// process's, so a low cap can be reached without touching the others.
    #[cfg(test)]
    pub(crate) fn with_daily_cap(mut self, cap: &'static DailyCap) -> Self {
        self.cap = cap;
        self
    }
    /// Tests: this client counts members against `cap` instead of the
    /// process's table (APP-215).
    #[cfg(test)]
    pub(crate) fn with_member_daily_cap(mut self, cap: &'static MemberDailyCap) -> Self {
        self.member_cap = cap;
        self
    }
    /// One slot of the member's allowance, then one of the daily cap, for
    /// `request` at `now`, before anything is sent (APP-215). A refusal takes
    /// nothing: a member slot the process cap then refuses is given back, and
    /// the refusal is the call's `model.call` line (`refused_cap`). An
    /// admitted call reports the usage thresholds it reached (APP-222).
    fn admit(&self, request: &ModelRequest, now: u64) -> Result<(), ModelError> {
        let key = request.caller.allowance_key();
        if let Some(key) = &key {
            if !self.member_cap.take(key, now) {
                log_refused(request, Scope::Member, self.member_cap.limit());
                return Err(ModelError::DailyCap);
            }
        }
        if !self.cap.take(now) {
            if let Some(key) = &key {
                self.member_cap.release(key, now);
            }
            log_refused(request, Scope::Organization, self.cap.limit());
            return Err(ModelError::DailyCap);
        }
        for threshold in self.cap.passed(now) {
            log_threshold(
                Scope::Organization,
                threshold,
                self.cap.limit(),
                &request.caller,
            );
        }
        if let Some(key) = &key {
            for threshold in self.member_cap.passed(key, now) {
                log_threshold(
                    Scope::Member,
                    threshold,
                    self.member_cap.limit(),
                    &request.caller,
                );
            }
        }
        Ok(())
    }
    /// Tests: [`Self::admit`] at an injected clock.
    #[cfg(test)]
    pub(crate) fn admit_at(&self, request: &ModelRequest, now: u64) -> Result<(), ModelError> {
        self.admit(request, now)
    }
    /// The client the environment configures: `KEY_ENV` (the only source of
    /// the key) and the optional endpoint override. `None`: disabled.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var(KEY_ENV).ok()?;
        let endpoint = std::env::var(ENDPOINT_ENV).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
        Self::new(&key, &endpoint)
    }

    /// One POST of `body` within `deadline`: the reply's bytes, or why not
    /// (the error, whether a retry may help, its class and the provider
    /// status for `model.call_failed`).
    async fn post(&self, body: &Value, deadline: Instant) -> Result<Vec<u8>, Failure> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Failure::TIMEOUT)?;
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
                    Failure::TIMEOUT
                } else {
                    Failure::TRANSPORT
                }
            })?;
        let status = response.status();
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(Failure::status(
                ModelError::Unavailable,
                true,
                status.as_u16(),
            ));
        }
        if !status.is_success() {
            return Err(Failure::status(ModelError::Refused, false, status.as_u16()));
        }
        capped_body(response, MAX_REPLY_BYTES)
            .await
            .map_err(|error| match error {
                ModelError::Unavailable => Failure::TRANSPORT,
                _ => Failure::DECODE,
            })
    }
}

/// Why one attempt produced no reply (APP-222 `model.call_failed`): the
/// error the caller sees, whether it is retried, the error class (`timeout`,
/// `status`, `transport` or `decode`) and the provider's HTTP status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Failure {
    error: ModelError,
    retryable: bool,
    class: &'static str,
    status: Option<u16>,
}
impl Failure {
    /// The budget ran out, before sending or while waiting.
    const TIMEOUT: Self = Self {
        error: ModelError::Timeout,
        retryable: false,
        class: "timeout",
        status: None,
    };
    /// The connection or the body read failed.
    const TRANSPORT: Self = Self {
        error: ModelError::Unavailable,
        retryable: true,
        class: "transport",
        status: None,
    };
    /// A reply the protocol does not describe, or one over the size bound.
    const DECODE: Self = Self {
        error: ModelError::Malformed,
        retryable: false,
        class: "decode",
        status: None,
    };
    fn status(error: ModelError, retryable: bool, status: u16) -> Self {
        Self {
            error,
            retryable,
            class: "status",
            status: Some(status),
        }
    }
}

/// Which allowance a usage line is about: the process cap (`organization`,
/// the whole app's, which the trial's one organization holds) or one
/// member's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    Organization,
    Member,
}
impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::Member => "member",
        }
    }
}

/// `model.call`, outcome `ok`: the reply's usage, never its text.
fn log_ok(
    request: &ModelRequest,
    model: Option<&str>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    latency_ms: u64,
    attempts: u32,
) {
    tracing::info!(
        event = "model.call",
        source = request.source.as_str(),
        outcome = "ok",
        latency_ms,
        input_tokens,
        output_tokens,
        attempts,
        requested_model = request.model,
        model,
        organization = request.caller.organization(),
        principal = request.caller.principal(),
        reason = request.caller.reason(),
        "model replied"
    );
}
/// `model.call`, outcome `refused_cap`: nothing was sent.
fn log_refused(request: &ModelRequest, scope: Scope, limit: u32) {
    tracing::warn!(
        event = "model.call",
        source = request.source.as_str(),
        outcome = "refused_cap",
        code = ModelError::DailyCap.code(),
        scope = scope.as_str(),
        limit,
        latency_ms = 0u64,
        requested_model = request.model,
        organization = request.caller.organization(),
        principal = request.caller.principal(),
        reason = request.caller.reason(),
        "daily model-call limit reached; nothing sent"
    );
}
/// `model.call`, outcome `failed`: every attempt failed or the budget ran out.
fn log_failed(request: &ModelRequest, error: ModelError, latency_ms: u64, attempts: u32) {
    tracing::warn!(
        event = "model.call",
        source = request.source.as_str(),
        outcome = "failed",
        code = error.code(),
        latency_ms,
        attempts,
        requested_model = request.model,
        organization = request.caller.organization(),
        principal = request.caller.principal(),
        reason = request.caller.reason(),
        "model call failed"
    );
}
/// `model.call_failed`: one failed attempt, its class and provider status.
fn log_attempt(request: &ModelRequest, failure: &Failure, attempt: u32, latency_ms: u64) {
    tracing::warn!(
        event = "model.call_failed",
        source = request.source.as_str(),
        class = failure.class,
        status = failure.status,
        code = failure.error.code(),
        attempt,
        retryable = failure.retryable,
        latency_ms,
        requested_model = request.model,
        "model attempt failed"
    );
}
/// `model.usage_threshold`: a scope's count first reached `threshold`
/// today (once per UTC day per scope and threshold).
fn log_threshold(scope: Scope, threshold: Threshold, limit: u32, caller: &Caller) {
    let member = scope == Scope::Member;
    tracing::warn!(
        event = "model.usage_threshold",
        scope = scope.as_str(),
        percent = threshold.percent,
        limit,
        count = threshold.count,
        organization = caller.organization().filter(|_| member),
        principal = caller.principal().filter(|_| member),
        "daily model-call usage threshold reached"
    );
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
    fn member_daily_cap_reached(&self, caller: &Caller) -> bool {
        caller
            .allowance_key()
            .is_some_and(|key| self.member_cap.reached(&key, unix_now()))
    }
    fn complete_with_documents<'a>(
        &'a self,
        request: &'a ModelRequest,
        documents: &'a [DocumentInput<'a>],
    ) -> CitedCompletion<'a> {
        use base64::Engine as _;
        Box::pin(async move {
            self.admit(request, unix_now())?;
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
            // One attempt: a document call is not retried.
            let outcome = self
                .post(&body, started + request.budget)
                .await
                .and_then(|bytes| parse_cited(&bytes).ok_or(Failure::DECODE));
            match outcome {
                Ok(reply) => {
                    log_ok(
                        request,
                        None,
                        reply.input_tokens,
                        reply.output_tokens,
                        elapsed_ms(started),
                        1,
                    );
                    Ok(reply)
                }
                Err(failure) => {
                    log_attempt(request, &failure, 0, elapsed_ms(started));
                    log_failed(request, failure.error, elapsed_ms(started), 1);
                    Err(failure.error)
                }
            }
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
            self.admit(request, unix_now())?;
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
            let mut attempts = 0;
            for attempt in 0..MAX_ATTEMPTS {
                if attempt > 0 {
                    let pause = BACKOFF[usize::from(attempt as u16 - 1).min(BACKOFF.len() - 1)];
                    if Instant::now() + pause >= deadline {
                        outcome = Err(ModelError::Timeout);
                        break;
                    }
                    tokio::time::sleep(pause).await;
                }
                attempts = attempt + 1;
                let reply = self
                    .post(&body, deadline)
                    .await
                    .and_then(|bytes| parse_reply(&bytes).ok_or(Failure::DECODE));
                match reply {
                    Ok(reply) => {
                        log_ok(
                            request,
                            Some(&reply.model),
                            reply.input_tokens,
                            reply.output_tokens,
                            elapsed_ms(started),
                            attempts,
                        );
                        return Ok(reply);
                    }
                    Err(failure) => {
                        log_attempt(request, &failure, attempt, elapsed_ms(started));
                        outcome = Err(failure.error);
                        if !failure.retryable {
                            break;
                        }
                    }
                }
            }
            if let Err(error) = outcome {
                log_failed(request, error, elapsed_ms(started), attempts);
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

    /// APP-215 (content M3): each member has their own daily allowance,
    /// counted within their organization; a refusal takes nothing, a slot
    /// the process cap refuses goes back, the day turns at 00:00 UTC and
    /// empties the table, and a full table refuses a new member.
    #[test]
    fn each_member_has_a_daily_allowance_that_resets_at_utc_midnight() {
        let cap = MemberDailyCap::new(2);
        let ana = member_key("org-a", "ana");
        let bo = member_key("org-a", "bo");
        let evening = MIDNIGHT - 60;
        assert!(!cap.reached(&ana, evening));
        assert!(cap.take(&ana, evening));
        assert!(cap.take(&ana, evening + 1));
        assert!(!cap.take(&ana, evening + 2), "the third call is refused");
        assert!(cap.reached(&ana, evening + 2));
        // Another member, and the same principal in another organization,
        // have their own counts.
        assert!(cap.take(&bo, evening + 2));
        assert!(cap.take(&member_key("org-b", "ana"), evening + 2));
        assert_eq!(cap.members(), 3);
        // A slot the process cap refused goes back; a release in a later day
        // gives back nothing.
        cap.release(&ana, evening + 3);
        assert!(!cap.reached(&ana, evening + 3));
        assert!(cap.take(&ana, evening + 3));
        assert!(!cap.take(&ana, evening + 4));
        // 00:00 UTC: everyone starts at zero and yesterday's table is gone.
        assert!(!cap.reached(&ana, MIDNIGHT));
        assert!(cap.take(&ana, MIDNIGHT));
        assert_eq!(cap.members(), 1);
        cap.release(&ana, MIDNIGHT - 1);
        assert!(
            cap.take(&ana, MIDNIGHT + 1),
            "the stale release changed nothing"
        );
        assert!(!cap.take(&ana, MIDNIGHT + 2));
        // A zero allowance refuses everyone; the table is bounded.
        let closed = MemberDailyCap::new(0);
        assert!(!closed.take(&ana, MIDNIGHT) && closed.reached(&ana, MIDNIGHT));
        assert_eq!(DEFAULT_MEMBER_DAILY_CALLS, 80);
        assert_eq!(member_limit_from(None), 80);
        assert_eq!(member_limit_from(Some(" 5 ")), 5);
        assert_eq!(member_limit_from(Some("many")), 80);
        assert_eq!(MAX_MEMBERS, 20_000);
    }

    /// APP-215: the client takes the member's slot before the process cap's
    /// and gives it back when the process cap refuses, so neither cap is
    /// spent by a refusal; a request without a member touches the member
    /// table not at all.
    #[tokio::test]
    async fn the_client_admits_against_the_member_allowance_then_the_process_cap() {
        static MEMBERS: MemberDailyCap = MemberDailyCap::new(1);
        static PROCESS: DailyCap = DailyCap::new(2);
        // An endpoint nothing listens on: an admitted call fails as
        // `Unavailable` after it was counted; a refused one never connects.
        let client = Anthropic::new("sk-ant-synthetic", "http://127.0.0.1:9/v1/messages")
            .unwrap()
            .with_daily_cap(&PROCESS)
            .with_member_daily_cap(&MEMBERS);
        let request = |caller: Caller| ModelRequest {
            model: "m".into(),
            system: String::new(),
            user: String::new(),
            max_tokens: 1,
            budget: Duration::from_millis(200),
            caller,
            source: Source::Canvas,
        };
        let member = |principal: &str| Caller::Member {
            organization: "org".into(),
            principal: principal.into(),
        };
        let now = unix_now();
        let ana = member_key("org", "ana");
        // An admitted call that never connects: `Unavailable`, or `Timeout`
        // when the retry's pause does not fit the budget; never `DailyCap`.
        let sent = |outcome: Result<ModelReply, ModelError>| {
            matches!(outcome, Err(ModelError::Unavailable | ModelError::Timeout))
        };
        assert!(sent(client.complete(&request(member("ana"))).await));
        assert!(MEMBERS.reached(&ana, now));
        assert!(!PROCESS.reached(now));
        assert_eq!(
            client.complete(&request(member("ana"))).await,
            Err(ModelError::DailyCap),
            "the member's allowance refuses first"
        );
        assert!(!PROCESS.reached(now), "and the process cap was not spent");
        assert!(client.member_daily_cap_reached(&member("ana")));
        // A system call takes the process cap's last slot and no member's.
        let system = Caller::System { reason: "test" };
        assert!(!client.member_daily_cap_reached(&system));
        assert!(sent(client.complete(&request(system)).await));
        assert_eq!(MEMBERS.members(), 1);
        assert!(PROCESS.reached(now) && client.daily_cap_reached());
        // Bo has allowance but the process cap is full: refused, and Bo's slot
        // goes back.
        let bo = member_key("org", "bo");
        assert_eq!(
            client.complete(&request(member("bo"))).await,
            Err(ModelError::DailyCap)
        );
        assert!(!MEMBERS.reached(&bo, now));
    }

    /// APP-222: with a process limit of 10 and a member allowance of 5, 30
    /// concurrent calls by two members admit exactly 10, and the usage
    /// thresholds fire exactly once per scope per UTC day: the process cap's
    /// 80 % and 100 %, and each member's. The 20 refusals are each one
    /// `model.call` line, `refused_cap`, scope `member`. A new UTC day (the
    /// clock is injected) fires them all again, once.
    #[test]
    fn usage_thresholds_fire_once_per_scope_per_day_under_concurrency() {
        static PROCESS: DailyCap = DailyCap::new(10);
        static MEMBERS: MemberDailyCap = MemberDailyCap::new(5);
        let client = Anthropic::new("sk-ant-synthetic", "http://127.0.0.1:9/v1/messages")
            .unwrap()
            .with_daily_cap(&PROCESS)
            .with_member_daily_cap(&MEMBERS);
        let request = |principal: &str| ModelRequest {
            model: "m".into(),
            system: String::new(),
            user: String::new(),
            max_tokens: 1,
            budget: Duration::from_millis(200),
            caller: Caller::Member {
                organization: "org".into(),
                principal: principal.into(),
            },
            source: Source::Canvas,
        };
        let requests = [request("ana"), request("bo")];
        let lines = crate::config::logs::testing::Lines::default();
        let dispatch = lines.dispatch();
        for day in 0..2u64 {
            let now = MIDNIGHT + day * DAY_SECONDS + 3600;
            let barrier = std::sync::Barrier::new(30);
            let admitted: usize = std::thread::scope(|scope| {
                let calls: Vec<_> = (0..30)
                    .map(|n| {
                        let (client, barrier, dispatch) = (&client, &barrier, &dispatch);
                        let request = &requests[n % 2];
                        scope.spawn(move || {
                            tracing::dispatcher::with_default(dispatch, || {
                                barrier.wait();
                                client.admit_at(request, now).is_ok()
                            })
                        })
                    })
                    .collect();
                calls
                    .into_iter()
                    .map(|call| usize::from(call.join().unwrap()))
                    .sum()
            });
            assert_eq!(admitted, 10, "day {day}: the process limit, 5 each");
            assert!(PROCESS.reached(now));
        }
        let thresholds = lines.events("model.usage_threshold");
        let mut seen: Vec<(u64, String, String, u64)> = thresholds
            .iter()
            .map(|line| {
                let count = line["count"].as_u64().unwrap();
                let percent = line["percent"].as_u64().unwrap();
                let limit = line["limit"].as_u64().unwrap();
                assert!(count * 100 >= limit * percent && count <= limit, "{line}");
                (
                    percent,
                    line["scope"].as_str().unwrap().to_owned(),
                    line["principal"].as_str().unwrap_or("-").to_owned(),
                    limit,
                )
            })
            .collect();
        seen.sort();
        let once = |percent: u64, scope: &str, principal: &str, limit: u64| {
            (percent, scope.to_owned(), principal.to_owned(), limit)
        };
        let mut day = [
            once(80, "member", "ana", 5),
            once(80, "member", "bo", 5),
            once(80, "organization", "-", 10),
            once(100, "member", "ana", 5),
            once(100, "member", "bo", 5),
            once(100, "organization", "-", 10),
        ];
        day.sort();
        let both_days: Vec<_> = day.iter().flat_map(|t| [t.clone(), t.clone()]).collect();
        assert_eq!(seen, both_days, "once per scope per threshold per day");
        for line in &thresholds {
            assert_eq!(line["level"], "WARN");
            let member = line["scope"] == "member";
            assert_eq!(line.get("organization").is_some(), member, "{line}");
        }
        let refused: Vec<_> = lines
            .events("model.call")
            .into_iter()
            .filter(|line| line["outcome"] == "refused_cap")
            .collect();
        assert_eq!(refused.len(), 40, "20 refusals a day");
        assert!(refused
            .iter()
            .all(|line| line["scope"] == "member" && line["source"] == "canvas"));
        // The once-guard itself: a count past a threshold reports nothing new.
        let mut marks = Marks::NONE;
        assert_eq!(marks.pass(7, 10), []);
        assert_eq!(
            marks.pass(8, 10),
            [Threshold {
                percent: 80,
                count: 8
            }]
        );
        assert_eq!(marks.pass(9, 10), []);
        assert_eq!(
            marks.pass(10, 10),
            [Threshold {
                percent: 100,
                count: 10
            }]
        );
        assert_eq!(marks.pass(10, 10), []);
        let mut one = Marks::NONE;
        assert_eq!(one.pass(1, 1).len(), 2, "a limit of 1 reaches both at once");
        let mut zero = Marks::NONE;
        assert_eq!(zero.pass(0, 0), [], "a zero limit never fires");
        assert_eq!(USAGE_THRESHOLDS, [80, 100]);
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
        // APP-215: a member whose own allowance is not reached reads the
        // workspace's refusal, unchanged; one whose allowance is reached reads
        // their own, same code and status, `scope` added.
        let of = |principal: &str| Caller::Member {
            organization: "org".into(),
            principal: principal.into(),
        };
        let fresh = of("fresh");
        let body = axum::body::to_bytes(daily_limit_for(&fresh).into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], br#"{"code":"daily_limit","status":429}"#);
        assert_eq!(daily_limit_text_for(&fresh), DAILY_LIMIT_TEXT);
        let spent = of("spent");
        let table = member_daily_cap();
        while table.take(&member_key("org", "spent"), unix_now()) {}
        let response = daily_limit_for(&spent);
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().contains_key(header::RETRY_AFTER));
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            &body[..],
            br#"{"code":"daily_limit","scope":"member","status":429}"#
        );
        assert_eq!(daily_limit_text_for(&spent), MEMBER_DAILY_LIMIT_TEXT);
        assert!(MEMBER_DAILY_LIMIT_TEXT.contains("your AI allowance"));
        assert!(MEMBER_DAILY_LIMIT_TEXT.contains("00:00 UTC"));
        assert!(MEMBER_DAILY_LIMIT_TEXT.contains("apps and entries keep working"));
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
