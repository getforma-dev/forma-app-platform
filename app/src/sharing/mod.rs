//! APP-095 (ADR-011 pitch, `pivot/realtime`): the share route and the members
//! list. Storage decides (`storage::sharing`); these routes carry a session,
//! a closed JSON body and the uniform answers.
//!
//! - `GET /v1/members`: `{"members":[{"principal_id":"<uuid>","label":"Member
//!   <8 hex>"}, …]}`, the other active members of the caller's organization
//!   (labels only; no provider identity; ADR-011 debt: 0002 principals carry
//!   no display name).
//! - `POST /v1/work/{app}/{work}/share`: body `{"grantee_principal_id":
//!   "<uuid>","record_write":false}` (`record_write` optional, default false;
//!   no other member), `application/json`, at most `JSON_BODY_LIMIT`. 200
//!   `{"app_id","work_id","grantee_principal_id","granted":[{"scope":"app"|
//!   "work"|"source","resource_id":"<uuid>","permission":"<name>","inserted":
//!   bool}, …]}`. Refusal order: fetch metadata (403 `cross_site`), the ids
//!   (uniform 404), the session (uniform 404), the body (413, then 400
//!   `malformed_share`), then storage: `Denied` is the uniform 404 (a work the
//!   sharer cannot open, no authority to share, a `record_write` the sharer
//!   does not hold, a grantee who is not an active member of the same
//!   organization or is the sharer; identical for existing and nonexistent
//!   works), 503 for a store outage. A share that wrote a grant announces
//!   `app.shared` on the event hub (`events`).
use crate::{
    auth::{cookies, csrf, Auth},
    events,
    http::{self, UniformPath},
    storage::{self, Lookup, TrustedRequest},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/v1/members", get(members))
        .route("/v1/work/{app}/{work}/share", post(share))
        .with_state(auth)
}

/// The live workspace session behind the request, or nothing (uniform 404);
/// shared with the event stream route.
pub(crate) async fn session(auth: &Auth, headers: &HeaderMap) -> Option<(String, TrustedRequest)> {
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

/// The closed share body: the grantee and the optional `record_write`.
pub(crate) fn parse_share(bytes: &[u8]) -> Option<(String, bool)> {
    let body = crate::admission::decode(bytes).ok()?;
    let object = body.as_object()?;
    if !object
        .keys()
        .all(|k| matches!(k.as_str(), "grantee_principal_id" | "record_write"))
    {
        return None;
    }
    let grantee = object.get("grantee_principal_id")?.as_str()?;
    if !storage::valid_identifier(grantee) {
        return None;
    }
    let record_write = match object.get("record_write") {
        None => false,
        Some(value) => value.as_bool()?,
    };
    Some((grantee.to_owned(), record_write))
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

async fn members(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let Some((_, trusted)) = session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.members(&trusted).await {
        Ok(members) => document(
            StatusCode::OK,
            &json!({"members": members.iter().map(|m| json!({"principal_id": m.principal, "label": m.label})).collect::<Vec<_>>()}),
        ),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn share(
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
    let Some((_, trusted)) = session(&auth, &parts.headers).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(bytes) = http::bounded(body, http::JSON_BODY_LIMIT).await else {
        return http::payload_too_large();
    };
    if content_type(&parts.headers) != "application/json" {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_share");
    }
    let Some((grantee, record_write)) = parse_share(&bytes) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_share");
    };
    let result = stores
        .workspace
        .share_work(&trusted, &app, &work, &grantee, record_write)
        .await;
    tracing::info!(
        outcome = match &result {
            Ok(shared) if shared.changed() => "shared",
            Ok(_) => "unchanged",
            Err(storage::Error::Denied) => "denied",
            Err(_) => "unavailable",
        },
        "share.submit"
    );
    match result {
        Ok(shared) => {
            if shared.changed() {
                events::hub().app_shared(trusted.organization(), &app, &work);
            }
            document(
                StatusCode::OK,
                &json!({
                    "app_id": app,
                    "work_id": work,
                    "grantee_principal_id": shared.grantee,
                    "granted": shared.granted.iter().map(|g| json!({
                        "scope": g.scope, "resource_id": g.resource,
                        "permission": g.permission, "inserted": g.inserted,
                    })).collect::<Vec<_>>(),
                }),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use tower::ServiceExt as _;

    const GRANTEE: &str = "00000000-0000-4000-8000-00000000006d";

    #[test]
    fn share_body_is_closed() {
        let ok = format!("{{\"grantee_principal_id\":\"{GRANTEE}\"}}");
        assert_eq!(parse_share(ok.as_bytes()), Some((GRANTEE.into(), false)));
        let write = format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"record_write\":true}}");
        assert_eq!(parse_share(write.as_bytes()), Some((GRANTEE.into(), true)));
        for bad in [
            String::from("{}"),
            String::from("[]"),
            String::from("not json"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"permission\":\"admin_policy\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"record_write\":\"yes\"}}"),
            String::from("{\"grantee_principal_id\":\"not-an-id\"}"),
            format!(
                "{{\"grantee_principal_id\":\"{}\"}}",
                GRANTEE.to_uppercase()
            ),
        ] {
            assert_eq!(parse_share(bad.as_bytes()), None, "{bad}");
        }
    }

    /// Without a configured store every protected answer is the uniform 404
    /// (and the cross-site refusal comes first on the POST).
    #[tokio::test]
    async fn routes_without_a_session_are_the_uniform_404() {
        let auth =
            Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
        let app = http::uniform(router(auth));
        let send = |method: &str, uri: &str, headers: &[(&str, &str)]| {
            let app = app.clone();
            let mut request = axum::http::Request::builder().method(method).uri(uri);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let request = request.body(Body::from("{}")).unwrap();
            async move {
                let response = app.oneshot(request).await.unwrap();
                let status = response.status();
                let body = to_bytes(response.into_body(), 1024).await.unwrap();
                (status, String::from_utf8(body.to_vec()).unwrap())
            }
        };
        let uri = format!("/v1/work/{GRANTEE}/{GRANTEE}/share");
        assert_eq!(
            send("GET", "/v1/members", &[]).await,
            (
                StatusCode::NOT_FOUND,
                "{\"code\":\"not_found\",\"status\":404}".into()
            )
        );
        assert_eq!(
            send("POST", &uri, &[("sec-fetch-site", "cross-site")])
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send("POST", &uri, &[("sec-fetch-site", "same-origin")])
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            send(
                "POST",
                "/v1/work/x/y/share",
                &[("sec-fetch-site", "same-origin")]
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
}
