//! `GET /v1/events` (module notes in `events`). Refusal order: a live
//! workspace session (else the uniform 404, like every protected route), the
//! stream bounds (429 `too_many_requests` with `Retry-After`), then the
//! stream. The response is `text/event-stream` with `no-store` and the
//! security headers; a comment heartbeat every `HEARTBEAT`. The body never
//! blocks the pump: frames the client has no room for are dropped and a
//! `stream.lagged` frame follows (review M1).
use super::{hub, pump, sink, Event, Gate, Refused, Verdict, HEARTBEAT, RECHECK, STREAM_LIFETIME};
use crate::{
    auth::Auth,
    http,
    sharing::session,
    storage::{self, Stores, TrustedRequest},
};
use axum::{
    extract::{Request, State},
    response::{
        sse::{self, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Router,
};
use sha2::{Digest as _, Sha256};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc;

/// D7.9 `Retry-After` when the stream bounds refuse.
const RETRY_AFTER_SECONDS: u64 = 2;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/v1/events", get(events))
        .with_state(auth)
}

/// The one-stream-per-session key: a domain-separated digest of the cookie,
/// never the cookie itself.
fn session_key(cookie: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"forma:event-stream:v1\x00");
    hasher.update(cookie.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The production gate: storage's re-authorization at delivery, once per
/// event for the organization (`Data::openable_principals`, review M2) and
/// read for this subscriber without I/O (`Openable::standing`).
pub struct StoreGate {
    stores: Arc<Stores>,
    request: TrustedRequest,
}
impl Gate for StoreGate {
    type Judgment = storage::Openable;
    async fn judge(&self, app: &str, work: &str, subscribers: &[String]) -> Option<Self::Judgment> {
        // A store outage (or a refused read) discloses nothing: the hint is
        // dropped for everyone and no stream is closed on it.
        self.stores
            .data
            .openable_principals(&self.request, app, work, subscribers)
            .await
            .ok()
    }
    fn verdict(&self, judgment: &Self::Judgment) -> Verdict {
        match judgment.standing(&self.request) {
            storage::Standing::Open => Verdict::Deliver,
            storage::Standing::Closed => Verdict::Skip,
            storage::Standing::NotCurrent => Verdict::Close,
        }
    }
    async fn current(&self) -> bool {
        !matches!(
            self.stores.data.member_current(&self.request).await,
            Err(storage::Error::Denied)
        )
    }
}

/// The SSE body: delivered events as `event: <kind>` / `data: <document>`.
pub struct EventStream {
    receiver: mpsc::Receiver<Event>,
}
impl futures_core::Stream for EventStream {
    type Item = Result<sse::Event, Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx).map(|next| {
            next.map(|event| {
                Ok(sse::Event::default()
                    .event(event.kind.name())
                    .data(event.document().to_string()))
            })
        })
    }
}

async fn events(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let Some((cookie, trusted)) = session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = auth.stores.clone() else {
        return http::not_found();
    };
    let organization = trusted.organization().to_owned();
    let registration =
        match hub().register(&organization, trusted.principal(), &session_key(&cookie)) {
            Ok(registration) => registration,
            Err(Refused::MemberLimit | Refused::GlobalLimit) => {
                return http::too_many_requests(RETRY_AFTER_SECONDS)
            }
        };
    let (tx, rx) = sink();
    tokio::spawn(pump(
        StoreGate {
            stores,
            request: trusted,
        },
        organization,
        hub().subscribe(),
        registration,
        tx,
        RECHECK,
        STREAM_LIFETIME,
    ));
    let mut response = Sse::new(EventStream { receiver: rx })
        .keep_alive(KeepAlive::new().interval(HEARTBEAT).text("heartbeat"))
        .into_response();
    http::secure(response.headers_mut());
    response
}

#[cfg(test)]
pub(crate) fn session_key_for_tests(cookie: &str) -> String {
    session_key(cookie)
}
