//! `POST /me/name` (D6 of the trial plan): the name other members see. The
//! canvas asks "What should others call you?" while the member has none
//! (`display_name` empty) and offers the same form later, in the top bar, to
//! change it. Storage keeps the name (`Workspace::set_display_name`: bounded,
//! printable, audited) and refreshes the directory every label reads.
//!
//! Order: fetch metadata (else 403 `cross_site`) → a live workspace session
//! (`query::routes::member`; else the uniform 404) → no query string (else
//! the uniform 404) → the form body (at most 4 KiB) → the member's shared
//! write window (`limits::MEMBER_WRITES`, APP-113; recheck N1) → exactly one
//! `csrf` and one `name`, in either order (any other shape, or a missing or
//! wrong synchronizer token, tied to the session cookie and
//! `csrf::Form::DisplayName`, is 403 `cross_site`) → the name
//! (`directory::usable_name` after trimming) → storage → `303` to `/canvas`.
//!
//! This is a plain form post, so every other outcome a member can reach is
//! a `303` back to the canvas with one notice from a closed set
//! ([`NameNotice`]; the only query `GET /canvas` accepts): a refused name,
//! a full write window, or a storage outage. Logs carry identities and
//! codes, never the name.
use crate::{
    auth::{csrf, Auth},
    directory,
    http::{self, limits::Limiter},
    query::routes::member,
    storage::{self, TrustedRequest, Workspace},
};
use axum::{
    extract::{Request, State},
    response::Response,
    routing::post,
    Router,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub const PATH: &str = "/me/name";

/// What `POST /me/name` sends the member back to the canvas with (review
/// m2, recheck N1 and N6): the closed set of the canvas's one query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameNotice {
    /// The name was refused (`directory::usable_name`).
    Invalid,
    /// The member's shared write window is full.
    Limited,
    /// Storage could not save it (an outage, or an unknown outcome).
    Unavailable,
}
impl NameNotice {
    const ALL: [Self; 3] = [Self::Invalid, Self::Limited, Self::Unavailable];
    pub fn query(self) -> &'static str {
        match self {
            Self::Invalid => "name=invalid",
            Self::Limited => "name=limited",
            Self::Unavailable => "name=unavailable",
        }
    }
    pub fn words(self) -> &'static str {
        match self {
            Self::Invalid => "That name can't be used. Use 1 to 80 characters with at least one letter, number or symbol, and no invisible or control characters.",
            Self::Limited => "Too many name changes in the last hour. Try again later.",
            Self::Unavailable => "Your name could not be saved just now. Reload the page to check it, then try again.",
        }
    }
}

/// How `GET /canvas` judges its query string (recheck N4: the gate lives
/// here so a unit test pins it).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasQuery {
    /// No query: the canvas as it is.
    Plain,
    /// Exactly one notice of the closed set.
    Notice(NameNotice),
    /// Anything else: the uniform 404.
    Refused,
}
pub fn canvas_query(query: Option<&str>) -> CanvasQuery {
    match query {
        None => CanvasQuery::Plain,
        Some(query) => NameNotice::ALL
            .into_iter()
            .find(|notice| notice.query() == query)
            .map_or(CanvasQuery::Refused, CanvasQuery::Notice),
    }
}

fn back(notice: NameNotice) -> Response {
    http::see_other(&format!("/canvas?{}", notice.query()))
}

/// Whether the canvas asks the member "What should others call you?": the
/// stored name was read and is empty, and no other name is known for them
/// (a DS-44 persona name of a listed demo organization counts as known). A
/// name that could not be read never prompts.
pub(crate) fn asks_for_name(stored: Option<Option<&str>>, known: Option<&str>) -> bool {
    matches!(stored, Some(name) if name.is_none_or(|name| name.trim().is_empty()))
        && known.is_none_or(|name| name.trim().is_empty())
}

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
    set(
        &stores.workspace,
        &auth.limits,
        &auth.csrf,
        &cookie,
        &member,
        &fields,
    )
    .await
}

/// The write window, the token, the name, then storage.
pub(crate) async fn set(
    store: &dyn NameStore,
    limits: &Limiter,
    csrf: &csrf::CsrfKey,
    cookie: &str,
    member: &TrustedRequest,
    fields: &[(String, String)],
) -> Response {
    let log = |code: &str| {
        tracing::info!(
            event = "member.display_name",
            code,
            organization_id = member.organization(),
            principal_id = member.principal(),
            "display name"
        );
    };
    // APP-113 (recheck N1): every changed name is a row and an audit event,
    // so the name shares the member's write window with the other routes.
    if limits
        .admit_member_write(member.organization(), member.principal())
        .is_err()
    {
        log("too_many_requests");
        return back(NameNotice::Limited);
    }
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
    let name = name.trim();
    if !directory::usable_name(name) {
        log("invalid_name");
        return back(NameNotice::Invalid);
    }
    match store.set(member, name).await {
        Ok(()) => {
            log("set");
            http::see_other("/canvas")
        }
        Err(storage::Error::Denied) => http::not_found(),
        Err(error) => {
            log(match error {
                storage::Error::UnknownOutcome => "unknown_outcome",
                _ => "unavailable",
            });
            back(NameNotice::Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView};
    use axum::{body::Body, http::StatusCode, response::IntoResponse};
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
        outcome: Option<storage::Error>,
    }
    impl NameStore for Fake {
        fn set<'a>(&'a self, _: &'a TrustedRequest, name: &'a str) -> Pending<'a> {
            self.set.lock().unwrap().push(name.to_owned());
            let outcome = self.outcome;
            Box::pin(async move { outcome.map_or(Ok(()), Err) })
        }
    }
    fn location(response: &Response) -> &str {
        response.headers()[axum::http::header::LOCATION]
            .to_str()
            .unwrap()
    }
    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn the_prompt_asks_only_a_member_with_no_known_name() {
        assert!(asks_for_name(Some(None), None));
        assert!(asks_for_name(Some(Some("  ")), None));
        assert!(!asks_for_name(Some(Some("Ana")), None));
        assert!(
            !asks_for_name(None, None),
            "an unreadable name never prompts"
        );
        assert!(
            !asks_for_name(Some(None), Some("Diego Ramos")),
            "a demo persona's name is known"
        );
    }

    #[tokio::test]
    async fn the_name_needs_the_token_is_judged_and_reaches_storage_trimmed() {
        let key = csrf::CsrfKey::new().unwrap();
        let member = storage::AgentCaller::for_tests(ORG, STUDENT, "t").request;
        let token = key.token(csrf::Form::DisplayName, COOKIE);
        let limits = Limiter::default();
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
            let response = set(&store, &limits, &key, COOKIE, &member, &bad).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{bad:?}");
        }
        let long = "x".repeat(directory::MAX_NAME_CHARACTERS + 1);
        for invalid in [
            "",
            "   ",
            "line\nbreak",
            long.as_str(),
            "Sarah\u{200B}Chen",
            "\u{202E}nehC haraS",
            "\u{3164}",
            "\u{0301}",
        ] {
            let response = set(
                &store,
                &limits,
                &key,
                COOKIE,
                &member,
                &fields(&[("csrf", &token), ("name", invalid)]),
            )
            .await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER, "{invalid:?}");
            assert_eq!(location(&response), "/canvas?name=invalid");
        }
        assert!(
            store.set.lock().unwrap().is_empty(),
            "nothing reached storage"
        );
        for good in [
            fields(&[("csrf", &token), ("name", "  Ana María  ")]),
            fields(&[("name", "Ana María"), ("csrf", &token)]),
        ] {
            let response = set(&store, &limits, &key, COOKIE, &member, &good).await;
            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            assert_eq!(location(&response), "/canvas");
        }
        assert_eq!(
            store.set.lock().unwrap().as_slice(),
            ["Ana María", "Ana María"]
        );
        let good = fields(&[("csrf", &token), ("name", "Ana")]);
        // Authority refused in storage: the uniform 404.
        let denied = Fake {
            outcome: Some(storage::Error::Denied),
            ..Fake::default()
        };
        let response = set(&denied, &limits, &key, COOKIE, &member, &good).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        // Recheck N6: an outage or an unknown outcome is a plain notice.
        for error in [storage::Error::Unavailable, storage::Error::UnknownOutcome] {
            let down = Fake {
                outcome: Some(error),
                ..Fake::default()
            };
            let response = set(&down, &limits, &key, COOKIE, &member, &good).await;
            assert_eq!(location(&response), "/canvas?name=unavailable", "{error:?}");
        }
    }

    /// Recheck N1: a name is a member write; it shares the member's
    /// `MEMBER_WRITES` window (120 an hour) with the other routes, and a
    /// full window writes nothing.
    #[tokio::test]
    async fn the_name_shares_the_member_write_window() {
        let key = csrf::CsrfKey::new().unwrap();
        let member = storage::AgentCaller::for_tests(ORG, STUDENT, "t").request;
        let token = key.token(csrf::Form::DisplayName, COOKIE);
        let limits = Limiter::default();
        let store = Fake::default();
        let per_hour = crate::http::limits::MEMBER_WRITES[0].limit;
        for i in 0..per_hour {
            let name = if i % 2 == 0 { "A" } else { "B" };
            let response = set(
                &store,
                &limits,
                &key,
                COOKIE,
                &member,
                &fields(&[("csrf", &token), ("name", name)]),
            )
            .await;
            assert_eq!(location(&response), "/canvas", "write {i}");
        }
        let response = set(
            &store,
            &limits,
            &key,
            COOKIE,
            &member,
            &fields(&[("csrf", &token), ("name", "C")]),
        )
        .await;
        assert_eq!(location(&response), "/canvas?name=limited");
        assert_eq!(
            store.set.lock().unwrap().len(),
            per_hour,
            "the last wrote nothing"
        );
        // The window is the member's own, shared with the other write routes.
        assert!(limits.admit_member_write(ORG, STUDENT).is_err());
        assert!(limits
            .admit_member_write(ORG, "00000000-0000-4000-8000-0000000000c3")
            .is_ok());
    }

    /// Recheck N4: the canvas's query gate. No query, or exactly one notice
    /// of the closed set; anything else is refused (the uniform 404).
    #[test]
    fn the_canvas_accepts_only_the_closed_notices() {
        assert_eq!(canvas_query(None), CanvasQuery::Plain);
        for notice in NameNotice::ALL {
            assert_eq!(
                canvas_query(Some(notice.query())),
                CanvasQuery::Notice(notice)
            );
            assert!(!notice.words().is_empty());
        }
        for other in [
            "",
            "name=",
            "name=valid",
            "name=invalid&",
            "name=invalid&x=1",
            "x=1&name=invalid",
            "name=Invalid",
            "name%3Dinvalid",
            "notice=removed",
        ] {
            assert_eq!(canvas_query(Some(other)), CanvasQuery::Refused, "{other:?}");
        }
    }
}
