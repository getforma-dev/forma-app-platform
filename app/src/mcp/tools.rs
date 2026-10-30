//! The closed, typed tool set of `POST /v1/mcp` (APP-099). Every tool acts as
//! the token's member with the member's `TrustedRequest` and calls the same
//! code the canvas routes call; a token never exceeds the member's rights:
//!
//! - `forma_list_apps`: `Workspace::composer_context` (the library apps the
//!   member may instantiate, with the record fields a draft may set) and
//!   `Data::openable_works` (the member's saved and shared work).
//! - `forma_run_view`: `Data::work_outline` and the JSON query route's
//!   [`crate::query::routes::run_released`]: authorization, the engine, the
//!   contract-checked receipt and the release, the receipt id with the rows
//!   (at most `MAX_ROWS` shown).
//! - `forma_compose_app`: the composer route's
//!   [`crate::composer::routes::compose_for`] under the same per-member limit
//!   (seam noted in APP-099: the route-level function, never `plan.rs`);
//!   `app.placed` reaches the member's open canvas.
//! - `forma_draft_record` / `forma_draft_decision`: the action path's
//!   [`crate::actions::resolve::resolve`] (which ends in
//!   `Data::dry_run_command`), fed by the agent's structured arguments
//!   instead of the router model's parse; values are typed under the
//!   published schema (I5). The result is a held draft, not a write.
//! - `forma_commit`: the held draft through the command route's
//!   [`crate::commands::routes::commit`] with its server-minted key
//!   (I4), then the share route's `Workspace::share_work` to the resolved
//!   approver; `record.committed` updates the teammates' canvases live.
//! - `forma_list_pending`: the records the member may decide now
//!   (`Data::list_records` over the member's writable works).
//! - `forma_share`: the share route's `Workspace::share_work`.
//!
//! Every denial is the same tool error (`not_found`, "Not found."): a foreign
//! organization's app id, another member's work and a nonexistent id are
//! indistinguishable. Arguments outside a tool's schema are refused before
//! any storage call (`invalid_arguments`).
use super::{drafts, Mcp};
use crate::{
    actions::{
        prompt::{self, CatalogApp},
        resolve::{self, Draft, Refusal, Resolution},
        Kind, Parsed,
    },
    composer::{self, routes::Refused},
    demo, directory, events,
    query::{
        routes::{run_released, ViewRefusal},
        Failure, QueryError,
    },
    storage::{self, AgentCaller, CommandResult, Stores},
};
use serde_json::{json, Map, Value};
use std::time::Instant;

pub const LIST_APPS: &str = "forma_list_apps";
pub const RUN_VIEW: &str = "forma_run_view";
pub const COMPOSE_APP: &str = "forma_compose_app";
pub const DRAFT_RECORD: &str = "forma_draft_record";
pub const DRAFT_DECISION: &str = "forma_draft_decision";
pub const COMMIT: &str = "forma_commit";
pub const LIST_PENDING: &str = "forma_list_pending";
pub const SHARE: &str = "forma_share";
/// APP-108: folders (the member-facing noun of a box).
pub const LIST_FOLDERS: &str = "forma_list_folders";
pub const FOLDER_FORMATION: &str = "forma_folder_formation";
pub const FOLDER_BRIEF: &str = "forma_folder_brief";
/// The ticket's closed tool set (its text says seven; its list names eight,
/// the two drafts sharing one bullet), and nothing else.
pub const NAMES: [&str; 11] = [
    LIST_APPS,
    RUN_VIEW,
    COMPOSE_APP,
    DRAFT_RECORD,
    DRAFT_DECISION,
    COMMIT,
    LIST_PENDING,
    SHARE,
    LIST_FOLDERS,
    FOLDER_FORMATION,
    FOLDER_BRIEF,
];
/// Rows of a view shown to the agent.
pub const MAX_ROWS: usize = 50;
/// Pending records listed.
const MAX_PENDING: usize = 50;
/// Writable works searched for pending records.
const MAX_PENDING_WORKS: usize = 20;
/// A text argument's bound (hints, names, notes, values).
const MAX_TEXT: usize = 500;

const UUID_SCHEMA: &str =
    "^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$";

fn read_only(title: &str) -> Value {
    json!({"title": title, "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false})
}
fn write(title: &str, idempotent: bool) -> Value {
    json!({"title": title, "readOnlyHint": false, "destructiveHint": false, "idempotentHint": idempotent, "openWorldHint": false})
}

/// `tools/list`: every tool's name, description, closed JSON Schema and
/// annotations.
pub fn definitions() -> Vec<Value> {
    let uuid = json!({"type": "string", "pattern": UUID_SCHEMA});
    vec![
        json!({
            "name": LIST_APPS,
            "title": "List apps",
            "description": "List the Forma apps this member can use: the library apps they may start (with the record fields a draft may set) and their saved and shared work, with canvas links. Says whether the data is synthetic.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only("List apps"),
        }),
        json!({
            "name": RUN_VIEW,
            "title": "Run a view",
            "description": "Run one view of a saved work through Forma's governed query engine. Returns typed rows (at most 50) and the receipt id that proves where every number came from. Omit `view` for the work's first view.",
            "inputSchema": {"type": "object", "properties": {
                "app_id": uuid, "work_id": uuid,
                "view": {"type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}$"},
                "parameters": {"type": "object", "description": "Optional: window {start, end} as YYYY-MM-DDTHH:MM:SS, destination, filter_epoch."},
            }, "required": ["app_id", "work_id"], "additionalProperties": false},
            "annotations": read_only("Run a view"),
        }),
        json!({
            "name": COMPOSE_APP,
            "title": "Compose an app",
            "description": "Describe an app the team needs; Forma picks it from the admitted library (retrieved), adapts one, or generates a typed definition through admission (generated), saves it for this member and places it on their canvas live. Returns the origin label and the canvas link.",
            "inputSchema": {"type": "object", "properties": {
                "description": {"type": "string", "minLength": 3, "maxLength": 500},
            }, "required": ["description"], "additionalProperties": false},
            "annotations": write("Compose an app", false),
        }),
        json!({
            "name": DRAFT_RECORD,
            "title": "Draft a record",
            "description": "Draft a new record (an expense claim, a time off, purchase, access or discount request, a vendor invoice) in one of the member's record apps. Nothing is written: the values are typed under the app's published schema and checked by storage, and a draft_id is returned. Show the draft to the user and call forma_commit only after they confirm. The status is set by the app's workflow.",
            "inputSchema": {"type": "object", "properties": {
                "app_hint": {"type": "string", "minLength": 1, "maxLength": 120, "description": "The app's id or title, e.g. \"Expense claims\"."},
                "values": {"type": "object", "description": "Field id to value, e.g. {\"amount\": \"86.00\", \"merchant\": \"Nobu\", \"category\": \"meals\", \"spent_on\": \"2026-10-29\", \"note\": \"client dinner\"}.",
                    "additionalProperties": {"type": ["string", "number", "boolean"]}},
                "approver_name": {"type": "string", "minLength": 1, "maxLength": 120, "description": "The teammate who approves, by name."},
            }, "required": ["app_hint", "values"], "additionalProperties": false},
            "annotations": write("Draft a record", true),
        }),
        json!({
            "name": DRAFT_DECISION,
            "title": "Draft a decision",
            "description": "Draft approving or rejecting a pending record the member may decide (see forma_list_pending). Approve takes the next approval step the app's declared rules allow (a purchase over $5,000 is approved by the manager first, then finance; access is granted; an approved invoice is paid). Nothing is written; returns a draft_id for forma_commit after the user confirms.",
            "inputSchema": {"type": "object", "properties": {
                "target_hint": {"type": "string", "minLength": 1, "maxLength": 120, "description": "The record: its id, or words from it such as \"Nobu dinner\"."},
                "decision": {"type": "string", "enum": ["approve", "reject"]},
                "note": {"type": "string", "maxLength": 200},
            }, "required": ["target_hint", "decision"], "additionalProperties": false},
            "annotations": write("Draft a decision", true),
        }),
        json!({
            "name": COMMIT,
            "title": "Commit a draft",
            "description": "Commit a draft the user confirmed, through Forma's audited command path (one transaction, idempotent, on the audit chain), then share it with the resolved approver. Committing the same draft again returns the same record. Drafts expire after 10 minutes.",
            "inputSchema": {"type": "object", "properties": {
                "draft_id": {"type": "string", "pattern": "^[0-9a-f]{32}$"},
            }, "required": ["draft_id"], "additionalProperties": false},
            "annotations": write("Commit a draft", true),
        }),
        json!({
            "name": LIST_PENDING,
            "title": "List pending decisions",
            "description": "List the records waiting for this member's decision (expense claims, time off, purchase, access and discount requests, vendor invoices), filed by teammates.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only("List pending decisions"),
        }),
        json!({
            "name": SHARE,
            "title": "Share a work",
            "description": "Share one of the member's saved works with a teammate of the same organization, by name. The teammate gets exactly the subset of the member's own permissions that opening it requires (and record_write only when asked for and held).",
            "inputSchema": {"type": "object", "properties": {
                "app_id": uuid, "work_id": uuid,
                "member_name": {"type": "string", "minLength": 1, "maxLength": 120},
                "record_write": {"type": "boolean"},
            }, "required": ["app_id", "work_id", "member_name"], "additionalProperties": false},
            "annotations": write("Share a work", true),
        }),
        json!({
            "name": LIST_FOLDERS,
            "title": "List folders",
            "description": "List the member's Forma folders (files, real CSV/XLSX data and the apps built from them): number, name, file count and the files with their types and whether their data is registered.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": read_only("List folders"),
        }),
        json!({
            "name": FOLDER_FORMATION,
            "title": "Formation from a folder",
            "description": "Act on one of the member's folders in words: build an app from its data (\"build a metrics app and explain it\") or brief me on it. The folder is its number or name from forma_list_folders. The app is generated from a capped context (file names, each confirmed schema and at most 20 sample rows), admitted, placed on the canvas, and traces back to the folder's files; receipts name the registered revisions. Returns the app link, origin label and the folder provenance.",
            "inputSchema": {"type": "object", "properties": {
                "folder": {"type": "string", "minLength": 1, "maxLength": 60},
                "instruction": {"type": "string", "minLength": 3, "maxLength": 500},
            }, "required": ["folder", "instruction"], "additionalProperties": false},
            "annotations": write("Formation from a folder", false),
        }),
        json!({
            "name": FOLDER_BRIEF,
            "title": "Brief on a folder",
            "description": "The folder's brief: an overview, a paragraph per file and key points; numbers from data files come from governed results with receipts, PDF statements are cited by page.",
            "inputSchema": {"type": "object", "properties": {
                "folder": {"type": "string", "minLength": 1, "maxLength": 60},
            }, "required": ["folder"], "additionalProperties": false},
            "annotations": write("Brief on a folder", false),
        }),
    ]
}

/// A tool's answer: a short text for the model (with the structured
/// document serialized, for clients that read only `content`), the
/// structured document, and whether it is a tool error. `code` is for the
/// log.
#[derive(Debug)]
pub struct Output {
    pub code: &'static str,
    pub text: String,
    pub structured: Value,
    pub is_error: bool,
}
impl Output {
    fn ok(code: &'static str, summary: String, structured: Value) -> Self {
        Self {
            code,
            text: summary,
            structured,
            is_error: false,
        }
    }
    fn error(code: &'static str, message: &str) -> Self {
        Self {
            code,
            text: message.to_owned(),
            structured: json!({"error": code, "message": message}),
            is_error: true,
        }
    }
    /// The uniform denial.
    fn not_found() -> Self {
        Self::error("not_found", "Not found.")
    }
    fn unavailable() -> Self {
        Self::error(
            "unavailable",
            "Forma is unavailable right now. Nothing was changed.",
        )
    }
    fn invalid(message: &str) -> Self {
        Self::error("invalid_arguments", message)
    }
    /// The `tools/call` result.
    pub fn result(&self) -> Value {
        let text = if self.is_error {
            self.text.clone()
        } else {
            format!(
                "{}\n\n{}",
                self.text,
                serde_json::to_string(&self.structured).unwrap_or_default()
            )
        };
        json!({
            "content": [{"type": "text", "text": text}],
            "structuredContent": self.structured,
            "isError": self.is_error,
        })
    }
}

/// The canvas link of a saved work.
fn link(state: &Mcp, app: &str, work: &str) -> String {
    format!("{}/w/{app}/{work}", state.auth().origin())
}

/// Exactly these argument members, each a member of `allowed`.
fn closed(arguments: &Value, allowed: &[&str]) -> Option<Map<String, Value>> {
    let object = arguments.as_object()?;
    object
        .keys()
        .all(|k| allowed.contains(&k.as_str()))
        .then(|| object.clone())
}
/// A bounded, single-line text argument.
fn text_arg(
    arguments: &Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Option<String>, Output> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty()
                || trimmed.chars().count() > max
                || trimmed.chars().any(char::is_control)
            {
                Err(Output::invalid(&format!(
                    "`{key}` must be 1 to {max} characters on one line."
                )))
            } else {
                Ok(Some(trimmed.to_owned()))
            }
        }
        Some(_) => Err(Output::invalid(&format!("`{key}` must be a string."))),
    }
}
fn id_arg(arguments: &Map<String, Value>, key: &str) -> Result<String, Output> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|id| storage::valid_identifier(id))
        .map(str::to_owned)
        .ok_or_else(|| Output::invalid(&format!("`{key}` must be a lowercase UUID.")))
}

fn storage_output(error: storage::Error) -> Output {
    match error {
        storage::Error::Denied => Output::not_found(),
        _ => Output::unavailable(),
    }
}

/// Runs one tool for the caller (argument checks, then storage).
pub async fn call(state: &Mcp, caller: &AgentCaller, name: &str, arguments: &Value) -> Output {
    let Some(stores) = state.auth().stores.clone() else {
        return Output::not_found();
    };
    let result = match name {
        LIST_APPS => list_apps(state, &stores, caller, arguments).await,
        RUN_VIEW => run_view(state, &stores, caller, arguments).await,
        COMPOSE_APP => compose_app(state, &stores, caller, arguments).await,
        DRAFT_RECORD => draft_record(state, &stores, caller, arguments).await,
        DRAFT_DECISION => draft_decision(state, &stores, caller, arguments).await,
        COMMIT => commit(state, &stores, caller, arguments).await,
        LIST_PENDING => list_pending(state, &stores, caller, arguments).await,
        SHARE => share(state, &stores, caller, arguments).await,
        LIST_FOLDERS => list_folders(state, caller, arguments).await,
        FOLDER_FORMATION => folder_formation(state, &stores, caller, arguments).await,
        FOLDER_BRIEF => folder_brief(caller, arguments).await,
        _ => Err(Output::not_found()),
    };
    result.unwrap_or_else(|refusal| refusal)
}

type ToolResult = Result<Output, Output>;

// --- forma_list_apps ---

fn record_fields(app: &CatalogApp) -> Value {
    let Some(create) = app.create_command() else {
        return json!([]);
    };
    let workflow: Vec<&str> = app
        .decide_command()
        .map(|d| d.transitions.iter().map(|(f, _, _)| f.as_str()).collect())
        .unwrap_or_default();
    Value::Array(
        app.fields
            .iter()
            .filter(|(id, _)| create.writable.contains(id) && !workflow.contains(&id.as_str()))
            .map(|(id, spec)| {
                let mut field =
                    json!({"id": id, "type": spec.get("type").cloned().unwrap_or(Value::Null)});
                if let Some(values) = spec.get("values") {
                    field["values"] = values.clone();
                }
                field
            })
            .collect(),
    )
}

async fn list_apps(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    closed(arguments, &[]).ok_or_else(|| Output::invalid("This tool takes no arguments."))?;
    let member = &caller.request;
    let context = stores
        .workspace
        .composer_context(member)
        .await
        .map_err(storage_output)?;
    let works = stores
        .data
        .openable_works(member)
        .await
        .map_err(storage_output)?;
    let catalog = prompt::catalog(&context.library);
    let library: Vec<Value> = context
        .library
        .iter()
        .map(|app| {
            let record = catalog.iter().find(|c| c.app == app.app);
            json!({
                "app_id": app.app,
                "title": app.title,
                "description": app.description,
                "files_records": record.is_some_and(|c| c.create_command().is_some()),
                "record_fields": record.map(record_fields).unwrap_or(json!([])),
            })
        })
        .collect();
    let library_apps: Vec<&str> = context.library.iter().map(|a| a.app.as_str()).collect();
    let works: Vec<Value> = works
        .iter()
        .map(|w| {
            json!({
                "app_id": w.app, "work_id": w.work, "title": w.title,
                "origin": if library_apps.contains(&w.app.as_str()) { "library" } else { "workspace" },
                "can_write": w.can_write,
                "link": link(state, &w.app, &w.work),
            })
        })
        .collect();
    let synthetic = demo::is_demo_org(member.organization());
    let summary = format!(
        "{} library app(s) you can start and {} saved or shared work(s).{}",
        library.len(),
        works.len(),
        if synthetic {
            " The data is synthetic."
        } else {
            ""
        }
    );
    Ok(Output::ok(
        "ok",
        summary,
        json!({"library": library, "works": works, "synthetic": synthetic}),
    ))
}

// --- forma_run_view ---

fn engine_output(error: &QueryError) -> Output {
    match error {
        QueryError::Busy | QueryError::Failed(Failure::QueueExpired) => {
            Output::error("busy", "The query engine is busy. Try again in a moment.")
        }
        QueryError::NarrowTheQuery
        | QueryError::Failed(Failure::Deadline)
        | QueryError::Failed(Failure::Killed) => Output::error(
            "narrow_the_query",
            "The query is too large for its budget; narrow the window.",
        ),
        QueryError::Refused(_) => Output::error("query_refused", "The view could not be run."),
        QueryError::SourceUnavailable => Output::error(
            "source_unavailable",
            "The view's data source is unavailable right now.",
        ),
        QueryError::Failed(_) => {
            Output::error("query_failed", "The view could not be run right now.")
        }
    }
}

async fn run_view(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(arguments, &["app_id", "work_id", "view", "parameters"])
        .ok_or_else(|| Output::invalid("Allowed arguments: app_id, work_id, view, parameters."))?;
    let app = id_arg(&arguments, "app_id")?;
    let work = id_arg(&arguments, "work_id")?;
    let view = match arguments.get("view") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) if storage::valid_local_id(v) => Some(v.clone()),
        Some(_) => return Err(Output::invalid("`view` must be a view id.")),
    };
    let parameters = match arguments.get("parameters") {
        None | Some(Value::Null) => json!({}),
        Some(value) => value.clone(),
    };
    if storage::parse_parameters(&parameters).is_none() {
        return Err(Output::invalid(
            "`parameters` may hold window {start, end} (YYYY-MM-DDTHH:MM:SS), destination and filter_epoch only.",
        ));
    }
    let member = &caller.request;
    let outline = stores
        .data
        .work_outline(member, &app, &work)
        .await
        .map_err(storage_output)?;
    let chosen = match &view {
        Some(id) => outline.views.iter().find(|v| &v.id == id),
        None => outline.views.first(),
    }
    .ok_or_else(Output::not_found)?;
    let document = run_released(
        &state.work,
        member,
        &app,
        &work,
        &chosen.id,
        &parameters,
        Instant::now(),
    )
    .await
    .map_err(|refusal| match refusal {
        ViewRefusal::Storage(error) => storage_output(error),
        ViewRefusal::Engine(error) => engine_output(&error),
        ViewRefusal::NoEngine => Output::unavailable(),
    })?;
    let rows = document["rows"].as_array().cloned().unwrap_or_default();
    let total = rows.len();
    let shown: Vec<Value> = rows.into_iter().take(MAX_ROWS).collect();
    let receipt_id = document["receipt"]["digest"].clone();
    let synthetic = demo::is_demo_org(member.organization());
    let summary = format!(
        "{} ({}): {} row(s){}. Receipt {}.{}",
        chosen.title,
        outline.title,
        total,
        if total > MAX_ROWS {
            format!(", first {MAX_ROWS} shown")
        } else {
            String::new()
        },
        receipt_id.as_str().unwrap_or(""),
        if synthetic { " Synthetic data." } else { "" }
    );
    Ok(Output::ok(
        "ok",
        summary,
        json!({
            "app_id": app, "work_id": work, "work_title": outline.title,
            "view_id": chosen.id, "view_title": chosen.title,
            "columns": document["columns"], "column_types": document["column_types"],
            "rows": shown, "rows_shown": total.min(MAX_ROWS), "rows_total": total,
            "truncated_page": document["truncated_page"],
            "receipt_id": receipt_id, "receipt": document["receipt"],
            "synthetic": synthetic, "link": link(state, &app, &work),
        }),
    ))
}

// --- forma_compose_app ---

async fn compose_app(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(arguments, &["description"])
        .ok_or_else(|| Output::invalid("Allowed arguments: description."))?;
    let intent = arguments
        .get("description")
        .and_then(Value::as_str)
        .and_then(composer::prompt::intent)
        .map(str::to_owned)
        .ok_or_else(|| Output::invalid("`description` must be 3 to 500 characters."))?;
    let member = &caller.request;
    if !state.composer.available_for(member.organization()) {
        return Err(Output::error(
            "composer_unavailable",
            "Composition is not configured on this server. Nothing was changed.",
        ));
    }
    // The composer route's own per-member limit, shared with the canvas.
    let key = format!("compose:{}:{}", member.organization(), member.principal());
    if state
        .auth()
        .limits
        .admit(&[(&key, composer::routes::COMPOSE_MEMBER)])
        .is_err()
    {
        return Err(Output::error(
            "too_many_requests",
            "Too many compositions in the last hour. Try again later.",
        ));
    }
    let composed = composer::routes::compose_for(stores, &state.composer, member, &intent)
        .await
        .map_err(|refused| match refused {
            Refused::NotFound => Output::not_found(),
            Refused::StoreUnavailable => Output::unavailable(),
            Refused::ComposerUnavailable => Output::error(
                "composer_unavailable",
                "Composition is not configured on this server. Nothing was changed.",
            ),
            Refused::CouldNotCompose(_) => Output::error(
                "could_not_compose",
                "That could not be composed into an admitted app. Nothing was placed.",
            ),
        })?;
    attribute(stores, caller, &composed.app, &composed.work, None).await;
    let origin = composed.origin.as_str();
    Ok(Output::ok(
        "ok",
        format!(
            "Placed \"{}\" on your canvas ({origin}). Open it at {}.",
            composed.title,
            link(state, &composed.app, &composed.work)
        ),
        json!({
            "app_id": composed.app, "work_id": composed.work, "title": composed.title,
            "origin": origin, "link": link(state, &composed.app, &composed.work),
        }),
    ))
}

/// Records the agent label for the "via" chip; a failure is logged only.
async fn attribute(
    stores: &Stores,
    caller: &AgentCaller,
    app: &str,
    work: &str,
    record: Option<(&str, i64)>,
) {
    if let Err(error) = stores
        .workspace
        .attribute_to_agent(caller, app, work, record)
        .await
    {
        tracing::warn!(
            event = "mcp.attribution_failed",
            token_id = caller.token_id,
            error = %error,
            "agent attribution not recorded"
        );
    }
}

// --- forma_draft_record / forma_draft_decision ---

/// The member's record catalog, or the uniform refusal for a member who may
/// act on nothing (as `POST /v1/act`).
async fn catalog_of(
    stores: &Stores,
    caller: &AgentCaller,
) -> Result<(Vec<storage::LibraryApp>, Vec<CatalogApp>), Output> {
    let context = stores
        .workspace
        .composer_context(&caller.request)
        .await
        .map_err(storage_output)?;
    let catalog = prompt::catalog(&context.library);
    if catalog.is_empty() {
        return Err(Output::not_found());
    }
    Ok((context.library, catalog))
}

/// The catalog app an agent's hint names: its id, its title or its schema
/// (case-insensitive), else the one app whose title contains the hint.
/// Anything else (a foreign organization's app id among them) is nothing.
fn app_for_hint<'a>(catalog: &'a [CatalogApp], hint: &str) -> Option<&'a CatalogApp> {
    let wanted = hint.trim().to_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let exact = catalog.iter().find(|app| {
        app.app == wanted
            || app.title.to_lowercase() == wanted
            || app.schema == wanted
            || app.schema.replace('_', " ") == wanted
    });
    if exact.is_some() {
        return exact;
    }
    let mut contains = catalog.iter().filter(|app| {
        let title = app.title.to_lowercase();
        title.contains(&wanted) || (wanted.len() >= 4 && wanted.contains(&title))
    });
    match (contains.next(), contains.next()) {
        (Some(app), None) => Some(app),
        _ => None,
    }
}

/// The agent's values for a create, judged before resolution: every key a
/// field the create command writes, each value a bounded string, number or
/// boolean, and the workflow field (status) at most its starting state.
/// Returns them as the parser's text hints.
fn create_hints(app: &CatalogApp, values: &Value) -> Result<Map<String, Value>, Output> {
    let object = values
        .as_object()
        .ok_or_else(|| Output::invalid("`values` must be an object of field id to value."))?;
    let create = app
        .create_command()
        .ok_or_else(|| Output::error("could_not_draft", "This app does not file records."))?;
    if object.len() > storage::MAX_COMMAND_VALUES {
        return Err(Output::invalid("Too many values."));
    }
    let starting = app
        .decide_command()
        .and_then(|d| d.transitions.first().cloned());
    let mut hints = Map::new();
    for (field, value) in object {
        if !storage::valid_local_id(field) || !create.writable.contains(field) {
            return Err(Output::invalid(&format!(
                "`{}` is not a field this app's records accept.",
                field.chars().take(64).collect::<String>()
            )));
        }
        let text = match value {
            Value::String(s) => s.trim().to_owned(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => continue,
            _ => {
                return Err(Output::invalid(&format!(
                    "`{field}` must be a string, number or boolean."
                )))
            }
        };
        if text.chars().count() > MAX_TEXT || text.chars().any(char::is_control) {
            return Err(Output::invalid(&format!(
                "`{field}` must be at most {MAX_TEXT} characters on one line."
            )));
        }
        if let Some((workflow, from, _)) = &starting {
            if field == workflow && text.to_lowercase() != *from {
                return Err(Output::invalid(&format!(
                    "`{field}` is set by the app's workflow: a new record starts as \"{from}\"."
                )));
            }
        }
        if !text.is_empty() {
            hints.insert(field.clone(), Value::String(text));
        }
    }
    Ok(hints)
}

fn could_not_draft(code: &str) -> Output {
    let message = match code {
        "missing_amount" => "An amount is required. Nothing was drafted.",
        "missing_text" => "A required text field is missing. Nothing was drafted.",
        "untypable_value" | "refused_by_schema" | "no_enum_value" => {
            "A value does not fit its field under the app's schema. Nothing was drafted."
        }
        "no_pending_record" => {
            "No pending record you may decide matches that. Nothing was drafted."
        }
        "no_transition" => "That decision is not possible for this record. Nothing was drafted.",
        "no_record_write" => {
            "You may read but not change records of this app. Nothing was drafted."
        }
        _ => "That could not be turned into a record. Nothing was drafted.",
    };
    Output::error("could_not_draft", message)
}

/// Resolves, dry-runs and holds a draft; the answer shows the typed values.
async fn hold(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    library: &[storage::LibraryApp],
    catalog: &[CatalogApp],
    parsed: &Parsed,
    context: &str,
) -> ToolResult {
    let resolution = resolve::resolve(
        stores,
        &caller.request,
        library,
        catalog,
        parsed,
        context,
        &crate::events::today(),
    )
    .await
    .map_err(|refusal| match refusal {
        Refusal::NotFound => Output::not_found(),
        Refusal::StoreUnavailable => Output::unavailable(),
        Refusal::CouldNotDraft(code) => {
            tracing::info!(
                event = "mcp.could_not_draft",
                token_id = caller.token_id,
                code,
                "draft refused"
            );
            could_not_draft(code)
        }
    })?;
    let Resolution::Draft(draft) = resolution else {
        return Err(could_not_draft("not_a_record_action"));
    };
    let draft: Draft = *draft;
    if draft.placed {
        attribute(stores, caller, &draft.app, &draft.work, None).await;
    }
    let structured = draft_document(state, &draft);
    let id = state.drafts.hold(caller, draft.clone()).ok_or_else(|| {
        Output::error(
            "too_many_drafts",
            "Too many open drafts. Commit or let some expire first.",
        )
    })?;
    let mut structured = structured;
    structured["draft_id"] = json!(id);
    let values: Vec<String> = draft
        .values
        .iter()
        .map(|v| format!("{} {}", v.label, v.display))
        .collect();
    let approver = draft
        .approver
        .as_ref()
        .map(|a| format!(" Approver: {}.", a.label))
        .unwrap_or_default();
    let target = draft
        .target
        .as_ref()
        .map(|t| format!(" Record: {} (v{}).", t.summary, t.version))
        .unwrap_or_default();
    // APP-107: the declared rule behind the status, and the PO match.
    let mut notes: Vec<String> = draft.rule.iter().cloned().collect();
    notes.extend(
        draft
            .flags
            .iter()
            .filter_map(|f| f.get("label").and_then(Value::as_str).map(str::to_owned)),
    );
    let target = if notes.is_empty() {
        target
    } else {
        format!("{target} {}.", notes.join("; "))
    };
    Ok(Output::ok(
        "drafted",
        format!(
            "Draft {id} ({}, not written yet): {}.{target}{approver} Ask the user to confirm, then call forma_commit with this draft_id. It expires in 10 minutes.",
            draft.title,
            values.join(" · ")
        ),
        structured,
    ))
}

fn draft_document(state: &Mcp, draft: &Draft) -> Value {
    json!({
        "action": draft.kind.as_str(),
        "app_id": draft.app, "work_id": draft.work, "title": draft.title,
        "placed": draft.placed,
        "record_id": draft.request.record,
        "values": draft.values.iter().map(|v| json!({"field_id": v.field, "label": v.label, "display": v.display})).collect::<Vec<_>>(),
        "approver": draft.approver.as_ref().map(|a| json!({"label": a.label})),
        "approvers": draft.approvers.iter().map(|a| json!({"label": a.label})).collect::<Vec<_>>(),
        "rule": draft.rule,
        "flags": draft.flags,
        "target": draft.target.as_ref().map(|t| json!({"record_id": t.record_id, "version": t.version, "summary": t.summary})),
        "expires_in_seconds": drafts::DRAFT_TTL.as_secs(),
        "written": false,
        "link": link(state, &draft.app, &draft.work),
    })
}

async fn draft_record(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(arguments, &["app_hint", "values", "approver_name"])
        .ok_or_else(|| Output::invalid("Allowed arguments: app_hint, values, approver_name."))?;
    let hint = text_arg(&arguments, "app_hint", 120)?
        .ok_or_else(|| Output::invalid("`app_hint` is required."))?;
    let approver = text_arg(&arguments, "approver_name", 120)?;
    let values = arguments
        .get("values")
        .ok_or_else(|| Output::invalid("`values` is required."))?;
    let (library, catalog) = catalog_of(stores, caller).await?;
    let app = app_for_hint(&catalog, &hint).ok_or_else(Output::not_found)?;
    let hints = create_hints(app, values)?;
    let parsed = Parsed {
        kind: Kind::Create,
        app_hint: app.app.clone(),
        values: hints,
        approver_name: approver,
        target_record_hint: None,
    };
    let context = format!("Filed through {}: {}", caller.label, app.title);
    hold(state, stores, caller, &library, &catalog, &parsed, &context).await
}

async fn draft_decision(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(arguments, &["target_hint", "decision", "note"])
        .ok_or_else(|| Output::invalid("Allowed arguments: target_hint, decision, note."))?;
    let target = text_arg(&arguments, "target_hint", 120)?
        .ok_or_else(|| Output::invalid("`target_hint` is required."))?;
    let decision = match arguments.get("decision").and_then(Value::as_str) {
        Some("approve") => "approved",
        Some("reject") => "rejected",
        _ => {
            return Err(Output::invalid(
                "`decision` must be \"approve\" or \"reject\".",
            ))
        }
    };
    let note = text_arg(&arguments, "note", 200)?;
    let (library, catalog) = catalog_of(stores, caller).await?;
    let mut values = Map::new();
    values.insert("status".into(), Value::String(decision.into()));
    if let Some(note) = note {
        values.insert("note".into(), Value::String(note));
    }
    let parsed = Parsed {
        kind: Kind::Decide,
        app_hint: String::new(),
        values,
        approver_name: None,
        target_record_hint: Some(target),
    };
    let context = format!("Decided through {}", caller.label);
    hold(state, stores, caller, &library, &catalog, &parsed, &context).await
}

// --- forma_commit ---

async fn commit(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(arguments, &["draft_id"])
        .ok_or_else(|| Output::invalid("Allowed arguments: draft_id."))?;
    let id = arguments
        .get("draft_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let refused = || {
        Output::error(
            "draft_not_found",
            "That draft does not exist or has expired. Draft it again.",
        )
    };
    let held = state.drafts.get(caller, id).ok_or_else(refused)?;
    let draft = held.draft;
    let result = crate::commands::routes::commit(stores, &caller.request, &draft.request).await;
    let outcome = match result {
        Ok(CommandResult::Committed(outcome)) | Ok(CommandResult::Replayed(outcome)) => outcome,
        Ok(CommandResult::Conflict(_)) | Ok(CommandResult::Stale(_)) => return Err(Output::error(
            "conflict",
            "The record or the app changed since the draft. Nothing was written; draft it again.",
        )),
        Ok(CommandResult::InFlight) | Ok(CommandResult::Busy) => {
            return Err(Output::error(
                "busy",
                "Forma is busy with this record. Try again in a moment.",
            ))
        }
        Err(storage::Error::Denied) => return Err(Output::not_found()),
        Err(storage::Error::UnknownOutcome) => {
            return Err(Output::error(
                "unknown_outcome",
                "The outcome is unknown; committing the same draft again is safe.",
            ))
        }
        Err(_) => return Err(Output::unavailable()),
    };
    // The draft's first commit, decided atomically (a concurrent commit of
    // the same draft replays the record and does not attribute or share).
    let first = state.drafts.spend(caller, id);
    let mut shared_with = Value::Null;
    let mut share_note = String::new();
    if first {
        attribute(
            stores,
            caller,
            &outcome.app,
            &outcome.work,
            Some((&outcome.record_id, outcome.record_version)),
        )
        .await;
        // APP-107: every approver named (a two-level workflow's second too).
        let mut shared_labels: Vec<String> = Vec::new();
        for approver in &draft.approvers {
            match stores
                .workspace
                .share_work(
                    &caller.request,
                    &draft.app,
                    &draft.work,
                    &approver.principal,
                    true,
                )
                .await
            {
                Ok(shared) => {
                    if shared.changed() {
                        events::hub().app_shared(
                            caller.request.organization(),
                            &draft.app,
                            &draft.work,
                        );
                    }
                    if shared_labels.is_empty() {
                        shared_with = json!(approver.label);
                    }
                    shared_labels.push(approver.label.clone());
                    share_note =
                        format!(" Shared with {} for approval.", shared_labels.join(" and "));
                }
                Err(error) => {
                    tracing::info!(event = "mcp.share_refused", token_id = caller.token_id, error = %error, "share after commit refused");
                    share_note = " It could not be shared with the approver.".into();
                }
            }
        }
    }
    let state_label = if first { "committed" } else { "replayed" };
    Ok(Output::ok(
        state_label,
        format!(
            "{} record v{} (commit {}, audit {}).{share_note} Open it at {}.",
            if first { "Filed" } else { "Already filed:" },
            outcome.record_version,
            outcome.commit_sequence,
            outcome.audit_event_id.get(..8).unwrap_or(""),
            link(state, &outcome.app, &outcome.work)
        ),
        json!({
            "state": state_label,
            "app_id": outcome.app, "work_id": outcome.work,
            "record_id": outcome.record_id, "record_version": outcome.record_version,
            "commit_sequence": outcome.commit_sequence, "audit_event_id": outcome.audit_event_id,
            "shared_with": shared_with,
            "link": link(state, &outcome.app, &outcome.work),
        }),
    ))
}

// --- forma_list_pending ---

async fn list_pending(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    closed(arguments, &[]).ok_or_else(|| Output::invalid("This tool takes no arguments."))?;
    let member = &caller.request;
    let works = stores
        .data
        .openable_works(member)
        .await
        .map_err(storage_output)?;
    let mut pending = Vec::new();
    for work in works.iter().filter(|w| w.can_write).take(MAX_PENDING_WORKS) {
        let listing = match stores
            .data
            .list_records(member, &work.app, &work.work)
            .await
        {
            Ok(listing) => listing,
            Err(storage::Error::Denied) => continue,
            Err(error) => return Err(storage_output(error)),
        };
        if !listing.can_write {
            continue;
        }
        for collection in &listing.collections {
            let Some(decide) = collection
                .commands
                .iter()
                .find(|c| !c.transitions.is_empty())
            else {
                continue;
            };
            let field = decide.transitions[0].0.clone();
            let froms: Vec<&str> = decide
                .transitions
                .iter()
                .filter(|(f, _, _)| *f == field)
                .map(|(_, from, _)| from.as_str())
                .collect();
            for record in listing.records.iter().filter(|r| {
                r.collection == collection.collection && r.creator != member.principal()
            }) {
                let status = record
                    .values
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|v| v["field_id"] == field.as_str())
                    .and_then(|v| v["value"]["value"].as_str())
                    .unwrap_or("");
                if !froms.contains(&status) || pending.len() >= MAX_PENDING {
                    continue;
                }
                pending.push(json!({
                    "app_id": work.app, "work_id": work.work, "app_title": work.title,
                    "record_id": record.record_id, "version": record.version,
                    "summary": resolve::summarize(record),
                    "status": status,
                    "filed_by": directory::label(&record.creator),
                    "values": record.values.as_array().into_iter().flatten().filter_map(|v| Some(json!({
                        "field_id": v.get("field_id")?, "display": resolve::display(v.get("value")?),
                    }))).collect::<Vec<_>>(),
                    "link": link(state, &work.app, &work.work),
                }));
            }
        }
    }
    let summary = if pending.is_empty() {
        "Nothing is waiting for your decision.".to_owned()
    } else {
        let lines: Vec<String> = pending
            .iter()
            .map(|p| {
                format!(
                    "{} — filed by {}",
                    p["summary"].as_str().unwrap_or(""),
                    p["filed_by"].as_str().unwrap_or("")
                )
            })
            .collect();
        format!(
            "{} waiting for your decision: {}",
            pending.len(),
            lines.join("; ")
        )
    };
    Ok(Output::ok("ok", summary, json!({"pending": pending})))
}

// --- forma_share ---

async fn share(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let arguments = closed(
        arguments,
        &["app_id", "work_id", "member_name", "record_write"],
    )
    .ok_or_else(|| {
        Output::invalid("Allowed arguments: app_id, work_id, member_name, record_write.")
    })?;
    let app = id_arg(&arguments, "app_id")?;
    let work = id_arg(&arguments, "work_id")?;
    let name = text_arg(&arguments, "member_name", 120)?
        .ok_or_else(|| Output::invalid("`member_name` is required."))?;
    let record_write = match arguments.get("record_write") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(Output::invalid("`record_write` must be a boolean.")),
    };
    let member = &caller.request;
    let members = stores
        .workspace
        .members(member)
        .await
        .map_err(storage_output)?;
    // No teammate of that name is the uniform denial too (a member of
    // another organization is never named).
    let grantee = resolve::match_approver(&name, &members).ok_or_else(Output::not_found)?;
    // The canvas route's own path, the composition's adaptation included.
    let shared = crate::sharing::share_with_lineage(
        stores,
        member,
        &app,
        &work,
        &grantee.principal,
        record_write,
    )
    .await
    .map_err(storage_output)?;
    Ok(Output::ok(
        "ok",
        format!(
            "Shared with {}. It appears on their canvas live.",
            grantee.label
        ),
        json!({
            "app_id": app, "work_id": work, "shared_with": grantee.label,
            "granted": shared.granted.iter().map(|g| json!({"scope": g.scope, "resource_id": g.resource, "permission": g.permission})).collect::<Vec<_>>(),
            "link": link(state, &app, &work),
        }),
    ))
}

#[cfg(test)]
pub(crate) fn app_for_hint_for_tests<'a>(
    catalog: &'a [CatalogApp],
    hint: &str,
) -> Option<&'a CatalogApp> {
    app_for_hint(catalog, hint)
}
#[cfg(test)]
pub(crate) fn create_hints_for_tests(
    app: &CatalogApp,
    values: &Value,
) -> Result<Map<String, Value>, Output> {
    create_hints(app, values)
}

// --- APP-108: folders ---

fn folder_ref(text: &str) -> crate::boxes::intent::FolderRef {
    match text.trim().trim_start_matches('#').parse::<usize>() {
        Ok(n) => crate::boxes::intent::FolderRef::Number(n),
        Err(_) => crate::boxes::intent::FolderRef::Name(text.trim().to_owned()),
    }
}

async fn resolve_folder(
    caller: &AgentCaller,
    arguments: &Value,
) -> Result<
    (
        std::sync::Arc<crate::boxes::routes::Boxes>,
        String,
        Map<String, Value>,
    ),
    Output,
> {
    let state = crate::boxes::intent::installed().ok_or_else(Output::unavailable)?;
    let arguments = closed(arguments, &["folder", "instruction"]).ok_or_else(|| {
        Output::invalid("Give `folder` (number or name) and, for formation, `instruction`.")
    })?;
    let folder = text_arg(&arguments, "folder", 60)?
        .ok_or_else(|| Output::invalid("`folder` is required."))?;
    match crate::boxes::intent::resolve(&state, &caller.request, &folder_ref(&folder)).await {
        crate::boxes::intent::Resolved::One(id, _) => Ok((state, id, arguments)),
        crate::boxes::intent::Resolved::Many(choices) => Err(Output::invalid(&format!(
            "Several folders match: {}. Name one by number.",
            choices
                .iter()
                .map(|(n, _, name)| format!("{n}. {name}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
        crate::boxes::intent::Resolved::None => Err(Output::not_found()),
    }
}

async fn list_folders(_state: &Mcp, caller: &AgentCaller, arguments: &Value) -> ToolResult {
    closed(arguments, &[]).ok_or_else(|| Output::invalid("No arguments."))?;
    let boxes = crate::boxes::intent::installed().ok_or_else(Output::unavailable)?;
    let rows = crate::boxes::routes::numbered(&boxes, &caller.request)
        .await
        .map_err(storage_output)?;
    let mut folders = Vec::new();
    for (number, row) in rows.iter().take(50) {
        let view = crate::boxes::routes::box_view_json(&boxes, &caller.request, &row.box_id)
            .await
            .map_err(storage_output)?;
        let files: Vec<Value> = view["files"].as_array().into_iter().flatten()
            .map(|f| json!({"name": f["name"], "type": f["kind"], "data_registered": !f["data"].is_null()})).collect();
        folders.push(json!({"number": number, "folder_id": row.box_id, "name": row.name, "file_count": row.file_count, "files": files}));
    }
    let summary = if folders.is_empty() {
        "No folders yet.".to_owned()
    } else {
        rows.iter()
            .map(|(n, r)| format!("{n}. {} ({} files)", r.name, r.file_count))
            .collect::<Vec<_>>()
            .join("; ")
    };
    Ok(Output::ok("ok", summary, json!({"folders": folders})))
}

async fn folder_formation(
    state: &Mcp,
    stores: &Stores,
    caller: &AgentCaller,
    arguments: &Value,
) -> ToolResult {
    let (boxes, box_id, arguments) = resolve_folder(caller, arguments).await?;
    let instruction = text_arg(&arguments, "instruction", MAX_TEXT)?
        .ok_or_else(|| Output::invalid("`instruction` is required."))?;
    match crate::boxes::intent::ask(&instruction) {
        Some(crate::boxes::intent::Ask::Brief) => {
            // The brief route's own per-member window, shared with the canvas.
            crate::boxes::routes::admit_explain(&boxes, &caller.request).map_err(folder_refused)?;
            let brief = crate::boxes::brief::generate(&boxes, &caller.request, &box_id)
                .await
                .map_err(folder_refused)?;
            Ok(Output::ok(
                "ok",
                "Folder brief ready (numbers from receipts, PDF statements cited by page).".into(),
                json!({"brief": brief}),
            ))
        }
        _ => {
            // The composer's per-member window, shared with the canvas.
            crate::boxes::routes::admit_compose(&boxes, &caller.request).map_err(folder_refused)?;
            let purpose = crate::boxes::intent::purpose(&instruction);
            let built = crate::boxes::routes::build_from_box(
                &boxes,
                &caller.request,
                &box_id,
                &purpose,
                None,
            )
            .await
            .map_err(|r| match r {
                crate::boxes::routes::BuildRefusal::NotFound => Output::not_found(),
                crate::boxes::routes::BuildRefusal::NoData => {
                    Output::invalid("This folder has no CSV or XLSX data to build from.")
                }
                crate::boxes::routes::BuildRefusal::CouldNotCompose(_) => Output::invalid(
                    "Forma could not compose an app from this folder; rephrase the purpose.",
                ),
                other => folder_refused(other),
            })?;
            attribute(stores, caller, &built.placed.app, &built.placed.work, None).await;
            let mut value = crate::boxes::routes::built_json(&built);
            value["link"] = json!(link(state, &built.placed.app, &built.placed.work));
            let summary = format!(
                "{} ({}, {}). {}",
                built.placed.title,
                built.origin,
                value["label"].as_str().unwrap_or(""),
                value["link"].as_str().unwrap_or("")
            );
            Ok(Output::ok("ok", summary, value))
        }
    }
}

async fn folder_brief(caller: &AgentCaller, arguments: &Value) -> ToolResult {
    let (boxes, box_id, _) = resolve_folder(caller, arguments).await?;
    crate::boxes::routes::admit_explain(&boxes, &caller.request).map_err(folder_refused)?;
    let brief = crate::boxes::brief::generate(&boxes, &caller.request, &box_id)
        .await
        .map_err(folder_refused)?;
    Ok(Output::ok(
        "ok",
        "Folder brief ready (numbers from receipts, PDF statements cited by page).".into(),
        json!({"brief": brief}),
    ))
}

/// A folder refusal as a tool error (the folder routes' windows included).
fn folder_refused(refused: crate::boxes::routes::BuildRefusal) -> Output {
    match refused {
        crate::boxes::routes::BuildRefusal::NotFound => Output::not_found(),
        crate::boxes::routes::BuildRefusal::TooMany(_) => Output::error(
            "too_many_requests",
            "Too many folder requests in the last hour. Try again later.",
        ),
        _ => Output::unavailable(),
    }
}
