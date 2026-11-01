//! Exact-share identity-only OAuth. This module never calls admission or mints
//! an organization session. Existing home workspace cookies are untouched.
use super::{cookies, csrf, pkce::Pkce, routes, Auth};
use crate::{
    http::limits,
    http::{self, UniformPath},
    storage::{self, Lookup, ShareAccount, ShareIdentityProof},
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap},
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/auth/shared/{share}/start", post(start))
        .route("/auth/shared/callback", get(callback))
        .route("/auth/shared/signout", post(signout))
        .with_state(auth)
}
/// Only the separate account cookie resolves here. A workspace session by
/// itself carries no durable verified-mailbox assertion and must reauthenticate.
pub async fn identity(auth: &Auth, headers: &HeaderMap) -> Option<ShareAccount> {
    let stores = auth.stores.as_ref()?;
    let cookie = cookies::read(headers, cookies::SHARED_SESSION)?;
    stores
        .workspace
        .resolve_shared_session(&cookie, &|r| auth.vault.contains(r))
        .await
        .ok()
        .flatten()
}
fn start_binding(headers: &HeaderMap, key: &str) -> Option<String> {
    cookies::read(headers, cookies::SHARED_SESSION)
        .or_else(|| cookies::read(headers, cookies::SESSION))
        .map(|cookie| format!("{cookie}\0{key}"))
}
/// Shared landing forms use this token when either identity cookie is present.
pub fn start_csrf(auth: &Auth, headers: &HeaderMap, key: &str) -> String {
    start_binding(headers, key)
        .map(|b| auth.csrf.token(csrf::Form::SharedStart, &b))
        .unwrap_or_default()
}
pub fn signout_csrf(auth: &Auth, headers: &HeaderMap) -> String {
    cookies::read(headers, cookies::SHARED_SESSION)
        .map(|cookie| auth.csrf.token(csrf::Form::SharedSignOut, &cookie))
        .unwrap_or_default()
}
/// End only this browser's account session. Normal workspace sign-out also
/// calls this, so a shared identity cannot silently survive a global sign-out.
pub(crate) async fn end_browser(auth: &Auth, headers: &HeaderMap) -> Result<(), storage::Error> {
    let Some(cookie) = cookies::read(headers, cookies::SHARED_SESSION) else {
        return Ok(());
    };
    let stores = auth.stores.as_ref().ok_or(storage::Error::Unavailable)?;
    if let Some(reference) = stores.workspace.destroy_shared_session(&cookie).await? {
        revoke_reference(auth, &reference).await;
    }
    Ok(())
}
async fn signout(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let Some(cookie) = cookies::read(&parts.headers, cookies::SHARED_SESSION) else {
        return http::not_found();
    };
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    if !csrf::submitted(&fields)
        .is_some_and(|token| auth.csrf.verify(csrf::Form::SharedSignOut, &cookie, token))
    {
        return http::cross_site();
    }
    if end_browser(&auth, &parts.headers).await.is_err() {
        return http::unavailable();
    }
    let mut response = http::see_other("/shared");
    for name in [cookies::SHARED_SESSION, cookies::SHARED_PRELOGIN] {
        routes::add_cookie(&mut response, cookies::clear(name));
    }
    response
}
fn valid_key(key: &str) -> bool {
    key.split_once('.').is_some_and(|(org, share)| {
        storage::valid_identifier(org) && storage::valid_identifier(share)
    })
}
fn context(key: &str, provider_org: &str) -> Vec<u8> {
    format!("forma:shared-pkce:v1\0{key}\0{provider_org}").into_bytes()
}
async fn start(
    State(auth): State<Arc<Auth>>,
    UniformPath(key): UniformPath<String>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let class = auth.address_class(&parts);
    if let Err(retry) = auth.limits.admit(&[
        (&format!("shared-start:{class}"), limits::CALLBACK_ADDRESS),
        ("shared-start:global", limits::CALLBACK_GLOBAL),
    ]) {
        return http::too_many_requests(retry);
    }
    if !valid_key(&key) || parts.uri.query().is_some() {
        return http::not_found();
    }
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    let Some((intent, token)) = start_fields(&fields) else {
        return http::not_found();
    };
    match start_binding(&parts.headers, &key) {
        Some(binding) if auth.csrf.verify(csrf::Form::SharedStart, &binding, token) => {}
        None if token.is_empty() => {}
        _ => return http::cross_site(),
    }
    match begin(&auth, &parts.headers, &key, intent).await {
        Ok((cookie, url)) => {
            let mut response = http::see_other(&url);
            routes::add_cookie(
                &mut response,
                cookies::set(cookies::SHARED_PRELOGIN, &cookie),
            );
            response
        }
        Err(()) => http::not_found(),
    }
}
fn start_fields(fields: &[(String, String)]) -> Option<(&str, &str)> {
    let (mut intent, mut token) = (None, None);
    for (name, value) in fields {
        match name.as_str() {
            "intent" if intent.is_none() && matches!(value.as_str(), "sign_in" | "sign_up") => {
                intent = Some(value.as_str())
            }
            "csrf" if token.is_none() => token = Some(value.as_str()),
            _ => return None,
        }
    }
    Some((intent?, token.unwrap_or("")))
}
fn invitation_key(key: &str, tenant: &str, email: &str) -> String {
    use sha2::{Digest, Sha256};
    // The broker also coalesces still-pending tenant/email invitations across
    // windows, so a retry at the boundary cannot revoke the earlier capability.
    let digest = Sha256::digest(format!(
        "forma:shared-invitation:v1\0{key}\0{tenant}\0{email}\0{}",
        super::now() / 600
    ));
    let mut id = digest[..16].to_vec();
    id[6] = (id[6] & 15) | 0x40;
    id[8] = (id[8] & 63) | 0x80;
    let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
async fn begin(
    auth: &Auth,
    headers: &HeaderMap,
    key: &str,
    intent: &str,
) -> Result<(String, String), ()> {
    let provider = auth.provider.as_ref().ok_or(())?;
    let stores = auth.stores.as_ref().ok_or(())?;
    let target = stores
        .workspace
        .shared_auth_target(key)
        .await
        .map_err(|_| ())?;
    // A current home member may reauthenticate in their own provider tenant.
    // The returned proof remains account-only and cannot change their home org.
    let home = if let Some(cookie) = cookies::read(headers, cookies::SESSION) {
        match stores
            .workspace
            .resolve_session(&cookie, &|r| auth.vault.contains(r))
            .await
        {
            Ok(Lookup::Live(member)) => Some(member.organization().to_owned()),
            _ => None,
        }
    } else {
        None
    };
    let provider_org = home.as_deref().unwrap_or(&target.organization);
    let route = provider.config.route(provider_org).ok_or(())?;
    stores
        .workspace
        .provider_tenant(provider_org, &provider.config.issuer)
        .ok_or(())?;
    let pkce = Pkce::fresh().ok_or(())?;
    let sealed = auth
        .sealer
        .seal(&context(key, provider_org), pkce.verifier.as_bytes())
        .ok_or(())?;
    let flow = stores
        .workspace
        .begin_shared_flow(key, provider_org, &sealed)
        .await
        .map_err(|_| ())?;
    let result = async {
        let invitation = if home.is_none() && intent == "sign_up" {
            let broker = auth.shared_broker.as_ref().ok_or(())?;
            let invitation_id = invitation_key(key, &route.tenant_slug, &target.recipient_email);
            Some(
                broker
                    .invitation(&route.tenant_slug, &target.recipient_email, &invitation_id)
                    .await
                    .ok_or(())?,
            )
        } else {
            None
        };
        let callback = format!("{}/auth/shared/callback", provider.config.app_origin);
        let uri = provider
            .client
            .par(
                &route.tenant_slug,
                &route.client_id,
                &callback,
                &flow.state,
                &pkce.challenge,
                invitation.as_deref(),
            )
            .await
            .map_err(|_| ())?;
        Ok(provider
            .client
            .authorize_url(&route.tenant_slug, &route.client_id, &uri))
    }
    .await;
    match result {
        Ok(url) => Ok((flow.cookie, url)),
        Err(()) => {
            let _ = stores.workspace.discard_shared_flow(&flow.cookie).await;
            Err(())
        }
    }
}
async fn revoke_reference(auth: &Auth, reference: &str) {
    if let (Some(provider), Some((slug, refresh))) =
        (&auth.provider, auth.vault.take(reference).await)
    {
        let _ = provider.client.logout(&slug, &refresh).await;
    }
}
enum Arrival {
    Verified { key: String, cookie: String },
    Pending { key: String },
}
async fn exchange_identity(
    provider: &super::Provider,
    token: &super::provider::Token,
    expected_tenant: &str,
    recipient_email: &str,
) -> Option<genui_auth_verifier::VerifiedIdentity> {
    if !super::token_shaped(token) {
        return None;
    }
    let identity = provider
        .verifier
        .verify(&token.access_token, expected_tenant)
        .await
        .ok()?;
    (identity.issuer == provider.config.issuer
        && identity.tenant_id == expected_tenant
        && identity.subject == token.user.id
        && identity.expires_at == token.expires_at
        && identity.expires_at.saturating_sub(identity.issued_at) == token.expires_in
        && storage::normalize_share_email(&token.user.email).as_deref() == Some(recipient_email))
    .then_some(identity)
}
async fn arrive(
    auth: &Auth,
    cookie: &str,
    code: &str,
    state: &str,
    previous: Option<&str>,
) -> Result<Arrival, ()> {
    let provider = auth.provider.as_ref().ok_or(())?;
    let stores = auth.stores.as_ref().ok_or(())?;
    let flow = stores
        .workspace
        .complete_shared_flow(cookie, state)
        .await
        .map_err(|_| ())?;
    let target = stores
        .workspace
        .shared_auth_target(&flow.key)
        .await
        .map_err(|_| ())?;
    let route = provider
        .config
        .route(&flow.provider_organization)
        .ok_or(())?;
    let expected_tenant = stores
        .workspace
        .provider_tenant(&flow.provider_organization, &provider.config.issuer)
        .ok_or(())?;
    let verifier = auth
        .sealer
        .open(
            &context(&flow.key, &flow.provider_organization),
            &flow.sealed_verifier,
        )
        .and_then(|v| String::from_utf8(v).ok())
        .ok_or(())?;
    let callback = format!("{}/auth/shared/callback", provider.config.app_origin);
    let token = provider
        .client
        .exchange(
            &route.tenant_slug,
            &route.client_id,
            &callback,
            code,
            &verifier,
        )
        .await
        .map_err(|_| ())?;
    let result = async {
        let identity =
            exchange_identity(provider, &token, &expected_tenant, &target.recipient_email)
                .await
                .ok_or(())?;
        // Recheck exact invitation after the provider round trip. Acceptance
        // subsequently repeats the live grant check under its own transaction.
        let current = stores
            .workspace
            .shared_auth_target(&flow.key)
            .await
            .map_err(|_| ())?;
        if current.recipient_email != target.recipient_email {
            return Err(());
        }
        if !token.user.email_verified {
            use sha2::{Digest, Sha256};
            let resend_key = format!(
                "shared-resend:{}",
                Sha256::digest(format!(
                    "{}\0{}\0{}",
                    identity.issuer, identity.tenant_id, identity.subject
                ))
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
            );
            if auth
                .limits
                .admit(&[(&resend_key, limits::RESEND_ACCOUNT)])
                .is_ok()
            {
                let _ = provider
                    .client
                    .resend(&route.tenant_slug, &token.access_token)
                    .await;
            }
            return Ok(Arrival::Pending {
                key: flow.key.clone(),
            });
        }
        let proof = ShareIdentityProof::verified(&identity, &token.user.email, true).ok_or(())?;
        // A verified new identity replaces this browser's account seat even
        // at the eight-session limit. The home workspace stays intact.
        if let Some(previous) = previous {
            if let Some(reference) = stores
                .workspace
                .destroy_shared_session(previous)
                .await
                .map_err(|_| ())?
            {
                revoke_reference(auth, &reference).await;
            }
        }
        let reference = auth.vault.insert(&route.tenant_slug, &token).ok_or(())?;
        match stores
            .workspace
            .mint_shared_session_live(&proof, &reference, &|r| auth.vault.contains(r))
            .await
        {
            Ok(cookie) => Ok(Arrival::Verified {
                key: flow.key.clone(),
                cookie,
            }),
            Err(_) => {
                revoke_reference(auth, &reference).await;
                Err(())
            }
        }
    }
    .await;
    if !matches!(result, Ok(Arrival::Verified { .. })) {
        let _ = provider
            .client
            .logout(&route.tenant_slug, &token.refresh_token)
            .await;
    }
    result
}
async fn callback(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let class = auth.address_class(&parts);
    if let Err(retry) = auth.limits.admit(&[
        (
            &format!("shared-callback:{class}"),
            limits::CALLBACK_ADDRESS,
        ),
        ("shared-callback:global", limits::CALLBACK_GLOBAL),
    ]) {
        return http::too_many_requests(retry);
    }
    let result = match (
        routes::callback_query(parts.uri.query()),
        cookies::read(&parts.headers, cookies::SHARED_PRELOGIN),
    ) {
        (Some((code, state)), Some(cookie)) => {
            arrive(
                &auth,
                &cookie,
                &code,
                &state,
                cookies::read(&parts.headers, cookies::SHARED_SESSION).as_deref(),
            )
            .await
        }
        _ => Err(()),
    };
    let mut response = match result {
        Ok(Arrival::Verified { key, cookie }) => {
            let mut response = http::see_other(&format!("/s/{key}"));
            routes::add_cookie(
                &mut response,
                cookies::set(cookies::SHARED_SESSION, &cookie),
            );
            response
        }
        Ok(Arrival::Pending { key }) => http::see_other(&format!("/s/{key}?verification=pending")),
        Err(()) => return http::not_found(),
    };
    routes::add_cookie(&mut response, cookies::clear(cookies::SHARED_PRELOGIN));
    // No ordinary SESSION, PRE_ACCESS, PRELOGIN, RUN or organization hint is
    // replaced, cleared or revoked on any success, pending or refusal path.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{
        synthetic_issuer::{Issuer, TokenMode},
        tests::{self, TestPages, NORTHSTAR_TENANT},
    };
    use axum::{
        body::Body,
        http::{Request as HttpRequest, StatusCode},
    };
    use tower::ServiceExt;
    const KEY: &str = "00000000-0000-4000-8000-000000000001.00000000-0000-4000-8000-000000000099";
    #[test]
    fn closed_intent_and_exact_share_csrf() {
        let auth = Auth::new(None, None, Arc::new(TestPages)).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("{}=home", cookies::SESSION).parse().unwrap(),
        );
        let token = start_csrf(&auth, &headers, KEY);
        assert!(auth.csrf.verify(
            csrf::Form::SharedStart,
            &start_binding(&headers, KEY).unwrap(),
            &token
        ));
        assert!(!auth.csrf.verify(
            csrf::Form::SharedStart,
            &start_binding(&headers, &KEY.replace("0099", "0098")).unwrap(),
            &token
        ));
        assert_eq!(
            start_fields(&[("intent".into(), "sign_in".into())]),
            Some(("sign_in", ""))
        );
        for fields in [
            vec![("intent".into(), "signin".into())],
            vec![
                ("intent".into(), "sign_in".into()),
                ("intent".into(), "sign_up".into()),
            ],
            vec![
                ("intent".into(), "sign_up".into()),
                ("email".into(), "x@example.test".into()),
            ],
        ] {
            assert!(start_fields(&fields).is_none());
        }
        assert!(storage::valid_identifier(&invitation_key(
            KEY,
            "northstar",
            "recipient@example.test"
        )));
        assert_ne!(
            invitation_key(KEY, "northstar", "recipient@example.test"),
            invitation_key(KEY, "canary", "recipient@example.test")
        );
    }
    #[tokio::test]
    async fn provider_proof_requires_exact_tenant_subject_mailbox_and_verification() {
        let issuer = Issuer::start(tests::tenants()).await;
        let auth = Auth::loopback(tests::config(&issuer.origin), None, Arc::new(TestPages));
        let provider = auth.provider.as_ref().unwrap();
        issuer.user("usr_share_identity", "Recipient@example.test", true);
        let token = tests::issued_token(&issuer, &provider.client, "usr_share_identity").await;
        let proof = exchange_identity(provider, &token, NORTHSTAR_TENANT, "recipient@example.test")
            .await
            .unwrap();
        assert!(ShareIdentityProof::verified(&proof, &token.user.email, true).is_some());
        assert!(ShareIdentityProof::verified(&proof, &token.user.email, false).is_none());
        assert!(
            exchange_identity(provider, &token, NORTHSTAR_TENANT, "other@example.test")
                .await
                .is_none()
        );
        assert!(
            exchange_identity(provider, &token, "ten_foreign", "recipient@example.test")
                .await
                .is_none()
        );
        for mode in [
            TokenMode::WrongIssuer,
            TokenMode::SubjectMismatch,
            TokenMode::ForeignTenant,
            TokenMode::Expired,
        ] {
            issuer.token_mode(mode);
            let token = tests::issued_token(&issuer, &provider.client, "usr_share_identity").await;
            assert!(
                exchange_identity(provider, &token, NORTHSTAR_TENANT, "recipient@example.test")
                    .await
                    .is_none(),
                "{mode:?}"
            );
        }
        issuer.token_mode(TokenMode::Normal);
        issuer.set_verified("usr_share_identity", false);
        let token = tests::issued_token(&issuer, &provider.client, "usr_share_identity").await;
        let identity =
            exchange_identity(provider, &token, NORTHSTAR_TENANT, "recipient@example.test")
                .await
                .unwrap();
        assert!(!token.user.email_verified);
        assert!(ShareIdentityProof::verified(
            &identity,
            &token.user.email,
            token.user.email_verified
        )
        .is_none());
    }
    #[tokio::test]
    async fn refused_callback_and_start_never_clear_home_session() {
        let auth = Arc::new(Auth::new(None, None, Arc::new(TestPages)).unwrap());
        let app = router(auth.clone());
        for uri in [
            "/auth/shared/callback",
            "/auth/shared/callback?code=bad&state=bad",
        ] {
            let response = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .uri(uri)
                        .header(
                            header::COOKIE,
                            format!("{}=home; {}=old-pre", cookies::SESSION, cookies::PRE_ACCESS),
                        )
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            let set: Vec<_> = response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect();
            assert!(set.is_empty());
        }
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/auth/shared/{KEY}/start"))
                    .header("sec-fetch-site", "cross-site")
                    .body(Body::from("intent=sign_up"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .count(),
            0
        );
    }
}
