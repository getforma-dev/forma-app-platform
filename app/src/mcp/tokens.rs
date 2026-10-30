//! The canvas's "Connect an agent" routes (APP-099): a signed-in member
//! mints, lists and revokes the member's own agent tokens. Same-origin,
//! session-gated JSON routes like the share route; storage decides
//! (`storage::agents`).
//!
//! - `POST /v1/agent-tokens`: body `{"label": "Claude"}` (1 to 40 plain
//!   characters), `application/json`, at most 4 KiB. 201 `{"token_id",
//!   "label", "created_at", "expires_at", "token", "mcp_url"}`: the raw token, shown this
//!   once and never again (only its digest is stored, never logged). 409
//!   `too_many_agent_tokens` at `MAX_LIVE_AGENT_TOKENS` live tokens; 429 at
//!   `MINT_MEMBER`.
//! - `GET /v1/agent-tokens`: `{"tokens": [{"token_id", "label",
//!   "created_at", "expires_at", "last_used_at"}], "mcp_url"}`, the
//!   member's live tokens (APP-113: a token expires 30 days after minting).
//! - `DELETE /v1/agent-tokens/{id}`: 204; another member's, a revoked, a
//!   foreign and a nonexistent token id alike are the uniform 404.
//!
//! Refusal order: fetch metadata on the writes (403 `cross_site`), the id
//! shape (uniform 404), the session (uniform 404), the body (413, then 400
//! `malformed_label`), the limit, then storage (`Denied` → uniform 404).
use crate::{
    auth::{csrf, Auth},
    http::{self, limits, UniformPath},
    sharing,
    storage::{self, agent_label_shaped, MAX_LIVE_AGENT_TOKENS},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub const PATH: &str = "/v1/agent-tokens";
/// Per member: ten mints an hour.
pub const MINT_MEMBER: &[limits::Window] = &[limits::per(10, 3600)];

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(PATH, get(list).post(mint))
        .route("/v1/agent-tokens/{token}", delete(revoke))
        .with_state(auth)
}

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

fn mcp_url(auth: &Auth) -> String {
    format!("{}{}", auth.origin(), super::PATH)
}

/// `{"label": "<1..40 plain characters>"}` and nothing else.
pub(crate) fn parse_label(bytes: &[u8]) -> Option<String> {
    let value = crate::admission::decode(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let label = object.get("label")?.as_str()?.trim();
    agent_label_shaped(label).then(|| label.to_owned())
}

async fn mint(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let Some((_, member)) = sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(bytes) = http::bounded(body, http::FORM_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let json = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        });
    let Some(label) = parse_label(&bytes).filter(|_| json && parts.uri.query().is_none()) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_label");
    };
    match stores.workspace.agent_tokens(&member).await {
        Ok(live) if live.len() >= usize::try_from(MAX_LIVE_AGENT_TOKENS).unwrap_or(0) => {
            return http::problem(StatusCode::CONFLICT, "too_many_agent_tokens");
        }
        Ok(_) => {}
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    }
    let key = format!(
        "agent-mint:{}:{}",
        member.organization(),
        member.principal()
    );
    if let Err(retry_after) = auth.limits.admit(&[(&key, MINT_MEMBER)]) {
        return http::too_many_requests(retry_after);
    }
    match stores.workspace.mint_agent_token(&member, &label).await {
        Ok(minted) => {
            tracing::info!(
                event = "agent_token.minted",
                organization_id = member.organization(),
                principal_id = member.principal(),
                token_id = minted.token_id,
                "agent token minted"
            );
            document(
                StatusCode::CREATED,
                &json!({
                    "token_id": minted.token_id,
                    "label": minted.label,
                    "created_at": minted.created_at,
                    "expires_at": minted.expires_at,
                    "token": minted.token,
                    "mcp_url": mcp_url(&auth),
                }),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn list(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = sharing::session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.agent_tokens(&member).await {
        Ok(tokens) => document(
            StatusCode::OK,
            &json!({
                "tokens": tokens.iter().map(|t| json!({
                    "token_id": t.token_id, "label": t.label,
                    "created_at": t.created_at, "expires_at": t.expires_at,
                    "last_used_at": t.last_used_at,
                })).collect::<Vec<_>>(),
                "mcp_url": mcp_url(&auth),
            }),
        ),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn revoke(
    State(auth): State<Arc<Auth>>,
    UniformPath(token): UniformPath<String>,
    request: Request,
) -> Response {
    if !csrf::same_origin(request.headers(), auth.origin()) {
        return http::cross_site();
    }
    if !storage::valid_identifier(&token) || request.uri().query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = sharing::session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.revoke_agent_token(&member, &token).await {
        Ok(()) => {
            tracing::info!(
                event = "agent_token.revoked",
                organization_id = member.organization(),
                principal_id = member.principal(),
                token_id = token,
                "agent token revoked"
            );
            let mut response = StatusCode::NO_CONTENT.into_response();
            http::secure(response.headers_mut());
            response
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}
