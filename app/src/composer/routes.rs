//! `POST /v1/compose` (APP-093; ADR-011 §1, §3; ADR-006; ADR-007 D7.9,
//! D7.14): the authoring route the canvas calls.
//!
//! Request: a same-origin `POST` (fetch metadata, as the JSON query route)
//! under a live workspace session, body `{"intent": "<3..500 characters>"}`
//! of at most 4 KiB, no query string. Answer: `200 application/json`
//! `{"app_id", "work_id", "origin": "retrieved"|"adapted"|"generated",
//! "title"}` (forma-json-v1), the saved work the canvas opens at
//! `GET /w/{app_id}/{work_id}`; or a design §9 refusal: the uniform 404 for
//! no, an unknown, an ended or a refused session and for a member storage
//! denies anywhere (a member who may compose nothing is indistinguishable
//! from a stranger); 403 `cross_site`; 413 `payload_too_large`; 422
//! `invalid_intent` (the body); 429 `too_many_requests` with `Retry-After`
//! (6 per member per hour, ADR-006 limits); 503 `composer_unavailable` when
//! no model key is configured and the organization is not a demo one
//! (neutral; everything else in the service works); 422 `could_not_compose` when the model's output was refused by
//! admission on every attempt, the model failed, or publication lost its
//! race (neutral; nothing of the refused output is disclosed); 503
//! `unavailable` for a store outage. The route runs under its own deadline
//! (`COMPOSE_DEADLINE`, `http::uniform_with`) because it waits on the model.
//!
//! Order (APP-093 review M1: nothing of the long deadline is spent before the
//! member is known): fetch metadata → session → composer available → member
//! limit → body (bounded in size and read under `http::HANDLER_DEADLINE`, so
//! a stalled body holds nothing past 5 s) → intent shape → [`compose_for`]:
//! context (one fenced read) → router → (instantiate | adapt | generate) →
//! draft → publish → save. In a demo organization (`Composer::demo_for`) a
//! rehearsed intent skips the router model (`Composer::route`). The
//! structured log records identities, origin, codes, confidence and
//! per-step durations; never the intent text, a definition or the key.
use super::{plan::Decision, Composer, GenerationFailure, Origin, COMPOSE_DEADLINE};
use crate::{
    auth::{cookies, csrf, Auth},
    http::{self, limits},
    storage::{
        composer_bindings, ComposerContext, Error, LibraryApp, Lookup, PublishResult,
        PublishedInput, Stores, TrustedRequest, WorkBinding,
    },
};
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Instant};

/// The route's path (the binary composes it under `http::uniform_with`).
pub const PATH: &str = "/v1/compose";
/// The intent body bound (D7.14 form-sized).
pub const COMPOSE_BODY_LIMIT: usize = 4096;
/// Per member: six compositions an hour.
pub const COMPOSE_MEMBER: &[limits::Window] = &[limits::per(6, 3600)];

/// The route's state: sessions, limits and stores from [`Auth`]; the
/// composer (disabled when no key is configured).
pub struct Compose {
    auth: Arc<Auth>,
    composer: Composer,
}
impl Compose {
    pub fn new(auth: Arc<Auth>, composer: Composer) -> Self {
        Self { auth, composer }
    }
}

pub fn router(state: Arc<Compose>) -> Router {
    Router::new().route(PATH, post(compose)).with_state(state)
}

/// A composed, saved work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Composed {
    pub app: String,
    pub work: String,
    pub origin: Origin,
    pub title: String,
}

/// Why a composition stopped; the HTTP shell maps each to a design §9 answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refused {
    /// The uniform 404: a denial anywhere, or a member who may compose
    /// nothing.
    NotFound,
    /// A store outage (503 `unavailable`).
    StoreUnavailable,
    /// No model key (503 `composer_unavailable`).
    ComposerUnavailable,
    /// A neutral 422 `could_not_compose`; the code is for the log only.
    CouldNotCompose(&'static str),
}

/// One composition for a live member (module documentation, from the
/// composer-enabled check on). Everything the model wrote reaches storage
/// only as an [`crate::admission::AdmittedDefinition`].
pub(crate) async fn compose_for(
    stores: &Stores,
    composer: &Composer,
    member: &TrustedRequest,
    intent: &str,
) -> Result<Composed, Refused> {
    let started = Instant::now();
    let deadline = tokio::time::Instant::now() + COMPOSE_DEADLINE;
    let refused = |code: &'static str, origin: Option<Origin>, refusal: Refused| {
        log_refused(member, code, origin, started);
        Err(refusal)
    };
    let demo = composer.demo_for(member.organization());
    if !composer.available_for(member.organization()) {
        return refused("composer_unavailable", None, Refused::ComposerUnavailable);
    }
    let context = match stores.workspace.composer_context(member).await {
        Ok(context) => context,
        Err(error) => return refused("context_denied", None, storage_refusal(&error)),
    };
    if context.library.is_empty() && context.authoring.is_none() {
        // Nothing this member may compose: indistinguishable from a stranger.
        return refused("nothing_composable", None, Refused::NotFound);
    }

    // Step 1: the router.
    let routed = Instant::now();
    let (decision, confidence) = match composer.route(intent, &context.library, demo).await {
        Ok(decision) => decision,
        Err(error) => {
            // The library cannot be consulted; generation is still possible.
            tracing::warn!(
                event = "composer.router_failed",
                code = error.code(),
                organization_id = member.organization(),
                duration_ms = elapsed_ms(routed),
                "router failed"
            );
            (Decision::Generate, "router_failed")
        }
    };
    tracing::info!(
        event = "composer.routed",
        organization_id = member.organization(),
        principal_id = member.principal(),
        library_size = context.library.len(),
        decision = match &decision {
            Decision::Retrieve { .. } => "retrieve",
            Decision::Adapt { .. } => "adapt",
            Decision::Generate => "generate",
        },
        confidence,
        duration_ms = elapsed_ms(routed),
        "intent routed"
    );

    match decision {
        Decision::Retrieve { index } => {
            retrieve(
                stores,
                member,
                &context,
                &context.library[index],
                intent,
                started,
            )
            .await
        }
        Decision::Adapt { index, title } => {
            let library = &context.library[index];
            let Some(authoring) = context.authoring.as_ref() else {
                // No app to draft on: the library app as it is.
                return retrieve(stores, member, &context, library, intent, started).await;
            };
            let description = format!("Adapted from {} for: {}", library.title, intent);
            match super::plan::plan_adapted(
                library,
                &title,
                &description,
                member.organization(),
                &authoring.app,
            ) {
                Ok(planned) => {
                    draft_publish_save(stores, member, &context, planned, intent, started).await
                }
                Err(refusal) => refused(
                    refusal.code,
                    Some(Origin::Adapted),
                    Refused::CouldNotCompose(refusal.code),
                ),
            }
        }
        Decision::Generate => {
            let Some(authoring) = context.authoring.as_ref() else {
                return refused(
                    "no_authoring_app",
                    Some(Origin::Generated),
                    Refused::CouldNotCompose("no_authoring_app"),
                );
            };
            let generating = Instant::now();
            match composer
                .generate(
                    intent,
                    &context.sources,
                    member.organization(),
                    &authoring.app,
                    deadline,
                )
                .await
            {
                Ok(generated) => {
                    tracing::info!(
                        event = "composer.generated",
                        organization_id = member.organization(),
                        principal_id = member.principal(),
                        attempts = generated.refused.len() + 1,
                        refused_codes = ?generated.refused,
                        view_count = generated.planned.admitted.view_count,
                        definition_digest = generated.planned.admitted.digest,
                        duration_ms = elapsed_ms(generating),
                        "definition generated and admitted"
                    );
                    draft_publish_save(stores, member, &context, generated.planned, intent, started)
                        .await
                }
                Err(GenerationFailure::Refused(codes)) => {
                    tracing::info!(
                        event = "composer.generation_refused",
                        organization_id = member.organization(),
                        principal_id = member.principal(),
                        attempts = codes.len(),
                        refused_codes = ?codes,
                        duration_ms = elapsed_ms(generating),
                        "every generation attempt refused by admission"
                    );
                    refused(
                        "admission_refused",
                        Some(Origin::Generated),
                        Refused::CouldNotCompose("admission_refused"),
                    )
                }
                Err(GenerationFailure::OutOfTime) => refused(
                    "out_of_time",
                    Some(Origin::Generated),
                    Refused::CouldNotCompose("out_of_time"),
                ),
                Err(GenerationFailure::Model(error)) => refused(
                    error.code(),
                    Some(Origin::Generated),
                    Refused::CouldNotCompose(error.code()),
                ),
            }
        }
    }
}

/// Origin `retrieved`: a new saved work of the library app's newest
/// publication, bound under the member's own authority.
async fn retrieve(
    stores: &Stores,
    member: &TrustedRequest,
    context: &ComposerContext,
    library: &LibraryApp,
    intent: &str,
    started: Instant,
) -> Result<Composed, Refused> {
    let Some(bindings) =
        composer_bindings(&library.inputs, &library.record_schemas, &context.sources)
    else {
        log_refused(member, "unbindable", Some(Origin::Retrieved), started);
        return Err(Refused::CouldNotCompose("unbindable"));
    };
    save(
        stores,
        member,
        &library.app,
        &library.publication,
        &bindings,
        Origin::Retrieved,
        &library.title,
        intent,
        started,
    )
    .await
}

/// Origins `adapted` and `generated`: draft the admitted definition on the
/// authoring app, owner-publish it, save a work bound under the member's
/// authority.
async fn draft_publish_save(
    stores: &Stores,
    member: &TrustedRequest,
    context: &ComposerContext,
    planned: super::plan::Planned,
    intent: &str,
    started: Instant,
) -> Result<Composed, Refused> {
    let origin = planned.origin;
    let refused = |code: &'static str, refusal: Refused| {
        log_refused(member, code, Some(origin), started);
        Err(refusal)
    };
    let Some(authoring) = context.authoring.as_ref() else {
        return refused(
            "no_authoring_app",
            Refused::CouldNotCompose("no_authoring_app"),
        );
    };
    let Some(policy) = context.owner_publish_policy.as_deref() else {
        return refused(
            "no_owner_publish_policy",
            Refused::CouldNotCompose("no_owner_publish_policy"),
        );
    };
    let step = Instant::now();
    let revision = match stores
        .workspace
        .create_definition_revision(member, &authoring.app, &planned.admitted)
        .await
    {
        Ok(revision) => revision,
        Err(error) => return refused("draft_refused", storage_refusal(&error)),
    };
    let publication = match stores
        .control
        .publish(member, &authoring.app, &revision.revision, policy)
        .await
    {
        Ok(PublishResult::Published(publication)) => publication,
        Ok(PublishResult::Stale(_)) => {
            return refused("publish_stale", Refused::CouldNotCompose("publish_stale"))
        }
        Err(error) => return refused("publish_refused", storage_refusal(&error)),
    };
    tracing::info!(
        event = "composer.published",
        organization_id = member.organization(),
        app_id = authoring.app,
        revision_id = revision.revision,
        publication_id = publication.publication_id,
        definition_digest = publication.definition_digest,
        origin = origin.as_str(),
        duration_ms = elapsed_ms(step),
        "composed definition published"
    );
    // Review M5: from here to the saved work, a cut (the route deadline, a
    // dropped connection) or a refusal leaves a published revision without a
    // work. The guard announces it in the log on every exit but a save.
    let mut orphan = PublishedGuard {
        organization: member.organization().to_owned(),
        app: authoring.app.clone(),
        revision: revision.revision.clone(),
        publication: publication.publication_id.clone(),
        origin,
        armed: true,
    };
    // The published inputs are the admitted content's; bind them.
    let Ok(content) = crate::admission::decode(&planned.admitted.canonical) else {
        return Err(Refused::StoreUnavailable);
    };
    let inputs = published_inputs(&content);
    let schemas = record_schemas(&content);
    let Some(bindings) = composer_bindings(&inputs, &schemas, &context.sources) else {
        return refused("unbindable", Refused::CouldNotCompose("unbindable"));
    };
    let saved = save(
        stores,
        member,
        &authoring.app,
        &publication.publication_id,
        &bindings,
        origin,
        &planned.title,
        intent,
        started,
    )
    .await;
    if saved.is_ok() {
        orphan.armed = false;
    }
    saved
}

/// A published revision not yet saved as a work (review M5). Dropped armed
/// (a refusal, the route deadline cancelling the future, a lost connection),
/// it logs `composer.publication_orphaned` with the ids, so the orphan is
/// announced and the operator can see it; a drafted-but-unsaved publication is
/// harmless to readers (no work names it) and is recorded as APP-097 debt.
struct PublishedGuard {
    organization: String,
    app: String,
    revision: String,
    publication: String,
    origin: Origin,
    armed: bool,
}
impl Drop for PublishedGuard {
    fn drop(&mut self) {
        if self.armed {
            tracing::warn!(
                event = "composer.publication_orphaned",
                organization_id = self.organization,
                app_id = self.app,
                revision_id = self.revision,
                publication_id = self.publication,
                origin = self.origin.as_str(),
                "published revision has no saved work"
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn save(
    stores: &Stores,
    member: &TrustedRequest,
    app: &str,
    publication: &str,
    bindings: &[WorkBinding],
    origin: Origin,
    title: &str,
    intent: &str,
    started: Instant,
) -> Result<Composed, Refused> {
    match stores
        .workspace
        .create_work(member, app, publication, intent, bindings)
        .await
    {
        Ok(work) => {
            // APP-095: the organization's event stream learns that an app was
            // placed (ids only; every subscriber is re-authorized at delivery).
            crate::events::hub().app_placed(member.organization(), app, &work.work);
            tracing::info!(
                event = "composer.composed",
                organization_id = member.organization(),
                principal_id = member.principal(),
                app_id = app,
                work_id = work.work,
                origin = origin.as_str(),
                duration_ms = elapsed_ms(started),
                "app composed"
            );
            Ok(Composed {
                app: app.to_owned(),
                work: work.work,
                origin,
                title: title.to_owned(),
            })
        }
        Err(error) => {
            log_refused(member, "save_refused", Some(origin), started);
            Err(storage_refusal(&error))
        }
    }
}

fn storage_refusal(error: &Error) -> Refused {
    match error {
        Error::Denied => Refused::NotFound,
        _ => Refused::StoreUnavailable,
    }
}

fn log_refused(request: &TrustedRequest, code: &str, origin: Option<Origin>, started: Instant) {
    tracing::info!(
        event = "composer.refused",
        code,
        organization_id = request.organization(),
        principal_id = request.principal(),
        origin = origin.map(Origin::as_str),
        duration_ms = elapsed_ms(started),
        "composition refused"
    );
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The admitted content's inputs as published inputs (id, kind, contract
/// digest, fields), for binding.
fn published_inputs(content: &Value) -> Vec<PublishedInput> {
    content
        .get("inputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|input| {
            let canonical = crate::admission::encode(input).ok()?;
            Some(PublishedInput {
                id: input.get("id")?.as_str()?.to_owned(),
                kind: input.get("kind")?.as_str()?.to_owned(),
                contract_digest: crate::admission::digest::digest(
                    crate::admission::digest::INPUT_CONTRACT,
                    &canonical,
                ),
                fields: input.get("fields")?.clone(),
            })
        })
        .collect()
}
fn record_schemas(content: &Value) -> Vec<(String, Value)> {
    content
        .get("record_schemas")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|schema| {
            Some((
                schema.get("id")?.as_str()?.to_owned(),
                schema.get("fields")?.clone(),
            ))
        })
        .collect()
}

// --- The HTTP shell ---

fn problem(status: StatusCode, code: &'static str) -> Response {
    http::problem(status, code)
}

/// A live workspace session and its trusted request, as the query route.
async fn member(auth: &Auth, headers: &HeaderMap) -> Result<TrustedRequest, Response> {
    let cookie = cookies::read(headers, cookies::SESSION).ok_or_else(http::not_found)?;
    let stores = auth.stores.as_ref().ok_or_else(http::not_found)?;
    let live = |reference: &str| auth.vault.contains(reference);
    match stores.workspace.resolve_session(&cookie, &live).await {
        Ok(Lookup::Live(request)) => Ok(request),
        Ok(Lookup::Ended(ended)) => {
            auth.revoke(&ended).await;
            Err(http::not_found())
        }
        Ok(Lookup::Absent) => Err(http::not_found()),
        Err(Error::Denied) => Err(http::not_found()),
        Err(_) => Err(http::unavailable()),
    }
}

/// `{"intent": "<text>"}` and nothing else.
pub(super) fn body_intent(bytes: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let text = object.get("intent")?.as_str()?;
    super::prompt::intent(text).map(str::to_owned)
}

fn json_response(body: Vec<u8>) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    http::secure(response.headers_mut());
    response
}

/// How a bounded body read ended short of bytes.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum BodyRefused {
    /// More than the bound, or the stream failed.
    TooLarge,
    /// The bytes did not arrive within the handler deadline (review M1).
    Stalled,
}

/// The intent body: at most `COMPOSE_BODY_LIMIT` bytes, arriving within
/// `deadline`; a caller that trickles or stalls holds the route no longer
/// than any other handler.
pub(super) async fn read_body(
    body: axum::body::Body,
    deadline: std::time::Duration,
) -> Result<axum::body::Bytes, BodyRefused> {
    match tokio::time::timeout(deadline, to_bytes(body, COMPOSE_BODY_LIMIT)).await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(_)) => Err(BodyRefused::TooLarge),
        Err(_) => Err(BodyRefused::Stalled),
    }
}

/// `POST /v1/compose` (order in the module notes).
async fn compose(State(state): State<Arc<Compose>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if !csrf::same_origin(&parts.headers, state.auth.origin()) {
        return http::cross_site();
    }
    // Review M1: the session first, so an anonymous caller spends nothing of
    // the long deadline (the body is not read until the member is known).
    let member = match member(&state.auth, &parts.headers).await {
        Ok(member) => member,
        Err(refusal) => return refusal,
    };
    let started = Instant::now();
    if !state.composer.available_for(member.organization()) {
        log_refused(&member, "composer_unavailable", None, started);
        return problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable");
    }
    let key = format!("compose:{}:{}", member.organization(), member.principal());
    if let Err(retry_after) = state.auth.limits.admit(&[(&key, COMPOSE_MEMBER)]) {
        log_refused(&member, "too_many_requests", None, started);
        return http::too_many_requests(retry_after);
    }
    let bytes = match read_body(body, http::HANDLER_DEADLINE).await {
        Ok(bytes) => bytes,
        Err(BodyRefused::TooLarge) => {
            log_refused(&member, "payload_too_large", None, started);
            return http::payload_too_large();
        }
        Err(BodyRefused::Stalled) => {
            // A body that never arrives is answered as any refusal of this
            // member's request shape: neutrally, with nothing composed.
            log_refused(&member, "body_stalled", None, started);
            return http::not_found();
        }
    };
    let Some(intent) = body_intent(&bytes).filter(|_| parts.uri.query().is_none()) else {
        log_refused(&member, "invalid_intent", None, started);
        return problem(StatusCode::UNPROCESSABLE_ENTITY, "invalid_intent");
    };
    let Some(stores) = state.auth.stores.as_ref() else {
        return http::not_found();
    };
    match compose_for(stores, &state.composer, &member, &intent).await {
        Ok(composed) => {
            let body = json!({
                "app_id": composed.app,
                "work_id": composed.work,
                "origin": composed.origin.as_str(),
                "title": composed.title,
            });
            match crate::admission::encode(&body) {
                Ok(bytes) => json_response(bytes),
                Err(_) => http::unavailable(),
            }
        }
        Err(Refused::NotFound) => http::not_found(),
        Err(Refused::StoreUnavailable) => http::unavailable(),
        Err(Refused::ComposerUnavailable) => {
            problem(StatusCode::SERVICE_UNAVAILABLE, "composer_unavailable")
        }
        Err(Refused::CouldNotCompose(_)) => {
            problem(StatusCode::UNPROCESSABLE_ENTITY, "could_not_compose")
        }
    }
}
