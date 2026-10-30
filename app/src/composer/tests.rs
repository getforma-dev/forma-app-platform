//! APP-093 composer tests without Docker and without the real endpoint: a
//! scripted [`Model`] drives planning (a valid definition is admitted and
//! labeled; markup, script, SQL and grant attempts are refused by admission
//! with their codes; retries carry the code and never a value; the composer's
//! own codes; the router's decisions; the adapted patch), the HTTPS client
//! is exercised against a loopback stub (retry on 5xx, no retry on 4xx, the
//! key sent as a header and nowhere else), and the route's refusal order
//! (cross-site, anonymous, oversized, wrong method) runs under the binary's
//! composition without a store. The authorized paths (draft, publish, save,
//! the labels on real rows) run in
//! `storage::composer_qualification::native_composer_campaign` (Docker).
use super::*;
use crate::{
    admission,
    auth::{Auth, DeniedView, InviteView, Pages, PendingView, SigninView, WorkspaceView},
    http,
    storage::{LibraryApp, LibraryView, PublishedInput, ReadableSource},
};
use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use model::{Anthropic, Model, ModelError, ModelReply, ModelRequest};
use plan::Decision;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tower::ServiceExt;

const ORG: &str = "00000000-0000-4000-8000-000000000001";
const APP: &str = "00000000-0000-4000-8000-000000000065";
const NORTHSTAR: &str =
    include_str!("../../../contracts/fixtures/impl-2026-10-28.1/northstar.json");
const SPEND: &str = include_str!("../../../contracts/fixtures/impl-2026-10-28.1/spend-review.json");

/// A model that answers from a script and records every request it saw.
struct Script {
    replies: Mutex<VecDeque<Result<String, ModelError>>>,
    requests: Mutex<Vec<ModelRequest>>,
}
impl Script {
    fn new(replies: Vec<Result<String, ModelError>>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        })
    }
    fn requests(&self) -> Vec<ModelRequest> {
        self.requests.lock().unwrap().clone()
    }
}
impl Model for Script {
    fn complete<'a>(&'a self, request: &'a ModelRequest) -> model::Completion<'a> {
        self.requests.lock().unwrap().push(request.clone());
        let next = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(ModelError::Unavailable));
        Box::pin(async move {
            next.map(|text| ModelReply {
                text,
                model: request.model.clone(),
                input_tokens: Some(10),
                output_tokens: Some(20),
            })
        })
    }
}

/// A route deadline that bounds nothing in these tests.
fn far() -> tokio::time::Instant {
    tokio::time::Instant::now() + COMPOSE_DEADLINE
}
/// A body whose bytes never arrive.
struct Stalled;
impl futures_core::Stream for Stalled {
    type Item = Result<axum::body::Bytes, std::io::Error>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

fn composer(script: &Arc<Script>) -> Composer {
    Composer::new(
        Some(script.clone() as Arc<dyn Model>),
        "router-model",
        "generation-model",
    )
}
fn bundle(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}
/// The fixture's source revisions as the member's readable sources.
fn sources(text: &str) -> Vec<ReadableSource> {
    bundle(text)["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| {
            let contract = source["input_contract"].clone();
            let canonical = admission::encode(&contract).unwrap();
            ReadableSource {
                source: source["source_id"].as_str().unwrap().to_owned(),
                revision: source["revision_id"].as_str().unwrap().to_owned(),
                contract_digest: admission::digest::digest(
                    admission::digest::INPUT_CONTRACT,
                    &canonical,
                ),
                contract,
            }
        })
        .collect()
}
/// The fixture's content with only runnable views (what a good model returns).
fn runnable_content(text: &str) -> Value {
    prompt::runnable_example(bundle(text)["definition"]["content"].clone())
}
fn library_app(text: &str, id: &str) -> LibraryApp {
    let content = runnable_content(text);
    LibraryApp {
        app: id.into(),
        publication: "00000000-0000-4000-8000-000000000068".into(),
        definition_digest: "sha256:0".into(),
        title: content["title"].as_str().unwrap().to_owned(),
        description: content["description"].as_str().unwrap().to_owned(),
        inputs: content["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|input| PublishedInput {
                id: input["id"].as_str().unwrap().to_owned(),
                kind: input["kind"].as_str().unwrap().to_owned(),
                contract_digest: "sha256:0".into(),
                fields: input["fields"].clone(),
            })
            .collect(),
        record_schemas: Vec::new(),
        views: content["views"]
            .as_array()
            .unwrap()
            .iter()
            .map(|view| LibraryView {
                id: view["id"].as_str().unwrap().to_owned(),
                kind: view["kind"].as_str().unwrap().to_owned(),
                title: view["title"].as_str().unwrap().to_owned(),
            })
            .collect(),
        content,
    }
}
fn with(mut content: Value, pointer: &str, value: Value) -> Value {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    content
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert(key.into(), value);
    content
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_valid_generated_definition_is_admitted_and_labeled_generated() {
    let content = runnable_content(NORTHSTAR);
    let script = Script::new(vec![Ok(format!(
        "Here is the app:\n```json\n{}\n```",
        serde_json::to_string_pretty(&content).unwrap()
    ))]);
    let generated = runtime()
        .block_on(composer(&script).generate(
            "show bytes per destination over time",
            &sources(NORTHSTAR),
            ORG,
            APP,
            far(),
        ))
        .unwrap();
    assert_eq!(generated.planned.origin, Origin::Generated);
    assert_eq!(generated.planned.origin.as_str(), "generated");
    assert_eq!(generated.planned.title, "Northstar investigation");
    assert!(generated.refused.is_empty());
    assert_eq!(generated.planned.admitted.schema_version, 1);
    assert_eq!(
        generated.planned.admitted.view_count,
        content["views"].as_array().unwrap().len()
    );
    // The admitted bytes are the content with its source inputs pinned to
    // the member's exact contracts (here identical), so a saved work binds.
    let admitted = admission::decode(&generated.planned.admitted.canonical).unwrap();
    assert_eq!(admitted["inputs"], content["inputs"]);
    let requests = script.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].model, "generation-model");
    let user: Value = serde_json::from_str(&requests[0].user).unwrap();
    assert_eq!(user["intent"], "show bytes per destination over time");
    assert!(user["refusal"].is_null());
    assert_eq!(user["sources"].as_array().unwrap().len(), 2);
    assert!(requests[0].system.contains("RUNNABLE OPERATORS"));
}

#[test]
fn hostile_outputs_are_refused_by_admission_and_retries_carry_codes_only() {
    let content = runnable_content(NORTHSTAR);
    let script = Script::new(vec![
        // Script under a key naming code.
        Ok(with(
            content.clone(),
            "/views/0/script",
            json!("<script>alert(1)</script>"),
        )
        .to_string()),
        // A grant claim.
        Ok(with(content.clone(), "/permissions", json!(["admin_policy"])).to_string()),
        // SQL under a key naming it.
        Ok(with(
            content.clone(),
            "/views/1/sql",
            json!("DROP TABLE genui.apps; --"),
        )
        .to_string()),
    ]);
    let failure = runtime()
        .block_on(composer(&script).generate(
            "x".repeat(3).as_str(),
            &sources(NORTHSTAR),
            ORG,
            APP,
            far(),
        ))
        .unwrap_err();
    assert_eq!(
        failure,
        GenerationFailure::Refused(vec![
            "executable_content",
            "client_grant",
            "executable_content",
        ])
    );
    let requests = script.requests();
    assert_eq!(requests.len(), MAX_GENERATION_ATTEMPTS);
    let feedback: Vec<Value> = requests
        .iter()
        .map(|request| serde_json::from_str::<Value>(&request.user).unwrap()["refusal"].clone())
        .collect();
    assert!(feedback[0].is_null());
    assert_eq!(feedback[1]["code"], "executable_content");
    assert_eq!(feedback[2]["code"], "client_grant");
    for request in &requests {
        for value in ["alert(1)", "<script>", "admin_policy", "DROP TABLE"] {
            assert!(!request.user.contains(value), "{value} echoed");
            assert!(!request.system.contains(value), "{value} echoed");
        }
    }
}

#[test]
fn markup_in_a_declared_text_member_of_the_wrong_type_is_refused() {
    // A title must be text; an object there is searched and the markup named.
    let content = with(
        runnable_content(SPEND),
        "/views/0/title",
        json!({"html": "<b>x</b>"}),
    );
    let refusal =
        plan::plan_generated(&content.to_string(), &sources(SPEND), ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "executable_content");
    // An unregistered operator and an unregistered renderer.
    let content = with(
        runnable_content(SPEND),
        "/views/0/query/operations/0/type",
        json!("pivot"),
    );
    let refusal =
        plan::plan_generated(&content.to_string(), &sources(SPEND), ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "unregistered_operator");
    let content = with(runnable_content(SPEND), "/views/0/kind", json!("gauge"));
    let refusal =
        plan::plan_generated(&content.to_string(), &sources(SPEND), ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "unregistered_renderer");
}

#[test]
fn the_composers_own_codes_cover_prose_invented_inputs_and_unrunnable_views() {
    let sources = sources(NORTHSTAR);
    let refusal =
        plan::plan_generated("I am unable to help with that.", &sources, ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "not_json");
    let refusal = plan::plan_generated("{\"title\": 1.5}", &sources, ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "not_json", "floats are not forma-json-v1");
    // An invented source input the member cannot read.
    let mut content = runnable_content(NORTHSTAR);
    content["inputs"][0]["id"] = json!("payroll");
    let refusal = plan::plan_generated(&content.to_string(), &sources, ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "unknown_source_input");
    // A source input whose fields the model changed is pinned back to the
    // member's exact contract before admission, so the work can bind it.
    let mut content = runnable_content(NORTHSTAR);
    content["inputs"][0]["fields"] = json!([{"id": "bytes", "value_type": {"type": "integer", "unit": "byte"}, "nullable": false}]);
    content["inputs"][0]["contract_id"] = json!("events_v9");
    let planned = plan::plan_generated(&content.to_string(), &sources, ORG, APP).unwrap();
    let admitted = admission::decode(&planned.admitted.canonical).unwrap();
    assert_eq!(admitted["inputs"][0], sources[0].contract);
    // The full fixture carries a two-input lookup and a ratio: not runnable.
    let content = bundle(NORTHSTAR)["definition"]["content"].clone();
    let refusal = plan::plan_generated(&content.to_string(), &sources, ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "not_runnable");
}

#[test]
fn a_model_failure_ends_generation_and_an_absent_key_disables_it() {
    let script = Script::new(vec![Err(ModelError::Timeout)]);
    let failure = runtime()
        .block_on(composer(&script).generate("abc", &sources(SPEND), ORG, APP, far()))
        .unwrap_err();
    assert_eq!(failure, GenerationFailure::Model(ModelError::Timeout));
    assert_eq!(script.requests().len(), 1);
    let disabled = Composer::new(None, "r", "g");
    assert!(!disabled.enabled());
    let failure = runtime()
        .block_on(disabled.generate("abc", &sources(SPEND), ORG, APP, far()))
        .unwrap_err();
    assert_eq!(failure, GenerationFailure::Model(ModelError::Unavailable));
    let library = [library_app(SPEND, APP)];
    assert_eq!(
        runtime()
            .block_on(disabled.route("abc", &library, false))
            .unwrap_err(),
        ModelError::Unavailable
    );
    assert_eq!(
        runtime()
            .block_on(disabled.route("abc", &[], false))
            .unwrap(),
        (Decision::Generate, "empty_library")
    );
}

#[test]
fn the_router_decides_only_within_the_catalog() {
    let library = [
        library_app(NORTHSTAR, "00000000-0000-4000-8000-000000000065"),
        library_app(SPEND, "00000000-0000-4000-8000-0000000000c9"),
    ];
    let decide = |text: &str| plan::decide(text, &library);
    assert_eq!(
        decide(
            r#"{"choice":"00000000-0000-4000-8000-0000000000c9","confidence":"high","reason":"fits","title":"Spend"}"#
        ),
        (Decision::Retrieve { index: 1 }, "high")
    );
    assert_eq!(
        decide(
            r#"{"choice":"00000000-0000-4000-8000-000000000065","confidence":"medium","reason":"close","title":"  Traffic review  "}"#
        ),
        (
            Decision::Adapt {
                index: 0,
                title: "Traffic review".into()
            },
            "medium"
        )
    );
    // A medium choice without a usable title keeps the library title.
    assert_eq!(
        decide(r#"{"choice":"00000000-0000-4000-8000-000000000065","confidence":"medium"}"#),
        (
            Decision::Adapt {
                index: 0,
                title: "Northstar investigation".into()
            },
            "medium"
        )
    );
    assert_eq!(
        decide(r#"{"choice":"00000000-0000-4000-8000-000000000065","confidence":"low"}"#),
        (Decision::Generate, "low")
    );
    assert_eq!(
        decide(r#"{"choice":"none","confidence":"low","reason":"nothing fits"}"#),
        (Decision::Generate, "none")
    );
    assert_eq!(
        decide(r#"{"choice":"00000000-0000-4000-8000-00000000ffff","confidence":"high"}"#),
        (Decision::Generate, "unknown")
    );
    assert_eq!(
        decide(r#"{"choice":"none","confidence":"low","grant":"admin"}"#),
        (Decision::Generate, "malformed"),
        "unknown members are refused"
    );
    assert_eq!(
        decide("Sure! The best app is Spend."),
        (Decision::Generate, "malformed")
    );
    // The router's catalog carries ids, titles, fields and views only.
    let user: Value = serde_json::from_str(&prompt::router_user("totals", &library)).unwrap();
    assert_eq!(user["intent"], "totals");
    assert_eq!(
        user["library"][1]["id"],
        "00000000-0000-4000-8000-0000000000c9"
    );
    assert!(user["library"][0]["fields"]
        .as_array()
        .unwrap()
        .contains(&json!("events.bytes")));
    let script = Script::new(vec![Ok(
        r#"{"choice":"00000000-0000-4000-8000-0000000000c9","confidence":"high","reason":"fits","title":"Spend"}"#.into(),
    )]);
    let routed = runtime()
        .block_on(composer(&script).route("review spend", &library, false))
        .unwrap();
    assert_eq!(routed, (Decision::Retrieve { index: 1 }, "high"));
    assert_eq!(script.requests()[0].model, "router-model");
    assert_eq!(script.requests()[0].system, prompt::ROUTER_SYSTEM);
}

#[test]
fn the_adapted_path_is_a_typed_patch_readmitted_whole() {
    let library = library_app(SPEND, APP);
    let planned =
        plan::plan_adapted(&library, "Vendor spend", "Adapted for: vendors", ORG, APP).unwrap();
    assert_eq!(planned.origin, Origin::Adapted);
    assert_eq!(planned.origin.as_str(), "adapted");
    assert_eq!(planned.title, "Vendor spend");
    let admitted = admission::decode(&planned.admitted.canonical).unwrap();
    assert_eq!(admitted["title"], "Vendor spend");
    assert_eq!(admitted["description"], "Adapted for: vendors");
    assert_eq!(admitted["views"], library.content["views"]);
    // A patched title over the text bound is refused like any other content.
    let refusal = plan::plan_adapted(&library, &"t".repeat(2001), "d", ORG, APP).unwrap_err();
    assert_eq!(refusal.code, "encoding");
    assert_eq!(Origin::Retrieved.as_str(), "retrieved");
}

// --- The HTTPS client against a loopback stub ---

/// A Messages stub: a scripted sequence of statuses; records the headers.
struct Stub {
    statuses: Mutex<VecDeque<u16>>,
    seen_keys: Mutex<Vec<String>>,
    bodies: Mutex<Vec<Value>>,
    calls: AtomicUsize,
}
async fn stub_server(statuses: Vec<u16>) -> (String, Arc<Stub>) {
    let stub = Arc::new(Stub {
        statuses: Mutex::new(statuses.into()),
        seen_keys: Mutex::new(Vec::new()),
        bodies: Mutex::new(Vec::new()),
        calls: AtomicUsize::new(0),
    });
    let state = stub.clone();
    let app = Router::new().route(
        "/v1/messages",
        axum::routing::post(
            move |headers: HeaderMap, body: axum::body::Bytes| {
                let state = state.clone();
                async move {
                    state.calls.fetch_add(1, Ordering::SeqCst);
                    if let Some(key) = headers.get("x-api-key") {
                        state
                            .seen_keys
                            .lock()
                            .unwrap()
                            .push(key.to_str().unwrap().to_owned());
                    }
                    state
                        .bodies
                        .lock()
                        .unwrap()
                        .push(serde_json::from_slice(&body).unwrap());
                    let status = state.statuses.lock().unwrap().pop_front().unwrap_or(200);
                    let reply = json!({"id": "msg", "model": "stub-model",
                        "content": [{"type": "text", "text": "{\"choice\":\"none\",\"confidence\":\"low\"}"}],
                        "usage": {"input_tokens": 5, "output_tokens": 7}});
                    (
                        StatusCode::from_u16(status).unwrap(),
                        [("content-type", "application/json")],
                        reply.to_string(),
                    )
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (endpoint, stub)
}

#[test]
fn the_client_retries_server_failures_once_each_and_never_a_client_refusal() {
    runtime().block_on(async {
        let (endpoint, stub) = stub_server(vec![503, 200]).await;
        let client = Anthropic::new("sk-ant-synthetic-test-key", &endpoint).unwrap();
        let request = ModelRequest {
            model: "stub-model".into(),
            system: "s".into(),
            user: "u".into(),
            max_tokens: 10,
            budget: std::time::Duration::from_secs(20),
        };
        let reply = client.complete(&request).await.unwrap();
        assert_eq!(reply.text, "{\"choice\":\"none\",\"confidence\":\"low\"}");
        assert_eq!(reply.model, "stub-model");
        assert_eq!(
            (reply.input_tokens, reply.output_tokens),
            (Some(5), Some(7))
        );
        assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
        // The key travels as the header and nowhere else: not in the body.
        assert_eq!(
            stub.seen_keys.lock().unwrap().as_slice(),
            ["sk-ant-synthetic-test-key"; 2]
        );
        for body in stub.bodies.lock().unwrap().iter() {
            assert!(!body.to_string().contains("sk-ant"));
            assert_eq!(body["model"], "stub-model");
            assert_eq!(body["temperature"], 0);
            assert_eq!(body["messages"][0]["content"], "u");
        }
        let (endpoint, stub) = stub_server(vec![400]).await;
        let client = Anthropic::new("sk-ant-synthetic-test-key", &endpoint).unwrap();
        assert_eq!(
            client.complete(&request).await.unwrap_err(),
            ModelError::Refused
        );
        assert_eq!(stub.calls.load(Ordering::SeqCst), 1);
        let (endpoint, stub) = stub_server(vec![500, 502, 503, 200]).await;
        let client = Anthropic::new("sk-ant-synthetic-test-key", &endpoint).unwrap();
        assert_eq!(
            client.complete(&request).await.unwrap_err(),
            ModelError::Unavailable
        );
        assert_eq!(
            stub.calls.load(Ordering::SeqCst),
            model::MAX_ATTEMPTS as usize
        );
    });
}

// --- The route without a store: the refusal order and the composition ---

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

fn app_without_store(composer: Composer) -> Router {
    let auth = Arc::new(Auth::new(None, None, Arc::new(NoPages)).unwrap());
    http::uniform_with(
        Router::new(),
        routes::PATH,
        routes::router(Arc::new(routes::Compose::new(auth, composer))),
    )
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 1 << 20).await.unwrap().to_vec();
    (parts.status, parts.headers, bytes)
}

#[test]
fn the_route_refuses_in_order_and_composes_under_the_uniform_404() {
    runtime().block_on(async {
        let script = Script::new(vec![]);
        let app = app_without_store(composer(&script));
        let same = [
            ("sec-fetch-site", "same-origin"),
            ("content-type", "application/json"),
        ];
        let body = br#"{"intent":"show spend by vendor"}"#.to_vec();
        // Fetch metadata first: a cross-site POST is 403 before anything else.
        let (status, headers, bytes) = send(
            &app,
            "POST",
            "/v1/compose",
            &[("sec-fetch-site", "cross-site")],
            body.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(headers["content-type"], "application/problem+json");
        assert_eq!(bytes, br#"{"code":"cross_site","status":403}"#);
        // Then the session (review M1: before the body is read, so an
        // anonymous oversized or stalled body costs nothing): anonymous is
        // the uniform 404, identical to an unknown path and to a wrong
        // method, with no Allow header.
        let (status, _, oversized) = send(
            &app,
            "POST",
            "/v1/compose",
            &same,
            vec![b'x'; routes::COMPOSE_BODY_LIMIT + 1],
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let stalled = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            let mut request = Request::builder().method("POST").uri("/v1/compose");
            for (name, value) in &same {
                request = request.header(*name, *value);
            }
            app.clone()
                .oneshot(request.body(Body::from_stream(Stalled)).unwrap())
                .await
                .unwrap()
                .status()
        })
        .await
        .expect("an anonymous stalled body is refused without waiting on it");
        assert_eq!(stalled, StatusCode::NOT_FOUND);
        let (status, headers, anonymous) =
            send(&app, "POST", "/v1/compose", &same, body.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(headers.get("allow").is_none());
        assert_eq!(headers["cache-control"], "no-store");
        let (status, headers, wrong_method) =
            send(&app, "GET", "/v1/compose", &same, Vec::new()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(headers.get("allow").is_none());
        let (status, _, unknown) = send(&app, "POST", "/v1/elsewhere", &same, body.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(anonymous, wrong_method);
        assert_eq!(anonymous, unknown);
        assert_eq!(anonymous, oversized);
        // A stale cookie without a store is the same 404.
        let (status, _, stale) = send(
            &app,
            "POST",
            "/v1/compose",
            &[same[0], same[1], ("cookie", "__Host-genui_session=abc")],
            body,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(anonymous, stale);
        // Nothing reached the model.
        assert!(script.requests().is_empty());
        // The deadline the composition applies to this route is the composer's.
        assert_eq!(http::COMPOSE_DEADLINE, COMPOSE_DEADLINE);
        assert!(COMPOSE_DEADLINE > http::HANDLER_DEADLINE);
    });
}

#[test]
fn the_body_is_one_intent_of_bounded_shape() {
    let intent = |text: &str| routes::body_intent(text.as_bytes());
    assert_eq!(
        intent(r#"{"intent":"show spend"}"#),
        Some("show spend".into())
    );
    assert_eq!(intent(r#"{"intent":"show spend","extra":1}"#), None);
    assert_eq!(intent(r#"{"intent":"ab"}"#), None);
    assert_eq!(intent(r#"{"intent":1}"#), None);
    assert_eq!(intent(r#"["show spend"]"#), None);
    assert_eq!(intent(""), None);
}

#[test]
fn the_body_read_is_bounded_in_size_and_time() {
    runtime().block_on(async {
        let short = std::time::Duration::from_millis(50);
        assert_eq!(
            routes::read_body(Body::from_stream(Stalled), short).await,
            Err(routes::BodyRefused::Stalled)
        );
        assert_eq!(
            routes::read_body(
                Body::from(vec![b'x'; routes::COMPOSE_BODY_LIMIT + 1]),
                short
            )
            .await,
            Err(routes::BodyRefused::TooLarge)
        );
        assert_eq!(
            routes::read_body(Body::from(r#"{"intent":"abc"}"#), short)
                .await
                .unwrap(),
            r#"{"intent":"abc"}"#
        );
        assert!(http::HANDLER_DEADLINE < COMPOSE_DEADLINE);
    });
}

#[test]
fn the_budgets_fit_the_route_deadline_and_an_attempt_needs_room() {
    // Review M5: the router, every generation attempt and the storage reserve
    // fit the route; no attempt is cut between publish and save.
    assert!(
        ROUTER_BUDGET + GENERATION_BUDGET * (MAX_GENERATION_ATTEMPTS as u32) + STORAGE_RESERVE
            <= COMPOSE_DEADLINE
    );
    assert!(MIN_GENERATION_ATTEMPT <= GENERATION_BUDGET);
    let now = tokio::time::Instant::now();
    let seconds = |n: u64| now + std::time::Duration::from_secs(n);
    assert_eq!(attempt_budget(now, seconds(120)), Some(GENERATION_BUDGET));
    assert_eq!(
        attempt_budget(now, seconds(20)),
        Some(std::time::Duration::from_secs(10)),
        "cut to what remains before the reserve"
    );
    assert_eq!(
        attempt_budget(now, seconds(14)),
        None,
        "less than one attempt"
    );
    assert_eq!(attempt_budget(now, seconds(10)), None);
    assert_eq!(attempt_budget(seconds(1), now), None, "past the deadline");
    // No room: nothing is called and the failure says so.
    let script = Script::new(vec![Ok("unused".into())]);
    let outcome = runtime().block_on(composer(&script).generate(
        "abc",
        &sources(SPEND),
        ORG,
        APP,
        tokio::time::Instant::now() + std::time::Duration::from_secs(12),
    ));
    assert_eq!(outcome.unwrap_err(), GenerationFailure::OutOfTime);
    assert!(script.requests().is_empty());
}

#[test]
fn demo_organizations_answer_rehearsed_intents_from_the_catalog_and_nobody_else_does() {
    let library = [
        library_app(NORTHSTAR, "00000000-0000-4000-8000-000000000065"),
        library_app(SPEND, "00000000-0000-4000-8000-0000000000c9"),
    ];
    let demo = "00000000-0000-4000-8000-00000000000d";
    // The deterministic router: keywords of the intent in the catalog's
    // titles and descriptions, never a title or an app the intent wrote.
    assert_eq!(
        plan::demo_route("Review expenses over $200", &library),
        (Decision::Retrieve { index: 1 }, "demo")
    );
    assert_eq!(
        plan::demo_route("Northstar investigation of egress", &library),
        (Decision::Retrieve { index: 0 }, "demo")
    );
    assert_eq!(
        plan::demo_route("zzz qqq", &library),
        (Decision::Generate, "demo_no_match")
    );
    assert_eq!(
        plan::demo_route("Review expenses", &[]),
        (Decision::Generate, "demo_no_match")
    );
    // The environment value: lowercase UUIDs only.
    assert_eq!(
        demo_organizations(&format!(" {demo}, not-a-uuid,{},", demo.to_uppercase())),
        [demo.to_owned()].into_iter().collect()
    );
    assert!(demo_organizations("").is_empty());
    // Scoping: without a model, only the demo organization composes.
    let disabled = Composer::new(None, "r", "g").with_demo_organizations([demo.to_owned()]);
    assert!(!disabled.enabled());
    assert!(disabled.demo_for(demo) && !disabled.demo_for(ORG));
    assert!(disabled.available_for(demo) && !disabled.available_for(ORG));
    assert!(
        !Composer::new(None, "r", "g").available_for(demo),
        "off by default"
    );
    runtime().block_on(async {
        // Demo organization: the rehearsed intent needs no model at all; an
        // unrehearsed one still needs the model.
        assert_eq!(
            disabled
                .route("Review expenses over $200", &library, true)
                .await,
            Ok((Decision::Retrieve { index: 1 }, "demo"))
        );
        assert_eq!(
            disabled.route("zzz qqq", &library, true).await,
            Err(ModelError::Unavailable)
        );
        // A real organization: never the canned path.
        assert_eq!(
            disabled
                .route("Review expenses over $200", &library, false)
                .await,
            Err(ModelError::Unavailable)
        );
        let reply =
            r#"{"choice":"00000000-0000-4000-8000-000000000065","confidence":"high","reason":"x"}"#;
        let script = Script::new(vec![Ok(reply.into()), Ok(reply.into())]);
        let with_model = composer(&script).with_demo_organizations([demo.to_owned()]);
        // Demo organization with a model: the rehearsed intent never reaches it...
        assert_eq!(
            with_model
                .route("Review expenses over $200", &library, true)
                .await,
            Ok((Decision::Retrieve { index: 1 }, "demo"))
        );
        assert!(script.requests().is_empty());
        // ...an unrehearsed one does, and the model decides as always.
        assert_eq!(
            with_model.route("zzz qqq", &library, true).await,
            Ok((Decision::Retrieve { index: 0 }, "high"))
        );
        assert_eq!(script.requests().len(), 1);
        // A real organization with a model: the model decides, even for the
        // rehearsed words.
        assert_eq!(
            with_model
                .route("Review expenses over $200", &library, false)
                .await,
            Ok((Decision::Retrieve { index: 0 }, "high"))
        );
        assert_eq!(script.requests().len(), 2);
    });
}

/// A one-shot smoke against the real endpoint. Ignored: it runs only when
/// `app/scripts/composer_smoke.sh` exports `ANTHROPIC_API_KEY` into this
/// process from the repository's gitignored `.env` (the Haiku model for
/// both steps); it prints decisions, codes, counts and durations only.
#[test]
#[ignore = "calls the real model; run through app/scripts/composer_smoke.sh with a key in the environment"]
fn real_model_smoke() {
    let composer = Composer::from_env();
    assert!(composer.enabled(), "no usable key in the environment");
    eprintln!(
        "smoke: router={} generation={}",
        composer.router_model(),
        composer.generation_model()
    );
    runtime().block_on(async {
        let library = [
            library_app(NORTHSTAR, "00000000-0000-4000-8000-000000000065"),
            library_app(SPEND, "00000000-0000-4000-8000-0000000000c9"),
        ];
        let started = std::time::Instant::now();
        let routed = composer
            .route(
                "Review expenses over $200 and record a decision on each",
                &library,
                false,
            )
            .await;
        eprintln!("smoke: routed={routed:?} in {} ms", started.elapsed().as_millis());
        let started = std::time::Instant::now();
        let routed = composer
            .route(
                "Which destinations receive the most bytes each minute?",
                &library,
                false,
            )
            .await;
        eprintln!("smoke: routed={routed:?} in {} ms", started.elapsed().as_millis());
        let started = std::time::Instant::now();
        let generated = composer
            .generate(
                "A traffic review: total bytes, bytes per destination sorted descending, and a minute timeline",
                &sources(NORTHSTAR),
                ORG,
                APP,
                far(),
            )
            .await;
        match &generated {
            Ok(generated) => eprintln!(
                "smoke: generated origin={} title_chars={} views={} digest={} refused={:?} in {} ms",
                generated.planned.origin.as_str(),
                generated.planned.title.chars().count(),
                generated.planned.admitted.view_count,
                generated.planned.admitted.digest,
                generated.refused,
                started.elapsed().as_millis()
            ),
            Err(failure) => eprintln!(
                "smoke: generation failed {failure:?} in {} ms",
                started.elapsed().as_millis()
            ),
        }
        assert!(generated.is_ok(), "generation did not produce an admitted definition");
    });
}
