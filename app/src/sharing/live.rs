//! APP-154: private named app guests and connected placement. Routes carry
//! separate account authority; they never manufacture a workspace request.
use crate::{
    auth::{csrf, shared, Auth},
    http::{self, UniformPath},
    query::{
        canvas,
        page::{self, CardProps, CellProps, Outcome, Reason},
        receipt,
        worker::{Failure, QueryError, INTERACTIVE_DEADLINE},
        Work,
    },
    storage::{self, GuestAccess, LiveShare, ShareAccount},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use forma_ir::PageProps;
use forma_server::{PageHandle, PolicyExtensions, RenderCtx};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedRecordRowProps {
    pub id: String,
    pub version: String,
    pub committed_at: String,
    pub commit_sequence: String,
    pub cells: Vec<CellProps>,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedRecordsCardProps {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub collection: String,
    pub has_rows: bool,
    pub has_no_rows: bool,
    pub columns: Vec<CellProps>,
    pub rows: Vec<SharedRecordRowProps>,
}
// Deliberately narrower than member PartProps: no command identity, form,
// collection, selector, Ask context or member route crosses this boundary.
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedPartProps {
    pub id: String,
    pub title: String,
    pub class: String,
    pub text: String,
    pub is_note: bool,
    pub is_form: bool,
    pub message: String,
    pub views: Vec<CardProps>,
    pub has_record_components: bool,
    pub record_components: Vec<crate::query::record_components::RecordComponentProps>,
}
fn shared_parts(
    outline: &storage::WorkOutline,
    views: &[CardProps],
    record_components: Vec<crate::query::record_components::RecordComponentProps>,
) -> Vec<SharedPartProps> {
    page::document_parts(
        outline,
        views,
        &page::WorkChrome {
            record_components,
            ..Default::default()
        },
    )
    .into_iter()
    .map(|part| SharedPartProps {
        id: part.id,
        title: part.title,
        class: part.class,
        text: part.text,
        is_note: part.is_note,
        is_form: part.is_form,
        message: if part.is_form {
            "This form is view only in a shared app.".into()
        } else {
            String::new()
        },
        views: part.views,
        has_record_components: part.has_record_components,
        record_components: part.record_components,
    })
    .collect()
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedPageProps {
    pub title: String,
    pub presentation: crate::composer::presentation::PresentationProps,
    pub share_id: String,
    pub status: String,
    pub owner_label: String,
    pub expires_at: String,
    pub can_add_to_canvas: bool,
    pub added_to_canvas: bool,
    pub available: bool,
    pub message: String,
    pub needs_sign_in: bool,
    pub can_accept: bool,
    pub sign_in_action: String,
    pub sign_in_csrf: String,
    pub can_sign_up: bool,
    pub can_sign_out: bool,
    pub csrf_signout: String,
    pub parts: Vec<SharedPartProps>,
    pub views: Vec<CardProps>,
    pub record_cards: Vec<SharedRecordsCardProps>,
    pub records_truncated: bool,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedAppProps {
    pub id: String,
    pub title: String,
    pub presentation: crate::composer::presentation::PresentationProps,
    pub href: String,
    pub owner_label: String,
    pub can_add_to_canvas: bool,
    pub parts: Vec<SharedPartProps>,
    pub views: Vec<CardProps>,
    pub record_cards: Vec<SharedRecordsCardProps>,
    pub records_truncated: bool,
}
#[derive(PageProps, Serialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedCanvasProps {
    pub account_label: String,
    pub has_workspace: bool,
    pub can_sign_out: bool,
    pub csrf_signout: String,
    pub has_apps: bool,
    pub apps: Vec<SharedAppProps>,
}

pub trait SharedPages: Send + Sync {
    fn app(&self, props: &SharedPageProps) -> Response;
    fn canvas(&self, props: &SharedCanvasProps) -> Response;
}
pub struct SsrSharedPages {
    /// Trusted configured provider origin, never a share/request parameter.
    /// Browsers apply form-action to the native start form's 303 handoff too.
    pub auth_origin: Option<String>,
    pub app: PageHandle<SharedPageProps>,
    pub canvas: PageHandle<SharedCanvasProps>,
}
impl SharedPages for SsrSharedPages {
    fn app(&self, props: &SharedPageProps) -> Response {
        let mut context = RenderCtx::new().with_title(&props.title);
        if props.needs_sign_in {
            if let Some(origin) = &self.auth_origin {
                context = context.with_policy(PolicyExtensions::new().form_action(origin.clone()));
            }
        }
        self.app
            .render(props, &context)
            .map(IntoResponse::into_response)
            .unwrap_or_else(|_| http::unavailable())
    }
    fn canvas(&self, props: &SharedCanvasProps) -> Response {
        self.canvas
            .render(props, &RenderCtx::new().with_title("Shared apps"))
            .map(IntoResponse::into_response)
            .unwrap_or_else(|_| http::unavailable())
    }
}
pub fn router(work: Arc<Work>) -> Router {
    Router::new()
        .route("/v1/work/{app}/{work}/shares", get(list).post(create))
        .route(
            "/v1/work/{app}/{work}/shares/{share}",
            axum::routing::patch(update).delete(revoke),
        )
        .route("/s/{share}", get(shared_app))
        .route("/shared", get(shared_canvas))
        .route("/v1/shared", get(linked))
        .route("/v1/shared/{share}/authority", get(authority))
        .route(
            "/v1/shared/{share}/record-components",
            get(record_components).post(record_components),
        )
        .route("/v1/shared/{share}/accept", post(accept))
        .route("/v1/shared/{share}/canvas", post(link).delete(unlink))
        .with_state(work)
}
fn response<T: Serialize>(value: &T) -> Response {
    match serde_json::to_value(value) {
        Ok(value) => super::document(StatusCode::OK, &value),
        Err(_) => http::unavailable(),
    }
}
fn refusal(error: storage::Error) -> Response {
    crate::query::routes::storage_refusal(&error)
}
fn locator(key: &str) -> bool {
    key.split_once('.')
        .is_some_and(|(org, id)| storage::valid_identifier(org) && storage::valid_identifier(id))
}
fn summary(share: LiveShare) -> Value {
    // The public share_id is a locator, never a capability. Internal row IDs
    // are not confused with cross-organization resource identifiers.
    let status = share.status.clone();
    json!({"share_id":share.key,"version":share.version,"status":status,"recipient_email":share.recipient_email,"allow_add_to_canvas":share.allow_add_to_canvas,"expires_at":share.expires_at,"href":format!("/s/{}",share.key)})
}
async fn member_write(
    auth: &Auth,
    headers: &axum::http::HeaderMap,
) -> Result<storage::TrustedRequest, Response> {
    if !csrf::same_origin(headers, auth.origin()) {
        return Err(http::cross_site());
    }
    let (_, member) = super::session(auth, headers)
        .await
        .ok_or_else(http::not_found)?;
    auth.limits
        .admit_member_write(member.organization(), member.principal())
        .map_err(http::too_many_requests)?;
    Ok(member)
}
async fn body<T: for<'de> Deserialize<'de>>(request: Request) -> Result<T, Response> {
    if request.uri().query().is_some() {
        return Err(http::not_found());
    }
    if super::content_type(request.headers()) != "application/json" {
        return Err(http::problem(StatusCode::BAD_REQUEST, "malformed_share"));
    }
    let bytes = http::bounded(request.into_body(), http::JSON_BODY_LIMIT)
        .await
        .ok_or_else(http::payload_too_large)?;
    let value = crate::admission::decode(&bytes)
        .map_err(|_| http::problem(StatusCode::BAD_REQUEST, "malformed_share"))?;
    serde_json::from_value(value)
        .map_err(|_| http::problem(StatusCode::BAD_REQUEST, "malformed_share"))
}
async fn list(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let Some((_, member)) = super::session(auth, request.headers()).await else {
        return http::not_found();
    };
    match stores
        .workspace
        .list_live_shares(&member, &app, &saved)
        .await
    {
        Ok(shares) => {
            response(&json!({"shares":shares.into_iter().map(summary).collect::<Vec<_>>()}))
        }
        Err(e) => refusal(e),
    }
}
async fn create(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved)): UniformPath<(String, String)>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let member = match member_write(auth, request.headers()).await {
        Ok(m) => m,
        Err(r) => return r,
    };
    let input: storage::NewLiveShare = match body(request).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    if storage::normalize_share_email(&input.recipient_email).is_none()
        || !(1..=30).contains(&input.expires_in_days)
        || !storage::valid_identifier(&input.idempotency_key)
    {
        return http::problem(StatusCode::BAD_REQUEST, "malformed_share");
    }
    match stores
        .workspace
        .create_live_share(&member, &app, &saved, &input)
        .await
    {
        Ok(s) => response(&json!({"share":summary(s)})),
        Err(e) => refusal(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    expected_version: i64,
    allow_add_to_canvas: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revoke {
    expected_version: i64,
}
fn owner_share_id<'a>(member: &storage::TrustedRequest, key: &'a str) -> Option<&'a str> {
    let (org, id) = key.split_once('.')?;
    (org == member.organization() && storage::valid_identifier(id)).then_some(id)
}
async fn update(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved, key)): UniformPath<(String, String, String)>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let member = match member_write(auth, request.headers()).await {
        Ok(m) => m,
        Err(r) => return r,
    };
    let Some(id) = owner_share_id(&member, &key) else {
        return http::not_found();
    };
    let input: Update = match body(request).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    match stores
        .workspace
        .update_live_share(
            &member,
            &app,
            &saved,
            id,
            input.expected_version,
            input.allow_add_to_canvas,
        )
        .await
    {
        Ok(Some(s)) => response(&json!({"share":summary(s)})),
        Ok(None) => http::problem(StatusCode::CONFLICT, "share_version_conflict"),
        Err(e) => refusal(e),
    }
}
async fn revoke(
    State(work): State<Arc<Work>>,
    UniformPath((app, saved, key)): UniformPath<(String, String, String)>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let member = match member_write(auth, request.headers()).await {
        Ok(m) => m,
        Err(r) => return r,
    };
    let Some(id) = owner_share_id(&member, &key) else {
        return http::not_found();
    };
    let input: Revoke = match body(request).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    match stores
        .workspace
        .revoke_live_share(&member, &app, &saved, id, input.expected_version)
        .await
    {
        Ok(Some(s)) => response(&json!({"share":summary(s)})),
        Ok(None) => http::problem(StatusCode::CONFLICT, "share_version_conflict"),
        Err(e) => refusal(e),
    }
}
async fn account(auth: &Auth, headers: &axum::http::HeaderMap) -> Result<ShareAccount, Response> {
    shared::identity(auth, headers)
        .await
        .ok_or_else(http::not_found)
}
async fn account_write(
    auth: &Auth,
    headers: &axum::http::HeaderMap,
    has_query: bool,
) -> Result<ShareAccount, Response> {
    if has_query {
        return Err(http::not_found());
    }
    if !csrf::same_origin(headers, auth.origin()) {
        return Err(http::cross_site());
    }
    let account = account(auth, headers).await?;
    auth.limits
        .admit_member_write("shared", account.account_id())
        .map_err(http::too_many_requests)?;
    Ok(account)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

async fn accept(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let account =
        match account_write(auth, request.headers(), request.uri().query().is_some()).await {
            Ok(a) => a,
            Err(r) => return r,
        };
    if let Err(refusal) = body::<Empty>(request).await {
        return refusal;
    }
    match stores.workspace.accept_live_share(&account, &key).await {
        Ok(_) => response(&json!({"accepted":true,"href":format!("/s/{key}")})),
        Err(e) => refusal(e),
    }
}
async fn link(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let account =
        match account_write(auth, request.headers(), request.uri().query().is_some()).await {
            Ok(a) => a,
            Err(r) => return r,
        };
    if let Err(refusal) = body::<Empty>(request).await {
        return refusal;
    }
    match stores.workspace.link_shared_app(&account, &key).await {
        Ok(_) => response(&json!({"added":true,"href":"/shared"})),
        Err(e) => refusal(e),
    }
}
async fn unlink(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let account =
        match account_write(auth, request.headers(), request.uri().query().is_some()).await {
            Ok(a) => a,
            Err(r) => return r,
        };
    if let Err(refusal) = body::<Empty>(request).await {
        return refusal;
    }
    match stores.workspace.unlink_shared_app(&account, &key).await {
        Ok(()) => response(&json!({"removed":true})),
        Err(e) => refusal(e),
    }
}
async fn app_entries(work: &Work, account: &ShareAccount) -> Result<Vec<SharedAppProps>, Response> {
    let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
    let links = stores
        .workspace
        .linked_apps(account)
        .await
        .map_err(refusal)?;
    Ok(links
        .into_iter()
        .map(|a| SharedAppProps {
            id: a.key.clone(),
            title: a.title,
            href: format!("/s/{}", a.key),
            owner_label: "Shared app".into(),
            can_add_to_canvas: a.can_add_to_canvas,
            ..Default::default()
        })
        .collect())
}
async fn linked(State(work): State<Arc<Work>>, request: Request) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let account = match account(work.auth(), request.headers()).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    match app_entries(&work, &account).await {
        Ok(apps) => response(&json!({"apps":apps})),
        Err(r) => r,
    }
}
async fn shared_canvas(State(work): State<Arc<Work>>, request: Request) -> Response {
    if request.uri().query().is_some() {
        return http::not_found();
    }
    let account = match account(work.auth(), request.headers()).await {
        Ok(a) => a,
        Err(_) => {
            let Some(pages) = &work.shared_pages else {
                return http::unavailable();
            };
            return secure_page(pages.canvas(&SharedCanvasProps {
                account_label: "You are signed out. Open a shared app link to sign in.".into(),
                ..Default::default()
            }));
        }
    };
    let apps = match app_entries(&work, &account).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let props = SharedCanvasProps {
        account_label: "Your shared apps".into(),
        has_workspace: super::session(work.auth(), request.headers())
            .await
            .is_some(),
        can_sign_out: true,
        csrf_signout: shared::signout_csrf(work.auth(), request.headers()),
        has_apps: !apps.is_empty(),
        apps,
    };
    let Some(pages) = &work.shared_pages else {
        return http::unavailable();
    };
    secure_page(pages.canvas(&props))
}
fn secure_page(mut response: Response) -> Response {
    http::secure(response.headers_mut());
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Accept"));
    response.headers_mut().insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow, noarchive"),
    );
    response
}
fn page_response(
    work: &Work,
    headers: &axum::http::HeaderMap,
    props: &SharedPageProps,
) -> Response {
    let response =
        if headers.get(header::ACCEPT).and_then(|h| h.to_str().ok()) == Some("application/json") {
            response(props)
        } else {
            let Some(pages) = &work.shared_pages else {
                return http::unavailable();
            };
            pages.app(props)
        };
    secure_page(response)
}
async fn shared_app(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    let pending = request.uri().query() == Some("verification=pending");
    if !locator(&key) || (request.uri().query().is_some() && !pending) {
        return http::not_found();
    }
    let auth = work.auth();
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let neutral = SharedPageProps {
        title: "Shared app".into(),
        share_id: key.clone(),
        status: "restricted".into(),
        needs_sign_in: true,
        sign_in_action: format!("/auth/shared/{key}/start"),
        sign_in_csrf: shared::start_csrf(auth, request.headers(), &key),
        can_sign_up: !pending
            && auth.shared_broker.is_some()
            && key
                .split_once('.')
                .and_then(|(organization, _)| auth.provider.as_ref()?.config.route(organization))
                .is_some_and(|route| route.shared_client_id.is_some())
            && matches!(
                shared::home_organization(auth, request.headers()).await,
                Ok(None)
            ),
        message: if pending {
            "Verify your email, then sign in again to open this app."
        } else {
            "Sign in with the account this app was shared with."
        }
        .into(),
        ..Default::default()
    };
    let Some(account) = shared::identity(auth, request.headers()).await else {
        return page_response(&work, request.headers(), &neutral);
    };
    let mut neutral = neutral;
    neutral.can_sign_out = true;
    neutral.csrf_signout = shared::signout_csrf(auth, request.headers());
    let access=match stores.workspace.resolve_guest_access(&account,&key).await{
        Ok(a)=>a,
        Err(storage::Error::Denied)=>{
            return match stores.workspace.pending_share(&account,&key).await{
                Ok(true)=>page_response(&work,request.headers(),&SharedPageProps{needs_sign_in:false,can_accept:true,status:"pending".into(),message:"Accept this invitation to view the live app. This does not join its organization.".into(),..neutral}),
                Ok(false)|Err(storage::Error::Denied)=>{
                    if request.headers().get(header::ACCEPT).and_then(|v|v.to_str().ok())==Some("application/json") { http::not_found() }
                    else { page_response(&work, request.headers(), &neutral) }
                },Err(e)=>refusal(e),
            }
        },Err(e)=>return refusal(e),
    };
    let mut props = match guest_document(work.clone(), &access).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    let links = match stores.workspace.linked_apps(&account).await {
        Ok(l) => l,
        Err(e) => return refusal(e),
    };
    props.added_to_canvas = links.iter().any(|l| l.key == key);
    props.can_sign_out = true;
    props.csrf_signout = shared::signout_csrf(auth, request.headers());
    if let Err(e) = stores.data.release_guest_access(&access).await {
        return refusal(e);
    }
    page_response(&work, request.headers(), &props)
}
async fn guest_document(
    work: Arc<Work>,
    access: &GuestAccess,
) -> Result<SharedPageProps, Response> {
    let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
    let read = stores
        .data
        .guest_work_reads(access)
        .await
        .map_err(refusal)?;
    let outline = read.outline.map_err(refusal)?;
    let listing = read
        .records
        .ok_or_else(http::unavailable)?
        .map_err(refusal)?;
    let mut tasks = tokio::task::JoinSet::new();
    for (i, view) in outline.views.iter().cloned().enumerate() {
        let work = work.clone();
        let access = access.clone();
        tasks.spawn(async move { (i, guest_view(&work, &access, &view).await) });
    }
    let mut views: Vec<Option<CardProps>> = vec![None; outline.views.len()];
    while let Some(result) = tasks.join_next().await {
        let (i, card) = result.map_err(|_| http::unavailable())?;
        views[i] = Some(card?);
    }
    let record_cards: Vec<SharedRecordsCardProps> = page::automatic_records(
        &outline,
        canvas::records_cards(access.app(), access.work(), &listing, access.account_id()),
    )
    .into_iter()
    .map(|c| SharedRecordsCardProps {
        id: c.id,
        title: c.title,
        summary: c.summary,
        collection: c.collection,
        has_rows: c.has_rows,
        has_no_rows: c.has_no_rows,
        columns: c.columns,
        rows: c
            .rows
            .into_iter()
            .map(|r| SharedRecordRowProps {
                id: r.id,
                version: r.version,
                committed_at: r.committed_at,
                commit_sequence: r.commit_sequence,
                cells: r.cells,
            })
            .collect(),
    })
    .collect();
    let record_components = if outline
        .parts
        .iter()
        .any(|part| crate::query::record_routes::is_record_kind(&part.kind))
    {
        match crate::query::record_routes::guest_components(
            &work,
            access,
            &crate::query::record_components::default_parameters(),
        )
        .await
        {
            Ok(batch) => batch.components,
            Err(response) if response.status() == StatusCode::NOT_FOUND => return Err(response),
            Err(_) => crate::query::record_routes::unavailable_outline(&outline),
        }
    } else {
        Vec::new()
    };
    stores
        .data
        .release_guest_access(access)
        .await
        .map_err(refusal)?;
    let views = views
        .into_iter()
        .map(|v| v.ok_or_else(http::unavailable))
        .collect::<Result<Vec<_>, _>>()?;
    let parts = shared_parts(&outline, &views, record_components);
    Ok(SharedPageProps {
        presentation: crate::composer::presentation::project(
            outline.presentation.as_ref(),
            &outline
                .parts
                .iter()
                .filter(|part| part.kind != "form" && access.view_ids().contains(&part.id))
                .map(|part| part.id.clone())
                .collect(),
        ),
        title: outline.title,
        share_id: access.key(),
        status: "active".into(),
        owner_label: access.owner_label().into(),
        expires_at: access.expires_at().into(),
        can_add_to_canvas: access.can_add_to_canvas(),
        available: true,
        parts,
        views,
        records_truncated: listing.truncated && !record_cards.is_empty(),
        record_cards,
        ..Default::default()
    })
}
async fn guest_view(
    work: &Work,
    access: &GuestAccess,
    view: &storage::WorkView,
) -> Result<CardProps, Response> {
    let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
    let run = stores
        .data
        .guest_run_query(access, &view.id, &json!({}))
        .await
        .map_err(refusal)?;
    let Some(executor) = work.executor() else {
        return Ok(page::card(view, Outcome::Failed(Reason::Store)));
    };
    let card = match executor
        .execute(run.engine_plan(), INTERACTIVE_DEADLINE)
        .await
    {
        Ok(result) => {
            let receipt = receipt::build_shared(&run, &result).ok_or_else(http::unavailable)?;
            stores
                .data
                .release_guest_result(access, &run)
                .await
                .map_err(refusal)?;
            page::card(view, Outcome::Released(&result, &receipt))
        }
        Err(QueryError::Busy | QueryError::Failed(Failure::QueueExpired)) => {
            page::card(view, Outcome::Loading)
        }
        Err(error) => page::card(
            view,
            Outcome::Failed(match error {
                QueryError::NarrowTheQuery => Reason::NarrowTheQuery,
                QueryError::Refused(_) => Reason::NotQualified,
                QueryError::SourceUnavailable => Reason::SourceUnavailable,
                QueryError::Failed(Failure::Deadline | Failure::Killed) => Reason::Deadline,
                _ => Reason::Engine,
            }),
        ),
    };
    Ok(card)
}

/// Record filters never turn an account-only guest into a workspace member.
async fn record_components(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() || !locator(&key) {
        return http::not_found();
    }
    if request.method() == axum::http::Method::POST
        && !csrf::same_origin(request.headers(), work.auth().origin())
    {
        return http::cross_site();
    }
    let account = match account(work.auth(), request.headers()).await {
        Ok(account) => account,
        Err(response) => return response,
    };
    let Some(stores) = &work.auth().stores else {
        return http::not_found();
    };
    let access = match stores.workspace.resolve_guest_access(&account, &key).await {
        Ok(access) => access,
        Err(error) => return refusal(error),
    };
    let parameters =
        match crate::query::record_routes::parameters(request, work.auth().origin()).await {
            Ok(value) => value,
            Err(response) => return response,
        };
    match crate::query::record_routes::guest_components(&work, &access, &parameters).await {
        Ok(batch) => crate::query::record_routes::response(&batch),
        Err(response) => response,
    }
}

/// Lightweight authorization lease; it never dispatches a data query. The
/// browser's 2s poll + 2s timeout + 1s clearing budget implements ADR-002.
async fn authority(
    State(work): State<Arc<Work>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    if request.uri().query().is_some() || !locator(&key) {
        return http::not_found();
    }
    let (parts, _) = request.into_parts();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let account = account(work.auth(), &parts.headers).await?;
        let stores = work.auth().stores.as_ref().ok_or_else(http::not_found)?;
        let access = stores.workspace.resolve_guest_access(&account, &key).await.map_err(refusal)?;
        stores.data.release_guest_access(&access).await.map_err(refusal)?;
        Ok::<_, Response>(response(&json!({"available":true,"version":access.share_version(),"can_add_to_canvas":access.can_add_to_canvas()})))
    }).await;
    match result {
        Ok(Ok(response)) => secure_page(response),
        Ok(Err(response)) => response,
        Err(_) => http::unavailable(),
    }
}
