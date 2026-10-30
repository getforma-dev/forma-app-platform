//! The command route, the outcome lookup and the record page (module notes
//! in `commands`). Refusal order for the `POST`: fetch metadata (403), the
//! session (uniform 404), the bounded body (413), the synchronizer token of a
//! form (403), the key and the shape (400), and only then storage.
use super::{payload, Notice, RecordPage};
use crate::{
    auth::{capability, cookies, csrf, Auth},
    http,
    storage::{
        self, idempotency_key_shaped, CommandRequest, CommandResult, Lookup, TrustedRequest,
        COMMAND_HANDLER_DEADLINE,
    },
};
use axum::{
    body::to_bytes,
    extract::{Path, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Map, Value};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

/// ADR-007 D7.14: the JSON command route's body limit (the maximum canonical
/// command: 32 values of up to 2,000 characters). The record page's form is
/// bounded the same way.
pub const COMMAND_BODY_LIMIT: usize = 262_144;
/// D7.9: `Retry-After` on `in_flight` and `busy`.
const RETRY_AFTER_SECONDS: u64 = 2;
/// Storage finishes before the 5 s handler deadline middleware answers 503.
const STORAGE_MARGIN: Duration = Duration::from_millis(400);

pub struct Commands {
    pub auth: Arc<Auth>,
    pub page: Arc<dyn RecordPage>,
}

pub fn router(state: Arc<Commands>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/commands/{command}", post(submit))
        .route(
            "/v1/work/{app}/{work}/commands/{command}/outcomes/{key}",
            get(outcome),
        )
        .route(
            "/work/{app}/{work}/records/{collection}/{record}",
            get(record_page),
        )
        .with_state(state)
}

/// The live workspace session behind the request, or nothing (uniform 404).
async fn session(auth: &Auth, headers: &HeaderMap) -> Option<(String, TrustedRequest)> {
    let cookie = cookies::read(headers, cookies::SESSION)?;
    let stores = auth.stores.as_ref()?;
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => Some((cookie, request)),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            None
        }
        Ok(Lookup::Absent) | Err(_) => None,
    }
}

fn retry_after(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&RETRY_AFTER_SECONDS.to_string()).expect("decimal"),
    );
    response
}
/// A forma-json-v1 document as `application/json` with the security headers.
fn document(status: StatusCode, value: &Value) -> Response {
    let Ok(bytes) = crate::admission::encode(value) else {
        return http::unavailable();
    };
    let mut response = (status, bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

/// How the request arrived, which decides the answer's shape.
enum Mode {
    Json,
    /// The record page's form: the collection for the redirect back.
    Form {
        collection: String,
    },
}

/// The route's answer for a storage result (D7.9 HTTP rows).
fn answer(
    mode: &Mode,
    app: &str,
    work: &str,
    record: &str,
    result: Result<CommandResult, storage::Error>,
) -> Response {
    match mode {
        Mode::Json => match result {
            Ok(CommandResult::Committed(outcome)) | Ok(CommandResult::Replayed(outcome)) => {
                document(StatusCode::OK, &outcome.document())
            }
            Ok(CommandResult::Conflict(view)) => http::problem_with(
                StatusCode::CONFLICT,
                "conflict",
                json!({"recovery_view": view.document()}),
            ),
            Ok(CommandResult::Stale(current)) => http::problem_with(
                StatusCode::PRECONDITION_FAILED,
                "stale",
                json!({"current_revision": {
                    "publication_id": current.publication,
                    "definition_revision_id": current.revision,
                    "definition_digest": current.definition_digest,
                    "work_version": current.work_version,
                }}),
            ),
            Ok(CommandResult::InFlight) => {
                retry_after(http::problem(StatusCode::CONFLICT, "in_flight"))
            }
            Ok(CommandResult::Busy) => retry_after(http::problem(StatusCode::CONFLICT, "busy")),
            Err(storage::Error::Denied) => http::not_found(),
            Err(storage::Error::UnknownOutcome) => {
                http::problem(StatusCode::SERVICE_UNAVAILABLE, "unknown_outcome")
            }
            Err(_) => http::unavailable(),
        },
        Mode::Form { collection } => {
            let notice = match result {
                Ok(CommandResult::Committed(_)) => Notice::Committed,
                Ok(CommandResult::Replayed(_)) => Notice::Replayed,
                Ok(CommandResult::Conflict(_)) => Notice::Conflict,
                Ok(CommandResult::Stale(_)) => Notice::Stale,
                Ok(CommandResult::InFlight) => Notice::InFlight,
                Ok(CommandResult::Busy) => Notice::Busy,
                Err(storage::Error::Denied) => return http::not_found(),
                Err(storage::Error::UnknownOutcome) => Notice::Unknown,
                Err(_) => return http::unavailable(),
            };
            http::see_other(&format!(
                "/work/{app}/{work}/records/{collection}/{record}?notice={}",
                notice.name()
            ))
        }
    }
}

fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64)
}
fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// The JSON body: the contract's `command_request`, whose app, work and
/// command must be the path's; the key comes from the header (or the body).
fn from_json(
    bytes: &[u8],
    app: &str,
    work: &str,
    command: &str,
    header_key: Option<&str>,
) -> Option<CommandRequest> {
    let body: Value = serde_json::from_slice(bytes).ok()?;
    let object = body.as_object()?;
    let kind_ok = object.get("kind").and_then(Value::as_str) == Some("command_request")
        && integer(object.get("schema_version")) == Some(1);
    if !kind_ok
        || string(object.get("app_id"))? != app
        || string(object.get("work_id"))? != work
        || string(object.get("command_id"))? != command
    {
        return None;
    }
    let key = match (header_key, string(object.get("idempotency_key"))) {
        (Some(header), Some(body)) if header != body => return None,
        (Some(header), _) => header.to_owned(),
        (None, Some(body)) => body,
        (None, None) => return None,
    };
    let allowed = [
        "kind",
        "schema_version",
        "app_id",
        "work_id",
        "publication_id",
        "command_id",
        "schema_id",
        "schema_version_expected",
        "record_id",
        "expected_record_version",
        "expected_work_version",
        "values",
        "idempotency_key",
    ];
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return None;
    }
    Some(CommandRequest {
        app: app.to_owned(),
        work: work.to_owned(),
        publication: string(object.get("publication_id"))?,
        command: command.to_owned(),
        schema: string(object.get("schema_id"))?,
        schema_version_expected: integer(object.get("schema_version_expected"))?,
        record: string(object.get("record_id"))?,
        expected_record_version: integer(object.get("expected_record_version"))?,
        expected_work_version: integer(object.get("expected_work_version"))?,
        values: object.get("values")?.clone(),
        idempotency_key: key,
    })
}

/// A form text under the published field's `value_type` (echoed by the form
/// as `spec.<field>`); an empty text is the null value. Storage judges the
/// result against the published schema, so the echo confers nothing.
pub(crate) fn typed_value(spec: &Value, raw: &str) -> Option<Value> {
    if raw.is_empty() {
        return Some(json!({"type": "null"}));
    }
    let kind = spec.get("type").and_then(Value::as_str)?;
    Some(match kind {
        "text" | "enum" | "date" | "timestamp" => json!({"type": kind, "value": raw}),
        "boolean" => json!({"type": "boolean", "value": matches!(raw, "true" | "on" | "1")}),
        "integer" => json!({"type": "integer", "value": raw, "unit": spec.get("unit")?}),
        "decimal" => {
            json!({"type": "decimal", "coefficient": raw, "scale": spec.get("scale")?, "unit": spec.get("unit")?})
        }
        "money" => {
            json!({"type": "money", "minor_units": raw, "currency": spec.get("currency")?, "minor_unit_exponent": spec.get("minor_unit_exponent")?})
        }
        "reference" => {
            json!({"type": "reference", "input_id": spec.get("target_input")?, "record_id": raw})
        }
        _ => return None,
    })
}

/// The record page's form (D7.16): hidden premise fields, `value.<field>`
/// texts with their `spec.<field>` echoes, the key and the token.
fn from_form(
    fields: &[(String, String)],
    app: &str,
    work: &str,
    command: &str,
) -> Option<(CommandRequest, String)> {
    let mut map: Map<String, Value> = Map::new();
    let mut values = Vec::new();
    let mut specs = Map::new();
    for (name, value) in fields {
        if let Some(field) = name.strip_prefix("value.") {
            values.push((field.to_owned(), value.clone()));
        } else if let Some(field) = name.strip_prefix("spec.") {
            specs.insert(field.to_owned(), serde_json::from_str(value).ok()?);
        } else if map
            .insert(name.clone(), Value::String(value.clone()))
            .is_some()
        {
            return None;
        }
    }
    let text = |key: &str| map.get(key).and_then(Value::as_str).map(str::to_owned);
    let number = |key: &str| text(key).and_then(|v| v.parse::<i64>().ok());
    let values = values
        .into_iter()
        .map(|(field, raw)| {
            let spec = specs.get(&field)?;
            Some(json!({"field_id": field, "value": typed_value(spec, &raw)?}))
        })
        .collect::<Option<Vec<_>>>()?;
    let request = CommandRequest {
        app: app.to_owned(),
        work: work.to_owned(),
        publication: text("publication_id")?,
        command: command.to_owned(),
        schema: text("schema_id")?,
        schema_version_expected: number("schema_version_expected")?,
        record: text("record_id")?,
        expected_record_version: number("expected_record_version")?,
        expected_work_version: number("expected_work_version")?,
        values: Value::Array(values),
        idempotency_key: text("idempotency_key")?,
    };
    Some((request, text("collection_id")?))
}

async fn submit(
    State(state): State<Arc<Commands>>,
    Path((app, work, command)): Path<(String, String, String)>,
    request: Request,
) -> Response {
    let auth = &state.auth;
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    if !(storage::valid_identifier(&app)
        && storage::valid_identifier(&work)
        && storage::valid_local_id(&command))
    {
        return http::not_found();
    }
    let Some((cookie, trusted)) = session(auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Ok(bytes) = to_bytes(body, COMMAND_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let content_type = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let header_key = parts
        .headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .map(str::to_owned);
    let (mode, request) = match content_type.as_str() {
        "application/json" => {
            if header_key.is_none()
                && serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|b| b.get("idempotency_key").cloned())
                    .is_none()
            {
                return http::problem(StatusCode::BAD_REQUEST, "missing_idempotency_key");
            }
            let Some(request) = from_json(&bytes, &app, &work, &command, header_key.as_deref())
            else {
                return http::problem(StatusCode::BAD_REQUEST, "malformed_command");
            };
            (Mode::Json, request)
        }
        "application/x-www-form-urlencoded" => {
            let fields: Vec<(String, String)> =
                url::form_urlencoded::parse(&bytes).into_owned().collect();
            let token = fields
                .iter()
                .find(|(name, _)| name == "csrf")
                .map(|(_, value)| value.as_str())
                .unwrap_or("");
            if !auth.csrf.verify(csrf::Form::Command, &cookie, token) {
                return http::cross_site();
            }
            if !fields.iter().any(|(name, _)| name == "idempotency_key") {
                return http::problem(StatusCode::BAD_REQUEST, "missing_idempotency_key");
            }
            let Some((request, collection)) = from_form(&fields, &app, &work, &command) else {
                return http::problem(StatusCode::BAD_REQUEST, "malformed_command");
            };
            if !storage::valid_identifier(&collection) {
                return http::problem(StatusCode::BAD_REQUEST, "malformed_command");
            }
            (Mode::Form { collection }, request)
        }
        _ => return http::problem(StatusCode::BAD_REQUEST, "malformed_command"),
    };
    if !idempotency_key_shaped(&request.idempotency_key) {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_idempotency_key");
    }
    if !request.well_formed() || payload::digest(&request).is_none() {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_command");
    }
    let deadline = Instant::now() + COMMAND_HANDLER_DEADLINE - STORAGE_MARGIN;
    let result = stores
        .data
        .commit_record_until(&trusted, &request, deadline)
        .await;
    tracing::info!(
        outcome = match &result {
            Ok(CommandResult::Committed(_)) => "committed",
            Ok(CommandResult::Replayed(_)) => "replayed",
            Ok(CommandResult::Conflict(_)) => "conflict",
            Ok(CommandResult::Stale(_)) => "stale",
            Ok(CommandResult::InFlight) => "in_flight",
            Ok(CommandResult::Busy) => "busy",
            Err(storage::Error::Denied) => "denied",
            Err(storage::Error::UnknownOutcome) => "unknown_outcome",
            Err(_) => "unavailable",
        },
        "command.submit"
    );
    answer(&mode, &app, &work, &request.record, result)
}

/// ADR-002: after a lost response the client looks the outcome up by key
/// under current authority; another principal's outcome is the uniform 404.
async fn outcome(
    State(state): State<Arc<Commands>>,
    Path((app, work, command, key)): Path<(String, String, String, String)>,
    request: Request,
) -> Response {
    let auth = &state.auth;
    if !(storage::valid_identifier(&app)
        && storage::valid_identifier(&work)
        && storage::valid_local_id(&command)
        && idempotency_key_shaped(&key))
    {
        return http::not_found();
    }
    let Some((_, trusted)) = session(auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores
        .data
        .read_outcome(&trusted, &app, &work, &command, &key)
        .await
    {
        Ok(Some(outcome)) => document(StatusCode::OK, &outcome.document()),
        Ok(None) | Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// The closed `notice` query value after a form's redirect; anything else
/// is ignored (the page reads typed props only).
fn notice(query: Option<&str>) -> Option<Notice> {
    let query = query?;
    let mut found = None;
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("notice", value)) if found.is_none() => found = Notice::parse(value),
            _ => return None,
        }
    }
    found
}

/// A fresh 128-bit key per render (D7.16), 32 lowercase hex digits.
fn fresh_key() -> Option<String> {
    let bytes = capability::random_bytes::<16>()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

async fn record_page(
    State(state): State<Arc<Commands>>,
    Path((app, work, collection, record)): Path<(String, String, String, String)>,
    request: Request,
) -> Response {
    let auth = &state.auth;
    if !(storage::valid_identifier(&app)
        && storage::valid_identifier(&work)
        && storage::valid_identifier(&collection)
        && storage::valid_identifier(&record))
    {
        return http::not_found();
    }
    let Some((cookie, trusted)) = session(auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let detail = match stores
        .data
        .read_record_detail(&trusted, &app, &work, &collection, &record)
        .await
    {
        Ok(detail) => detail,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    let Some(key) = fresh_key() else {
        return http::unavailable();
    };
    let view = super::view(
        &app,
        &work,
        &detail,
        auth.csrf.token(csrf::Form::Command, &cookie),
        key,
        notice(request.uri().query()),
    );
    state.page.record(&view)
}

#[cfg(test)]
pub(crate) fn parse_form_for_tests(
    fields: &[(String, String)],
    app: &str,
    work: &str,
    command: &str,
) -> Option<(CommandRequest, String)> {
    from_form(fields, app, work, command)
}
#[cfg(test)]
pub(crate) fn parse_json_for_tests(
    bytes: &[u8],
    app: &str,
    work: &str,
    command: &str,
    key: Option<&str>,
) -> Option<CommandRequest> {
    from_json(bytes, app, work, command, key)
}
#[cfg(test)]
pub(crate) fn notice_for_tests(query: Option<&str>) -> Option<Notice> {
    notice(query)
}
