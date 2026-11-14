//! `POST /me/name` (D6 of the trial plan): the name other members see. The
//! canvas asks "What should others call you?" while the member has none
//! (`display_name` empty) and offers the same form later, in the top bar, to
//! change it. Storage keeps the name (`Workspace::set_display_name`: bounded,
//! printable, audited) and refreshes the directory every label reads.
//!
//! Order: fetch metadata (else 403 `cross_site`) → a live workspace session
//! (`query::routes::member`; else the uniform 404) → no query string (else
//! the uniform 404) → the form body (at most 4 KiB, exactly one `csrf` and
//! one `name`, in either order; any other shape, or a missing or wrong
//! synchronizer token, tied to the session cookie and
//! `csrf::Form::DisplayName`, is 403 `cross_site`) → the name
//! (`directory::usable_name` after trimming; else 422 `invalid_name`) →
//! storage → `303` to `/canvas`. Logs carry identities and codes, never the
//! name.
use crate::{
    auth::{csrf, Auth},
    directory, http,
    query::routes::member,
    storage::{self, TrustedRequest, Workspace},
};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    routing::post,
    Router,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub const PATH: &str = "/me/name";

type Pending<'a> = Pin<Box<dyn Future<Output = Result<(), storage::Error>> + Send + 'a>>;

/// Where the name is kept (the workspace store; a fake in tests).
pub trait NameStore: Send + Sync {
    fn set<'a>(&'a self, request: &'a TrustedRequest, name: &'a str) -> Pending<'a>;
}
impl NameStore for Workspace {
    fn set<'a>(&'a self, request: &'a TrustedRequest, name: &'a str) -> Pending<'a> {
        Box::pin(self.set_display_name(request, name))
    }
}

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new().route(PATH, post(name_route)).with_state(auth)
}

async fn name_route(State(auth): State<Arc<Auth>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, auth.origin()) {
        return http::cross_site();
    }
    let (cookie, member) = match member(&auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    if parts.uri.query().is_some() {
        return http::not_found();
    }
    let Some(stores) = auth.stores.as_ref() else {
        return http::not_found();
    };
    let Some(fields) = http::form(body).await else {
        return http::payload_too_large();
    };
    set(&stores.workspace, &auth.csrf, &cookie, &member, &fields).await
}

/// The token, the name, then storage.
pub(crate) async fn set(
    store: &dyn NameStore,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    fields: &[(String, String)],
) -> Response {
    let one = |field: &str| {
        let mut values = fields.iter().filter(|(name, _)| name == field);
        match (values.next(), values.next()) {
            (Some((_, value)), None) => Some(value.as_str()),
            _ => None,
        }
    };
    let (Some(token), Some(name), 2) = (one("csrf"), one("name"), fields.len()) else {
        return http::cross_site();
    };
    if !csrf.verify(csrf::Form::DisplayName, cookie, token) {
        return http::cross_site();
    }
    let log = |code: &str| {
        tracing::info!(
            event = "member.display_name",
            code,
            organization_id = member.organization(),
            principal_id = member.principal(),
            "display name"
        );
    };
    let name = name.trim();
    if !directory::usable_name(name) {
        log("invalid_name");
        return http::problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_name");
    }
    match store.set(member, name).await {
        Ok(()) => {
            log("set");
            http::see_other("/canvas")
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(_) => http::unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView};
    use axum::{body::Body, response::IntoResponse};
    use std::sync::Mutex;
    use tower::ServiceExt;

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

    /// The route as mounted, without a store: a cross-site post is 403
    /// first; every same-origin caller is a stranger (the uniform 404).
    #[tokio::test]
    async fn the_route_refuses_cross_site_posts_and_strangers() {
        let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
        let app = http::uniform(router(auth));
        let send = |headers: &[(&str, &str)], uri: &str| {
            let mut request = axum::http::Request::builder().method("POST").uri(uri);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let request = request.body(Body::from("csrf=x&name=Ana")).unwrap();
            let app = app.clone();
            async move { app.oneshot(request).await.unwrap().status() }
        };
        assert_eq!(
            send(&[("sec-fetch-site", "cross-site")], PATH).await,
            StatusCode::FORBIDDEN
        );
        let form = [
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/x-www-form-urlencoded"),
        ];
        assert_eq!(send(&form, PATH).await, StatusCode::NOT_FOUND);
        assert_eq!(
            send(
                &[form[0], form[1], ("cookie", "__Host-genui_session=abc")],
                PATH
            )
            .await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(send(&form, "/me/name?x=1").await, StatusCode::NOT_FOUND);
    }

    const ORG: &str = "00000000-0000-4000-8000-000000000002";
    const STUDENT: &str = "00000000-0000-4000-8000-0000000000b2";
    const COOKIE: &str = "session-cookie-value";

    #[derive(Default)]
    struct Fake {
        set: Mutex<Vec<String>>,
        denied: bool,
    }
    impl NameStore for Fake {
        fn set<'a>(&'a self, _: &'a TrustedRequest, name: &'a str) -> Pending<'a> {
            self.set.lock().unwrap().push(name.to_owned());
            let denied = self.denied;
            Box::pin(async move {
                if denied {
                    Err(storage::Error::Denied)
                } else {
                    Ok(())
                }
            })
        }
    }
    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[tokio::test]
    async fn the_name_needs_the_token_is_judged_and_reaches_storage_trimmed() {
        let key = csrf::CsrfKey::new().unwrap();
        let member = storage::AgentCaller::for_tests(ORG, STUDENT, "t").request;
        let token = key.token(csrf::Form::DisplayName, COOKIE);
        let store = Fake::default();
        let other_form = key.token(csrf::Form::OrgAdminRemove, COOKIE);
        let other_session = key.token(csrf::Form::DisplayName, "another-cookie");
        for bad in [
            fields(&[("name", "Ana")]),
            fields(&[("csrf", &other_form), ("name", "Ana")]),
            fields(&[("csrf", &other_session), ("name", "Ana")]),
            fields(&[("csrf", &token), ("name", "Ana"), ("name", "Bea")]),
            fields(&[("csrf", &token), ("name", "Ana"), ("principal", STUDENT)]),
        ] {
            let response = set(&store, &key, COOKIE, &member, &bad).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{bad:?}");
        }
        let long = "x".repeat(directory::MAX_NAME_CHARACTERS + 1);
        for invalid in ["", "   ", "line\nbreak", long.as_str()] {
            let response = set(
                &store,
                &key,
                COOKIE,
                &member,
                &fields(&[("csrf", &token), ("name", invalid)]),
            )
            .await;
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        }
        assert!(
            store.set.lock().unwrap().is_empty(),
            "nothing reached storage"
        );
        for good in [
            fields(&[("csrf", &token), ("name", "  Ana María  ")]),
            fields(&[("name", "Ana María"), ("csrf", &token)]),
        ] {
            let response = set(&store, &key, COOKIE, &member, &good).await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(response.headers()[axum::http::header::LOCATION], "/canvas");
        }
        assert_eq!(
            store.set.lock().unwrap().as_slice(),
            ["Ana María", "Ana María"]
        );
        let denied = Fake {
            denied: true,
            ..Fake::default()
        };
        let response = set(
            &denied,
            &key,
            COOKIE,
            &member,
            &fields(&[("csrf", &token), ("name", "Ana")]),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
