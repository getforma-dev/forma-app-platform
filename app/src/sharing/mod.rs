//! APP-095 (ADR-011 pitch, `pivot/realtime`): the share route and the members
//! list. Storage decides (`storage::sharing`); these routes carry a session,
//! a closed JSON body and the uniform answers.
//!
//! - `GET /v1/members`: `{"members":[{"principal_id":"<uuid>","label":"Member
//!   <8 hex>"}, …]}`, the other active members of the caller's organization
//!   (labels only; no provider identity; ADR-011 debt: 0002 principals carry
//!   no display name).
//! - `POST /v1/work/{app}/{work}/share`: body `{"grantee_principal_id":
//!   "<uuid>","role":"viewer"|"contributor"|"reviewer"|"none"}` (APP-192:
//!   both members required, no other member; the earlier `record_write` flag
//!   is refused, so a role has one encoding), `application/json`, at most
//!   `JSON_BODY_LIMIT`. 200 `{"app_id","work_id","grantee_principal_id",
//!   "granted":[{"scope":"app"|"work"|"source","resource_id":"<uuid>",
//!   "permission":"<name>"}, …]}`: the sharer's subset now held by the
//!   teammate, the same shape whether this call inserted rows or found them
//!   (review m1: the sharer learns nothing of the teammate's prior holdings).
//!   A lower role than the teammate holds takes the surplus away (W4, f1).
//!   `none` removes the teammate's access to the work
//!   (`Workspace::revoke_work_share`: the app's owner alone; anyone else,
//!   the owner naming themselves and a nonexistent work get the uniform 404
//!   before any standing is judged): 200 `{"app_id","work_id",
//!   "grantee_principal_id","role":"none"}`, the same body whether anything
//!   was held.
//! - `POST /v1/work/{app}/{work}/share/organization` (APP-192, D9): body
//!   `{"role":"viewer"|"contributor"|"reviewer"|"none"}`: shares the work with
//!   every current and future active member of the organization in that role
//!   (replacing a live one of another role), or `none` ends it for all of
//!   them at once. 200 `{"app_id","work_id","role":"<role>"|"none"}`.
//! - `GET /v1/work/{app}/{work}/share` (APP-192): what the share dialog shows
//!   a member with share authority: `{"app_id","work_id","roles":[…],
//!   "can_revoke_members":bool,"owner_principal_id":"<uuid>"|null,
//!   "private_lists":bool,"organization":
//!   {"available":bool,"role":"<role>"|null},"members":
//!   [{"principal_id","label","role"}, …]}` (`Workspace::work_sharing`; the
//!   roles this sharer may grant; whether it may remove a person's access
//!   (the app's owner); the app's owner, whom the picker leaves out (no
//!   share names them, W4 r1); whether the app has private lists; members
//!   by label, never by provider identity, each with the role of their live
//!   permissions on the work).
//!
//! Refusal order on both POSTs: fetch metadata (403 `cross_site`), the ids
//! (uniform 404), the session (uniform 404), the member write limit (429),
//! the body (413, then 400 `malformed_share`), then the sharer's own
//! standing (`Workspace::share_standing`): `Denied` is the uniform 404 (a work
//! the sharer cannot open, no authority to share; identical for existing and
//! nonexistent works). A sharer **with** share authority then learns, in 422,
//! only what concerns its own standing, judged before any grantee is
//! considered: `reviewer_not_allowed` (a reviewer role from a sharer who is
//! not a reviewer of the work), `contributor_not_allowed` (a contributor
//! role without the sharer's `record_write`), `organization_share_unavailable`
//! (the work has source dependencies or folder files: shared person by
//! person, the v1 limit). Then storage: `Denied` is the uniform 404 (a
//! grantee who is not an active member of the same organization or is the
//! sharer; the standing changed meanwhile), 503 for a store outage. The
//! definers still judge every rule on their own (`share_work`,
//! `set_work_everyone`) and write the audit rows. A person share that wrote
//! a grant announces `app.shared` on the event hub (`events`); an
//! organization-wide change announces it only when it widens access (a first
//! share or a wider role; `announces_organization`), so a narrower role or a
//! stop tells nobody anything. Every member who can then open the work
//! receives it.
//!
//! Session resolution (`session`): an absent or ended session and a store
//! error alike answer the uniform 404 (fail closed; review m2: the deliberate
//! choice, so that a store hiccup never distinguishes an existing work from a
//! nonexistent one on these routes); the store error is logged.
pub mod live;

use crate::{
    auth::{cookies, csrf, Auth},
    events,
    http::{self, UniformPath},
    storage::{self, Lookup, ShareRole, TrustedRequest},
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
        .route("/v1/work/{app}/{work}/share", get(sharing).post(share))
        .route(
            "/v1/work/{app}/{work}/share/organization",
            post(share_organization),
        )
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
        Ok(Lookup::Absent) => None,
        Err(error) => {
            tracing::warn!(outcome = "session_unavailable", error = %error, "session.resolve");
            None
        }
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

/// The closed share body: the grantee and the role (`none`: no role, the
/// owner's removal of the grantee's access), both required, nothing else
/// (APP-192: `record_write` is refused, so a role has one encoding).
pub(crate) fn parse_share(bytes: &[u8]) -> Option<(String, Option<ShareRole>)> {
    let body = crate::admission::decode(bytes).ok()?;
    let object = body.as_object()?;
    if object.len() != 2
        || !object
            .keys()
            .all(|k| matches!(k.as_str(), "grantee_principal_id" | "role"))
    {
        return None;
    }
    let grantee = object.get("grantee_principal_id")?.as_str()?;
    if !storage::valid_identifier(grantee) {
        return None;
    }
    let role = match object.get("role")?.as_str()? {
        "none" => None,
        name => Some(ShareRole::from_name(name)?),
    };
    Some((grantee.to_owned(), role))
}

/// The closed organization-wide share body: `{"role": …}` and nothing else;
/// `none` (no role) ends the share.
pub(crate) fn parse_organization_share(bytes: &[u8]) -> Option<Option<ShareRole>> {
    let body = crate::admission::decode(bytes).ok()?;
    let object = body.as_object()?;
    if object.len() != 1 {
        return None;
    }
    match object.get("role")?.as_str()? {
        "none" => Some(None),
        name => ShareRole::from_name(name).map(Some),
    }
}

/// The plain code a sharer with share authority receives for a role it may
/// not grant (`roles`: `ShareStanding::roles`), or none when it may.
pub(crate) fn role_refusal(role: ShareRole, roles: &[ShareRole]) -> Option<&'static str> {
    if roles.contains(&role) {
        return None;
    }
    Some(match role {
        ShareRole::Reviewer => "reviewer_not_allowed",
        ShareRole::Contributor => "contributor_not_allowed",
        // Viewer is always among the roles of a sharer with authority.
        ShareRole::Viewer => "viewer_not_allowed",
    })
}

/// Whether an organization-wide change that `changed` the rows, from the
/// role live `before` to the role live `after`, gives members something to
/// be told about: a first share, or a wider role. A narrower or equal role,
/// a stop and a call that wrote nothing announce nothing (review of W2b,
/// item 6: no notice on a change that adds nothing for a member).
pub(crate) fn announces_organization(
    before: Option<ShareRole>,
    after: Option<ShareRole>,
    changed: bool,
) -> bool {
    let rank = |role: Option<ShareRole>| match role {
        None => 0,
        Some(ShareRole::Viewer) => 1,
        Some(ShareRole::Contributor) => 2,
        Some(ShareRole::Reviewer) => 3,
    };
    changed && rank(after) > rank(before)
}

/// Whether a person share is announced: only when it granted something. A
/// demotion that only took rows away gives nobody news (review of W2b,
/// item 6), and a repeat writes nothing.
pub(crate) fn announces_person(shared: &storage::Shared) -> bool {
    shared.granted.iter().any(|g| g.inserted)
}

/// The share dialog's document (`GET /v1/work/{app}/{work}/share`).
pub(crate) fn sharing_json(app: &str, work: &str, sharing: &storage::WorkSharing) -> Value {
    let standing = &sharing.standing;
    json!({
        "app_id": app,
        "work_id": work,
        "roles": standing.roles.iter().map(|r| r.name()).collect::<Vec<_>>(),
        "can_revoke_members": sharing.can_revoke_members,
        "owner_principal_id": sharing.owner,
        "private_lists": sharing.private_lists,
        "organization": {
            "available": standing.organization_available,
            "role": standing.organization.map(|r| r.name()),
        },
        "members": sharing.members.iter().map(|m| json!({
            "principal_id": m.principal, "label": m.label, "role": m.role.name(),
        })).collect::<Vec<_>>(),
    })
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

/// The common front of both share POSTs, in the module's refusal order: the
/// fetch-metadata rule, the ids, the session, the member write limit and a
/// bounded `application/json` body. `Err` is the answer to send.
async fn share_request(
    auth: &Auth,
    app: &str,
    work: &str,
    request: Request,
) -> Result<(TrustedRequest, axum::body::Bytes), Response> {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return Err(http::cross_site());
    }
    if !(storage::valid_identifier(app) && storage::valid_identifier(work)) {
        return Err(http::not_found());
    }
    let Some((_, trusted)) = session(auth, &parts.headers).await else {
        return Err(http::not_found());
    };
    if let Err(retry_after) = auth
        .limits
        .admit_member_write(trusted.organization(), trusted.principal())
    {
        return Err(http::too_many_requests(retry_after));
    }
    if auth.stores.is_none() {
        return Err(http::not_found());
    }
    let Some(bytes) = http::bounded(body, http::JSON_BODY_LIMIT).await else {
        return Err(http::payload_too_large());
    };
    if content_type(&parts.headers) != "application/json" {
        return Err(http::problem(StatusCode::BAD_REQUEST, "malformed_share"));
    }
    Ok((trusted, bytes))
}

/// The sharer's own standing on the work: the uniform 404 without share
/// authority (or for a work it cannot open), 503 for a store outage.
async fn standing(
    stores: &storage::Stores,
    trusted: &TrustedRequest,
    app: &str,
    work: &str,
) -> Result<storage::ShareStanding, Response> {
    match stores.workspace.share_standing(trusted, app, work).await {
        Ok(standing) => Ok(standing),
        Err(storage::Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

async fn sharing(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if !(storage::valid_identifier(&app) && storage::valid_identifier(&work)) {
        return http::not_found();
    }
    let Some((_, trusted)) = session(&auth, request.headers()).await else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    match stores.workspace.work_sharing(&trusted, &app, &work).await {
        Ok(sharing) => document(StatusCode::OK, &sharing_json(&app, &work, &sharing)),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn share(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (trusted, bytes) = match share_request(&auth, &app, &work, request).await {
        Ok(admitted) => admitted,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some((grantee, role)) = parse_share(&bytes) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_share");
    };
    let Some(role) = role else {
        return remove_access(stores, &trusted, &app, &work, &grantee).await;
    };
    let standing = match standing(stores, &trusted, &app, &work).await {
        Ok(standing) => standing,
        Err(refusal) => return refusal,
    };
    if let Some(code) = role_refusal(role, &standing.roles) {
        tracing::info!(outcome = code, role = role.name(), "share.submit");
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, code);
    }
    let result = share_with_lineage(stores, &trusted, &app, &work, &grantee, role).await;
    tracing::info!(
        outcome = match &result {
            Ok(shared) if shared.changed() => "shared",
            Ok(_) => "unchanged",
            Err(storage::Error::Denied) => "denied",
            Err(_) => "unavailable",
        },
        role = role.name(),
        "share.submit"
    );
    match result {
        Ok(shared) => document(
            StatusCode::OK,
            &json!({
                "app_id": app,
                "work_id": work,
                "grantee_principal_id": shared.grantee,
                "granted": shared.granted.iter().map(|g| json!({
                    "scope": g.scope, "resource_id": g.resource,
                    "permission": g.permission,
                })).collect::<Vec<_>>(),
            }),
        ),
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

async fn share_organization(
    State(auth): State<Arc<Auth>>,
    UniformPath((app, work)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let (trusted, bytes) = match share_request(&auth, &app, &work, request).await {
        Ok(admitted) => admitted,
        Err(refusal) => return refusal,
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some(role) = parse_organization_share(&bytes) else {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_share");
    };
    let standing = match standing(stores, &trusted, &app, &work).await {
        Ok(standing) => standing,
        Err(refusal) => return refusal,
    };
    let name = role.map_or("none", ShareRole::name);
    if let Some(role) = role {
        let refusal = if standing.organization_available {
            role_refusal(role, &standing.roles)
        } else {
            Some("organization_share_unavailable")
        };
        if let Some(code) = refusal {
            tracing::info!(outcome = code, role = name, "share.organization");
            return http::problem(StatusCode::UNPROCESSABLE_ENTITY, code);
        }
    }
    let result = match role {
        Some(role) => stores
            .workspace
            .share_work(&trusted, &app, &work, storage::Audience::Organization, role)
            .await
            .map(|shared| shared.changed()),
        None => {
            stores
                .workspace
                .revoke_organization_share(&trusted, &app, &work)
                .await
        }
    };
    tracing::info!(
        outcome = match &result {
            Ok(true) if role.is_some() => "shared",
            Ok(true) => "revoked",
            Ok(false) => "unchanged",
            Err(storage::Error::Denied) => "denied",
            Err(_) => "unavailable",
        },
        role = name,
        "share.organization"
    );
    match result {
        Ok(changed) => {
            // The role live before is the standing's read, taken before the
            // write: an announcement is a hint, never an authority.
            if announces_organization(standing.organization, role, changed) {
                events::hub().app_shared(trusted.organization(), &app, &work);
            }
            document(
                StatusCode::OK,
                &json!({"app_id": app, "work_id": work, "role": name}),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// The owner's removal of one person's access (`role: none`): storage alone
/// judges it (`Workspace::revoke_work_share`: the app's owner, not naming
/// themselves, an existing work), so no share standing is asked first and a
/// refusal is the uniform 404. The answer is the same whether the person held
/// anything (nothing tells the owner what they held). The composer lineage
/// forgets the person, so a later adaptation is not shared with them.
async fn remove_access(
    stores: &storage::Stores,
    trusted: &TrustedRequest,
    app: &str,
    work: &str,
    grantee: &str,
) -> Response {
    let result = stores
        .workspace
        .revoke_work_share(trusted, app, work, grantee)
        .await;
    tracing::info!(
        outcome = match &result {
            Ok(revoked) if revoked.changed() => "revoked",
            Ok(_) => "unchanged",
            Err(storage::Error::Denied) => "denied",
            Err(_) => "unavailable",
        },
        "share.remove"
    );
    match result {
        Ok(_) => {
            crate::composer::lineage::lineage().forget(trusted.organization(), work, grantee);
            document(
                StatusCode::OK,
                &json!({"app_id": app, "work_id": work, "grantee_principal_id": grantee, "role": "none"}),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// Shares a work with a teammate under the sharer's own authority, with its
/// event, and (APP-101) a composition's standard work shared by the member
/// who composed it carries its live adaptation through this same path
/// (`composer::lineage`); the teammate's canvas reshapes. The one share path
/// of the canvas route and MCP's `forma_share`.
pub async fn share_with_lineage(
    stores: &storage::Stores,
    trusted: &TrustedRequest,
    app: &str,
    work: &str,
    grantee: &str,
    role: ShareRole,
) -> Result<storage::Shared, storage::Error> {
    let shared = stores
        .workspace
        .share_work(trusted, app, work, storage::Audience::Member(grantee), role)
        .await?;
    if announces_person(&shared) {
        events::hub().app_shared(trusted.organization(), app, work);
    }
    if let Some((adapted_app, adapted_work)) = crate::composer::lineage::lineage().shared(
        trusted.organization(),
        trusted.principal(),
        work,
        grantee,
        role,
    ) {
        let adapted = stores
            .workspace
            .share_work(
                trusted,
                &adapted_app,
                &adapted_work,
                storage::Audience::Member(grantee),
                role,
            )
            .await;
        tracing::info!(
            outcome = if adapted.is_ok() { "shared" } else { "refused" },
            "share.adapted"
        );
        if adapted.is_ok() {
            events::hub().app_adapted(
                trusted.organization(),
                &adapted_app,
                &adapted_work,
                app,
                work,
            );
        }
    }
    Ok(shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use tower::ServiceExt as _;

    const GRANTEE: &str = "00000000-0000-4000-8000-00000000006d";

    #[test]
    fn share_body_is_closed() {
        for (role, expected) in [
            ("viewer", ShareRole::Viewer),
            ("contributor", ShareRole::Contributor),
            ("reviewer", ShareRole::Reviewer),
        ] {
            let ok = format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"{role}\"}}");
            assert_eq!(
                parse_share(ok.as_bytes()),
                Some((GRANTEE.into(), Some(expected))),
                "{ok}"
            );
            // The members may come in either order.
            let swapped = format!("{{\"role\":\"{role}\",\"grantee_principal_id\":\"{GRANTEE}\"}}");
            assert_eq!(
                parse_share(swapped.as_bytes()),
                Some((GRANTEE.into(), Some(expected)))
            );
        }
        // `none`: the owner's removal of the grantee's access.
        let none = format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"none\"}}");
        assert_eq!(parse_share(none.as_bytes()), Some((GRANTEE.into(), None)));
        for bad in [
            String::from("{}"),
            String::from("[]"),
            String::from("not json"),
            // One encoding (APP-192): the role is named; the earlier
            // `record_write` flag is refused alone and beside a role.
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"record_write\":true}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"record_write\":false}}"),
            format!(
                "{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"viewer\",\"record_write\":false}}"
            ),
            // Unknown members, a permission name, other roles and spellings.
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"permission\":\"admin_policy\"}}"),
            format!(
                "{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"viewer\",\"permission\":\"share_work\"}}"
            ),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"owner\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"None\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"Viewer\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"record_read\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":\"\"}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":true}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":null}}"),
            format!("{{\"grantee_principal_id\":\"{GRANTEE}\",\"role\":[\"viewer\"]}}"),
            // The grantee is required and canonical.
            String::from("{\"role\":\"viewer\"}"),
            String::from("{\"grantee_principal_id\":\"not-an-id\",\"role\":\"viewer\"}"),
            format!(
                "{{\"grantee_principal_id\":\"{}\",\"role\":\"viewer\"}}",
                GRANTEE.to_uppercase()
            ),
        ] {
            assert_eq!(parse_share(bad.as_bytes()), None, "{bad}");
        }
    }

    #[test]
    fn organization_body_names_a_role_or_none() {
        for (role, expected) in [
            ("viewer", Some(ShareRole::Viewer)),
            ("contributor", Some(ShareRole::Contributor)),
            ("reviewer", Some(ShareRole::Reviewer)),
            ("none", None),
        ] {
            let ok = format!("{{\"role\":\"{role}\"}}");
            assert_eq!(
                parse_organization_share(ok.as_bytes()),
                Some(expected),
                "{ok}"
            );
        }
        for bad in [
            String::from("{}"),
            String::from("[]"),
            String::from("not json"),
            String::from("{\"role\":\"everyone\"}"),
            String::from("{\"role\":\"None\"}"),
            String::from("{\"role\":\"\"}"),
            String::from("{\"role\":null}"),
            String::from("{\"role\":false}"),
            String::from("{\"record_write\":true}"),
            String::from("{\"role\":\"viewer\",\"record_write\":false}"),
            format!("{{\"role\":\"viewer\",\"grantee_principal_id\":\"{GRANTEE}\"}}"),
            String::from("{\"role\":\"viewer\",\"everyone\":true}"),
        ] {
            assert_eq!(parse_organization_share(bad.as_bytes()), None, "{bad}");
        }
    }

    /// A sharer with share authority learns which role it may not grant, in
    /// one plain code per role; a viewer share is never refused here.
    #[test]
    fn a_role_the_sharer_may_not_grant_has_its_own_code() {
        use ShareRole::*;
        let all = [Viewer, Contributor, Reviewer];
        for role in all {
            assert_eq!(role_refusal(role, &all), None);
        }
        assert_eq!(
            role_refusal(Reviewer, &[Viewer, Contributor]),
            Some("reviewer_not_allowed")
        );
        assert_eq!(
            role_refusal(Reviewer, &[Viewer]),
            Some("reviewer_not_allowed")
        );
        assert_eq!(
            role_refusal(Contributor, &[Viewer]),
            Some("contributor_not_allowed")
        );
        assert_eq!(role_refusal(Viewer, &[Viewer]), None);
    }

    #[test]
    fn the_sharing_answer_names_roles_and_labels_only() {
        let sharing = storage::WorkSharing {
            standing: storage::ShareStanding {
                roles: vec![ShareRole::Viewer, ShareRole::Contributor],
                organization_available: true,
                organization: Some(ShareRole::Viewer),
            },
            can_revoke_members: true,
            owner: Some(GRANTEE.into()),
            private_lists: true,
            members: vec![storage::ShareEntry {
                principal: GRANTEE.into(),
                label: "Ana".into(),
                role: ShareRole::Contributor,
            }],
        };
        assert_eq!(
            sharing_json(GRANTEE, GRANTEE, &sharing),
            json!({
                "app_id": GRANTEE,
                "work_id": GRANTEE,
                "roles": ["viewer", "contributor"],
                "can_revoke_members": true,
                "owner_principal_id": GRANTEE,
                "private_lists": true,
                "organization": {"available": true, "role": "viewer"},
                "members": [{"principal_id": GRANTEE, "label": "Ana", "role": "contributor"}],
            })
        );
        let alone = storage::WorkSharing {
            standing: storage::ShareStanding {
                roles: vec![ShareRole::Viewer],
                organization_available: false,
                organization: None,
            },
            can_revoke_members: false,
            owner: None,
            private_lists: false,
            members: vec![],
        };
        assert_eq!(
            sharing_json(GRANTEE, GRANTEE, &alone)["organization"],
            json!({"available": false, "role": null})
        );
    }

    /// An organization-wide change is announced to members only when it gives
    /// them something: a first share or a wider role. A narrower role, the
    /// same role, a stop, or a call that wrote nothing announces nothing.
    #[test]
    fn an_organization_share_is_announced_only_when_it_widens() {
        use ShareRole::*;
        assert!(announces_organization(None, Some(Viewer), true));
        assert!(announces_organization(None, Some(Reviewer), true));
        assert!(announces_organization(
            Some(Viewer),
            Some(Contributor),
            true
        ));
        assert!(announces_organization(
            Some(Contributor),
            Some(Reviewer),
            true
        ));
        for (before, after) in [
            (Some(Reviewer), Some(Viewer)),
            (Some(Contributor), Some(Viewer)),
            (Some(Reviewer), Some(Contributor)),
            (Some(Viewer), None),
            (Some(Reviewer), None),
            (None, None),
        ] {
            assert!(
                !announces_organization(before, after, true),
                "{before:?} -> {after:?}"
            );
        }
        // Nothing written (a replay, or a stop with nothing live).
        assert!(!announces_organization(None, Some(Viewer), false));
        assert!(!announces_organization(Some(Viewer), Some(Viewer), false));
    }

    #[test]
    fn a_person_share_is_announced_only_when_it_grants() {
        let granted = |inserted: bool| storage::Granted {
            scope: "work".into(),
            resource: GRANTEE.into(),
            permission: "record_read".into(),
            inserted,
        };
        let shared = |inserted: bool, removed: &[&str]| storage::Shared {
            grantee: Some(GRANTEE.into()),
            granted: vec![granted(inserted)],
            removed: removed.iter().map(|r| (*r).to_owned()).collect(),
        };
        assert!(announces_person(&shared(true, &[])));
        assert!(announces_person(&shared(true, &["record_review"])));
        // A demotion that only removed rows, and a repeat.
        let demotion = shared(false, &["record_review", "record_write"]);
        assert!(demotion.changed(), "storage counts the removal as a change");
        assert!(!announces_person(&demotion));
        assert!(!announces_person(&shared(false, &[])));
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
        // APP-192: the dialog's read and the organization-wide share.
        let not_found = (
            StatusCode::NOT_FOUND,
            String::from("{\"code\":\"not_found\",\"status\":404}"),
        );
        assert_eq!(send("GET", &uri, &[]).await, not_found);
        assert_eq!(send("GET", "/v1/work/x/y/share", &[]).await, not_found);
        let everyone = format!("{uri}/organization");
        for refused in [
            vec![("sec-fetch-site", "cross-site")],
            vec![("sec-fetch-site", "same-site")],
            vec![("sec-fetch-site", "none")],
            // No fetch metadata and no matching Origin (none is configured).
            vec![],
            vec![("origin", "https://elsewhere.example")],
            // Two values are refused, even if one is same-origin.
            vec![
                ("sec-fetch-site", "same-origin"),
                ("sec-fetch-site", "cross-site"),
            ],
        ] {
            for path in [&uri, &everyone] {
                assert_eq!(
                    send("POST", path, &refused).await,
                    (
                        StatusCode::FORBIDDEN,
                        String::from("{\"code\":\"cross_site\",\"status\":403}")
                    ),
                    "{path} {refused:?}"
                );
            }
        }
        assert_eq!(
            send("POST", &everyone, &[("sec-fetch-site", "same-origin")]).await,
            not_found
        );
        assert_eq!(
            send(
                "POST",
                "/v1/work/x/y/share/organization",
                &[("sec-fetch-site", "same-origin")]
            )
            .await,
            not_found
        );
        // Only GET and POST are routed; another method is refused.
        assert_ne!(
            send("DELETE", &everyone, &[("sec-fetch-site", "same-origin")])
                .await
                .0,
            StatusCode::OK
        );
    }
}
