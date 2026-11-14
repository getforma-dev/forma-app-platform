//! `GET /admin` and `POST /admin/members/{principal}/remove` (D4 of the
//! trial plan): the organization's own admin area, in the product, beside
//! the operator's audited back office. Server-rendered (`orgAdmin` in
//! `admin/build.ts`), no script.
//!
//! Order, both routes: (`POST`: fetch metadata, else 403 `cross_site`) → a
//! live workspace session (`query::routes::member`; else the uniform 404) →
//! the query string (`GET`: none, or exactly `notice=<code>` from the closed
//! set below; `POST`: none; else the uniform 404) → `is_org_admin` (false,
//! or any authority refusal: the uniform 404, before any member data is read;
//! a store outage 503) → `POST`: the form body (at most 4 KiB, `csrf` and
//! `confirm` only; a missing or wrong synchronizer token 403 `cross_site`,
//! tied to the session cookie and `csrf::Form::OrgAdminRemove`) → the
//! storage call.
//!
//! Remove answers `303` to `/admin?notice=<code>`: `removed`, `unconfirmed`
//! (the confirmation box was not ticked; nothing was called), or the refusal
//! storage named (`self`, `last_admin`, `not_member`), each shown in plain
//! words on the page. A removal ends the member's sessions at their next
//! request (storage advances the epoch). Logs carry identities and codes;
//! never a name or a note.
use crate::{
    auth::{csrf, Auth},
    http::{self, UniformPath},
    query::routes::member,
    storage::{self, MemberRow, RemoveOutcome, TrustedRequest, Workspace},
};
use axum::{
    extract::{Request, State},
    response::Response,
    routing::{get, post},
    Router,
};
use forma_ir::PageProps;
use forma_server::{PageHandle, RenderCtx};
use std::{future::Future, pin::Pin, sync::Arc};

pub const PATH: &str = "/admin";
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
    /// "Admin", "Member", with " · you" on the viewer's own row.
    pub role: String,
    pub consent: String,
    pub last_active: String,
    pub apps: String,
    pub works: String,
    pub records: String,
    pub uploads: String,
    /// An active member: the Remove form is offered (storage still judges
    /// `self` and `last_admin`).
    pub removable: bool,
    pub action: String,
    pub csrf: String,
    pub confirm_label: String,
}

/// What `?notice=` may name, and the words the page shows for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Notice {
    Removed,
    Unconfirmed,
    Refused(&'static str),
}
impl Notice {
    fn parse(query: &str) -> Option<Self> {
        match query.strip_prefix("notice=")? {
            "removed" => Some(Self::Removed),
            "unconfirmed" => Some(Self::Unconfirmed),
            "self" => Some(Self::Refused("self")),
            "last_admin" => Some(Self::Refused("last_admin")),
            "not_member" => Some(Self::Refused("not_member")),
            _ => None,
        }
    }
    fn code(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Unconfirmed => "unconfirmed",
            Self::Refused(code) => code,
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
                "You can't remove yourself. Another admin of this organization can.",
                "refused",
            ),
            Self::Refused("last_admin") => (
                "You can't remove the last admin of this organization.",
                "refused",
            ),
            Self::Refused(_) => ("That person is no longer an active member.", "refused"),
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
        crate::pages::respond(
            PATH,
            self.page
                .render(props, &RenderCtx::new().with_title("Organization admin")),
        )
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

/// The members table for an organization admin; the uniform 404 for anyone
/// else, decided before any member data is read.
pub(crate) async fn show(
    store: &dyn AdminStore,
    pages: &dyn AdminPages,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    notice: Option<Notice>,
) -> Response {
    if let Err(refusal) = admin(store, member).await {
        return refusal;
    }
    let rows = match store.overview(member).await {
        Ok(rows) => rows,
        Err(storage::Error::Denied) => return http::not_found(),
        Err(_) => return http::unavailable(),
    };
    let organization = store.organization_name(member).await.unwrap_or_default();
    tracing::info!(
        event = "org_admin.viewed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        members = rows.len(),
        notice = notice.map(Notice::code),
        "organization admin page"
    );
    pages.page(&props(csrf, cookie, member, organization, &rows, notice))
}

/// `POST /admin/members/{principal}/remove`.
async fn remove_route(
    State(state): State<Arc<OrgAdmin>>,
    UniformPath(principal): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    let (cookie, member) = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    if parts.uri.query().is_some() || !storage::valid_identifier(&principal) {
        return http::not_found();
    }
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    if let Err(refusal) = admin(&stores.workspace, &member).await {
        return refusal;
    }
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
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

/// The removal of `principal` by an admin already judged (`admin`): the
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
    let count = |field: &str| fields.iter().filter(|(name, _)| name == field).count();
    let value = |field: &str| {
        fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value.as_str())
    };
    let shaped = fields
        .iter()
        .all(|(name, _)| name == "csrf" || name == "confirm")
        && count("csrf") == 1
        && count("confirm") <= 1;
    let token = value("csrf").unwrap_or("");
    if !shaped || !csrf.verify(csrf::Form::OrgAdminRemove, cookie, token) {
        return http::cross_site();
    }
    let confirmed = value("confirm") == Some("remove");
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
        Err(_) => http::unavailable(),
    }
}

/// `Ok` for an admin of the request's organization; the uniform 404 for
/// anyone else (any authority refusal included), 503 for a store outage.
async fn admin(store: &dyn AdminStore, member: &TrustedRequest) -> Result<(), Response> {
    match store.is_org_admin(member).await {
        Ok(true) => Ok(()),
        Ok(false) | Err(storage::Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

fn notice(notice: Notice) -> Response {
    http::see_other(&format!("{PATH}?notice={}", notice.code()))
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
    let active = rows.iter().filter(|row| row.status == "active").count();
    let members: Vec<AdminMemberProps> = rows
        .iter()
        .map(|row| {
            let mine = row.principal == member.principal();
            let role = match (row.is_admin, mine) {
                (true, true) => "Admin · you",
                (true, false) => "Admin",
                (false, true) => "Member · you",
                (false, false) => "Member",
            };
            AdminMemberProps {
                id: row.principal.clone(),
                label: row.label.clone(),
                note: row.note.clone().unwrap_or_default(),
                status: match row.status.as_str() {
                    "active" => "Active".into(),
                    "revoked" => "Removed".into(),
                    other => other.to_owned(),
                },
                role: role.into(),
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
                removable: row.status == "active",
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
        outcome: Result<RemoveOutcome, storage::Error>,
        overviews: AtomicUsize,
        removed: Mutex<Vec<String>>,
    }
    impl Fake {
        fn new(admin: Result<bool, storage::Error>) -> Self {
            Self {
                admin,
                rows: vec![row(ADMIN, "Victor", true), row(STUDENT, "Ana <b>", false)],
                outcome: Ok(RemoveOutcome::Removed),
                overviews: AtomicUsize::new(0),
                removed: Mutex::new(Vec::new()),
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
            Box::pin(async move { Ok(rows) })
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

    fn row(principal: &str, label: &str, is_admin: bool) -> MemberRow {
        MemberRow {
            principal: principal.into(),
            label: label.into(),
            note: is_admin.then(|| "teacher".to_owned()),
            status: "active".into(),
            is_admin,
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
        storage::AgentCaller::for_tests(ORG, ADMIN, "t").request
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
        // The admin: every row, typed as text, with the Remove form's token.
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
            ("Victor", "Admin · you", "teacher")
        );
        let student = &props.members[1];
        assert_eq!(
            student.label, "Ana <b>",
            "the renderer escapes; props stay text"
        );
        assert_eq!(student.role, "Member");
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
        assert!(student.removable);
        assert_eq!(student.action, format!("/admin/members/{STUDENT}/remove"));
        assert!(key.verify(csrf::Form::OrgAdminRemove, COOKIE, &student.csrf));
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
        // A store outage is 503, still before the overview.
        let store = Fake::new(Err(storage::Error::Unavailable));
        let response = show(&store, &Recorded::default(), &key, COOKIE, &member, None).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(store.overviews.load(Ordering::SeqCst), 0);
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
        let other_session = key.token(csrf::Form::OrgAdminRemove, "another-cookie");
        for bad in [
            fields(&[("confirm", "remove")]),
            fields(&[("csrf", "x".repeat(43).as_str()), ("confirm", "remove")]),
            fields(&[("csrf", &other_form), ("confirm", "remove")]),
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
                Ok(RemoveOutcome::Refused("last_admin")),
                "/admin?notice=last_admin",
            ),
            (
                Ok(RemoveOutcome::Refused("not_member")),
                "/admin?notice=not_member",
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
        // The notices the page shows, in plain words, from a closed set.
        let words = |query: &str| Notice::parse(query).map(|n| n.words().0);
        assert_eq!(
            words("notice=self"),
            Some("You can't remove yourself. Another admin of this organization can.")
        );
        assert_eq!(
            words("notice=last_admin"),
            Some("You can't remove the last admin of this organization.")
        );
        assert_eq!(Notice::parse("notice=<script>"), None);
        assert_eq!(Notice::parse("notice=removed&x=1"), None);
        let props = props(
            &key,
            COOKIE,
            &member,
            String::new(),
            &[],
            Notice::parse("notice=last_admin"),
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
        let form = [
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ];
        let (status, cross) = send(
            "POST",
            &remove,
            &[("sec-fetch-site", "cross-site")],
            "csrf=x&confirm=remove",
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(&cross[..], br#"{"code":"cross_site","status":403}"#);
        let (_, unknown) = send("GET", "/elsewhere", &[], "").await;
        for (method, uri, headers, body) in [
            ("GET", PATH, &[][..], ""),
            ("GET", "/admin?notice=removed", &[][..], ""),
            (
                "GET",
                PATH,
                &[("cookie", "__Host-genui_session=abc")][..],
                "",
            ),
            ("POST", remove.as_str(), &form[..], "csrf=x&confirm=remove"),
            ("POST", PATH, &form[..], ""),
        ] {
            let (status, body) = send(method, uri, headers, body).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
            assert_eq!(body, unknown, "{method} {uri}: the uniform 404");
        }
    }
}
