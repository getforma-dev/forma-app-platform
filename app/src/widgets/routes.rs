//! `GET /v1/widgets/weather` and `POST /v1/widgets/customize` (module notes
//! in `widgets`).
//!
//! Both answer only a live workspace session (the uniform 404 otherwise) and
//! run under the long deadline (each may wait on an outside service).
//!
//! `GET /v1/widgets/weather?location=<place>&units=imperial|metric`: the
//! card's document (`weather::document`): `location`, `temperature`,
//! `apparent`, `condition`, `icon`, `wind`, `humidity`, `precipitation`,
//! `units`, `provider_label`, `source` (`live` or `fixture`). Refusals: 404,
//! 422 `invalid_request` (a location outside `state::location`, units outside
//! the set, any other parameter), 422 `location_not_found`, 429.
//!
//! `POST /v1/widgets/customize`, same-origin, body of at most
//! `CUSTOMIZE_BODY_LIMIT`: `{"widget": "weather"|"calculator"|"clock",
//! "state": <the widget's closed state>, "text": "<1..300 characters>"}`.
//! Answers `{"widget", "state", "source": "rehearsed"|"model", "changed",
//! "duration_ms"}`. Refusals: 404, 403 `cross_site`, 413, 422
//! `invalid_request`, 422 `could_not_customize` (the model answered outside
//! the closed state; the widget keeps its state), 429, 503
//! `composer_unavailable`. The log carries identities, the widget, the
//! source, whether the state changed and durations; never the sentence, the
//! location or a state value.
use super::{
    customize::{self, Customizer, Failure},
    state::{self, Widget},
    weather::{self, FetchError, OpenMeteo},
};
use crate::{
    auth::{cookies, csrf, Auth},
    demo,
    http::{self, limits},
    storage::{self, Lookup, TrustedRequest},
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};

pub const CUSTOMIZE_BODY_LIMIT: usize = 4096;
/// Per member: a weather card refreshes on placement and on demand.
pub const WEATHER_MEMBER: &[limits::Window] = &[limits::per(240, 3600)];
/// Per member: restyles (several per rehearsal).
pub const CUSTOMIZE_MEMBER: &[limits::Window] = &[limits::per(120, 3600)];

pub struct Widgets {
    auth: Arc<Auth>,
    customizer: Customizer,
    weather: OpenMeteo,
}
impl Widgets {
    pub fn new(auth: Arc<Auth>, customizer: Customizer, weather: OpenMeteo) -> Self {
        Self {
            auth,
            customizer,
            weather,
        }
    }
}

/// Both routes (the binary mounts this router under the long deadline at
/// each of `WEATHER_PATH` and `CUSTOMIZE_PATH`).
pub fn router(state: Arc<Widgets>) -> Router {
    Router::new()
        .route(super::WEATHER_PATH, get(weather_route))
        .route(super::CUSTOMIZE_PATH, post(customize_route))
        .with_state(state)
}

async fn member(auth: &Auth, headers: &HeaderMap) -> Result<TrustedRequest, Response> {
    let cookie = cookies::read(headers, cookies::SESSION).ok_or_else(http::not_found)?;
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => Ok(request),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            Err(http::not_found())
        }
        Ok(Lookup::Absent) | Err(storage::Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

fn document(value: &Value) -> Response {
    let Ok(bytes) = crate::admission::encode(value) else {
        return http::unavailable();
    };
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The weather query: exactly `location` and `units`, each once.
pub(super) fn weather_query(query: Option<&str>) -> Option<(String, bool)> {
    let mut location = None;
    let mut units = None;
    for (key, value) in url::form_urlencoded::parse(query?.as_bytes()) {
        let slot = match key.as_ref() {
            "location" => &mut location,
            "units" => &mut units,
            _ => return None,
        };
        if slot.replace(value.into_owned()).is_some() {
            return None;
        }
    }
    let location = state::location(&location?)?;
    let metric = match units.as_deref().unwrap_or("imperial") {
        "metric" => true,
        "imperial" => false,
        _ => return None,
    };
    Some((location, metric))
}

/// `GET /v1/widgets/weather`.
async fn weather_route(State(state): State<Arc<Widgets>>, request: Request) -> Response {
    let member = match member(&state.auth, request.headers()).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let Some((location, metric)) = weather_query(request.uri().query()) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    };
    let key = format!(
        "widgets.weather:{}:{}",
        member.organization(),
        member.principal()
    );
    if let Err(retry_after) = state.auth.limits.admit(&[(&key, WEATHER_MEMBER)]) {
        return http::too_many_requests(retry_after);
    }
    let demo = demo::is_demo_org(member.organization());
    let (answer, outcome) = if demo {
        (weather::fixture(&location, metric), "fixture_demo")
    } else {
        match state.weather.current(&location, metric).await {
            Ok(answer) => (answer, "live"),
            Err(FetchError::NotFound) => {
                tracing::info!(
                    event = "widgets.weather",
                    outcome = "location_not_found",
                    organization_id = member.organization(),
                    principal_id = member.principal(),
                    duration_ms = elapsed_ms(started),
                    "weather refused"
                );
                return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "location_not_found");
            }
            Err(FetchError::Unavailable) => {
                (weather::fixture(&location, metric), "fixture_unreachable")
            }
        }
    };
    tracing::info!(
        event = "widgets.weather",
        outcome,
        organization_id = member.organization(),
        principal_id = member.principal(),
        duration_ms = elapsed_ms(started),
        "weather answered"
    );
    document(&answer)
}

/// The customize body: exactly `widget`, `state` and `text`.
pub(super) fn customize_body(bytes: &[u8]) -> Option<(Widget, Value, String)> {
    let value: Value = crate::admission::decode(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 3 {
        return None;
    }
    let widget = Widget::parse(object.get("widget")?.as_str()?)?;
    let current = state::validate(widget, object.get("state")?)?;
    let text = customize::text(object.get("text")?.as_str()?)?.to_owned();
    Some((widget, current, text))
}

/// `POST /v1/widgets/customize`.
async fn customize_route(State(state): State<Arc<Widgets>>, request: Request) -> Response {
    let (parts, body_stream) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let Ok(bytes) = to_bytes(body_stream, CUSTOMIZE_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let member = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let Some((widget, current, text)) =
        customize_body(&bytes).filter(|_| parts.uri.query().is_none())
    else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    };
    let key = format!(
        "widgets.customize:{}:{}",
        member.organization(),
        member.principal()
    );
    if let Err(retry_after) = state.auth.limits.admit(&[(&key, CUSTOMIZE_MEMBER)]) {
        return http::too_many_requests(retry_after);
    }
    let demo = demo::is_demo_org(member.organization());
    let log_refused = |code: &str| {
        tracing::info!(
            event = "widgets.customize_refused",
            code,
            widget = widget.as_str(),
            organization_id = member.organization(),
            principal_id = member.principal(),
            duration_ms = elapsed_ms(started),
            "widget customization refused"
        );
    };
    match state
        .customizer
        .customize(widget, &current, &text, demo)
        .await
    {
        Ok(outcome) => {
            let changed = outcome.state != current;
            tracing::info!(
                event = "widgets.customized",
                widget = widget.as_str(),
                source = outcome.source.as_str(),
                changed,
                organization_id = member.organization(),
                principal_id = member.principal(),
                model_ms = outcome.duration_ms,
                duration_ms = elapsed_ms(started),
                "widget customized"
            );
            document(&json!({
                "widget": widget.as_str(),
                "state": outcome.state,
                "source": outcome.source.as_str(),
                "changed": changed,
                "duration_ms": outcome.duration_ms,
            }))
        }
        Err(Failure::Unavailable) => {
            log_refused("composer_unavailable");
            http::problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable")
        }
        Err(Failure::Refused(code)) => {
            log_refused(code);
            http::problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_customize")
        }
    }
}
