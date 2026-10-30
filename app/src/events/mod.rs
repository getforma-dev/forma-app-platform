//! APP-095 (ADR-011 pitch, `pivot/realtime`): real-time updates.
//!
//! One in-process hub (`hub()`, a `tokio::sync::broadcast` channel) carries
//! three kinds of announcement, each naming an organization, an app and a
//! saved work and nothing else: `record.committed` (the command route, after
//! `CommandResult::Committed`), `app.shared` (the share route, after a share
//! that wrote a grant) and `app.placed` (the composer's hook,
//! [`Hub::app_placed`], when it places an app on the canvas). The hub never
//! judges authority: `GET /v1/events` (`routes`) resolves the caller's
//! workspace session, and the per-connection pump re-authorizes **every**
//! event at delivery (`storage::Data::can_open`: `discover` on the app and
//! `record_read` on the work, on current state, at the session's epoch) before
//! an id reaches the wire. An event for a work the member cannot open is
//! dropped silently; an event of another organization is dropped before any
//! lookup; a member who is no longer current (sign-out or an operator
//! revocation advanced the epoch, the entitlement lapsed) closes the stream
//! (`storage::Data::member_current`, re-checked on every refusal and every
//! `RECHECK` tick). Streams are one per session (a newer stream for the same
//! session closes the earlier one), at most `MAX_STREAMS_PER_MEMBER` per
//! member and `MAX_STREAMS` in the process (429 past them), heartbeat every
//! `HEARTBEAT`, and end after `STREAM_LIFETIME` so the client re-resolves its
//! session by reconnecting.
//!
//! Wire shape, one SSE message per event:
//! `event: <kind>` and `data: {"kind":"record.committed"|"app.shared"|
//! "app.placed","app_id":"<uuid>","work_id":"<uuid>","at":"<whole-second UTC>"}`.
//!
//! Recorded limitation (ADR-011 debt, APP-095): the hub is in-process, which
//! is exactly right for ADR-007 D7.18's single service task (stop-then-start);
//! a multi-task deployment (APP-061) needs a shared channel or a database
//! notification, or members connected to another task miss events.
pub mod routes;
#[cfg(test)]
mod tests;

use serde_json::{json, Value};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, mpsc, Notify};

/// Open streams one member (organization, principal) may hold.
pub const MAX_STREAMS_PER_MEMBER: usize = 4;
/// Open streams the process holds in all.
pub const MAX_STREAMS: usize = 256;
/// SSE comment heartbeat interval.
pub const HEARTBEAT: Duration = Duration::from_secs(15);
/// How often an idle stream re-checks that its member is still current.
pub const RECHECK: Duration = Duration::from_secs(30);
/// A stream ends after this long; the client reconnects (re-resolving its
/// session, ADR-006 lifetimes), so a stream never outlives a session by more.
pub const STREAM_LIFETIME: Duration = Duration::from_secs(3600);
/// Broadcast capacity; a subscriber that lags past it skips the gap (an
/// event is a hint to re-read, never the data itself).
const CHANNEL_CAPACITY: usize = 256;
/// Per-connection delivery queue toward the SSE body.
const SINK_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    RecordCommitted,
    AppShared,
    AppPlaced,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::RecordCommitted => "record.committed",
            Self::AppShared => "app.shared",
            Self::AppPlaced => "app.placed",
        }
    }
}

/// One announcement: ids only, never a value, a title or a principal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: Kind,
    pub organization: String,
    pub app: String,
    pub work: String,
    /// Whole-second UTC, as the audit chain's `occurred_at`.
    pub at: String,
}
impl Event {
    fn now(kind: Kind, organization: &str, app: &str, work: &str) -> Self {
        Self {
            kind,
            organization: organization.to_owned(),
            app: app.to_owned(),
            work: work.to_owned(),
            at: timestamp(unix_seconds()),
        }
    }
    /// The wire document (the organization is the subscriber's own and is
    /// not repeated).
    pub fn document(&self) -> Value {
        json!({
            "kind": self.kind.name(),
            "app_id": self.app,
            "work_id": self.work,
            "at": self.at,
        })
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
/// `YYYY-MM-DDTHH:MM:SSZ` for a Unix second (proleptic Gregorian).
pub fn timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rem = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[derive(Default)]
struct Streams {
    total: usize,
    per_member: HashMap<(String, String), usize>,
    /// One stream per session: the newer registration closes the earlier.
    per_session: HashMap<String, Arc<Notify>>,
}

/// Why a stream was not opened (429 with `Retry-After`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    MemberLimit,
    GlobalLimit,
}

pub struct Hub {
    sender: broadcast::Sender<Event>,
    streams: Mutex<Streams>,
}
impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

static HUB: OnceLock<Hub> = OnceLock::new();
/// The process's one hub (ADR-007 D7.18: one task, so in-process is complete).
pub fn hub() -> &'static Hub {
    HUB.get_or_init(Hub::new)
}

impl Hub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self {
            sender,
            streams: Mutex::new(Streams::default()),
        }
    }
    pub fn publish(&self, event: Event) {
        // No subscriber is not an error: the announcement is best effort.
        let _ = self.sender.send(event);
    }
    /// The command route's hook after `CommandResult::Committed`.
    pub fn record_committed(&self, organization: &str, app: &str, work: &str) {
        self.publish(Event::now(Kind::RecordCommitted, organization, app, work));
    }
    /// The share route's hook after a share that wrote a grant.
    pub fn app_shared(&self, organization: &str, app: &str, work: &str) {
        self.publish(Event::now(Kind::AppShared, organization, app, work));
    }
    /// The composer's hook (`pivot/composer`): an app placed on the canvas as
    /// saved work `work` of `app` in `organization`. Subscribers who can open
    /// that work learn of it; nobody else does.
    pub fn app_placed(&self, organization: &str, app: &str, work: &str) {
        self.publish(Event::now(Kind::AppPlaced, organization, app, work));
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
    /// Open streams now (tests and the bounds).
    pub fn open_streams(&self) -> usize {
        self.streams.lock().map(|s| s.total).unwrap_or(0)
    }
    /// Counts a stream for `(organization, principal)` under `session`; a
    /// stream already open for the same session is told to close.
    pub fn register(
        &'static self,
        organization: &str,
        principal: &str,
        session: &str,
    ) -> Result<Registration, Refused> {
        let mut streams = self.streams.lock().map_err(|_| Refused::GlobalLimit)?;
        let member = (organization.to_owned(), principal.to_owned());
        if streams.total >= MAX_STREAMS {
            return Err(Refused::GlobalLimit);
        }
        let held = streams.per_member.get(&member).copied().unwrap_or(0);
        // Replacing the same session's stream does not raise the member's count.
        let replacing = streams.per_session.contains_key(session);
        if held >= MAX_STREAMS_PER_MEMBER && !replacing {
            return Err(Refused::MemberLimit);
        }
        let closed = Arc::new(Notify::new());
        if let Some(earlier) = streams
            .per_session
            .insert(session.to_owned(), closed.clone())
        {
            earlier.notify_one();
        }
        streams.total += 1;
        *streams.per_member.entry(member.clone()).or_insert(0) += 1;
        Ok(Registration {
            hub: self,
            member,
            session: session.to_owned(),
            closed,
        })
    }
    fn unregister(&self, registration: &Registration) {
        let Ok(mut streams) = self.streams.lock() else {
            return;
        };
        streams.total = streams.total.saturating_sub(1);
        if let Some(count) = streams.per_member.get_mut(&registration.member) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                streams.per_member.remove(&registration.member);
            }
        }
        // Only this registration's own entry; a newer stream of the same
        // session keeps its place.
        if streams
            .per_session
            .get(&registration.session)
            .is_some_and(|current| Arc::ptr_eq(current, &registration.closed))
        {
            streams.per_session.remove(&registration.session);
        }
    }
}

/// A counted open stream; dropping it releases its slots.
pub struct Registration {
    hub: &'static Hub,
    member: (String, String),
    session: String,
    closed: Arc<Notify>,
}
impl Registration {
    /// Signalled when a newer stream of the same session replaced this one.
    pub fn closed(&self) -> Arc<Notify> {
        self.closed.clone()
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.hub.unregister(self);
    }
}

/// The delivery decision for one event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Deliver,
    /// The member cannot open this work now; nothing is sent.
    Skip,
    /// The member is no longer current; the stream ends.
    Close,
}
/// What the pump asks before every delivery and on every re-check tick. The
/// production gate is `routes::StoreGate` (storage re-authorization); tests
/// substitute a scripted one.
pub trait Gate: Send + Sync + 'static {
    fn judge(&self, app: &str, work: &str) -> impl Future<Output = Verdict> + Send;
    fn current(&self) -> impl Future<Output = bool> + Send;
}

/// The per-connection pump: events of the subscriber's organization are
/// judged by `gate` one by one and the delivered ones go to `sink` (the SSE
/// body). Ends when the gate closes the stream, the member is no longer
/// current at a re-check, a newer stream replaced this session's, the body
/// was dropped (client gone), the lifetime passed or the hub closed.
pub async fn pump<G: Gate>(
    gate: G,
    organization: String,
    mut receiver: broadcast::Receiver<Event>,
    registration: Registration,
    sink: mpsc::Sender<Event>,
    recheck: Duration,
    lifetime: Duration,
) {
    let closed = registration.closed();
    let start = tokio::time::Instant::now();
    let lifetime = tokio::time::sleep_until(start + lifetime);
    tokio::pin!(lifetime);
    let mut ticker = tokio::time::interval_at(start + recheck, recheck);
    loop {
        tokio::select! {
            _ = &mut lifetime => break,
            _ = closed.notified() => break,
            _ = sink.closed() => break,
            _ = ticker.tick() => {
                if !gate.current().await {
                    break;
                }
            }
            received = receiver.recv() => match received {
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Ok(event) => {
                    if event.organization != organization {
                        continue;
                    }
                    match gate.judge(&event.app, &event.work).await {
                        Verdict::Deliver => {
                            if sink.send(event).await.is_err() {
                                break;
                            }
                        }
                        Verdict::Skip => {}
                        Verdict::Close => break,
                    }
                }
            },
        }
    }
    drop(registration);
}

/// Opens the per-connection queue the pump feeds and the body drains.
pub fn sink() -> (mpsc::Sender<Event>, mpsc::Receiver<Event>) {
    mpsc::channel(SINK_CAPACITY)
}
