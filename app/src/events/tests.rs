//! Hub, bounds, delivery gate and route refusals without a database. The
//! storage-backed gate (`StoreGate`) and the HTTP stream end to end are
//! exercised by `storage::sharing_qualification::native_sharing_campaign`.
//! The review's two probes (independent review of `b704274`, M1) are kept
//! here as the tests that now pass the other way.
use super::*;
use crate::{auth::Auth, http};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use std::sync::atomic::{AtomicBool, Ordering};
use tower::ServiceExt as _;

const ORG: &str = "00000000-0000-4000-8000-000000000001";
const OTHER: &str = "00000000-0000-4000-8000-000000000002";
const APP: &str = "00000000-0000-4000-8000-000000000065";
const OPEN: &str = "00000000-0000-4000-8000-000000000069";
const CLOSED_WORK: &str = "00000000-0000-4000-8000-00000000006a";
const PRINCIPAL: &str = "00000000-0000-4000-8000-000000000066";

fn fresh_hub() -> &'static Hub {
    Box::leak(Box::new(Hub::new()))
}

#[test]
fn timestamps_are_whole_second_utc() {
    assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
    assert_eq!(timestamp(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(timestamp(1_761_782_400), "2025-10-30T00:00:00Z");
    assert_eq!(timestamp(4_102_444_799), "2099-12-31T23:59:59Z");
}

#[test]
fn documents_carry_ids_and_kind_only() {
    let event = Event {
        kind: Kind::RecordCommitted,
        organization: ORG.into(),
        app: APP.into(),
        work: OPEN.into(),
        at: "2026-10-30T12:00:00Z".into(),
    };
    assert_eq!(
        event.document(),
        serde_json::json!({"kind": "record.committed", "app_id": APP, "work_id": OPEN, "at": "2026-10-30T12:00:00Z"})
    );
    assert_eq!(Kind::AppShared.name(), "app.shared");
    assert_eq!(Kind::AppPlaced.name(), "app.placed");
    assert_eq!(Kind::StreamLagged.name(), "stream.lagged");
    assert!(!event.document().to_string().contains(ORG));
    // The lag marker names no app or work: the client refetches what it shows.
    let lagged = Event::lagged(ORG);
    let document = lagged.document();
    assert_eq!(
        document.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["at", "kind"]
    );
    assert_eq!(document["kind"], "stream.lagged");
}

#[test]
fn streams_are_bounded_per_member_and_one_per_session() {
    let hub = fresh_hub();
    let mut held = Vec::new();
    for i in 0..MAX_STREAMS_PER_MEMBER {
        held.push(hub.register(ORG, PRINCIPAL, &format!("s{i}")).unwrap());
    }
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER);
    assert_eq!(
        hub.register(ORG, PRINCIPAL, "s-extra").err(),
        Some(Refused::MemberLimit)
    );
    // Another member is not affected by this member's bound.
    let other = hub.register(ORG, OTHER, "o1").unwrap();
    assert_eq!(
        hub.subscribed_principals(ORG),
        vec![OTHER.to_owned(), PRINCIPAL.to_owned()]
    );
    assert!(hub.subscribed_principals(OTHER).is_empty());
    // The same session again takes over its earlier stream's slot (which is
    // told to close): neither the member's nor the process's count moves.
    let replaced = held.remove(0);
    let replacing = hub.register(ORG, PRINCIPAL, "s0").unwrap();
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER + 1);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), replaced.closed().notified())
            .await
            .expect("the earlier stream of the session is told to close");
    });
    // Dropping the replaced registration releases nothing: the slot is the
    // newer stream's now.
    drop(replaced);
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER + 1);
    assert!(hub.streams.lock().unwrap().per_session.contains_key("s0"));
    drop(replacing);
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER);
    assert!(!hub.streams.lock().unwrap().per_session.contains_key("s0"));
    drop(held);
    drop(other);
    assert_eq!(hub.open_streams(), 0);
    assert!(hub.streams.lock().unwrap().per_member.is_empty());
}

#[test]
fn the_global_bound_refuses_the_next_stream() {
    let hub = fresh_hub();
    let mut held = Vec::new();
    for i in 0..MAX_STREAMS {
        let principal = format!("{:0>36}", i); // distinct members, under the per-member bound
        held.push(hub.register(ORG, &principal, &format!("g{i}")).unwrap());
    }
    assert_eq!(
        hub.register(ORG, PRINCIPAL, "g-extra").err(),
        Some(Refused::GlobalLimit)
    );
    drop(held);
    assert_eq!(hub.open_streams(), 0);
}

/// Review probe 2, inverted: the same session may re-register as often as it
/// likes and never holds more than one stream, so one member cannot climb
/// past its bound (let alone to `MAX_STREAMS`) by reconnecting on one cookie.
#[test]
fn re_registration_of_the_same_session_never_adds_a_stream() {
    let hub = fresh_hub();
    let mut held = Vec::new();
    for _ in 0..MAX_STREAMS {
        held.push(hub.register(ORG, PRINCIPAL, "same-session").unwrap());
    }
    assert_eq!(hub.open_streams(), 1);
    assert_eq!(
        hub.streams.lock().unwrap().per_member[&(ORG.to_owned(), PRINCIPAL.to_owned())],
        1
    );
    // Everyone else still gets in; the member itself has three more sessions.
    let someone = hub.register(ORG, OTHER, "other").unwrap();
    let mut more = Vec::new();
    for i in 1..MAX_STREAMS_PER_MEMBER {
        more.push(
            hub.register(ORG, PRINCIPAL, &format!("session-{i}"))
                .unwrap(),
        );
    }
    assert_eq!(
        hub.register(ORG, PRINCIPAL, "one-too-many").err(),
        Some(Refused::MemberLimit)
    );
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER + 1);
    // Only the last registration of the session holds the slot; dropping the
    // replaced ones releases nothing, dropping the holder releases the slot.
    let holder = held.pop().unwrap();
    drop(held);
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER + 1);
    drop(holder);
    assert_eq!(hub.open_streams(), MAX_STREAMS_PER_MEMBER);
    drop(more);
    drop(someone);
    assert_eq!(hub.open_streams(), 0);
}

/// A scripted gate: `OPEN` is openable, anything else is not; `current`
/// flips to false to simulate a sign-out or revocation. Its judgment is the
/// work judged; `judged` logs every organization-wide judgment (one per
/// event, however many pumps share it) and `populations` the subscribed
/// principals each judgment was made for.
struct Scripted {
    current: Arc<AtomicBool>,
    judged: Arc<Mutex<Vec<String>>>,
    populations: Arc<Mutex<Vec<Vec<String>>>>,
}
impl Scripted {
    fn new(current: &Arc<AtomicBool>, judged: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            current: current.clone(),
            judged: judged.clone(),
            populations: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
impl Gate for Scripted {
    type Judgment = String;
    async fn judge(&self, _app: &str, work: &str, subscribers: &[String]) -> Option<String> {
        self.judged.lock().unwrap().push(work.to_owned());
        self.populations.lock().unwrap().push(subscribers.to_vec());
        Some(work.to_owned())
    }
    fn verdict(&self, work: &String) -> Verdict {
        if !self.current.load(Ordering::SeqCst) {
            return Verdict::Close;
        }
        if work == OPEN {
            Verdict::Deliver
        } else {
            Verdict::Skip
        }
    }
    async fn current(&self) -> bool {
        self.current.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn the_pump_delivers_only_openable_works_of_the_own_organization_and_closes_when_the_member_is_not_current(
) {
    let hub = fresh_hub();
    let current = Arc::new(AtomicBool::new(true));
    let judged = Arc::new(Mutex::new(Vec::new()));
    let registration = hub.register(ORG, PRINCIPAL, "p1").unwrap();
    let (tx, mut rx) = sink();
    let receiver = hub.subscribe();
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        receiver,
        registration,
        tx,
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    ));
    hub.record_committed(ORG, APP, OPEN);
    let delivered = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (delivered.kind, delivered.work.as_str()),
        (Kind::RecordCommitted, OPEN)
    );
    // Another organization's event is dropped before any lookup; a work the
    // member cannot open is judged and dropped.
    hub.app_placed(OTHER, APP, OPEN);
    hub.app_shared(ORG, APP, CLOSED_WORK);
    hub.app_shared(ORG, APP, OPEN);
    let next = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((next.kind, next.work.as_str()), (Kind::AppShared, OPEN));
    assert_eq!(*judged.lock().unwrap(), vec![OPEN, CLOSED_WORK, OPEN]);
    assert_eq!(hub.open_streams(), 1);
    // The member signs out: the next event closes the stream and releases
    // the slot; the body ends.
    current.store(false, Ordering::SeqCst);
    hub.record_committed(ORG, APP, CLOSED_WORK);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("pump ends")
        .unwrap();
    assert!(rx.recv().await.is_none());
    assert_eq!(hub.open_streams(), 0);
}

/// Review M2: one committed record with N subscribers of the organization
/// costs one judgment (one storage read in production), made for exactly the
/// subscribed principals; every subscriber reads its verdict from it.
#[tokio::test]
async fn one_event_with_n_subscribers_is_judged_once() {
    const N: usize = 8;
    let hub = fresh_hub();
    let current = Arc::new(AtomicBool::new(true));
    let judged = Arc::new(Mutex::new(Vec::new()));
    let populations = Arc::new(Mutex::new(Vec::new()));
    let mut bodies = Vec::new();
    let mut tasks = Vec::new();
    let mut principals: Vec<String> = (0..N).map(|i| format!("{i:0>36}")).collect();
    for principal in &principals {
        let registration = hub
            .register(ORG, principal, &format!("s-{principal}"))
            .unwrap();
        let (tx, rx) = sink();
        bodies.push(rx);
        tasks.push(tokio::spawn(pump(
            Scripted {
                current: current.clone(),
                judged: judged.clone(),
                populations: populations.clone(),
            },
            ORG.into(),
            hub.subscribe(),
            registration,
            tx,
            Duration::from_secs(3600),
            Duration::from_secs(3600),
        )));
    }
    // One stream of another organization never judges this organization's event.
    let foreign = hub.register(OTHER, PRINCIPAL, "foreign").unwrap();
    let (tx, foreign_rx) = sink();
    let foreign_judged = Arc::new(Mutex::new(Vec::new()));
    tasks.push(tokio::spawn(pump(
        Scripted::new(&current, &foreign_judged),
        OTHER.into(),
        hub.subscribe(),
        foreign,
        tx,
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    )));
    hub.record_committed(ORG, APP, OPEN);
    for rx in &mut bodies {
        let delivered = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(delivered.work, OPEN);
    }
    assert_eq!(*judged.lock().unwrap(), vec![OPEN]);
    principals.sort_unstable();
    assert_eq!(*populations.lock().unwrap(), vec![principals]);
    assert!(foreign_judged.lock().unwrap().is_empty());
    // A second event is one more judgment, not N more.
    hub.app_shared(ORG, APP, CLOSED_WORK);
    hub.app_placed(ORG, APP, OPEN);
    for rx in &mut bodies {
        let delivered = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (delivered.kind, delivered.work.as_str()),
            (Kind::AppPlaced, OPEN)
        );
    }
    assert_eq!(*judged.lock().unwrap(), vec![OPEN, CLOSED_WORK, OPEN]);
    drop(bodies);
    drop(foreign_rx);
    for task in tasks {
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("pumps end when their bodies are dropped")
            .unwrap();
    }
    assert_eq!(hub.open_streams(), 0);
}

#[tokio::test]
async fn the_pump_ends_on_the_recheck_when_the_member_is_not_current_and_when_replaced() {
    let hub = fresh_hub();
    let current = Arc::new(AtomicBool::new(false));
    let judged = Arc::new(Mutex::new(Vec::new()));
    let registration = hub.register(ORG, PRINCIPAL, "r1").unwrap();
    let (tx, _rx) = sink();
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        hub.subscribe(),
        registration,
        tx,
        Duration::from_millis(50),
        Duration::from_secs(3600),
    ));
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the re-check ends an idle stream of a member who is no longer current")
        .unwrap();
    assert_eq!(hub.open_streams(), 0);
    // A newer stream of the same session closes the earlier one.
    current.store(true, Ordering::SeqCst);
    let first = hub.register(ORG, PRINCIPAL, "same").unwrap();
    let (tx, _rx) = sink();
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        hub.subscribe(),
        first,
        tx,
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    ));
    tokio::task::yield_now().await;
    let second = hub.register(ORG, PRINCIPAL, "same").unwrap();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("replaced stream ends")
        .unwrap();
    assert_eq!(hub.open_streams(), 1);
    drop(second);
    // The lifetime ends a stream too.
    let late = hub.register(ORG, PRINCIPAL, "late").unwrap();
    let (tx, _rx) = sink();
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        hub.subscribe(),
        late,
        tx,
        Duration::from_secs(3600),
        Duration::from_millis(20),
    ));
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("lifetime ends the stream")
        .unwrap();
    assert_eq!(hub.open_streams(), 0);
}

/// Review probe 1, inverted: with the per-connection queue full (a client
/// that stopped reading) the pump still polls its lifetime, its re-check and
/// its replacement, so a stalled consumer pins nothing and the replaced
/// registration releases its slot to the newer stream.
#[tokio::test]
async fn a_stalled_consumer_does_not_pin_the_pump() {
    let hub = fresh_hub();
    let current = Arc::new(AtomicBool::new(true));
    let judged = Arc::new(Mutex::new(Vec::new()));
    let registration = hub.register(ORG, PRINCIPAL, "stall").unwrap();
    let (tx, rx) = sink(); // held but never read: a stalled client
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        hub.subscribe(),
        registration,
        tx,
        Duration::from_millis(50),  // re-check
        Duration::from_millis(300), // lifetime
    ));
    // More deliverable events than the queue holds (SINK_CAPACITY = 32).
    for _ in 0..40 {
        hub.record_committed(ORG, APP, OPEN);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    // The member signs out: the re-check closes the stream...
    current.store(false, Ordering::SeqCst);
    // ...and a newer stream of the same session replaces this one anyway.
    let replacement = hub.register(ORG, PRINCIPAL, "stall").unwrap();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the pump ends although its client reads nothing")
        .unwrap();
    assert_eq!(hub.open_streams(), 1);
    drop(replacement);
    assert_eq!(hub.open_streams(), 0);
    drop(rx);
}

/// A client that falls behind loses hints, never the stream: the queue's
/// worth of events is delivered, then one `stream.lagged` marker as soon as
/// there is room, and later events flow again.
#[tokio::test]
async fn a_full_queue_drops_hints_and_marks_the_stream_lagged() {
    let hub = fresh_hub();
    let current = Arc::new(AtomicBool::new(true));
    let judged = Arc::new(Mutex::new(Vec::new()));
    let registration = hub.register(ORG, PRINCIPAL, "slow").unwrap();
    let (tx, mut rx) = sink();
    let task = tokio::spawn(pump(
        Scripted::new(&current, &judged),
        ORG.into(),
        hub.subscribe(),
        registration,
        tx,
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    ));
    for _ in 0..(SINK_CAPACITY + 8) {
        hub.record_committed(ORG, APP, OPEN);
    }
    // Every event was judged (the pump never parked); the queue holds its
    // capacity's worth, the rest were dropped.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(judged.lock().unwrap().len(), SINK_CAPACITY + 8);
    let mut kinds = Vec::new();
    for _ in 0..(SINK_CAPACITY + 1) {
        let frame = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        kinds.push(frame.kind);
    }
    assert!(kinds[..SINK_CAPACITY]
        .iter()
        .all(|kind| *kind == Kind::RecordCommitted));
    assert_eq!(kinds[SINK_CAPACITY], Kind::StreamLagged);
    // The stream is alive and delivers again.
    hub.app_placed(ORG, APP, OPEN);
    let next = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.kind, Kind::AppPlaced);
    assert_eq!(hub.open_streams(), 1);
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the pump ends when the body is dropped")
        .unwrap();
    assert_eq!(hub.open_streams(), 0);
}

#[test]
fn session_keys_are_digests_not_cookies() {
    let key = routes::session_key_for_tests("cookie-a");
    assert_eq!(key.len(), 64);
    assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(key, routes::session_key_for_tests("cookie-b"));
    assert!(!key.contains("cookie"));
}

/// No session (and no store) is the uniform 404, with no stream opened.
#[tokio::test]
async fn the_stream_without_a_session_is_the_uniform_404() {
    let auth = Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
    let app = http::uniform(routes::router(auth));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(body, &b"{\"code\":\"not_found\",\"status\":404}"[..]);
    assert_eq!(hub().open_streams(), 0);
}
