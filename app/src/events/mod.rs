//! APP-095 (ADR-011 pitch, `pivot/realtime`): real-time updates.
//!
//! One in-process hub (`hub()`, a `tokio::sync::broadcast` channel) carries
//! three kinds of announcement, each naming an organization, an app and a
//! saved work and nothing else: `record.committed` (the command route, after
//! `CommandResult::Committed`), `app.shared` (the share route, after a share
//! that wrote a grant) and `app.placed` (the composer's hook,
//! [`Hub::app_placed`], when it places an app on the canvas). The hub never
//! judges authority. `GET /v1/events` (`routes`) resolves the caller's
//! workspace session, and every event is judged **once per event for the
//! whole organization** before an id reaches any wire (review M2): the first
//! per-connection pump to reach an announcement runs one scoped, lock-free
//! storage read (`storage::Data::openable_principals`) that names, among the
//! organization's subscribed principals, who is current (active organization
//! and membership at the subscriber's epoch, live entitlement) and who can
//! open `(app, work)` now (`discover` on the app and `record_read` on the work,
//! the workspace listing's clause); every other pump of that organization
//! shares the judgment through a once-cell on the announcement and spends no
//! storage transaction. A subscriber who cannot open the work gets nothing; a
//! subscriber who is no longer current (sign-out or an operator revocation
//! advanced the epoch, the entitlement lapsed) has its stream closed, as it
//! does at the `RECHECK` tick (`storage::Data::member_current`, an unfenced
//! read). An event of another organization is dropped before any lookup.
//!
//! Bounds (review M1): streams are one per session (a newer stream for the
//! same session **replaces** the earlier one and takes over its slot, so a
//! re-registration never adds a stream), at most `MAX_STREAMS_PER_MEMBER` per
//! member and `MAX_STREAMS` in the process (429 past them). The pump never
//! awaits the client: a delivery is `try_send` into the per-connection queue;
//! when the queue is full (a client that stopped reading) the hint is dropped
//! and the stream is marked lagged, and a `stream.lagged` frame follows as
//! soon as the queue has room so the client re-reads what it shows. The
//! lifetime, the replacement signal, the body's closure and the re-check are
//! therefore always polled, and the registration is released on every exit.
//! Heartbeat every `HEARTBEAT`; a stream ends after `STREAM_LIFETIME` so the
//! client re-resolves its session by reconnecting.
//!
//! APP-101 adds `app.adapted` (the composer's live transform saved an
//! adapted work; the document also names the standard work it came from as
//! `from_app_id`/`from_work_id`, judged on the adapted work) and `app.kept`
//! (the transform kept the standard view; judged on the standard work).
//!
//! Wire shape, one SSE message per event:
//! `event: <kind>` and `data: {"kind":"record.committed"|"app.shared"|
//! "app.placed","app_id":"<uuid>","work_id":"<uuid>","at":"<whole-second UTC>"}`;
//! the lag marker is `event: stream.lagged` and `data: {"kind":"stream.lagged",
//! "at":"<whole-second UTC>"}` (no ids: the client refetches what it shows).
//!
//! Recorded limitation (ADR-011 debt, APP-095): the hub is in-process, which
//! is exactly right for ADR-007 D7.18's single service task (stop-then-start);
//! a multi-task deployment (APP-061) needs a shared channel or a database
//! notification, or members connected to another task miss events. The
//! residual amplification is one storage read per event per organization with
//! a subscriber, plus one unfenced read per stream per `RECHECK` (ADR-011
//! §Consequences, debt item 6).
pub mod routes;
#[cfg(test)]
mod tests;

use serde_json::{json, Value};
use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, mpsc, Notify, OnceCell};

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
/// event is a hint to re-read, never the data itself) and is marked lagged.
const CHANNEL_CAPACITY: usize = 256;
/// Per-connection delivery queue toward the SSE body.
const SINK_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    RecordCommitted,
    AppShared,
    AppPlaced,
    /// APP-101: the composer saved an adapted work tailored from a standard
    /// one (`from`); the canvas reshapes the standard card in place.
    AppAdapted,
    /// APP-101: the transform kept the standard view (ids of the standard
    /// work); the canvas ends its "tailoring" state with a neutral note.
    AppKept,
    /// The stream dropped at least one hint (the client was not reading, or
    /// the subscriber lagged behind the hub); the client refetches.
    StreamLagged,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::RecordCommitted => "record.committed",
            Self::AppShared => "app.shared",
            Self::AppPlaced => "app.placed",
            Self::AppAdapted => "app.adapted",
            Self::AppKept => "app.kept",
            Self::StreamLagged => "stream.lagged",
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
    /// `app.adapted` only: the standard `(app, work)` the adapted work was
    /// tailored from (ids only, like the others).
    pub from: Option<(String, String)>,
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
            from: None,
            at: timestamp(unix_seconds()),
        }
    }
    /// The lag marker for one stream: no ids.
    fn lagged(organization: &str) -> Self {
        Self::now(Kind::StreamLagged, organization, "", "")
    }
    /// The wire document (the organization is the subscriber's own and is
    /// not repeated; the lag marker carries no ids).
    pub fn document(&self) -> Value {
        if self.kind == Kind::StreamLagged {
            return json!({"kind": self.kind.name(), "at": self.at});
        }
        let mut document = json!({
            "kind": self.kind.name(),
            "app_id": self.app,
            "work_id": self.work,
            "at": self.at,
        });
        if let Some((app, work)) = &self.from {
            document["from_app_id"] = json!(app);
            document["from_work_id"] = json!(work);
        }
        document
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

/// The organization-wide judgment of one announcement, type-erased so that
/// the hub (which knows no gate) can carry it: `None` when the judging read
/// failed (a store outage drops the hint for everyone and discloses nothing).
type Judged = OnceCell<Option<Arc<dyn Any + Send + Sync>>>;

/// What the hub broadcasts: the event and the once-per-event judgment cell
/// every pump of the event's organization shares (review M2).
#[derive(Clone)]
pub struct Announcement {
    pub event: Event,
    judged: Arc<Judged>,
}
impl Announcement {
    fn new(event: Event) -> Self {
        Self {
            event,
            judged: Arc::new(OnceCell::new()),
        }
    }
}

/// One session's slot: the member it counts against and the signal that
/// tells the stream holding it to close when a newer stream takes the slot.
struct Slot {
    member: (String, String),
    closed: Arc<Notify>,
}

#[derive(Default)]
struct Streams {
    total: usize,
    per_member: HashMap<(String, String), usize>,
    /// One slot per session: the newer registration takes the earlier one's
    /// slot (and tells it to close) instead of adding a stream.
    per_session: HashMap<String, Slot>,
}

/// Why a stream was not opened (429 with `Retry-After`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    MemberLimit,
    GlobalLimit,
}

pub struct Hub {
    sender: broadcast::Sender<Announcement>,
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
        let _ = self.sender.send(Announcement::new(event));
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
    /// APP-101: the adapted `(app, work)` tailored from `(from_app,
    /// from_work)`; judged, like every event, on the work it names (the
    /// adapted one).
    pub fn app_adapted(
        &self,
        organization: &str,
        app: &str,
        work: &str,
        from_app: &str,
        from_work: &str,
    ) {
        let mut event = Event::now(Kind::AppAdapted, organization, app, work);
        event.from = Some((from_app.to_owned(), from_work.to_owned()));
        self.publish(event);
    }
    /// APP-101: the transform of the standard `(app, work)` kept its defaults.
    pub fn app_kept(&self, organization: &str, app: &str, work: &str) {
        self.publish(Event::now(Kind::AppKept, organization, app, work));
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Announcement> {
        self.sender.subscribe()
    }
    /// Open streams now (tests and the bounds): one per session slot.
    pub fn open_streams(&self) -> usize {
        self.streams.lock().map(|s| s.total).unwrap_or(0)
    }
    /// The principals of `organization` holding a stream now, sorted and
    /// distinct: the population one event's judgment is made for (at most
    /// `MAX_STREAMS` by construction).
    pub fn subscribed_principals(&self, organization: &str) -> Vec<String> {
        let Ok(streams) = self.streams.lock() else {
            return Vec::new();
        };
        let mut principals: Vec<String> = streams
            .per_member
            .keys()
            .filter(|(org, _)| org == organization)
            .map(|(_, principal)| principal.clone())
            .collect();
        principals.sort_unstable();
        principals
    }
    /// Counts a stream for `(organization, principal)` under `session`. A
    /// stream already open for the same session is told to close and its
    /// slot passes to the new one: the member's and the process's counts do
    /// not move (review M1: a re-registration replaces, never adds), and the
    /// replaced registration's drop releases nothing. A new session of a
    /// member at `MAX_STREAMS_PER_MEMBER` is refused even while an earlier
    /// stream of that member is still winding down.
    pub fn register(
        &'static self,
        organization: &str,
        principal: &str,
        session: &str,
    ) -> Result<Registration, Refused> {
        let mut streams = self.streams.lock().map_err(|_| Refused::GlobalLimit)?;
        let member = (organization.to_owned(), principal.to_owned());
        let closed = Arc::new(Notify::new());
        if let Some(earlier) = streams.per_session.get_mut(session) {
            if earlier.member == member {
                // The slot changes hands; the counts are already right.
                let replaced = std::mem::replace(&mut earlier.closed, closed.clone());
                replaced.notify_one();
                return Ok(Registration {
                    hub: self,
                    session: session.to_owned(),
                    closed,
                });
            }
        }
        // A session key resolving to another member (it cannot: the key is a
        // digest of the session cookie, which names one member) closes the
        // earlier stream and releases its slot before a new one is judged.
        if let Some(earlier) = streams.per_session.remove(session) {
            Self::release(&mut streams, &earlier.member);
            earlier.closed.notify_one();
        }
        if streams.total >= MAX_STREAMS {
            return Err(Refused::GlobalLimit);
        }
        let held = streams.per_member.get(&member).copied().unwrap_or(0);
        if held >= MAX_STREAMS_PER_MEMBER {
            return Err(Refused::MemberLimit);
        }
        streams.total += 1;
        *streams.per_member.entry(member.clone()).or_insert(0) += 1;
        streams.per_session.insert(
            session.to_owned(),
            Slot {
                member,
                closed: closed.clone(),
            },
        );
        Ok(Registration {
            hub: self,
            session: session.to_owned(),
            closed,
        })
    }
    fn release(streams: &mut Streams, member: &(String, String)) {
        streams.total = streams.total.saturating_sub(1);
        if let Some(count) = streams.per_member.get_mut(member) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                streams.per_member.remove(member);
            }
        }
    }
    fn unregister(&self, registration: &Registration) {
        let Ok(mut streams) = self.streams.lock() else {
            return;
        };
        // Only the registration that holds the session's slot releases it; a
        // replaced registration already handed its slot to the newer stream.
        let holds = streams
            .per_session
            .get(&registration.session)
            .is_some_and(|slot| Arc::ptr_eq(&slot.closed, &registration.closed));
        if holds {
            if let Some(slot) = streams.per_session.remove(&registration.session) {
                Self::release(&mut streams, &slot.member);
            }
        }
    }
}

/// A counted open stream; dropping it releases its slot (unless a newer
/// stream of the same session took the slot over).
pub struct Registration {
    hub: &'static Hub,
    session: String,
    closed: Arc<Notify>,
}
impl Registration {
    /// Signalled when a newer stream of the same session replaced this one.
    pub fn closed(&self) -> Arc<Notify> {
        self.closed.clone()
    }
    /// The hub this registration counts on.
    pub fn hub(&self) -> &'static Hub {
        self.hub
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        self.hub.unregister(self);
    }
}

/// The delivery decision for one event and one subscriber.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Deliver,
    /// The member cannot open this work now; nothing is sent.
    Skip,
    /// The member is no longer current; the stream ends.
    Close,
}
/// What the pumps ask. `judge` runs **once per event** for the whole
/// organization (whichever pump reaches the announcement first runs it; the
/// others await the same cell) and names the population it was made for:
/// `subscribers` are the organization's subscribed principals at that moment.
/// `verdict` is this subscriber's reading of that judgment and performs no
/// I/O. `current` is the idle re-check. The production gate is
/// `routes::StoreGate` (storage); tests substitute scripted ones.
pub trait Gate: Send + Sync + 'static {
    /// The organization-wide judgment of `(app, work)` among `subscribers`.
    type Judgment: Send + Sync + 'static;
    /// `None` is a store outage: the hint is dropped for every subscriber.
    fn judge(
        &self,
        app: &str,
        work: &str,
        subscribers: &[String],
    ) -> impl Future<Output = Option<Self::Judgment>> + Send;
    fn verdict(&self, judgment: &Self::Judgment) -> Verdict;
    fn current(&self) -> impl Future<Output = bool> + Send;
}

/// The per-connection pump: announcements of the subscriber's organization
/// are judged (once per event, shared) and the delivered ones go to `sink`
/// (the SSE body) without ever waiting on it. Ends when the judgment or a
/// re-check finds the member no longer current, a newer stream replaced this
/// session's, the body was dropped (client gone), the lifetime passed or the
/// hub closed. The registration is released on every exit.
pub async fn pump<G: Gate>(
    gate: G,
    organization: String,
    mut receiver: broadcast::Receiver<Announcement>,
    registration: Registration,
    sink: mpsc::Sender<Event>,
    recheck: Duration,
    lifetime: Duration,
) {
    let closed = registration.closed();
    let hub = registration.hub();
    let start = tokio::time::Instant::now();
    let lifetime = tokio::time::sleep_until(start + lifetime);
    tokio::pin!(lifetime);
    let mut ticker = tokio::time::interval_at(start + recheck, recheck);
    // A stall is not a burst: one re-check after a pause, then the cadence.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // At least one hint was dropped since the client last had room: the next
    // frame with room is the lag marker, so the client refetches.
    let mut lagged = false;
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
            permit = sink.reserve(), if lagged => match permit {
                Ok(permit) => {
                    permit.send(Event::lagged(&organization));
                    lagged = false;
                }
                Err(_) => break,
            },
            received = receiver.recv() => match received {
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => lagged = true,
                Ok(announcement) => {
                    let event = &announcement.event;
                    if event.organization != organization {
                        continue;
                    }
                    let judged = announcement.judged.get_or_init(|| async {
                        let subscribers = hub.subscribed_principals(&organization);
                        gate.judge(&event.app, &event.work, &subscribers)
                            .await
                            .map(|judgment| Arc::new(judgment) as Arc<dyn Any + Send + Sync>)
                    }).await;
                    // A failed judgment, or one made by a gate of another
                    // type (tests only), delivers nothing and closes nothing.
                    let verdict = judged
                        .as_ref()
                        .and_then(|judgment| judgment.downcast_ref::<G::Judgment>())
                        .map_or(Verdict::Skip, |judgment| gate.verdict(judgment));
                    match verdict {
                        Verdict::Deliver => match sink.try_send(event.clone()) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => lagged = true,
                            Err(mpsc::error::TrySendError::Closed(_)) => break,
                        },
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
