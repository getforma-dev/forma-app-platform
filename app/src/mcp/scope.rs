//! APP-115 quick wins: an agent token's scope, enforced in the MCP layer.
//!
//! Every tool maps to one action (`ACTIONS`); a call whose action the token
//! does not allow is refused before any storage call (`scope_action`). The
//! resource a tool names is judged by its space: a library app by its
//! sidecar category (the library pane's: `finance`, `security`,
//! `operations`, `sales`, `support`, `people`, `utilities`), any other app by
//! `mine` (the member's own and composed apps), a folder by `folders`. A
//! call naming an app by id is judged before any storage call; a tool that
//! resolves its app, record or folder (a hint, a pending record, a held
//! draft) is judged again where it resolves it (`scope_space`). Listings are
//! narrowed to the scope instead of refused.
//!
//! A scope only narrows: every tool still acts as the member, with exactly
//! the member's rights, so a scope naming an app the member cannot use adds
//! nothing. A refusal is the uniform tool error ("Not found.", identical to
//! a storage denial); only the access log records which rule refused it.
use crate::storage::{AgentCaller, AgentScope};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

/// The space of every app outside the library sidecar.
pub const MINE: &str = "mine";
/// The space of folders.
pub const FOLDERS: &str = "folders";

/// The action each tool takes.
pub fn action_of(tool: &str) -> &'static str {
    use super::tools::*;
    match tool {
        LIST_APPS | RUN_VIEW | LIST_PENDING | LIST_FOLDERS | FOLDER_BRIEF => "read",
        DRAFT_RECORD => "draft",
        DRAFT_DECISION => "decide",
        COMMIT => "commit",
        SHARE => "share",
        COMPOSE_APP | FOLDER_FORMATION => "build",
        _ => "read",
    }
}

/// The library pane's category of a capability prefix (`query::canvas`'s
/// CATEGORIES: `ops` and `operations` are one).
fn category(prefix: &str) -> &'static str {
    match prefix {
        "finance" => "finance",
        "security" => "security",
        "ops" | "operations" => "operations",
        "sales" => "sales",
        "support" => "support",
        "people" => "people",
        "utilities" => "utilities",
        _ => MINE,
    }
}

/// Library app id → (space, title), from the sidecar.
fn library() -> &'static BTreeMap<String, (&'static str, String)> {
    static LIBRARY: OnceLock<BTreeMap<String, (&'static str, String)>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        crate::storage::library_sidecar()
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| {
                let app = entry.get("app_id")?.as_str()?.to_owned();
                let prefix = entry
                    .get("routing")
                    .and_then(|r| r.get("capabilities"))
                    .and_then(Value::as_array)
                    .and_then(|c| c.first())
                    .and_then(Value::as_str)
                    .and_then(|c| c.split('.').next())
                    .unwrap_or_default();
                let title = entry.get("title")?.as_str()?.to_owned();
                Some((app, (category(prefix), title)))
            })
            .collect()
    })
}

/// The space an app belongs to.
pub fn space_of(app: &str) -> &'static str {
    library().get(app).map_or(MINE, |(space, _)| space)
}
/// A library app's title, for the log's display label.
pub fn library_title(app: &str) -> Option<&'static str> {
    library().get(app).map(|(_, title)| title.as_str())
}
/// How a space is shown ("People", "My apps").
pub fn space_title(space: &str) -> &'static str {
    match space {
        "finance" => "Finance",
        "security" => "Security",
        "operations" => "Operations",
        "sales" => "Sales",
        "support" => "Support",
        "people" => "People",
        "utilities" => "Widgets",
        MINE => "My apps",
        FOLDERS => "Folders",
        _ => "Other",
    }
}

/// Whether the caller's scope allows the tool's action.
pub fn allows_action(caller: &AgentCaller, tool: &str) -> bool {
    caller.scope.allows_action(action_of(tool))
}
/// Whether the caller's scope reaches this app.
pub fn allows_app(scope: &AgentScope, app: &str) -> bool {
    scope.allows(space_of(app), Some(app))
}
/// Whether the caller's scope reaches folders.
pub fn allows_folders(scope: &AgentScope) -> bool {
    scope.allows(FOLDERS, None)
}
