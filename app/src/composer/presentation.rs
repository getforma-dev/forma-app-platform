//! One exact saved work's immutable presentation. No action supplies code,
//! URLs, grants or another app's component. Projection only removes targets.
use forma_ir::PageProps;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(PageProps, Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PresentationActionProps {
    pub id: String,
    pub label: String,
    pub behavior: String,
    pub target: String,
}
#[derive(PageProps, Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresentationProps {
    pub has_description: bool,
    pub description: String,
    pub mode: String,
    pub accent: String,
    pub density: String,
    pub font: String,
    pub actions: Vec<PresentationActionProps>,
    pub json: String,
}
impl Default for PresentationProps {
    fn default() -> Self {
        Self {
            has_description: false,
            description: String::new(),
            mode: "default".into(),
            accent: "default".into(),
            density: "default".into(),
            font: "default".into(),
            actions: Vec::new(),
            json: "{}".into(),
        }
    }
}
/// Only call with re-admitted content and the IDs this response actually
/// releases. Guest callers omit forms and all unavailable components.
pub fn project(value: Option<&Value>, allowed: &BTreeSet<String>) -> PresentationProps {
    let Some(value) = value else {
        return PresentationProps::default();
    };
    let mut value = value.clone();
    let text = |value: Option<&Value>| value.and_then(Value::as_str).unwrap_or_default().to_owned();
    if let Some(sections) = value.get_mut("sections").and_then(Value::as_array_mut) {
        for section in sections.iter_mut() {
            if let Some(ids) = section.get_mut("components").and_then(Value::as_array_mut) {
                ids.retain(|id| id.as_str().is_some_and(|id| allowed.contains(id)));
            }
        }
        sections.retain(|s| {
            s["components"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
        });
    }
    if let Some(actions) = value.get_mut("actions").and_then(Value::as_array_mut) {
        actions.retain(|a| a["target"].as_str().is_some_and(|id| allowed.contains(id)));
    }
    let actions = value["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| PresentationActionProps {
            id: text(a.get("id")),
            label: text(a.get("label")),
            behavior: text(a.get("behavior")),
            target: text(a.get("target")),
        })
        .collect();
    let token = |key| {
        value
            .get("theme")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_owned()
    };
    PresentationProps {
        has_description: !text(value.get("description")).is_empty(),
        description: text(value.get("description")),
        mode: token("mode"),
        accent: token("accent"),
        density: token("density"),
        font: token("font"),
        actions,
        json: serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()),
    }
}

use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    storage::{self, Error},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/presentation", get(read).post(save))
        .with_state(auth)
}
fn document(value: Value) -> Response {
    let mut response = (
        StatusCode::OK,
        serde_json::to_vec(&value).unwrap_or_default(),
    )
        .into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    http::secure(response.headers_mut());
    response
}
fn descriptor(value: &storage::WorkPresentation) -> Value {
    let components: Vec<_> = value.content["views"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| json!({"id":v["id"],"title":v["title"],"kind":v["kind"]}))
        .collect();
    json!({"publication_id":value.publication,"definition_digest":value.digest,"work_version":value.version,"can_edit":value.can_edit,"presentation":value.content.get("presentation"),"components":components})
}
async fn read(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if request.uri().query().is_some()
        || !storage::valid_identifier(&app)
        || !storage::valid_identifier(&work)
        || super::matcher::seeded_work(&work)
    {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores
        .workspace
        .work_presentation(&member, &app, &work)
        .await
    {
        Ok(value) => document(descriptor(&value)),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}
fn edit_body(bytes: &[u8]) -> Option<(String, String, i64, Value)> {
    let value = crate::admission::decode(bytes).ok()?;
    let map = value.as_object()?;
    if map.len() != 4 {
        return None;
    }
    let publication = map.get("expected_publication_id")?.as_str()?;
    let digest = map.get("expected_definition_digest")?.as_str()?;
    let version = map.get("expected_work_version")?.as_i64()?;
    if !storage::valid_identifier(publication)
        || !digest.starts_with("sha256:")
        || digest.len() != 71
        || !digest[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !(1..9_007_199_254_740_991).contains(&version)
    {
        return None;
    }
    let presentation = map.get("presentation")?;
    if !presentation.is_object() {
        return None;
    }
    Some((
        publication.to_owned(),
        digest.to_owned(),
        version,
        presentation.clone(),
    ))
}
async fn save(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    if parts.uri.query().is_some()
        || !storage::valid_identifier(&app)
        || !storage::valid_identifier(&work)
        || super::matcher::seeded_work(&work)
    {
        return http::not_found();
    }
    let Some((_, member)) = crate::sharing::session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    if let Err(retry) = auth
        .limits
        .admit_member_write(member.organization(), member.principal())
    {
        return http::too_many_requests(retry);
    }
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    if parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return http::problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    }
    let Some(bytes) = http::bounded(body, 32_768).await else {
        return http::payload_too_large();
    };
    let Some((publication, digest, version, presentation)) = edit_body(&bytes) else {
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_presentation");
    };
    let expected = match stores
        .workspace
        .work_presentation(&member, &app, &work)
        .await
    {
        Ok(value) if value.can_edit => value,
        Ok(_) | Err(Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    if expected.publication != publication
        || expected.digest != digest
        || expected.version != version
    {
        return http::problem(StatusCode::CONFLICT, "version_conflict");
    }
    let mut content = expected.content.clone();
    content["presentation"] = presentation;
    let admitted = match super::plan::admit(&content, member.organization(), &app) {
        Ok(value) => value,
        Err(_) => return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_presentation"),
    };
    if admitted.digest == expected.digest {
        return document(descriptor(&expected));
    }
    let result = async {
        let revision = stores
            .workspace
            .create_definition_revision(&member, &app, &admitted)
            .await?;
        let published = stores
            .control
            .publish(&member, &app, &revision.revision, &expected.policy)
            .await?;
        let storage::PublishResult::Published(published) = published else {
            return Ok(None);
        };
        if !stores
            .workspace
            .adopt_presentation(&member, &app, &work, &expected, &published.publication_id)
            .await?
        {
            return Ok(None);
        }
        stores
            .workspace
            .work_presentation(&member, &app, &work)
            .await
            .map(Some)
    }
    .await;
    match result {
        Ok(Some(value)) => document(descriptor(&value)),
        Ok(None) => http::problem(StatusCode::CONFLICT, "version_conflict"),
        Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guest_projection_drops_forms_actions_and_sections_without_inventing_targets() {
        let value = json!({"version":1,"description":"Safe text","sections":[{"id":"data","components":["ledger"]},{"id":"edit","components":["add"]}],"actions":[{"id":"create","label":"Add","behavior":"dialog","target":"add"},{"id":"browse","label":"Browse","behavior":"navigate","target":"ledger"}],"theme":{"mode":"dark","accent":"violet","density":"compact","font":"mono"}});
        let projected = project(Some(&value), &BTreeSet::from(["ledger".into()]));
        let json: Value = serde_json::from_str(&projected.json).unwrap();
        assert_eq!(projected.actions.len(), 1);
        assert_eq!(projected.actions[0].target, "ledger");
        assert_eq!(json["sections"].as_array().unwrap().len(), 1);
        assert_eq!(projected.font, "mono");
        assert_eq!(value["actions"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn edit_body_is_closed_and_has_exact_work_premises() {
        let mut v = json!({"expected_publication_id":"00000000-0000-4000-8000-000000000001","expected_definition_digest":format!("sha256:{}","a".repeat(64)),"expected_work_version":1,"presentation":{"version":1}});
        assert!(edit_body(&serde_json::to_vec(&v).unwrap()).is_some());
        v["owner"] = json!("me");
        assert!(edit_body(&serde_json::to_vec(&v).unwrap()).is_none());
    }
}
