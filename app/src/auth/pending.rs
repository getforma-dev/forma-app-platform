//! ADR-006 verification-pending state: `GET /auth/pending` and the
//! backend-mediated `POST /auth/pending/resend`. The browser never holds a
//! provider bearer; the backend refreshes the stored session when needed and
//! maps fAuth's resend outcomes to two sanitized results (`requested`,
//! `unavailable`), never presented as delivery. `already_verified`, a
//! provider 401/404, a failed refresh, a lapsed pending admission or a
//! verified identity destroy the pre-access record and send the browser to a
//! fresh sign-in (`303 /signin/{handle}`), because only a new exchange
//! establishes `email_verified` true.
use super::{
    cookies, csrf,
    provider::Resend,
    routes::{add_cookie, denied},
    vault::AccessFailure,
    Auth, PendingView, ResendResult,
};
use crate::{
    http::{self, limits},
    storage::{handle_digest, Lookup, PreAccess},
};
use axum::{extract::Request, extract::State, http::HeaderMap, response::Response};
use std::sync::Arc;

fn hours_remaining(pre: &PreAccess) -> u32 {
    let left = pre
        .pending_expires_at()
        .map(|at| at - super::now() as i64)
        .unwrap_or(0);
    if left <= 0 {
        0
    } else {
        u32::try_from((left + 3599) / 3600).unwrap_or(24)
    }
}

fn view(auth: &Auth, cookie: &str, pre: &PreAccess, result: Option<ResendResult>) -> PendingView {
    let key = format!("resend:{}", pre.account_key());
    let admission = auth.limits.would_admit(&[(&key, limits::RESEND_ACCOUNT)]);
    PendingView {
        csrf_resend: auth.csrf.token(csrf::Form::Resend, cookie),
        csrf_signout: auth.csrf.token(csrf::Form::SignOut, cookie),
        resend_available: admission.is_ok(),
        cooldown_seconds: admission
            .err()
            .map(|s| u32::try_from(s).unwrap_or(u32::MAX))
            .unwrap_or(0),
        hours_remaining: hours_remaining(pre),
        result,
    }
}

async fn live(auth: &Auth, cookie: &str) -> Option<PreAccess> {
    let stores = auth.stores.as_ref()?;
    match stores.workspace.resolve_pre_access(cookie).await {
        Ok(Lookup::Live(pre)) => Some(pre),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            None
        }
        Ok(Lookup::Absent) | Err(_) => None,
    }
}

pub(crate) async fn page(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let Some(cookie) = cookies::read(request.headers(), cookies::PRE_ACCESS) else {
        return http::not_found();
    };
    let Some(pre) = live(&auth, &cookie).await else {
        return http::not_found();
    };
    if pre.email_verified() {
        // A verified identity that is not admitted has no pending state.
        return denied(&auth, &cookie);
    }
    auth.pages.pending(&view(&auth, &cookie, &pre, None))
}

/// `303 /signin/{handle}` after destroying the pre-access record. The handle
/// comes from the routing hint only when it still names this organization;
/// the record itself never stores a handle.
async fn fresh_sign_in(
    auth: &Auth,
    headers: &HeaderMap,
    cookie: &str,
    pre: &PreAccess,
) -> Response {
    if let Some(stores) = &auth.stores {
        if let Ok(Some(ended)) = stores.workspace.destroy_pre_access(cookie).await {
            auth.revoke(&ended).await;
        }
    }
    let mut location = "/signin".to_owned();
    if let (Some(stores), Some(handle)) = (
        &auth.stores,
        cookies::read(headers, cookies::ORGANIZATION_HINT),
    ) {
        if let Some(digest) = handle_digest(&handle) {
            if let Ok(Some(organization)) = stores.workspace.resolve_handle(&digest).await {
                if organization == pre.organization() {
                    location = format!("/signin/{handle}");
                }
            }
        }
    }
    let mut response = http::see_other(&location);
    add_cookie(&mut response, cookies::clear(cookies::PRE_ACCESS));
    response
}

pub(crate) async fn resend(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let Some(cookie) = cookies::read(&parts.headers, cookies::PRE_ACCESS) else {
        return http::not_found();
    };
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    let token = csrf::submitted(&fields).unwrap_or_default();
    if !auth.csrf.verify(csrf::Form::Resend, &cookie, token) {
        return http::cross_site();
    }
    let Some(pre) = live(&auth, &cookie).await else {
        return http::not_found();
    };
    let pending_lapsed = pre
        .pending_expires_at()
        .is_some_and(|at| at <= super::now() as i64);
    if pre.email_verified() || pending_lapsed {
        return fresh_sign_in(&auth, &parts.headers, &cookie, &pre).await;
    }
    let render = |result| {
        auth.pages
            .pending(&view(&auth, &cookie, &pre, Some(result)))
    };
    let key = format!("resend:{}", pre.account_key());
    if auth
        .limits
        .admit(&[(&key, limits::RESEND_ACCOUNT)])
        .is_err()
    {
        tracing::info!(outcome = "genui_cooldown", "auth.resend");
        return render(ResendResult::Unavailable);
    }
    let Some(provider) = &auth.provider else {
        return render(ResendResult::Unavailable);
    };
    let Some(route) = provider.config.route(pre.organization()) else {
        return render(ResendResult::Unavailable);
    };
    let access = match auth.vault.access(pre.refresh_ref(), &provider.client).await {
        Ok(access) => access,
        Err(
            failure
            @ (AccessFailure::Missing | AccessFailure::Unauthorized | AccessFailure::Failed),
        ) => {
            tracing::info!(failure = ?failure, "auth.resend_refresh");
            return fresh_sign_in(&auth, &parts.headers, &cookie, &pre).await;
        }
    };
    let outcome = provider.client.resend(&route.tenant_slug, &access).await;
    drop(access);
    // The exact provider status, sanitized, stays server-side (evidence).
    tracing::info!(outcome = ?outcome, "auth.resend");
    match outcome {
        Resend::Requested => render(ResendResult::Requested),
        Resend::Unavailable { .. } => render(ResendResult::Unavailable),
        Resend::AlreadyVerified | Resend::SignInAgain { .. } => {
            fresh_sign_in(&auth, &parts.headers, &cookie, &pre).await
        }
    }
}
