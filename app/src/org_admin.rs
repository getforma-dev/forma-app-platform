//! `GET /admin`, `POST /admin/members/{principal}/role` and `POST
//! /admin/members/{principal}/remove` (D4 of the trial plan; ADR-021
//! organization roles, APP-225): the organization's own admin area, in the
//! product, beside the operator's audited back office. Server-rendered
//! (`orgAdmin` in `admin/build.ts`), no script.
//!
//! Order, all routes: (`POST`: fetch metadata, else 403 `cross_site`) → a
//! live workspace session (`query::routes::member`; else the uniform 404) →
//! the query string (`GET`: none, or exactly `notice=<code>` from the closed
//! set below; `POST`: none; else the uniform 404) → `is_org_admin` (false,
//! or any authority refusal: the uniform 404, before any member data is read;
//! a store outage 503) → `POST`: the form body (at most 4 KiB, the form's
//! own fields only; a missing or wrong synchronizer token 403 `cross_site`,
//! tied to the session cookie and the form's `csrf::Form`) → the storage
//! call.
//!
//! Change role (`csrf::Form::OrgAdminRole`; fields `csrf`, `role`, `from`
//! and `confirm`) answers `303` to `/admin?notice=<code>`: `role_changed`,
//! `role_unchanged`, `role_unconfirmed` (the chosen or the current role is
//! owner or admin and the confirmation box was not ticked; nothing was
//! called), the refusal storage named (`role_not_allowed`, `role_last_owner`,
//! `role_self`, `role_not_member`, `role_unknown`), or `unavailable`. The
//! page offers each row only the roles the caller may set on it
//! (`OrgRole::assignable`); the server enforces the ladder regardless, in
//! Rust and again in the definer. Remove (`csrf::Form::OrgAdminRemove`;
//! `csrf` and `confirm`) answers `removed`, `unconfirmed`, the refusal
//! storage named (`self`, `last_owner`, `not_member`, `not_allowed`), or
//! `unavailable` (a storage outage, a concurrent change that PostgreSQL
//! aborted, or an unknown outcome; recheck N6), each shown in plain words
//! on the page. A storage outage on `GET /admin` itself is the same page
//! with that notice and no member data (503), never a raw JSON body. A role
//! change or a removal ends the member's sessions at their next request
//! (storage advances the epoch). Logs carry identities and codes; never a
//! name or a note.
use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    query::routes::member,
    storage::{self, MemberRow, OrgRole, RemoveOutcome, RoleOutcome, TrustedRequest, Workspace},
};
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use forma_ir::PageProps;
use forma_server::{PageHandle, RenderCtx};
use std::{future::Future, pin::Pin, sync::Arc};

pub const PATH: &str = "/admin";
pub const ROLE_PATH: &str = "/admin/members/{principal}/role";
pub const REMOVE_PATH: &str = "/admin/members/{principal}/remove";

/// The page (`admin/src/org-admin/app.tsx`).
#[derive(PageProps, Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminProps {
    pub organization: String,
    /// "3 members · 2 active".
    pub summary: String,
    pub has_notice: bool,
    pub notice: String,
    /// `done` or `refused` (the notice's style).
    pub notice_kind: String,
    pub has_members: bool,
    /// The list was read and has no member ("No members yet.").
    pub empty: bool,
    pub members: Vec<AdminMemberProps>,
    pub csrf_signout: String,
}

/// One row of the members table.
#[derive(PageProps, Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminMemberProps {
    /// The principal id (the row key).
    pub id: String,
    pub label: String,
    pub note: String,
    pub status: String,
    /// "Owner", "Admin", "Manager", "Member", "Viewer", with " · you" on the
    /// viewer's own row.
    pub role: String,
    /// The stored role (`owner` … `viewer`): the Change role form's `from`.
    pub role_code: String,
    /// The caller may set a role on this active member: the select and the
    /// Change role form are offered (storage judges the ladder again).
    pub changeable: bool,
    /// The inverse of `changeable` (the role is shown as text).
    pub fixed_role: bool,
    /// The roles the caller may set on this row, highest first, the current
    /// one selected.
    pub options: Vec<RoleOptionProps>,
    /// The confirmation box is shown: the row's role or an offered role is
    /// owner or admin (the server requires it for those changes).
    pub confirm_role: bool,
    /// "Role for <name>": the select's accessible name.
    pub role_select_label: String,
    pub role_action: String,
    pub csrf_role: String,
    pub consent: String,
    pub last_active: String,
    pub apps: String,
    pub works: String,
    pub records: String,
    pub uploads: String,
    /// An active member other than the viewer whose role the caller manages:
    /// the Remove form is offered (storage still judges `self`, `last_owner`
    /// and `not_allowed`).
    pub removable: bool,
    pub action: String,
    pub csrf: String,
    pub confirm_label: String,
}

/// One `<option>` of a row's role select.
#[derive(PageProps, Clone, Debug, Default, Eq, PartialEq)]
pub struct RoleOptionProps {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

/// What `?notice=` may name, and the words the page shows for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    Removed,
    Unconfirmed,
    /// A removal refusal storage named.
    Refused(&'static str),
    RoleChanged,
    RoleUnchanged,
    RoleUnconfirmed,
    /// A role-change refusal storage named.
    RoleRefused(&'static str),
    /// Recheck N6: storage could not complete or confirm the request.
    Unavailable,
}
impl Notice {
    fn parse(query: &str) -> Option<Self> {
        match query.strip_prefix("notice=")? {
            "removed" => Some(Self::Removed),
            "unconfirmed" => Some(Self::Unconfirmed),
            "self" => Some(Self::Refused("self")),
            "last_owner" => Some(Self::Refused("last_owner")),
            "not_member" => Some(Self::Refused("not_member")),
            "not_allowed" => Some(Self::Refused("not_allowed")),
            "role_changed" => Some(Self::RoleChanged),
            "role_unchanged" => Some(Self::RoleUnchanged),
            "role_unconfirmed" => Some(Self::RoleUnconfirmed),
            "role_not_allowed" => Some(Self::RoleRefused("not_allowed")),
            "role_last_owner" => Some(Self::RoleRefused("last_owner")),
            "role_self" => Some(Self::RoleRefused("self")),
            "role_not_member" => Some(Self::RoleRefused("not_member")),
            "role_unknown" => Some(Self::RoleRefused("unknown_role")),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
    fn code(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Unconfirmed => "unconfirmed",
            Self::Refused(code) => code,
            Self::RoleChanged => "role_changed",
            Self::RoleUnchanged => "role_unchanged",
            Self::RoleUnconfirmed => "role_unconfirmed",
            Self::RoleRefused("not_allowed") => "role_not_allowed",
            Self::RoleRefused("last_owner") => "role_last_owner",
            Self::RoleRefused("self") => "role_self",
            Self::RoleRefused("not_member") => "role_not_member",
            Self::RoleRefused(_) => "role_unknown",
            Self::Unavailable => "unavailable",
        }
    }
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Removed => (
                "The member was removed. Their sessions end at their next request.",
                "done",
            ),
            Self::Unconfirmed => (
                "Nothing was removed. Tick the confirmation box to remove a member.",
                "refused",
            ),
            Self::Refused("self") => (
                "You can't remove yourself. Another owner or admin of this organization can.",
                "refused",
            ),
            Self::Refused("last_owner") => (
                "You can't remove the last owner of this organization.",
                "refused",
            ),
            Self::Refused("not_allowed") => (
                "You can't remove that member: their role is above what your role manages.",
                "refused",
            ),
            Self::Refused(_) => ("That person is no longer an active member.", "refused"),
            Self::RoleChanged => (
                "The role was changed. Their open sessions pick it up at their next request.",
                "done",
            ),
            Self::RoleUnchanged => ("That member already has that role. Nothing changed.", "done"),
            Self::RoleUnconfirmed => (
                "Nothing changed. Tick the confirmation box to change an owner or admin role.",
                "refused",
            ),
            Self::RoleRefused("not_allowed") => (
                "You can't set that role: it, or the member's current role, is above what your role manages.",
                "refused",
            ),
            Self::RoleRefused("last_owner") => (
                "You can't change the last owner's role. Make someone else an owner first.",
                "refused",
            ),
            Self::RoleRefused("self") => ("You can't raise your own role.", "refused"),
            Self::RoleRefused("not_member") => {
                ("That person is no longer an active member.", "refused")
            }
            Self::RoleRefused(_) => ("That is not a role here.", "refused"),
            Self::Unavailable => (
                "That could not be completed or confirmed just now. Reload this page in a minute to see the current list.",
                "refused",
            ),
        }
    }
}

type Pending<'a, T> = Pin<Box<dyn Future<Output = Result<T, storage::Error>> + Send + 'a>>;

/// The storage the routes read and write (the workspace store; a fake in
/// tests). Every call runs under the member's own fenced transaction.
pub trait AdminStore: Send + Sync {
    fn is_org_admin<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, bool>;
    fn organization_name<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, String>;
    fn overview<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, Vec<MemberRow>>;
    fn set_role<'a>(
        &'a self,
        request: &'a TrustedRequest,
        principal: &'a str,
        role: OrgRole,
    ) -> Pending<'a, RoleOutcome>;
    fn remove<'a>(
        &'a self,
        request: &'a TrustedRequest,
        principal: &'a str,
    ) -> Pending<'a, RemoveOutcome>;
}
impl AdminStore for Workspace {
    fn is_org_admin<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, bool> {
        Box::pin(Workspace::is_org_admin(self, request))
    }
    fn organization_name<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, String> {
        Box::pin(Workspace::organization_name(self, request))
    }
    fn overview<'a>(&'a self, request: &'a TrustedRequest) -> Pending<'a, Vec<MemberRow>> {
        Box::pin(self.org_admin_overview(request))
    }
    fn set_role<'a>(
        &'a self,
        request: &'a TrustedRequest,
        principal: &'a str,
        role: OrgRole,
    ) -> Pending<'a, RoleOutcome> {
        Box::pin(self.set_member_role(request, principal, role))
    }
    fn remove<'a>(
        &'a self,
        request: &'a TrustedRequest,
        principal: &'a str,
    ) -> Pending<'a, RemoveOutcome> {
        Box::pin(self.admin_remove_member(request, principal))
    }
}

/// How the page is rendered (the native page in the binary).
pub trait AdminPages: Send + Sync {
    fn page(&self, props: &AdminProps) -> Response;
}

/// The Forma native page (`orgAdmin` in `admin/build.ts`).
pub struct SsrAdminPages {
    pub page: PageHandle<AdminProps>,
}
impl AdminPages for SsrAdminPages {
    fn page(&self, props: &AdminProps) -> Response {
        match self
            .page
            .render(props, &RenderCtx::new().with_title("Organization admin"))
        {
            Ok(page) => page.into_response(),
            Err(error) => {
                tracing::error!(code = error.code(), route = PATH, "{error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [
                        (header::CACHE_CONTROL, "no-store"),
                        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                    ],
                    "Internal Server Error",
                )
                    .into_response()
            }
        }
    }
}

/// The routes' state.
pub struct OrgAdmin {
    pub auth: Arc<Auth>,
    pub pages: Arc<dyn AdminPages>,
}

pub fn router(state: Arc<OrgAdmin>) -> Router {
    Router::new()
        .route(PATH, get(show_route))
        .route(ROLE_PATH, post(role_route))
        .route(REMOVE_PATH, post(remove_route))
        .with_state(state)
}

/// `GET /admin`.
async fn show_route(State(state): State<Arc<OrgAdmin>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let (cookie, member) = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let notice = match parts.uri.query() {
        None => None,
        Some(query) => match Notice::parse(query) {
            Some(notice) => Some(notice),
            None => return http::not_found(),
        },
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    show(
        &stores.workspace,
        state.pages.as_ref(),
        &state.auth.csrf,
        &cookie,
        &member,
        notice,
    )
    .await
}

/// The members table for an organization owner, admin or manager; the
/// uniform 404 for anyone else, decided before any member data is read.
pub(crate) async fn show(
    store: &dyn AdminStore,
    pages: &dyn AdminPages,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    notice: Option<Notice>,
) -> Response {
    match admin(store, member).await {
        Standing::Admin => {}
        Standing::Stranger => return http::not_found(),
        Standing::Outage => return outage(pages, csrf, cookie),
    }
    let rows = match store.overview(member).await {
        Ok(rows) => rows,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return outage(pages, csrf, cookie),
    };
    let organization = store.organization_name(member).await.unwrap_or_default();
    tracing::info!(
        event = "org_admin.viewed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        role = member.role().as_str(),
        members = rows.len(),
        notice = notice.map(Notice::code),
        "organization admin page"
    );
    pages.page(&props(csrf, cookie, member, organization, &rows, notice))
}

/// The common front of both `POST` routes: fetch metadata, the session, the
/// path, the standing, then the form body.
async fn post_front(
    state: &OrgAdmin,
    principal: &str,
    request: Request,
) -> Result<(String, TrustedRequest, Vec<(String, String)>), Response> {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return Err(http::cross_site());
    }
    let (cookie, member) = member(&state.auth, &parts.headers).await?;
    if parts.uri.query().is_some() || !storage::valid_identifier(principal) {
        return Err(http::not_found());
    }
    let Some(stores) = state.auth.stores.as_ref() else {
        return Err(http::not_found());
    };
    match admin(&stores.workspace, &member).await {
        Standing::Admin => {}
        Standing::Stranger => return Err(http::not_found()),
        Standing::Outage => return Err(notice(Notice::Unavailable)),
    }
    let Some(fields) = http::form(body).await else {
        return Err(http::payload_too_large());
    };
    Ok((cookie, member, fields))
}

/// `POST /admin/members/{principal}/role`.
async fn role_route(
    State(state): State<Arc<OrgAdmin>>,
    UniformPath(principal): UniformPath<String>,
    request: Request,
) -> Response {
    let (cookie, member, fields) = match post_front(&state, &principal, request).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    set_role(
        &stores.workspace,
        &state.auth.csrf,
        &cookie,
        &member,
        &principal,
        &fields,
    )
    .await
}

/// `POST /admin/members/{principal}/remove`.
async fn remove_route(
    State(state): State<Arc<OrgAdmin>>,
    UniformPath(principal): UniformPath<String>,
    request: Request,
) -> Response {
    let (cookie, member, fields) = match post_front(&state, &principal, request).await {
        Ok(front) => front,
        Err(refusal) => return refusal,
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    remove(
        &stores.workspace,
        &state.auth.csrf,
        &cookie,
        &member,
        &principal,
        &fields,
    )
    .await
}

/// One form's fields: every name from the closed set, `csrf` exactly once,
/// the others at most once.
fn shaped(fields: &[(String, String)], names: &[&str]) -> bool {
    let count = |field: &str| fields.iter().filter(|(name, _)| name == field).count();
    fields
        .iter()
        .all(|(name, _)| names.contains(&name.as_str()))
        && count("csrf") == 1
        && names.iter().all(|name| count(name) <= 1)
}
fn value<'a>(fields: &'a [(String, String)], field: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(name, _)| name == field)
        .map(|(_, value)| value.as_str())
}

/// ADR-021: the role change of `principal` by a caller already judged
/// (`admin`): the token, the role, the confirmation for an owner or admin
/// change, the Rust-side ladder (`OrgRole::may_set`, judged again by the
/// definer), then storage; a `303` to the page with the outcome's notice.
pub(crate) async fn set_role(
    store: &dyn AdminStore,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    principal: &str,
    fields: &[(String, String)],
) -> Response {
    let token = value(fields, "csrf").unwrap_or("");
    if !shaped(fields, &["csrf", "role", "from", "confirm"])
        || !csrf.verify(csrf::Form::OrgAdminRole, cookie, token)
    {
        return http::cross_site();
    }
    let log = |code: &str| {
        tracing::info!(
            event = "org_admin.set_role",
            code,
            organization_id = member.organization(),
            principal_id = member.principal(),
            member_id = principal,
            role = value(fields, "role").unwrap_or(""),
            "organization admin role change"
        );
    };
    let Some(role) = value(fields, "role").and_then(OrgRole::parse) else {
        log("unknown_role");
        return notice(Notice::RoleRefused("unknown_role"));
    };
    // The row's role as rendered (a hint for the confirmation only; the
    // definer reads the stored role).
    let from = value(fields, "from").and_then(OrgRole::parse);
    let sensitive = |role: OrgRole| matches!(role, OrgRole::Owner | OrgRole::Admin);
    if (sensitive(role) || from.is_some_and(sensitive)) && value(fields, "confirm") != Some("role")
    {
        log("role_unconfirmed");
        return notice(Notice::RoleUnconfirmed);
    }
    // ADR-021 in Rust first: the ladder as the caller's resolved role knows
    // it (the definer judges the stored roles under the fence).
    if let Some(from) = from {
        if !member
            .role()
            .may_set(from, role, principal == member.principal())
        {
            log("not_allowed");
            return notice(Notice::RoleRefused("not_allowed"));
        }
    }
    match store.set_role(member, principal, role).await {
        Ok(RoleOutcome::Changed) => {
            log("changed");
            notice(Notice::RoleChanged)
        }
        Ok(RoleOutcome::Unchanged) => {
            log("unchanged");
            notice(Notice::RoleUnchanged)
        }
        Ok(RoleOutcome::Refused(code)) => {
            log(code);
            notice(Notice::RoleRefused(match code {
                "not_allowed" => "not_allowed",
                "last_owner" => "last_owner",
                "self" => "self",
                "not_member" => "not_member",
                _ => "unknown_role",
            }))
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(error) => {
            log(match error {
                storage::Error::UnknownOutcome => "unknown_outcome",
                _ => "unavailable",
            });
            notice(Notice::Unavailable)
        }
    }
}

/// The removal of `principal` by a caller already judged (`admin`): the
/// token, the confirmation, then storage; a `303` to the page with the
/// outcome's notice.
pub(crate) async fn remove(
    store: &dyn AdminStore,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    principal: &str,
    fields: &[(String, String)],
) -> Response {
    let token = value(fields, "csrf").unwrap_or("");
    if !shaped(fields, &["csrf", "confirm"])
        || !csrf.verify(csrf::Form::OrgAdminRemove, cookie, token)
    {
        return http::cross_site();
    }
    let confirmed = value(fields, "confirm") == Some("remove");
    let log = |code: &str| {
        tracing::info!(
            event = "org_admin.remove",
            code,
            organization_id = member.organization(),
            principal_id = member.principal(),
            member_id = principal,
            "organization admin removal"
        );
    };
    if !confirmed {
        log("unconfirmed");
        return notice(Notice::Unconfirmed);
    }
    match store.remove(member, principal).await {
        Ok(RemoveOutcome::Removed) => {
            log("removed");
            notice(Notice::Removed)
        }
        Ok(RemoveOutcome::Refused(code)) => {
            log(code);
            notice(
                Notice::parse(&format!("notice={code}")).unwrap_or(Notice::Refused("not_member")),
            )
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(error) => {
            // Recheck N6: an outage, a concurrent change PostgreSQL aborted
            // (two admins' locks), or an unknown outcome. The database is as
            // storage left it; the page says to reload and look.
            log(match error {
                storage::Error::UnknownOutcome => "unknown_outcome",
                _ => "unavailable",
            });
            notice(Notice::Unavailable)
        }
    }
}

/// Where the request's member stands in its organization.
enum Standing {
    Admin,
    /// Not an owner, admin or manager, or any authority refusal: the uniform 404.
    Stranger,
    /// The store could not say.
    Outage,
}
async fn admin(store: &dyn AdminStore, member: &TrustedRequest) -> Standing {
    match store.is_org_admin(member).await {
        Ok(true) => Standing::Admin,
        Ok(false) | Err(storage::Error::Denied) => Standing::Stranger,
        Err(_) => Standing::Outage,
    }
}

/// The page with the outage notice and no member data (503): the same
/// for every member, since the store could not say who administers.
fn outage(pages: &dyn AdminPages, csrf: &csrf::CsrfKey, cookie: &str) -> Response {
    let (words, kind) = Notice::Unavailable.words();
    let mut response = pages.page(&AdminProps {
        organization: "Your organization".into(),
        summary: String::new(),
        has_notice: true,
        notice: words.into(),
        notice_kind: kind.into(),
        has_members: false,
        empty: false,
        members: Vec::new(),
        csrf_signout: csrf.token(csrf::Form::SignOut, cookie),
    });
    *response.status_mut() = axum::http::StatusCode::SERVICE_UNAVAILABLE;
    response
}

fn notice(notice: Notice) -> Response {
    http::see_other(&format!("{PATH}?notice={}", notice.code()))
}

/// The role's word on the page.
pub(crate) fn role_word(role: OrgRole) -> &'static str {
    match role {
        OrgRole::Owner => "Owner",
        OrgRole::Admin => "Admin",
        OrgRole::Manager => "Manager",
        OrgRole::Member => "Member",
        OrgRole::Viewer => "Viewer",
    }
}

/// The page props from storage's rows (every value is text the page shows
/// as is; names and notes are escaped by the renderer).
pub(crate) fn props(
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    organization: String,
    rows: &[MemberRow],
    notice: Option<Notice>,
) -> AdminProps {
    let remove_token = csrf.token(csrf::Form::OrgAdminRemove, cookie);
    let role_token = csrf.token(csrf::Form::OrgAdminRole, cookie);
    let active = rows.iter().filter(|row| row.status == "active").count();
    let caller = member.role();
    let members: Vec<AdminMemberProps> = rows
        .iter()
        .map(|row| {
            let mine = row.principal == member.principal();
            let live = row.status == "active";
            let options: Vec<RoleOptionProps> = if live {
                caller
                    .assignable(row.role, mine)
                    .into_iter()
                    .map(|role| RoleOptionProps {
                        value: role.as_str().into(),
                        label: role_word(role).into(),
                        selected: role == row.role,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            // One role to offer besides the current one, else no form.
            let changeable = options.iter().any(|option| !option.selected);
            let sensitive = |role: OrgRole| matches!(role, OrgRole::Owner | OrgRole::Admin);
            let confirm_role = changeable
                && (sensitive(row.role)
                    || options
                        .iter()
                        .any(|option| OrgRole::parse(&option.value).is_some_and(sensitive)));
            AdminMemberProps {
                id: row.principal.clone(),
                label: row.label.clone(),
                note: row.note.clone().unwrap_or_default(),
                status: match row.status.as_str() {
                    "active" => "Active".into(),
                    "revoked" => "Removed".into(),
                    other => other.to_owned(),
                },
                role: if mine {
                    format!("{} · you", role_word(row.role))
                } else {
                    role_word(row.role).into()
                },
                role_code: row.role.as_str().into(),
                changeable,
                fixed_role: !changeable,
                options,
                confirm_role,
                role_select_label: format!("Role for {}", row.label),
                role_action: format!("/admin/members/{}/role", row.principal),
                csrf_role: role_token.clone(),
                consent: match (row.consent_version, row.consented_at.as_deref()) {
                    (Some(version), Some(at)) => format!("Notice v{version}, {}", day(at)),
                    (Some(version), None) => format!("Notice v{version}"),
                    (None, _) => "Not recorded".into(),
                },
                last_active: row
                    .last_active
                    .as_deref()
                    .map(minute)
                    .unwrap_or_else(|| "Never".into()),
                apps: row.apps.to_string(),
                works: row.works.to_string(),
                records: row.records.to_string(),
                uploads: row.uploads.to_string(),
                removable: live && !mine && caller.may_remove(row.role),
                action: format!("/admin/members/{}/remove", row.principal),
                csrf: remove_token.clone(),
                confirm_label: format!("Yes, remove {}", row.label),
            }
        })
        .collect();
    let (notice_text, notice_kind) = notice.map(Notice::words).unwrap_or(("", ""));
    AdminProps {
        organization: if organization.is_empty() {
            "Your organization".into()
        } else {
            organization
        },
        summary: format!(
            "{} {} · {active} active",
            rows.len(),
            if rows.len() == 1 { "member" } else { "members" }
        ),
        has_notice: notice.is_some(),
        notice: notice_text.into(),
        notice_kind: notice_kind.into(),
        has_members: !members.is_empty(),
        empty: members.is_empty(),
        members,
        csrf_signout: csrf.token(csrf::Form::SignOut, cookie),
    }
}

/// `2026-11-29T14:03:12Z` → `2026-11-29`.
fn day(at: &str) -> String {
    at.get(..10).unwrap_or(at).to_owned()
}
/// `2026-11-29T14:03:12Z` → `2026-11-29 14:03 UTC`.
fn minute(at: &str) -> String {
    match (at.get(..10), at.get(11..16)) {
        (Some(day), Some(time)) if at.as_bytes().get(10) == Some(&b'T') => {
            format!("{day} {time} UTC")
        }
        _ => at.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView};
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request as HttpRequest, StatusCode},
        response::IntoResponse,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use tower::ServiceExt;

    const ORG: &str = "00000000-0000-4000-8000-000000000002";
    const ADMIN: &str = "00000000-0000-4000-8000-0000000000a1";
    const STUDENT: &str = "00000000-0000-4000-8000-0000000000b2";
    const COOKIE: &str = "session-cookie-value";

    struct Fake {
        admin: Result<bool, storage::Error>,
        rows: Vec<MemberRow>,
        /// The overview's own outage (recheck N6).
        listing: Option<storage::Error>,
        outcome: Result<RemoveOutcome, storage::Error>,
        role_outcome: Result<RoleOutcome, storage::Error>,
        overviews: AtomicUsize,
        removed: Mutex<Vec<String>>,
        roles: Mutex<Vec<(String, OrgRole)>>,
    }
    impl Fake {
        fn new(admin: Result<bool, storage::Error>) -> Self {
            Self {
                admin,
                rows: vec![
                    row(ADMIN, "Victor", OrgRole::Owner),
                    row(STUDENT, "Ana <b>", OrgRole::Member),
                ],
                listing: None,
                outcome: Ok(RemoveOutcome::Removed),
                role_outcome: Ok(RoleOutcome::Changed),
                overviews: AtomicUsize::new(0),
                removed: Mutex::new(Vec::new()),
                roles: Mutex::new(Vec::new()),
            }
        }
    }
    impl AdminStore for Fake {
        fn is_org_admin<'a>(&'a self, _: &'a TrustedRequest) -> Pending<'a, bool> {
            let admin = self.admin;
            Box::pin(async move { admin })
        }
        fn organization_name<'a>(&'a self, _: &'a TrustedRequest) -> Pending<'a, String> {
            Box::pin(async { Ok("Weekend class".to_owned()) })
        }
        fn overview<'a>(&'a self, _: &'a TrustedRequest) -> Pending<'a, Vec<MemberRow>> {
            self.overviews.fetch_add(1, Ordering::SeqCst);
            let rows = self.rows.clone();
            let listing = self.listing;
            Box::pin(async move { listing.map_or(Ok(rows), Err) })
        }
        fn set_role<'a>(
            &'a self,
            _: &'a TrustedRequest,
            principal: &'a str,
            role: OrgRole,
        ) -> Pending<'a, RoleOutcome> {
            self.roles
                .lock()
                .unwrap()
                .push((principal.to_owned(), role));
            let outcome = self.role_outcome;
            Box::pin(async move { outcome })
        }
        fn remove<'a>(
            &'a self,
            _: &'a TrustedRequest,
            principal: &'a str,
        ) -> Pending<'a, RemoveOutcome> {
            self.removed.lock().unwrap().push(principal.to_owned());
            let outcome = self.outcome;
            Box::pin(async move { outcome })
        }
    }

    fn row(principal: &str, label: &str, role: OrgRole) -> MemberRow {
        MemberRow {
            principal: principal.into(),
            label: label.into(),
            note: (role == OrgRole::Owner).then(|| "teacher".to_owned()),
            status: "active".into(),
            role,
            consent_version: Some(1),
            consented_at: Some("2026-11-29T09:12:40Z".into()),
            last_active: Some("2026-11-29T14:03:12Z".into()),
            apps: 2,
            works: 3,
            records: 41,
            uploads: 1,
            code_status: Some("redeemed".into()),
        }
    }

    #[derive(Default)]
    struct Recorded(Mutex<Option<AdminProps>>);
    impl AdminPages for Recorded {
        fn page(&self, props: &AdminProps) -> Response {
            *self.0.lock().unwrap() = Some(props.clone());
            StatusCode::OK.into_response()
        }
    }

    fn admin_request() -> TrustedRequest {
        request_as(OrgRole::Owner)
    }
    fn request_as(role: OrgRole) -> TrustedRequest {
        storage::AgentCaller::for_tests_with_role(ORG, ADMIN, "t", role).request
    }
    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    fn location(response: &Response) -> &str {
        response.headers()[header::LOCATION].to_str().unwrap()
    }

    #[tokio::test]
    async fn an_admin_sees_every_member_and_anyone_else_the_uniform_404_before_any_read() {
        let key = csrf::CsrfKey::new().unwrap();
        let member = admin_request();
        // The owner: every row, typed as text, with both forms' tokens.
        let store = Fake::new(Ok(true));
        let pages = Recorded::default();
        let response = show(&store, &pages, &key, COOKIE, &member, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let props = pages.0.lock().unwrap().clone().unwrap();
        assert_eq!(props.organization, "Weekend class");
        assert_eq!(props.summary, "2 members · 2 active");
        assert!(props.has_members && !props.has_notice);
        let mine = &props.members[0];
        assert_eq!(
            (mine.label.as_str(), mine.role.as_str(), mine.note.as_str()),
            ("Victor", "Owner · you", "teacher")
        );
        assert!(!mine.removable, "no Remove on the owner's own row");
        // The owner may lower their own role (another owner must remain,
        // which storage judges), never raise it: the select offers every
        // role at or below owner, the current one selected.
        assert!(mine.changeable && mine.confirm_role);
        assert_eq!(
            mine.options
                .iter()
                .map(|o| (o.value.as_str(), o.selected))
                .collect::<Vec<_>>(),
            [
                ("owner", true),
                ("admin", false),
                ("manager", false),
                ("member", false),
                ("viewer", false)
            ]
        );
        let student = &props.members[1];
        assert_eq!(
            student.label, "Ana <b>",
            "the renderer escapes; props stay text"
        );
        assert_eq!(
            (student.role.as_str(), student.role_code.as_str()),
            ("Member", "member")
        );
        assert_eq!(student.status, "Active");
        assert_eq!(student.consent, "Notice v1, 2026-11-29");
        assert_eq!(student.last_active, "2026-11-29 14:03 UTC");
        assert_eq!(
            (
                student.apps.as_str(),
                student.works.as_str(),
                student.records.as_str(),
                student.uploads.as_str()
            ),
            ("2", "3", "41", "1")
        );
        assert!(student.removable && student.changeable && student.confirm_role);
        assert_eq!(student.action, format!("/admin/members/{STUDENT}/remove"));
        assert_eq!(
            student.role_action,
            format!("/admin/members/{STUDENT}/role")
        );
        assert!(key.verify(csrf::Form::OrgAdminRemove, COOKIE, &student.csrf));
        assert!(key.verify(csrf::Form::OrgAdminRole, COOKIE, &student.csrf_role));
        assert!(!key.verify(csrf::Form::OrgAdminRole, COOKIE, &student.csrf));
        assert!(!key.verify(csrf::Form::SignOut, COOKIE, &student.csrf));
        assert!(key.verify(csrf::Form::SignOut, COOKIE, &props.csrf_signout));
        // A member who is not an admin, and an authority refusal: the uniform
        // 404, and the overview is never read.
        for admin in [Ok(false), Err(storage::Error::Denied)] {
            let store = Fake::new(admin);
            let pages = Recorded::default();
            let response = show(&store, &pages, &key, COOKIE, &member, None).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(store.overviews.load(Ordering::SeqCst), 0);
            assert!(pages.0.lock().unwrap().is_none());
        }
        // Recheck N6: a store outage, before or at the overview, is the
        // plain page with the outage notice and no member data (503), never
        // a raw JSON body; before the overview, nothing is read.
        let outage_page = |pages: &Recorded| {
            let props = pages.0.lock().unwrap().clone().unwrap();
            assert!(props.has_notice && props.notice_kind == "refused");
            assert!(props.notice.starts_with("That could not be completed"));
            assert!(props.members.is_empty() && !props.has_members && !props.empty);
            assert_eq!(props.organization, "Your organization");
        };
        let store = Fake::new(Err(storage::Error::Unavailable));
        let pages = Recorded::default();
        let response = show(&store, &pages, &key, COOKIE, &member, None).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(store.overviews.load(Ordering::SeqCst), 0);
        outage_page(&pages);
        let store = Fake {
            listing: Some(storage::Error::Unavailable),
            ..Fake::new(Ok(true))
        };
        let pages = Recorded::default();
        let response = show(&store, &pages, &key, COOKIE, &member, None).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        outage_page(&pages);
        let store = Fake {
            listing: Some(storage::Error::Denied),
            ..Fake::new(Ok(true))
        };
        let response = show(&store, &Recorded::default(), &key, COOKIE, &member, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// ADR-021: the select offers each caller only the roles it may set on
    /// each row; a row it cannot manage has no form.
    #[test]
    fn the_select_offers_only_the_roles_the_caller_may_set() {
        let key = csrf::CsrfKey::new().unwrap();
        let rows = [
            row(
                "00000000-0000-4000-8000-000000000001",
                "Owner O",
                OrgRole::Owner,
            ),
            row(ADMIN, "Me", OrgRole::Admin),
            row(
                "00000000-0000-4000-8000-000000000003",
                "Manager M",
                OrgRole::Manager,
            ),
            row(STUDENT, "Member S", OrgRole::Member),
            row(
                "00000000-0000-4000-8000-000000000005",
                "Viewer V",
                OrgRole::Viewer,
            ),
        ];
        let offered = |caller: OrgRole| -> Vec<(String, Vec<String>, bool, bool)> {
            props(
                &key,
                COOKIE,
                &request_as(caller),
                String::new(),
                &rows,
                None,
            )
            .members
            .into_iter()
            .map(|m| {
                (
                    m.label,
                    m.options.into_iter().map(|o| o.value).collect(),
                    m.removable,
                    m.confirm_role,
                )
            })
            .collect()
        };
        let roles = |names: &[&str]| names.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        // An admin (the caller's own row is `Me`): managers, members and
        // viewers among manager/member/viewer; nothing on the owner, nothing
        // on their own row (an admin manages below admin), no confirmation box.
        assert_eq!(
            offered(OrgRole::Admin),
            vec![
                ("Owner O".into(), roles(&[]), false, false),
                ("Me".into(), roles(&[]), false, false),
                (
                    "Manager M".into(),
                    roles(&["manager", "member", "viewer"]),
                    true,
                    false
                ),
                (
                    "Member S".into(),
                    roles(&["manager", "member", "viewer"]),
                    true,
                    false
                ),
                (
                    "Viewer V".into(),
                    roles(&["manager", "member", "viewer"]),
                    true,
                    false
                ),
            ]
        );
        // A manager: member and viewer on members and viewers only.
        assert_eq!(
            offered(OrgRole::Manager),
            vec![
                ("Owner O".into(), roles(&[]), false, false),
                ("Me".into(), roles(&[]), false, false),
                ("Manager M".into(), roles(&[]), false, false),
                ("Member S".into(), roles(&["member", "viewer"]), true, false),
                ("Viewer V".into(), roles(&["member", "viewer"]), true, false),
            ]
        );
        // An owner: every role on everyone, with the confirmation box; never
        // Remove on their own row.
        let all = roles(&["owner", "admin", "manager", "member", "viewer"]);
        assert_eq!(
            offered(OrgRole::Owner),
            vec![
                ("Owner O".into(), all.clone(), true, true),
                ("Me".into(), all.clone(), false, true),
                ("Manager M".into(), all.clone(), true, true),
                ("Member S".into(), all.clone(), true, true),
                ("Viewer V".into(), all, true, true),
            ]
        );
        // A removed member gets no form at all.
        let gone = [MemberRow {
            status: "revoked".into(),
            ..row(STUDENT, "Gone", OrgRole::Member)
        }];
        let m = &props(
            &key,
            COOKIE,
            &request_as(OrgRole::Owner),
            String::new(),
            &gone,
            None,
        )
        .members[0];
        assert!(!m.changeable && !m.removable && m.options.is_empty());
    }

    /// ADR-021: the Change role form needs its own token, a known role, the
    /// confirmation for an owner or admin change, and the ladder holds
    /// server-side before storage; a forged form for a disallowed role never
    /// reaches storage.
    #[tokio::test]
    async fn change_role_needs_the_token_the_confirmation_and_the_ladder() {
        let key = csrf::CsrfKey::new().unwrap();
        let token = key.token(csrf::Form::OrgAdminRole, COOKIE);
        let remove_token = key.token(csrf::Form::OrgAdminRemove, COOKIE);
        let owner = request_as(OrgRole::Owner);
        let store = Fake::new(Ok(true));
        // A missing, wrong, other-form or other-session token, an unknown
        // field or a repeated one is 403; storage is never called.
        let other_session = key.token(csrf::Form::OrgAdminRole, "another-cookie");
        for bad in [
            fields(&[("role", "admin")]),
            fields(&[
                ("csrf", &remove_token),
                ("role", "admin"),
                ("confirm", "role"),
            ]),
            fields(&[
                ("csrf", &other_session),
                ("role", "admin"),
                ("confirm", "role"),
            ]),
            fields(&[("csrf", &token), ("role", "admin"), ("principal", STUDENT)]),
            fields(&[("csrf", &token), ("csrf", &token), ("role", "admin")]),
            fields(&[("csrf", &token), ("role", "admin"), ("role", "viewer")]),
        ] {
            let response = set_role(&store, &key, COOKIE, &owner, STUDENT, &bad).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{bad:?}");
        }
        assert!(store.roles.lock().unwrap().is_empty());
        // An unknown role, and an owner or admin change without the box.
        let response = set_role(
            &store,
            &key,
            COOKIE,
            &owner,
            STUDENT,
            &fields(&[("csrf", &token), ("role", "teacher")]),
        )
        .await;
        assert_eq!(location(&response), "/admin?notice=role_unknown");
        for unconfirmed in [
            fields(&[("csrf", &token), ("role", "admin"), ("from", "member")]),
            fields(&[
                ("csrf", &token),
                ("role", "owner"),
                ("from", "member"),
                ("confirm", "yes"),
            ]),
            fields(&[("csrf", &token), ("role", "member"), ("from", "admin")]),
        ] {
            let response = set_role(&store, &key, COOKIE, &owner, STUDENT, &unconfirmed).await;
            assert_eq!(location(&response), "/admin?notice=role_unconfirmed");
        }
        assert!(store.roles.lock().unwrap().is_empty());
        // Member to viewer needs no box; admin with the box goes through.
        let response = set_role(
            &store,
            &key,
            COOKIE,
            &owner,
            STUDENT,
            &fields(&[("csrf", &token), ("role", "viewer"), ("from", "member")]),
        )
        .await;
        assert_eq!(location(&response), "/admin?notice=role_changed");
        let response = set_role(
            &store,
            &key,
            COOKIE,
            &owner,
            STUDENT,
            &fields(&[
                ("confirm", "role"),
                ("role", "admin"),
                ("from", "member"),
                ("csrf", &token),
            ]),
        )
        .await;
        assert_eq!(location(&response), "/admin?notice=role_changed");
        assert_eq!(
            store.roles.lock().unwrap().as_slice(),
            [
                (STUDENT.to_owned(), OrgRole::Viewer),
                (STUDENT.to_owned(), OrgRole::Admin)
            ]
        );
        // The forged form: a manager posting `admin`, an admin posting
        // `owner` or acting on an admin, a manager raising themselves; the
        // Rust ladder refuses before storage.
        for (caller, body) in [
            (
                OrgRole::Manager,
                fields(&[
                    ("csrf", &token),
                    ("role", "admin"),
                    ("from", "member"),
                    ("confirm", "role"),
                ]),
            ),
            (
                OrgRole::Admin,
                fields(&[
                    ("csrf", &token),
                    ("role", "owner"),
                    ("from", "member"),
                    ("confirm", "role"),
                ]),
            ),
            (
                OrgRole::Admin,
                fields(&[
                    ("csrf", &token),
                    ("role", "member"),
                    ("from", "admin"),
                    ("confirm", "role"),
                ]),
            ),
            (
                OrgRole::Manager,
                fields(&[("csrf", &token), ("role", "viewer"), ("from", "manager")]),
            ),
        ] {
            let store = Fake::new(Ok(true));
            let response =
                set_role(&store, &key, COOKIE, &request_as(caller), STUDENT, &body).await;
            assert_eq!(
                location(&response),
                "/admin?notice=role_not_allowed",
                "{caller:?} {body:?}"
            );
            assert!(store.roles.lock().unwrap().is_empty());
        }
        let store = Fake::new(Ok(true));
        let response = set_role(
            &store,
            &key,
            COOKIE,
            &request_as(OrgRole::Admin),
            ADMIN,
            &fields(&[
                ("csrf", &token),
                ("role", "owner"),
                ("from", "admin"),
                ("confirm", "role"),
            ]),
        )
        .await;
        assert_eq!(location(&response), "/admin?notice=role_not_allowed");
        assert!(store.roles.lock().unwrap().is_empty());
        // Without a `from` hint the definer decides: every storage outcome
        // comes back as its plain notice; authority as 404; outages as the
        // reload notice.
        for (outcome, expected) in [
            (Ok(RoleOutcome::Unchanged), "/admin?notice=role_unchanged"),
            (
                Ok(RoleOutcome::Refused("not_allowed")),
                "/admin?notice=role_not_allowed",
            ),
            (
                Ok(RoleOutcome::Refused("last_owner")),
                "/admin?notice=role_last_owner",
            ),
            (Ok(RoleOutcome::Refused("self")), "/admin?notice=role_self"),
            (
                Ok(RoleOutcome::Refused("not_member")),
                "/admin?notice=role_not_member",
            ),
            (
                Ok(RoleOutcome::Refused("unknown_role")),
                "/admin?notice=role_unknown",
            ),
            (
                Err(storage::Error::Unavailable),
                "/admin?notice=unavailable",
            ),
            (
                Err(storage::Error::UnknownOutcome),
                "/admin?notice=unavailable",
            ),
        ] {
            let store = Fake {
                role_outcome: outcome,
                ..Fake::new(Ok(true))
            };
            let response = set_role(
                &store,
                &key,
                COOKIE,
                &owner,
                STUDENT,
                &fields(&[("csrf", &token), ("role", "viewer")]),
            )
            .await;
            assert_eq!(location(&response), expected, "{outcome:?}");
        }
        let store = Fake {
            role_outcome: Err(storage::Error::Denied),
            ..Fake::new(Ok(true))
        };
        let response = set_role(
            &store,
            &key,
            COOKIE,
            &owner,
            STUDENT,
            &fields(&[("csrf", &token), ("role", "viewer")]),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        // The notices, in plain words, from a closed set.
        let words = |query: &str| Notice::parse(query).map(|n| n.words().0);
        assert_eq!(
            words("notice=role_self"),
            Some("You can't raise your own role.")
        );
        assert_eq!(
            words("notice=role_last_owner"),
            Some("You can't change the last owner's role. Make someone else an owner first.")
        );
        assert_eq!(Notice::parse("notice=role_<script>"), None);
        for code in [
            "role_changed",
            "role_unchanged",
            "role_unconfirmed",
            "role_not_allowed",
            "role_last_owner",
            "role_self",
            "role_not_member",
            "role_unknown",
        ] {
            assert_eq!(
                Notice::parse(&format!("notice={code}")).map(Notice::code),
                Some(code)
            );
        }
    }

    #[tokio::test]
    async fn remove_needs_the_token_and_the_confirmation_and_shows_refusals_plainly() {
        let key = csrf::CsrfKey::new().unwrap();
        let member = admin_request();
        let token = key.token(csrf::Form::OrgAdminRemove, COOKIE);
        let store = Fake::new(Ok(true));
        // A missing, wrong, other-form or other-session token is 403, and
        // storage is never called; so is an unknown field.
        let other_form = key.token(csrf::Form::SignOut, COOKIE);
        let role_form = key.token(csrf::Form::OrgAdminRole, COOKIE);
        let other_session = key.token(csrf::Form::OrgAdminRemove, "another-cookie");
        for bad in [
            fields(&[("confirm", "remove")]),
            fields(&[("csrf", "x".repeat(43).as_str()), ("confirm", "remove")]),
            fields(&[("csrf", &other_form), ("confirm", "remove")]),
            fields(&[("csrf", &role_form), ("confirm", "remove")]),
            fields(&[("csrf", &other_session), ("confirm", "remove")]),
            fields(&[
                ("csrf", &token),
                ("confirm", "remove"),
                ("principal", ADMIN),
            ]),
            fields(&[("csrf", &token), ("note", "x")]),
            fields(&[("csrf", &token), ("csrf", &token), ("confirm", "remove")]),
        ] {
            let response = remove(&store, &key, COOKIE, &member, STUDENT, &bad).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{bad:?}");
        }
        assert!(store.removed.lock().unwrap().is_empty());
        // No confirmation: nothing is called, and the page says so.
        for unconfirmed in [
            fields(&[("csrf", &token)]),
            fields(&[("csrf", &token), ("confirm", "yes")]),
        ] {
            let response = remove(&store, &key, COOKIE, &member, STUDENT, &unconfirmed).await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(location(&response), "/admin?notice=unconfirmed");
        }
        assert!(store.removed.lock().unwrap().is_empty());
        let good = fields(&[("csrf", &token), ("confirm", "remove")]);
        let response = remove(&store, &key, COOKIE, &member, STUDENT, &good).await;
        assert_eq!(location(&response), "/admin?notice=removed");
        assert_eq!(store.removed.lock().unwrap().as_slice(), [STUDENT]);
        // The field order is the browser's; either order is the same form.
        let reversed = fields(&[("confirm", "remove"), ("csrf", &token)]);
        let response = remove(&store, &key, COOKIE, &member, STUDENT, &reversed).await;
        assert_eq!(location(&response), "/admin?notice=removed");
        // Storage's refusals come back as plain notices; authority as 404.
        for (outcome, expected) in [
            (Ok(RemoveOutcome::Refused("self")), "/admin?notice=self"),
            (
                Ok(RemoveOutcome::Refused("last_owner")),
                "/admin?notice=last_owner",
            ),
            (
                Ok(RemoveOutcome::Refused("not_member")),
                "/admin?notice=not_member",
            ),
            (
                Ok(RemoveOutcome::Refused("not_allowed")),
                "/admin?notice=not_allowed",
            ),
        ] {
            let store = Fake {
                outcome,
                ..Fake::new(Ok(true))
            };
            let response = remove(&store, &key, COOKIE, &member, ADMIN, &good).await;
            assert_eq!(location(&response), expected);
        }
        let store = Fake {
            outcome: Err(storage::Error::Denied),
            ..Fake::new(Ok(true))
        };
        let response = remove(&store, &key, COOKIE, &member, STUDENT, &good).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        // Recheck N6: an outage, an aborted concurrent removal or an unknown
        // outcome is the plain notice, not a raw JSON body.
        for error in [
            storage::Error::Unavailable,
            storage::Error::Deadline,
            storage::Error::UnknownOutcome,
        ] {
            let store = Fake {
                outcome: Err(error),
                ..Fake::new(Ok(true))
            };
            let response = remove(&store, &key, COOKIE, &member, STUDENT, &good).await;
            assert_eq!(
                location(&response),
                "/admin?notice=unavailable",
                "{error:?}"
            );
        }
        assert_eq!(
            Notice::parse("notice=unavailable").map(|n| n.words().1),
            Some("refused")
        );
        // The notices the page shows, in plain words, from a closed set.
        let words = |query: &str| Notice::parse(query).map(|n| n.words().0);
        assert_eq!(
            words("notice=self"),
            Some("You can't remove yourself. Another owner or admin of this organization can.")
        );
        assert_eq!(
            words("notice=last_owner"),
            Some("You can't remove the last owner of this organization.")
        );
        assert_eq!(Notice::parse("notice=last_admin"), None);
        assert_eq!(Notice::parse("notice=<script>"), None);
        assert_eq!(Notice::parse("notice=removed&x=1"), None);
        let props = props(
            &key,
            COOKIE,
            &member,
            String::new(),
            &[],
            Notice::parse("notice=last_owner"),
        );
        assert!(props.has_notice && props.notice_kind == "refused");
        assert_eq!(props.organization, "Your organization");
        assert!(!props.has_members);
    }

    struct NoPages;
    impl Pages for NoPages {
        fn invite(&self, _: &InviteView) -> Response {
            StatusCode::OK.into_response()
        }
        fn signin(&self, _: &SigninView) -> Response {
            StatusCode::OK.into_response()
        }
        fn pending(&self, _: &PendingView) -> Response {
            StatusCode::OK.into_response()
        }
        fn denied(&self, _: &DeniedView) -> Response {
            StatusCode::OK.into_response()
        }
        fn workspace(&self, _: &WorkspaceView) -> Response {
            StatusCode::OK.into_response()
        }
    }

    /// The routes as mounted, without a store: no session can resolve, so
    /// every caller is a stranger.
    #[tokio::test]
    async fn the_routes_refuse_strangers_and_cross_site_posts_before_anything_else() {
        let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
        let app = http::uniform(router(Arc::new(OrgAdmin {
            auth,
            pages: Arc::new(Recorded::default()),
        })));
        let send = |method: &str, uri: &str, headers: &[(&str, &str)], body: &str| {
            let mut request = HttpRequest::builder().method(method).uri(uri);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let request = request.body(Body::from(body.to_owned())).unwrap();
            let app = app.clone();
            async move {
                let response = app.oneshot(request).await.unwrap();
                let status = response.status();
                let body = to_bytes(response.into_body(), 1 << 16).await.unwrap();
                (status, body)
            }
        };
        let remove = format!("/admin/members/{STUDENT}/remove");
        let role = format!("/admin/members/{STUDENT}/role");
        let form = [
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ];
        for uri in [remove.as_str(), role.as_str()] {
            let (status, cross) = send(
                "POST",
                uri,
                &[("sec-fetch-site", "cross-site")],
                "csrf=x&confirm=remove",
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(&cross[..], br#"{"code":"cross_site","status":403}"#);
        }
        let (_, unknown) = send("GET", "/elsewhere", &[], "").await;
        for (method, uri, headers, body) in [
            ("GET", PATH, &[][..], ""),
            ("GET", "/admin?notice=removed", &[][..], ""),
            ("GET", "/admin?notice=role_changed", &[][..], ""),
            (
                "GET",
                PATH,
                &[("cookie", "__Host-genui_session=abc")][..],
                "",
            ),
            ("POST", remove.as_str(), &form[..], "csrf=x&confirm=remove"),
            (
                "POST",
                role.as_str(),
                &form[..],
                "csrf=x&role=admin&confirm=role",
            ),
            ("POST", PATH, &form[..], ""),
        ] {
            let (status, body) = send(method, uri, headers, body).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
            assert_eq!(body, unknown, "{method} {uri}: the uniform 404");
        }
    }
}
