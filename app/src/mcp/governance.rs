//! APP-115 quick wins: the member's agent governance routes, read by the
//! canvas's "Agents" panel (`admin/src/canvas/governance.ts`).
//!
//! - `GET /v1/governance/agents`: `{"agents": [{"token_id", "label",
//!   "scope", "created_at", "expires_at", "last_used_at", "status", "calls",
//!   "refused", "last_call_at"}], "organization_wide": false}`, the member's
//!   own agents (live and ended), newest first.
//! - `GET /v1/governance/agents/{token}/activity`: `{"agent": {...},
//!   "entries": [...]}`, that agent's access log, newest first (at most
//!   `storage::ACTIVITY_LIMIT`): tool, touched ids, outcome, reason, space,
//!   resource label, duration and time. Another member's, a foreign and a
//!   nonexistent token id are the uniform 404.
//!
//! Member-scoped only: no organization admin role exists yet, so an
//! organization-wide view is carried to APP-115 full. Session-gated reads
//! like `GET /v1/agent-tokens`; revocation is that route's `DELETE`.
use super::scope;
use crate::{
    auth::Auth,
    http::{self, UniformPath},
    sharing,
    storage::{self, AccessRow, AgentSummary},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub const AGENTS_PATH: &str = "/v1/governance/agents";

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(AGENTS_PATH, get(agents))
        .route("/v1/governance/agents/{token}/activity", get(activity))
        .with_state(auth)
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

/// A tool as the timeline names it.
pub fn tool_title(tool: &str) -> &'static str {
    use super::tools::*;
    match tool {
        LIST_APPS => "List apps",
        RUN_VIEW => "Run a view",
        COMPOSE_APP => "Compose an app",
        DRAFT_RECORD => "Draft a record",
        DRAFT_DECISION => "Draft a decision",
        COMMIT => "Commit a draft",
        LIST_PENDING => "List pending decisions",
        SHARE => "Share a work",
        LIST_FOLDERS => "List folders",
        FOLDER_FORMATION => "Build from a folder",
        FOLDER_BRIEF => "Brief on a folder",
        _ => "Tool call",
    }
}

/// A refusal's reason as the timeline shows it.
pub fn reason_title(reason: &str) -> &'static str {
    match reason {
        "scope_space" => "outside scope",
        "scope_action" => "action not allowed",
        "not_found" => "not found or not permitted",
        "invalid_arguments" => "invalid arguments",
        "draft_not_found" => "draft expired or unknown",
        "could_not_draft" => "could not draft",
        "conflict" => "changed since the draft",
        "too_many_requests" | "too_many_drafts" => "rate limited",
        "unavailable" | "busy" => "unavailable",
        _ => "refused",
    }
}

fn agent_json(agent: &AgentSummary) -> Value {
    let mut scope = agent.scope.document();
    scope["space_titles"] = json!(agent
        .scope
        .spaces
        .iter()
        .map(|s| scope::space_title(s))
        .collect::<Vec<_>>());
    scope["app_titles"] = json!(agent
        .scope
        .apps
        .iter()
        .map(|a| scope::library_title(a).unwrap_or("an app"))
        .collect::<Vec<_>>());
    json!({
        "token_id": agent.token_id, "label": agent.label, "scope": scope,
        "created_at": agent.created_at, "expires_at": agent.expires_at,
        "last_used_at": agent.last_used_at, "status": agent.status,
        "calls": agent.calls, "refused": agent.refused, "last_call_at": agent.last_call_at,
    })
}

fn entry_json(row: &AccessRow) -> Value {
    json!({
        "entry_id": row.entry_id, "sequence": row.sequence,
        "tool": row.tool, "tool_title": tool_title(&row.tool),
        "touched": {"apps": row.apps, "works": row.works, "records": row.records, "folders": row.folders, "files": row.files},
        "outcome": row.outcome, "reason": row.reason,
        "reason_title": row.reason.as_deref().map(reason_title),
        "space": row.space, "space_title": row.space.as_deref().map(scope::space_title),
        "resource_label": row.resource_label,
        "duration_ms": row.duration_ms, "occurred_at": row.occurred_at,
    })
}

async fn agents(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = sharing::session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.agent_governance(&member).await {
        Ok(agents) => document(&json!({
            "agents": agents.iter().map(agent_json).collect::<Vec<_>>(),
            "organization_wide": false,
        })),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn activity(
    State(auth): State<Arc<Auth>>,
    UniformPath(token): UniformPath<String>,
    request: Request,
) -> Response {
    if !storage::valid_identifier(&token) || request.uri().query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = sharing::session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.agent_activity(&member, &token).await {
        Ok((agent, rows)) => document(&json!({
            "agent": agent_json(&agent),
            "entries": rows.iter().map(entry_json).collect::<Vec<_>>(),
        })),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}
