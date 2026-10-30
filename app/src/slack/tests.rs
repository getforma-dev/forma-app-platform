//! Unit tests: the signature check against Slack's documented example, every
//! refusal before parsing, the link token, escaping and the disabled routes.
use super::{
    link::Links,
    signature::{self, verify, Replays},
    text, Config, Secret, Slack,
};
use crate::{actions::Parser, auth::Auth, http};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt as _;

// Slack's documented example ("Verifying requests from Slack").
const EXAMPLE_SECRET: &str = "8f742231b10e8888abcd99yyyzzz85a5";
const EXAMPLE_TS: &str = "1531420618";
const EXAMPLE_BODY: &str = "token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J&team_domain=testteamnow&channel_id=G8PSS9T3V&channel_name=foobar&user_id=U2CERLKJA&user_name=roadrunner&command=%2Fwebhook-collect&text=&response_url=https%3A%2F%2Fhooks.slack.com%2Fcommands%2FT1DC2JH3J%2F397700885554%2F96rGlfmibIGlgcZRskXaIFfN&trigger_id=398738663015.47445629121.803a0bc887a14d10d2c447fce8b6703c";
const EXAMPLE_SIGNATURE: &str =
    "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503";

#[test]
fn slacks_documented_example_verifies() {
    let now = 1_531_420_618 + 60;
    assert!(verify(
        EXAMPLE_SECRET.as_bytes(),
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        EXAMPLE_BODY.as_bytes(),
        now
    ));
    assert_eq!(
        signature::sign(
            EXAMPLE_SECRET.as_bytes(),
            EXAMPLE_TS,
            EXAMPLE_BODY.as_bytes()
        ),
        EXAMPLE_SIGNATURE
    );
}

#[test]
fn bad_missing_stale_and_replayed_signatures_are_refused() {
    let secret = EXAMPLE_SECRET.as_bytes();
    let body = EXAMPLE_BODY.as_bytes();
    let now = 1_531_420_618;
    // Missing either header.
    assert!(!verify(secret, None, Some(EXAMPLE_SIGNATURE), body, now));
    assert!(!verify(secret, Some(EXAMPLE_TS), None, body, now));
    // Stale (and future) beyond 300 s.
    assert!(!verify(
        secret,
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        body,
        now + 301
    ));
    assert!(!verify(
        secret,
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        body,
        now - 301
    ));
    assert!(verify(
        secret,
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        body,
        now + 300
    ));
    // The same body under a new timestamp (a replay with a fresh header).
    let later = (now + 10).to_string();
    assert!(!verify(
        secret,
        Some(&later),
        Some(EXAMPLE_SIGNATURE),
        body,
        now + 10
    ));
    // A tampered body, another secret, malformed signatures and timestamps.
    let mut tampered = body.to_vec();
    tampered[0] ^= 1;
    assert!(!verify(
        secret,
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        &tampered,
        now
    ));
    assert!(!verify(
        b"another",
        Some(EXAMPLE_TS),
        Some(EXAMPLE_SIGNATURE),
        body,
        now
    ));
    for bad in [
        "",
        "v0=",
        "v1=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503",
        "v0=A2114D57B48EAC39B9AD189DD8316235A7B4A8D21A10BD27519666489C69B503",
        "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b50",
        "v0=a2114d57b48eac39b9ad189dd8316235a7b4a8d21a10bd27519666489c69b503 ",
    ] {
        assert!(
            !verify(secret, Some(EXAMPLE_TS), Some(bad), body, now),
            "{bad}"
        );
    }
    for bad in ["", "-1", "1531420618.0", "15314206180000000"] {
        assert!(
            !verify(secret, Some(bad), Some(EXAMPLE_SIGNATURE), body, now),
            "{bad}"
        );
    }
    // A signature accepted once is refused within its window.
    let replays = Replays::default();
    assert!(replays.first(EXAMPLE_SIGNATURE, now));
    assert!(!replays.first(EXAMPLE_SIGNATURE, now + 5));
}

fn secret() -> Secret {
    Secret::new("synthetic-signing-secret".into()).unwrap()
}

#[test]
fn link_tokens_expire_refuse_tampering_and_are_single_use() {
    let links = Links::new(&secret());
    let now = 1_800_000_000;
    let token = links.mint("T0001", "U0SARAH", now).unwrap();
    let claim = links.check(&token, now + 10).unwrap();
    assert_eq!(
        (claim.team.as_str(), claim.user.as_str()),
        ("T0001", "U0SARAH")
    );
    // Expired at ten minutes.
    assert!(links
        .check(&token, now + super::link::LINK_SECONDS)
        .is_none());
    // Tampered payload or tag; another key; a foreign shape.
    let (payload, tag) = token.split_once('.').unwrap();
    let mut flipped = payload.as_bytes().to_vec();
    flipped[3] = if flipped[3] == b'A' { b'B' } else { b'A' };
    let forged = format!("{}.{tag}", String::from_utf8(flipped).unwrap());
    assert!(links.check(&forged, now).is_none());
    assert!(links
        .check(&format!("{payload}.{}", &tag[1..]), now)
        .is_none());
    let other = Links::new(&Secret::new("another-secret".into()).unwrap());
    assert!(other.check(&token, now).is_none());
    assert!(links.check("not-a-token", now).is_none());
    // A Slack user id is never accepted lowercase or empty.
    assert!(links.mint("t0001", "U1", now).is_none());
    assert!(links.mint("T0001", "", now).is_none());
    // Single use.
    assert!(links.consume(&claim, now + 20));
    assert!(links.check(&token, now + 21).is_none());
    assert!(!links.consume(&claim, now + 22));
}

#[test]
fn every_posted_text_is_escaped() {
    assert_eq!(
        text::escape("<!channel> & <https://evil|click>"),
        "&lt;!channel&gt; &amp; &lt;https://evil|click&gt;"
    );
    assert_eq!(text::unescape("a &lt;b&gt; &amp; c"), "a <b> & c");
    let fields = text::fields(&[("Merchant".into(), "<@U1> & co".into())]);
    assert_eq!(
        fields["fields"][0]["text"],
        "*Merchant*\n&lt;@U1&gt; &amp; co"
    );
    // A link is ours only over https; otherwise plain, escaped text.
    assert_eq!(
        text::link("https://beta.getforma.dev/canvas", "Open <x>"),
        "<https://beta.getforma.dev/canvas|Open &lt;x&gt;>"
    );
    assert_eq!(text::link("/canvas", "Open"), "Open (/canvas)");
    assert!(format!("{:?}", secret()).contains("redacted"));
}

fn slack_state(api_base: &str) -> Arc<Slack> {
    let auth = Arc::new(Auth::new(None, None, Arc::new(crate::auth::tests::TestPages)).unwrap());
    Slack::new(
        auth,
        Config {
            signing_secret: secret(),
            bot_token: Secret::new("xoxb-synthetic-not-a-token".into()).unwrap(),
            api_base: api_base.into(),
        },
        Parser::new(None, "stub"),
    )
    .unwrap()
}

async fn post(
    app: &axum::Router,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (StatusCode, String) {
    let mut request = Request::builder().method("POST").uri(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn unconfigured_slack_routes_are_the_uniform_404() {
    let app = http::uniform(super::router(None));
    for path in [
        super::COMMANDS_PATH,
        super::INTERACTIVITY_PATH,
        super::EVENTS_PATH,
        super::link::PATH,
    ] {
        assert_eq!(
            post(&app, path, &[], "x").await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    // Configuration needs both values.
    std::env::remove_var(super::BOT_TOKEN_ENV);
    std::env::remove_var(super::SIGNING_SECRET_ENV);
    assert!(Config::from_env().is_none());
}

#[tokio::test]
async fn requests_are_refused_before_parsing_and_url_verification_needs_a_signature() {
    let app = http::uniform(super::router(Some(slack_state("http://127.0.0.1:9/api"))));
    let body = r#"{"type":"url_verification","challenge":"c0ffee"}"#;
    let before = signature::refused_count();
    // No signature, a bad one, a stale one, an oversized body: one uniform 401.
    let now = super::now().to_string();
    let stale = (super::now() - 400).to_string();
    let good = signature::sign(b"synthetic-signing-secret", &now, body.as_bytes());
    let stale_sig = signature::sign(b"synthetic-signing-secret", &stale, body.as_bytes());
    let unauthorized = (
        StatusCode::UNAUTHORIZED,
        String::from("{\"code\":\"unauthorized\",\"status\":401}"),
    );
    assert_eq!(
        post(&app, super::EVENTS_PATH, &[], body).await,
        unauthorized
    );
    assert_eq!(
        post(
            &app,
            super::EVENTS_PATH,
            &[
                ("x-slack-request-timestamp", &now),
                ("x-slack-signature", "v0=00")
            ],
            body
        )
        .await,
        unauthorized
    );
    assert_eq!(
        post(
            &app,
            super::EVENTS_PATH,
            &[
                ("x-slack-request-timestamp", &stale),
                ("x-slack-signature", &stale_sig)
            ],
            body
        )
        .await,
        unauthorized
    );
    let large = "x".repeat(super::BODY_LIMIT + 1);
    let large_sig = signature::sign(b"synthetic-signing-secret", &now, large.as_bytes());
    assert_eq!(
        post(
            &app,
            super::COMMANDS_PATH,
            &[
                ("x-slack-request-timestamp", &now),
                ("x-slack-signature", &large_sig)
            ],
            &large
        )
        .await,
        unauthorized
    );
    assert!(signature::refused_count() >= before + 4);
    // The valid signature: the challenge is echoed.
    let (status, echoed) = post(
        &app,
        super::EVENTS_PATH,
        &[
            ("x-slack-request-timestamp", &now),
            ("x-slack-signature", &good),
        ],
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(echoed, r#"{"challenge":"c0ffee"}"#);
    // The same signed request again is a replay.
    assert_eq!(
        post(
            &app,
            super::EVENTS_PATH,
            &[
                ("x-slack-request-timestamp", &now),
                ("x-slack-signature", &good)
            ],
            body
        )
        .await,
        unauthorized
    );
}

#[tokio::test]
async fn commands_answer_help_and_link_at_once_and_refuse_foreign_response_urls() {
    let state = slack_state("http://127.0.0.1:9/api");
    let app = http::uniform(super::router(Some(state.clone())));
    let send = |text: &'static str, response_url: &'static str| {
        let app = app.clone();
        async move {
            let body = format!(
                "team_id=T0001&user_id=U0DIEGO&command=%2Fforma&text={}&response_url={}",
                url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>(),
                url::form_urlencoded::byte_serialize(response_url.as_bytes()).collect::<String>()
            );
            let now = super::now().to_string();
            let sig = signature::sign(b"synthetic-signing-secret", &now, body.as_bytes());
            post(
                &app,
                super::COMMANDS_PATH,
                &[
                    ("x-slack-request-timestamp", &now),
                    ("x-slack-signature", &sig),
                ],
                &body,
            )
            .await
        }
    };
    let (status, help) = send("help", "https://hooks.slack.com/commands/T0001/1/x").await;
    assert_eq!(status, StatusCode::OK);
    assert!(help.contains("/forma what's waiting for me"), "{help}");
    let (_, link) = send("link", "https://hooks.slack.com/commands/T0001/1/x").await;
    assert!(link.contains("/slack/link?t="), "{link}");
    let token = link
        .split("t=")
        .nth(1)
        .unwrap()
        .split([')', '"', ' '])
        .next()
        .unwrap();
    assert!(state.links.check(token, super::now()).is_some());
    // An utterance whose response_url is not Slack's (or the stub's) is refused at once.
    let (_, refused) = send("I spent $86 at Nobu", "https://evil.example/hook").await;
    assert!(refused.contains("couldn't do that"), "{refused}");
    let (_, working) = send("I spent $86 at Nobu", "http://127.0.0.1:9/hook").await;
    assert!(working.contains("Working on it"), "{working}");
}
