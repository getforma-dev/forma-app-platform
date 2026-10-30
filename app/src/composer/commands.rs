//! APP-101 slash commands: the explicit lane of the canvas intent bar,
//! instant and never a guess.
//!
//! `GET /v1/commands` lists the member's commands: every library app the
//! member may compose now (`Workspace::composer_context`) with the slug and
//! aliases the library sidecar names (`contracts/make_library.py`
//! `COMMANDS`), its title and one-line purpose and whether it keeps records
//! (the action path's catalog); and every named saved work the member may
//! open now (`Data::named_works`, the workspace listing's clause). Nothing
//! else is listed and nothing is written. The canvas resolves `/slug rest`
//! from this list and posts the pinned compose (`{"intent", "app_id"}`), the
//! pinned action (`{"utterance", "app_id"}`) or opens a named work directly.
//!
//! `POST /v1/work/{app}/{work}/name {"slug"}` names a work the member may
//! open: a new saved work of the same publication and bindings is created
//! through the APP-010 `create_work` (its own precheck: `run` on the app,
//! `source_read` on every dependency) whose `context` starts with
//! `storage::COMMAND_MARKER` and the slug. No migration: the slug lives on
//! the saved work's existing `context`. A teammate the named work is shared
//! with sees it in their list (the listing clause). The slug is
//! `[a-z][a-z0-9-]{1,31}`, never `apps`, a library slug or alias, or a name
//! the member already sees (422 `name_taken`).
//!
//! Refusals: the uniform 404 for no session, a work the member may not open
//! and a member storage denies; 403 `cross_site`; 413; 422 `invalid_name`;
//! 503 `unavailable`.
use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    storage::{self, Error, TrustedRequest, WorkBinding, COMMAND_MARKER},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub const COMMANDS_PATH: &str = "/v1/commands";
pub const NAME_PATH: &str = "/v1/work/{app}/{work}/name";
/// Reserved: the canvas's own list command.
pub const RESERVED: &[&str] = &["apps", "help", "new"];
const NAME_BODY_LIMIT: usize = 1024;
/// The context kept after the marker line (`saved_work.context` ≤ 2000).
const CONTEXT_KEPT: usize = 1500;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(COMMANDS_PATH, get(list))
        .route(NAME_PATH, post(name))
        .with_state(auth)
}

/// A command slug's shape: `[a-z][a-z0-9-]{1,31}`, no `--`, no trailing `-`.
pub fn valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && !slug.contains("--")
        && !slug.ends_with('-')
}

fn document(status: StatusCode, value: &Value) -> Response {
    match crate::admission::encode(value) {
        Ok(bytes) => {
            let mut response = (status, bytes).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            http::secure(response.headers_mut());
            response
        }
        Err(_) => http::unavailable(),
    }
}

fn refusal(error: &Error) -> Response {
    match error {
        Error::Denied => http::not_found(),
        _ => http::unavailable(),
    }
}

/// The member's commands as JSON entries (module notes).
async fn commands(stores: &storage::Stores, member: &TrustedRequest) -> Result<Vec<Value>, Error> {
    let context = stores.workspace.composer_context(member).await?;
    let catalog = crate::actions::prompt::catalog(&context.library);
    let mut listed = Vec::new();
    for app in &context.library {
        let Some((slug, aliases)) = super::matcher::command(app) else {
            continue;
        };
        let record = catalog.iter().any(|entry| {
            entry.app == app.app
                && (entry.create_command().is_some() || entry.decide_command().is_some())
        });
        listed.push(json!({
            "slug": slug,
            "aliases": aliases,
            "title": app.title,
            "purpose": super::matcher::purpose(app),
            "kind": "library",
            "app_id": app.app,
            "record": record,
        }));
    }
    for named in stores.data.named_works(member).await? {
        listed.push(json!({
            "slug": named.slug,
            "aliases": [],
            "title": named.work.title,
            "purpose": format!("Saved app: {}", named.work.title),
            "kind": "saved",
            "app_id": named.work.app,
            "work_id": named.work.work,
            "record": !named.work.collections.is_empty(),
        }));
    }
    Ok(listed)
}

/// `GET /v1/commands`.
async fn list(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match commands(stores, &member).await {
        Ok(listed) => document(StatusCode::OK, &json!({"commands": listed})),
        Err(error) => refusal(&error),
    }
}

/// `{"slug": "<slug>"}` and nothing else.
pub fn body_slug(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let slug = object.get("slug")?.as_str()?.trim().trim_start_matches('/');
    valid_slug(slug).then(|| slug.to_owned())
}

/// The bindings of a saved work document (`Workspace::read_work`).
fn bindings(document: &Value) -> Option<Vec<WorkBinding>> {
    document
        .get("bindings")?
        .as_array()?
        .iter()
        .map(|binding| {
            let input = binding.get("input_id")?.as_str()?.to_owned();
            match binding.get("kind")?.as_str()? {
                "source" => Some(WorkBinding::Source {
                    input,
                    source: binding.get("source_id")?.as_str()?.to_owned(),
                    revision: binding.get("source_revision_id")?.as_str()?.to_owned(),
                }),
                "records" => Some(WorkBinding::Records {
                    input,
                    schema: binding.get("schema_id")?.as_str()?.to_owned(),
                }),
                _ => None,
            }
        })
        .collect()
}

/// The organization's naming lock: the slug uniqueness check and the create
/// of the named work run under it, so two concurrent namings in this process
/// cannot both pass the check (review: check-then-create race). Slugs are
/// organization-wide. Across tasks a storage constraint is the full fix
/// (APP-113 carries it; the pitch runs one task).
fn naming_lock(organization: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    > = std::sync::OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks.entry(organization.to_owned()).or_default().clone()
}

/// `POST /v1/work/{app}/{work}/name`.
async fn name(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    if !(storage::valid_identifier(&app) && storage::valid_identifier(&work)) {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry_after) = auth
        .limits
        .admit_member_write(member.organization(), member.principal())
    {
        return http::too_many_requests(retry_after);
    }
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(bytes) = http::bounded(body, NAME_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    let Some(slug) = body_slug(&bytes).filter(|_| parts.uri.query().is_none()) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_name");
    };
    // The source work, under the member's own authority (uniform 404).
    let detail = match stores.workspace.read_work(&member, &app, &work).await {
        Ok(detail) => detail,
        Err(error) => return refusal(&error),
    };
    if RESERVED.contains(&slug.as_str()) || super::matcher::library_slugs().contains(&slug) {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "name_taken");
    }
    let lock = naming_lock(member.organization());
    let _naming = lock.lock().await;
    match stores.data.named_works(&member).await {
        Ok(named) if named.iter().any(|n| n.slug == slug) => {
            return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "name_taken");
        }
        Ok(_) => {}
        Err(error) => return refusal(&error),
    }
    let Ok(document_) = crate::admission::decode(&detail.canonical) else {
        return http::unavailable();
    };
    let Some(bindings) = bindings(&document_) else {
        return http::unavailable();
    };
    let previous = document_
        .get("context")
        .and_then(Value::as_str)
        .unwrap_or("");
    // A named work's own marker line is not carried into the new name.
    let previous = match storage::command_slug(previous) {
        Some(_) => previous.split_once('\n').map_or("", |(_, rest)| rest),
        None => previous,
    };
    let context = format!(
        "{COMMAND_MARKER}{slug}\n{}",
        previous.chars().take(CONTEXT_KEPT).collect::<String>()
    );
    let saved = match stores
        .workspace
        .create_work(&member, &app, &detail.publication, &context, &bindings)
        .await
    {
        Ok(saved) => saved,
        Err(error) => return refusal(&error),
    };
    let title = stores
        .data
        .work_outline(&member, &app, &saved.work)
        .await
        .map(|outline| outline.title)
        .unwrap_or_default();
    tracing::info!(
        event = "composer.named",
        organization_id = member.organization(),
        principal_id = member.principal(),
        app_id = app,
        work_id = saved.work,
        "saved work named as a slash command"
    );
    document(
        StatusCode::OK,
        &json!({"app_id": app, "work_id": saved.work, "slug": slug, "title": title}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_short_lowercase_words() {
        for slug in ["ab", "expense-200", "q4-review", "x1"] {
            assert!(valid_slug(slug), "{slug}");
        }
        for slug in [
            "a",
            "Ab",
            "1ab",
            "a--b",
            "ab-",
            "a b",
            "a/b",
            "<b>",
            &"a".repeat(33),
        ] {
            assert!(!valid_slug(slug), "{slug}");
        }
        assert_eq!(
            body_slug(br#"{"slug":"/big-claims"}"#).as_deref(),
            Some("big-claims")
        );
        assert_eq!(body_slug(br#"{"slug":"big claims"}"#), None);
        assert_eq!(body_slug(br#"{"slug":"ok-name","x":1}"#), None);
        // Library slugs and aliases are reserved.
        let reserved = crate::composer::matcher::library_slugs();
        for slug in ["expense", "expense-review", "spend", "traffic", "mrr"] {
            assert!(reserved.contains(slug), "{slug}");
        }
    }

    #[tokio::test]
    async fn namings_in_one_organization_are_serialized() {
        let org = "00000000-0000-4000-8000-0000000000c1";
        let first = naming_lock(org);
        let held = first.lock().await;
        assert!(
            naming_lock(org).try_lock().is_err(),
            "a second naming waits for the first's check and create"
        );
        assert!(naming_lock("00000000-0000-4000-8000-0000000000c2")
            .try_lock()
            .is_ok());
        drop(held);
        assert!(naming_lock(org).try_lock().is_ok());
    }

    #[test]
    fn named_context_carries_the_marker_line() {
        assert_eq!(
            storage::command_slug("forma-command:/big-claims\nreview expenses"),
            Some("big-claims")
        );
        assert_eq!(storage::command_slug("review expenses"), None);
        let document_ = json!({"bindings": [
            {"input_id": "expense_claims", "kind": "source", "source_id": "s", "source_revision_id": "r"},
            {"input_id": "decisions", "kind": "records", "schema_id": "decision"}]});
        assert_eq!(
            bindings(&document_),
            Some(vec![
                WorkBinding::Source {
                    input: "expense_claims".into(),
                    source: "s".into(),
                    revision: "r".into()
                },
                WorkBinding::Records {
                    input: "decisions".into(),
                    schema: "decision".into()
                },
            ])
        );
    }
}
