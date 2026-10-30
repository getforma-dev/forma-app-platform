//! ADR-006 routes, exactly: `GET /signin`, `GET|POST /signin/{handle}`,
//! `GET /auth/callback`, `GET /auth/pending`, `POST /auth/pending/resend`,
//! `POST /auth/signout`, `POST /auth/signout-all`, and the protected
//! workspace entry `GET /workspace`, and APP-074's `GET /invite` and `POST
//! /invite/validate` (`invite.rs`). Refusal order for a `POST`: fetch
//! metadata (403), then the limit (429), then the bounded body (413), then the
//! synchronizer token (403), and only then any hashing, database or provider
//! work. Every protected refusal is the uniform 404.
use super::{
    cookies, csrf, invite, pending, pkce::Pkce, Auth, DeniedView, Route, SigninView, WorkspaceView,
};
use crate::{
    http::{self, limits},
    storage::{handle_digest, Completion, Error, FlowKind, Lookup, PendingAdmission},
};
use axum::{
    body::Body,
    extract::{rejection::PathRejection, ConnectInfo, Path, Request, State},
    http::{header, request::Parts, HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, sync::Arc};

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/invite", get(invite::entry))
        .route("/invite/validate", post(invite::validate))
        .route("/signin", get(signin_page))
        .route(
            "/signin/{handle}",
            get(signin_handle_page).post(signin_start),
        )
        .route("/auth/callback", get(callback))
        .route("/auth/pending", get(pending::page))
        .route("/auth/pending/resend", post(pending::resend))
        .route("/auth/signout", post(signout))
        .route("/auth/signout-all", post(signout_all))
        .route("/workspace", get(workspace))
        .with_state(auth)
}

pub(crate) fn peer(parts: &Parts) -> Option<SocketAddr> {
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0)
}

/// Sets the response status only over a successful render; a render failure
/// keeps its own 500.
pub(crate) fn with_status(mut response: Response, status: StatusCode) -> Response {
    if response.status() == StatusCode::OK {
        *response.status_mut() = status;
    }
    response
}
pub(crate) fn add_cookie(response: &mut Response, value: HeaderValue) {
    response.headers_mut().append(header::SET_COOKIE, value);
}
pub(crate) fn clear_all(response: &mut Response) {
    for name in [cookies::PRELOGIN, cookies::PRE_ACCESS, cookies::SESSION] {
        add_cookie(response, cookies::clear(name));
    }
}

/// A well-formed sign-in handle (no lookup: shape only).
fn handle_action(handle: &str) -> Option<String> {
    (handle.len() <= 64 && handle_digest(handle).is_some()).then(|| format!("/signin/{handle}"))
}

fn signin(auth: &Auth, action: Option<String>, failed: bool) -> Response {
    let response = auth.pages.signin(&SigninView { action, failed });
    if failed {
        with_status(response, StatusCode::BAD_REQUEST)
    } else {
        response
    }
}
/// The uniform neutral failure of the anonymous sign-in steps.
pub(crate) fn neutral_failure(auth: &Auth) -> Response {
    signin(auth, None, true)
}

async fn signin_page(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let hint = cookies::read(request.headers(), cookies::ORGANIZATION_HINT);
    signin(&auth, hint.as_deref().and_then(handle_action), false)
}
/// A path the extractor cannot decode (invalid percent-encoding or UTF-8)
/// is no handle at all: the same page as a malformed one, never axum's
/// plain-text rejection (APP-009 review m2).
fn handle_of(path: Result<Path<String>, PathRejection>) -> String {
    path.map(|Path(handle)| handle).unwrap_or_default()
}

async fn signin_handle_page(
    State(auth): State<Arc<Auth>>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    signin(&auth, handle_action(&handle_of(path)), false)
}

/// The fresh server-side flow and the PAR hand-off (ADR-006 `POST
/// /signin/{handle}`); `Err` is the uniform neutral failure.
async fn start(auth: &Auth, handle: &str, class: &str) -> Result<(String, String), ()> {
    let provider = auth.provider.as_ref().ok_or(())?;
    let stores = auth.stores.as_ref().ok_or(())?;
    let digest = handle_digest(handle).ok_or(())?;
    let organization = stores
        .workspace
        .resolve_handle(&digest)
        .await
        .map_err(|_| ())?
        .ok_or(())?;
    let Route {
        tenant_slug,
        client_id,
        ..
    } = provider.config.route(&organization).ok_or(())?;
    let pkce = Pkce::fresh().ok_or(())?;
    let sealed = auth
        .sealer
        .seal(&verifier_context(&organization), pkce.verifier.as_bytes())
        .ok_or(())?;
    let flow = stores
        .workspace
        .begin_flow(&organization, FlowKind::SignIn, &sealed, class)
        .await
        .map_err(|_| ())?
        .ok_or(())?;
    match provider
        .client
        .par(
            tenant_slug,
            client_id,
            &provider.config.callback,
            &flow.state,
            &pkce.challenge,
            None,
        )
        .await
    {
        Ok(request_uri) => Ok((
            flow.cookie,
            provider
                .client
                .authorize_url(tenant_slug, client_id, &request_uri),
        )),
        Err(failure) => {
            tracing::info!(failure = ?failure, "auth.par");
            let _ = stores.workspace.discard_flow(&flow.cookie).await;
            Err(())
        }
    }
}
pub(crate) fn verifier_context(organization: &str) -> Vec<u8> {
    format!("forma:prelogin-verifier:v1\0{organization}").into_bytes()
}

async fn signin_start(
    State(auth): State<Arc<Auth>>,
    path: Result<Path<String>, PathRejection>,
    request: Request,
) -> Response {
    let handle = handle_of(path);
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let class = auth.address_class(&parts);
    if let Err(retry) = auth.limits.admit(&[
        (&format!("signin:{class}"), limits::SIGNIN_ADDRESS),
        ("signin:global", limits::SIGNIN_GLOBAL),
    ]) {
        return http::too_many_requests(retry);
    }
    if http::form(body).await.is_none() {
        return http::payload_too_large();
    }
    if handle_action(&handle).is_none() {
        return neutral_failure(&auth);
    }
    match start(&auth, &handle, &class).await {
        Ok((cookie, location)) => {
            let mut response = http::see_other(&location);
            add_cookie(&mut response, cookies::set(cookies::PRELOGIN, &cookie));
            add_cookie(
                &mut response,
                cookies::set(cookies::ORGANIZATION_HINT, &handle),
            );
            response
        }
        Err(()) => neutral_failure(&auth),
    }
}

/// The two callback parameters, each exactly once and canonical.
fn callback_query(query: Option<&str>) -> Option<(String, String)> {
    let query = query?;
    if query.len() > 256 {
        return None;
    }
    let (mut code, mut state) = (None, None);
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if !crate::storage::capability_shaped(value) {
            return None;
        }
        match name {
            "code" if code.is_none() => code = Some(value.to_owned()),
            "state" if state.is_none() => state = Some(value.to_owned()),
            _ => return None,
        }
    }
    Some((code?, state?))
}

enum Arrival {
    Pending(String),
    Workspace(String),
    Denied(String),
}

/// ADR-006 `GET /auth/callback`: consume the flow, exchange, verify, mint
/// the pre-access session, then admit or mint the workspace session.
async fn arrive(auth: &Auth, cookie: &str, code: &str, state: &str) -> Result<Arrival, ()> {
    let provider = auth.provider.as_ref().ok_or(())?;
    let stores = auth.stores.as_ref().ok_or(())?;
    let flow = stores
        .workspace
        .complete_flow(cookie, state)
        .await
        .map_err(|_| ())?;
    let organization = flow.organization().to_owned();
    let verifier = auth
        .sealer
        .open(&verifier_context(&organization), flow.sealed_verifier())
        .and_then(|v| String::from_utf8(v).ok())
        .ok_or(())?;
    let route = provider.config.route(&organization).ok_or(())?;
    let expected_tenant = stores
        .workspace
        .provider_tenant(&organization, &provider.config.issuer)
        .ok_or(())?;
    let token = provider
        .client
        .exchange(
            &route.tenant_slug,
            &route.client_id,
            &provider.config.callback,
            code,
            &verifier,
        )
        .await
        .map_err(|failure| tracing::info!(failure = ?failure, "auth.exchange"))?;
    // From here a provider session exists: every refusal revokes it.
    let shaped = super::token_shaped(&token);
    let identity = if shaped {
        provider
            .verifier
            .verify(&token.access_token, &expected_tenant)
            .await
            .ok()
    } else {
        None
    };
    let Some(identity) = identity.filter(|identity| {
        identity.issuer == provider.config.issuer
            && identity.tenant_id == expected_tenant
            && identity.subject == token.user.id
            && identity.expires_at == token.expires_at
            && identity.expires_at.saturating_sub(identity.issued_at) == token.expires_in
    }) else {
        tracing::info!("auth.identity_refused");
        if shaped {
            let _ = provider
                .client
                .logout(&route.tenant_slug, &token.refresh_token)
                .await;
        }
        return Err(());
    };
    let Some(reference) = auth.vault.insert(&route.tenant_slug, &token) else {
        let _ = provider
            .client
            .logout(&route.tenant_slug, &token.refresh_token)
            .await;
        return Err(());
    };
    let email_verified = token.user.email_verified;
    drop(token);
    let started = match stores
        .workspace
        .mint_pre_access(&flow, &identity, email_verified, &reference)
        .await
    {
        Ok(started) => started,
        Err(_) => {
            if let Some((slug, refresh)) = auth.vault.take(&reference).await {
                let _ = provider.client.logout(&slug, &refresh).await;
            }
            return Err(());
        }
    };
    let refused_signup = matches!(
        started.signup,
        Some(
            PendingAdmission::ReservationExpired
                | PendingAdmission::Expired
                | PendingAdmission::Revoked
                | PendingAdmission::Exhausted
        )
    );
    if refused_signup {
        return Ok(Arrival::Denied(started.cookie));
    }
    if !email_verified {
        return Ok(Arrival::Pending(started.cookie));
    }
    // A refusal here (no admission, a revoked entitlement under the fence, a
    // store failure) leaves the verified identity with the neutral denial.
    let outcome = match stores.workspace.complete_admission(&started.pre).await {
        Ok(Completion::Member(member)) => stores
            .workspace
            .mint_session(&started.pre, &member)
            .await
            .map(Some),
        Ok(Completion::Denied) => Ok(None),
        Err(error) => Err(error),
    };
    match outcome {
        Ok(Some(session)) => Ok(Arrival::Workspace(session)),
        Ok(None) => {
            tracing::info!("auth.admission_denied");
            Ok(Arrival::Denied(started.cookie))
        }
        Err(error) => {
            tracing::info!(code = %error, "auth.mint_refused");
            Ok(Arrival::Denied(started.cookie))
        }
    }
}

async fn callback(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let failed = |auth: &Auth| {
        let mut response = neutral_failure(auth);
        add_cookie(&mut response, cookies::clear(cookies::PRELOGIN));
        response
    };
    let (Some((code, state)), Some(cookie)) = (
        callback_query(parts.uri.query()),
        cookies::read(&parts.headers, cookies::PRELOGIN),
    ) else {
        return failed(&auth);
    };
    let arrival = arrive(&auth, &cookie, &code, &state).await;
    // One pre-access record and one workspace session per browser: a new
    // arrival replaces (destroys and revokes) the records the browser's
    // earlier cookies named; an earlier workspace session ends exactly as a
    // sign-out does, with its epoch advance (APP-009 review m1).
    if let (Ok(_), Some(stores)) = (&arrival, &auth.stores) {
        if let Some(earlier) = cookies::read(&parts.headers, cookies::PRE_ACCESS) {
            if let Ok(Some(ended)) = stores.workspace.destroy_pre_access(&earlier).await {
                auth.revoke(&ended).await;
            }
        }
        if let Some(earlier) = cookies::read(&parts.headers, cookies::SESSION) {
            match stores.workspace.sign_out(&earlier).await {
                Ok(Some(signed)) => {
                    tracing::info!(epoch = signed.epoch, "auth.superseded");
                    auth.revoke(&signed.ended).await;
                }
                Ok(None) => {}
                Err(error) => tracing::info!(code = %error, "auth.supersede_failed"),
            }
        }
    }
    match arrival {
        Ok(Arrival::Pending(pre)) => {
            let mut response = http::see_other("/auth/pending");
            add_cookie(&mut response, cookies::clear(cookies::PRELOGIN));
            add_cookie(&mut response, cookies::clear(cookies::SESSION));
            add_cookie(&mut response, cookies::set(cookies::PRE_ACCESS, &pre));
            response
        }
        Ok(Arrival::Workspace(session)) => {
            let mut response = http::see_other("/workspace");
            add_cookie(&mut response, cookies::clear(cookies::PRELOGIN));
            add_cookie(&mut response, cookies::clear(cookies::PRE_ACCESS));
            add_cookie(&mut response, cookies::set(cookies::SESSION, &session));
            response
        }
        Ok(Arrival::Denied(pre)) => {
            let mut response = denied(&auth, &pre);
            add_cookie(&mut response, cookies::clear(cookies::PRELOGIN));
            add_cookie(&mut response, cookies::clear(cookies::SESSION));
            add_cookie(&mut response, cookies::set(cookies::PRE_ACCESS, &pre));
            response
        }
        Err(()) => failed(&auth),
    }
}

/// ADR-006: a verified identity with no admission: sign-out only.
pub(crate) fn denied(auth: &Auth, pre_cookie: &str) -> Response {
    with_status(
        auth.pages.denied(&DeniedView {
            csrf_signout: auth.csrf.token(csrf::Form::SignOut, pre_cookie),
        }),
        StatusCode::FORBIDDEN,
    )
}

/// The protected workspace entry: a live workspace session, the uniform 404
/// for no, an unknown, an ended or a refused session, or 503 when the
/// workspace store itself is unavailable or saturated (D7.9; the APP-012
/// query routes answer the same).
async fn workspace(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let Some(cookie) = cookies::read(request.headers(), cookies::SESSION) else {
        return http::not_found();
    };
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(_request)) => auth.pages.workspace(&WorkspaceView {
            csrf_signout: auth.csrf.token(csrf::Form::SignOut, &cookie),
            csrf_signout_all: auth.csrf.token(csrf::Form::SignOutAll, &cookie),
        }),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            http::not_found()
        }
        Ok(Lookup::Absent) | Err(Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

/// The submitted synchronizer token: `None` for an oversize body (413),
/// `Some("")` when the form carries none or other fields.
async fn body_token(body: Body) -> Option<String> {
    let fields = http::form(body).await?;
    Some(csrf::submitted(&fields).unwrap_or_default().to_owned())
}

/// ADR-006 `POST /auth/signout` (any session; idempotent). The workspace
/// record is destroyed with the epoch advance first, then the provider token.
async fn signout(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let session = cookies::read(&parts.headers, cookies::SESSION);
    let pre = cookies::read(&parts.headers, cookies::PRE_ACCESS);
    let Some(token) = body_token(body).await else {
        return http::payload_too_large();
    };
    let done = || {
        let mut response = http::see_other("/");
        clear_all(&mut response);
        response
    };
    if session.is_none() && pre.is_none() {
        return done();
    }
    let tied = session
        .iter()
        .chain(pre.iter())
        .any(|cookie| auth.csrf.verify(csrf::Form::SignOut, cookie, &token));
    if !tied {
        return http::cross_site();
    }
    if let Some(stores) = &auth.stores {
        if let Some(cookie) = &session {
            match stores.workspace.sign_out(cookie).await {
                Ok(Some(signed)) => {
                    tracing::info!(epoch = signed.epoch, "auth.sign_out");
                    auth.revoke(&signed.ended).await;
                }
                Ok(None) => {}
                Err(_) => return http::unavailable(),
            }
        }
        if let Some(cookie) = &pre {
            match stores.workspace.destroy_pre_access(cookie).await {
                Ok(Some(ended)) => auth.revoke(&ended).await,
                Ok(None) => {}
                Err(_) => return http::unavailable(),
            }
        }
    }
    done()
}

/// ADR-006 `POST /auth/signout-all` (workspace session): every session of
/// the member in this organization, one epoch advance.
async fn signout_all(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let Some(cookie) = cookies::read(&parts.headers, cookies::SESSION) else {
        return http::not_found();
    };
    let Some(token) = body_token(body).await else {
        return http::payload_too_large();
    };
    if !auth.csrf.verify(csrf::Form::SignOutAll, &cookie, &token) {
        return http::cross_site();
    }
    let Some(stores) = &auth.stores else {
        return http::not_found();
    };
    let live = |reference: &str| auth.vault.contains(reference);
    let request = match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => request,
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            return http::not_found();
        }
        Ok(Lookup::Absent) | Err(_) => return http::not_found(),
    };
    match stores.workspace.sign_out_all(&request).await {
        Ok((epoch, ended)) => {
            tracing::info!(epoch, sessions = ended.len(), "auth.sign_out_all");
            for record in &ended {
                auth.revoke(record).await;
            }
            let mut response = http::see_other("/");
            clear_all(&mut response);
            response
        }
        Err(_) => http::unavailable(),
    }
}
