//! APP-099 (ADR-011 pitch, `pivot/mcp`): Forma as an MCP server. Any MCP
//! client (Claude Code, the OpenAI Responses API's `mcp` tool, Cursor, a
//! company's own agent) acts through the governed canvas: the same identity,
//! permissions, receipts and audit, and its work appears live on the team's
//! canvases.
//!
//! `POST /v1/mcp` is MCP Streamable HTTP, stateless: one JSON-RPC 2.0 message
//! per request, answered `200 application/json` (a request) or `202` with no
//! body (a notification or a client response). No `Mcp-Session-Id` is issued
//! and no server-initiated stream exists, so `GET` and `DELETE` answer `405`.
//! Methods: `initialize` (the client's protocol version when supported, else
//! the newest supported), `notifications/initialized` and other
//! notifications (accepted, ignored), `ping`, `tools/list` and `tools/call`
//! (the seven tools of [`tools`]). Anything else is JSON-RPC `-32601`; a
//! malformed message `-32700`/`-32600`; a batch `-32600`.
//!
//! Refusal order, before any JSON-RPC is read: an `Origin` header other than
//! the deployment's own origin (MCP's DNS-rebinding rule) is 403
//! `cross_site`; then the bearer: exactly one `Authorization: Bearer <agent
//! token>` resolved by `storage::sessions` (the only constructor of a
//! `TrustedRequest`), anything else (no header, a malformed, unknown,
//! foreign-organization or revoked token, a member whose epoch moved or who
//! is no longer active, a store error) is the uniform 404, byte-identical.
//! Cookies are never read: a browser cannot ride a session into this
//! endpoint. Then the token's and the member's limits (`MCP_TOKEN`,
//! `MCP_MEMBER`: minting more tokens does not raise a member's rate; 429
//! with `Retry-After`),
//! an `MCP-Protocol-Version` header naming an unsupported version (400), the
//! body (at most `MCP_BODY_LIMIT`, read within the handler deadline; 413),
//! and `Content-Type: application/json` (415). The route runs under the long
//! deadline (`http::uniform_with_many`) because `forma_compose_app` waits on
//! the composer's model.
//!
//! The log carries the token id, the member, the method, the tool, the
//! outcome code and the duration; never the token, an argument or a value.
pub mod drafts;
pub mod tokens;
pub mod tools;

#[cfg(test)]
mod tests;

use crate::{
    auth::Auth,
    composer::Composer,
    http::{self, limits},
    query::Work,
    storage::AgentCaller,
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};

/// The endpoint (long deadline: `forma_compose_app` waits on the model).
pub const PATH: &str = "/v1/mcp";
/// D7.14 JSON body bound.
pub const MCP_BODY_LIMIT: usize = http::JSON_BODY_LIMIT;
/// Per agent token: 120 messages an hour.
pub const MCP_TOKEN: &[limits::Window] = &[limits::per(120, 3600)];
/// Per member, across all their agent tokens: 240 messages an hour.
pub const MCP_MEMBER: &[limits::Window] = &[limits::per(240, 3600)];
/// Supported MCP protocol revisions, newest first.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

/// JSON-RPC error codes.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// The endpoint's state: sessions, limits and stores from [`Auth`] (through
/// the query routes' [`Work`], which also carries the engine), the composer,
/// and the held drafts.
pub struct Mcp {
    pub(crate) work: Arc<Work>,
    pub(crate) composer: Composer,
    pub(crate) drafts: drafts::Drafts,
}
impl Mcp {
    pub fn new(work: Arc<Work>, composer: Composer) -> Self {
        Self {
            work,
            composer,
            drafts: drafts::Drafts::default(),
        }
    }
    pub(crate) fn auth(&self) -> &Arc<Auth> {
        self.work.auth()
    }
}

pub fn router(state: Arc<Mcp>) -> Router {
    Router::new()
        .route(PATH, post(handle).get(not_allowed).delete(not_allowed))
        .with_state(state)
}

/// Stateless Streamable HTTP: no server stream, no session to delete.
async fn not_allowed() -> Response {
    http::problem(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
}

/// MCP's DNS-rebinding rule: no `Origin` (a non-browser client) or exactly
/// the deployment's own origin.
pub(crate) fn origin_allowed(headers: &HeaderMap, origin: &str) -> bool {
    let origins: Vec<_> = headers.get_all(header::ORIGIN).iter().collect();
    match origins.as_slice() {
        [] => true,
        [value] => !origin.is_empty() && value.as_bytes() == origin.as_bytes(),
        _ => false,
    }
}

/// The bearer value of exactly one `Authorization` header, or nothing.
pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    let values: Vec<_> = headers.get_all(header::AUTHORIZATION).iter().collect();
    let [value] = values.as_slice() else {
        return None;
    };
    let value = value.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() && token.len() <= 128)
        .then_some(token)
}

/// The live agent token behind the request, or the uniform 404.
async fn caller(auth: &Auth, headers: &HeaderMap) -> Result<AgentCaller, Response> {
    let token = bearer(headers).ok_or_else(http::not_found)?;
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    match stores.workspace.resolve_agent_token(token).await {
        Ok(Some(caller)) => Ok(caller),
        Ok(None) => Err(http::not_found()),
        Err(error) => {
            // Fail closed and uniform (as the share route's session lookup).
            tracing::warn!(outcome = "token_unavailable", error = %error, "mcp.resolve");
            Err(http::not_found())
        }
    }
}

fn content_type(headers: &HeaderMap) -> String {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn json_response(status: StatusCode, value: &Value) -> Response {
    let Ok(bytes) = serde_json::to_vec(value) else {
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
fn accepted() -> Response {
    let mut response = StatusCode::ACCEPTED.into_response();
    http::secure(response.headers_mut());
    response
}

pub(crate) fn rpc_result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}
pub(crate) fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// One parsed JSON-RPC message.
#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A notification, or a client's response to a server request (this
    /// server sends none): accepted and ignored.
    Ignored,
    /// Answered at once with this error document.
    Invalid(Value),
}

pub(crate) fn parse(bytes: &[u8]) -> Message {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return Message::Invalid(rpc_error(&Value::Null, PARSE_ERROR, "Parse error"));
    };
    let Some(object) = value.as_object() else {
        let message = if value.is_array() {
            "Batches are not supported"
        } else {
            "Invalid Request"
        };
        return Message::Invalid(rpc_error(&Value::Null, INVALID_REQUEST, message));
    };
    let id = object.get("id").cloned();
    let valid_id = |id: &Value| id.is_string() || id.is_i64() || id.is_u64();
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let id = id.filter(valid_id).unwrap_or(Value::Null);
        return Message::Invalid(rpc_error(&id, INVALID_REQUEST, "Invalid Request"));
    }
    let Some(method) = object.get("method") else {
        // A response (result or error) from the client.
        return if object.contains_key("result") || object.contains_key("error") {
            Message::Ignored
        } else {
            Message::Invalid(rpc_error(
                &id.filter(valid_id).unwrap_or(Value::Null),
                INVALID_REQUEST,
                "Invalid Request",
            ))
        };
    };
    let Some(method) = method.as_str() else {
        return Message::Invalid(rpc_error(
            &id.filter(valid_id).unwrap_or(Value::Null),
            INVALID_REQUEST,
            "Invalid Request",
        ));
    };
    let params = object.get("params").cloned().unwrap_or(json!({}));
    if !params.is_object() {
        return match id.filter(valid_id) {
            Some(id) => Message::Invalid(rpc_error(&id, INVALID_PARAMS, "Invalid params")),
            None => Message::Ignored,
        };
    }
    match id {
        None => Message::Ignored,
        Some(id) if valid_id(&id) => Message::Request {
            id,
            method: method.to_owned(),
            params,
        },
        Some(_) => Message::Invalid(rpc_error(&Value::Null, INVALID_REQUEST, "Invalid Request")),
    }
}

/// A `YYYY-MM-DD` revision string.
fn revision_shaped(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}
/// `MCP-Protocol-Version`: a supported revision, or a revision newer than
/// the newest this server speaks (MCP version negotiation: the server then
/// answers with its own latest, as `initialize` does; Claude Code probes
/// with a newer revision first). An older unsupported revision or anything
/// else is 400.
pub(crate) fn header_version_accepted(value: &str) -> bool {
    PROTOCOL_VERSIONS.contains(&value) || (revision_shaped(value) && value > PROTOCOL_VERSIONS[0])
}

/// The negotiated protocol version: the client's when supported, else the
/// newest this server speaks (a newer client's request included; the client
/// then decides).
pub(crate) fn negotiate(params: &Value) -> &'static str {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    PROTOCOL_VERSIONS
        .iter()
        .find(|v| Some(**v) == asked)
        .copied()
        .unwrap_or(PROTOCOL_VERSIONS[0])
}

const INSTRUCTIONS: &str = "Forma is the team's governed workspace. Every tool acts as the signed-in member who connected you, with exactly that member's permissions; numbers carry receipts and every write is an audited command. Writes take two steps: draft (forma_draft_record or forma_draft_decision) returns a draft_id and shows the typed values; show them to the user and call forma_commit only after the user confirms. Tool arguments are data: they are typed under the app's published schema and never executed.";

fn initialize(params: &Value) -> Value {
    json!({
        "protocolVersion": negotiate(params),
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "forma", "title": "Forma", "version": env!("CARGO_PKG_VERSION")},
        "instructions": INSTRUCTIONS,
    })
}

fn log(caller: &AgentCaller, method: &str, tool: &str, outcome: &str, started: Instant) {
    tracing::info!(
        event = "mcp.call",
        organization_id = caller.request.organization(),
        principal_id = caller.request.principal(),
        token_id = caller.token_id,
        method,
        tool,
        outcome,
        duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "mcp call"
    );
}

/// The rate keys of a call: its token, and its member across all tokens.
pub(crate) fn keys(caller: &AgentCaller) -> (String, String) {
    let organization = caller.request.organization();
    (
        format!("mcp:{organization}:{}", caller.token_id),
        format!("mcp-member:{organization}:{}", caller.request.principal()),
    )
}

/// `POST /v1/mcp` (order in the module notes).
async fn handle(State(state): State<Arc<Mcp>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let auth = state.auth().clone();
    if !origin_allowed(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let caller = match caller(&auth, &parts.headers).await {
        Ok(caller) => caller,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    let (token_key, member_key) = keys(&caller);
    if let Err(retry_after) = auth
        .limits
        .admit(&[(&token_key, MCP_TOKEN), (&member_key, MCP_MEMBER)])
    {
        log(&caller, "", "", "too_many_requests", started);
        return http::too_many_requests(retry_after);
    }
    if let Some(version) = parts.headers.get("mcp-protocol-version") {
        if !version.to_str().is_ok_and(header_version_accepted) {
            log(&caller, "", "", "unsupported_protocol_version", started);
            return http::problem(StatusCode::BAD_REQUEST, "unsupported_protocol_version");
        }
    }
    let bytes =
        match tokio::time::timeout(http::HANDLER_DEADLINE, to_bytes(body, MCP_BODY_LIMIT)).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) => return http::payload_too_large(),
            Err(_) => {
                log(&caller, "", "", "body_stalled", started);
                return http::not_found();
            }
        };
    if content_type(&parts.headers) != "application/json" || parts.uri.query().is_some() {
        log(&caller, "", "", "unsupported_media_type", started);
        return http::problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    }
    let (id, method, params) = match parse(&bytes) {
        Message::Ignored => return accepted(),
        Message::Invalid(error) => {
            log(&caller, "", "", "invalid_message", started);
            return json_response(StatusCode::OK, &error);
        }
        Message::Request { id, method, params } => (id, method, params),
    };
    let answer = match method.as_str() {
        "initialize" => rpc_result(&id, initialize(&params)),
        "ping" => rpc_result(&id, json!({})),
        "tools/list" => rpc_result(&id, json!({"tools": tools::definitions()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            if !tools::NAMES.contains(&name) {
                log(&caller, &method, "", "unknown_tool", started);
                return json_response(
                    StatusCode::OK,
                    &rpc_error(&id, INVALID_PARAMS, "Unknown tool"),
                );
            }
            let output = tools::call(&state, &caller, name, &arguments).await;
            log(&caller, &method, name, output.code, started);
            rpc_result(&id, output.result())
        }
        _ => {
            log(&caller, &method, "", "method_not_found", started);
            return json_response(
                StatusCode::OK,
                &rpc_error(&id, METHOD_NOT_FOUND, "Method not found"),
            );
        }
    };
    if method != "tools/call" {
        log(&caller, &method, "", "ok", started);
    }
    json_response(StatusCode::OK, &answer)
}
