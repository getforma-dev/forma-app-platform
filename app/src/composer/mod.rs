//! APP-093 composer (ADR-011 §1, §3 I1/I2/I5/I6, §4): a signed-in member
//! describes an app; the composer answers with a saved work the canvas can
//! open, labeled by origin.
//!
//! **Library first.** Step 1 is one fast router call (the Haiku model by
//! default) over the member's intent and the compact catalog of library apps
//! the member may `discover` and `run` and can bind now
//! (`Workspace::composer_context`). A confident choice is instantiated as a
//! new saved work of that app's newest publication, bound to the
//! organization's registered source revisions under the member's own
//! `source_read`: origin `retrieved`, no generation call. A medium-confidence
//! choice becomes `adapted`: a typed patch (title, description) of the
//! library definition, re-admitted whole, drafted on the app the member owns
//! and owner-published. Step 2, only when the library has no fit: the
//! generation model (Sonnet by default) is asked for one JSON content object
//! under the closed grammar with the two fixtures as examples and only the
//! operators the engine runs; the text is parsed strictly, pinned to the
//! member's exact source contracts, enveloped under the trusted scope and
//! admitted whole; a refusal is retried at most twice with its code alone;
//! the admitted definition is drafted, owner-published and saved: origin
//! `generated`.
//!
//! **APP-101: instant pick and live transform.** Before any model call, the
//! local matcher (`matcher`) scores the intent against the library's routing
//! metadata in every organization; a confident pick is retrieved at once (no
//! model call), a near tie or no hit goes to the router model as above. When
//! the request says more than the picked app's defaults, the fast model (or,
//! in a demo organization, a rehearsed sentence) turns the rest into a
//! closed, typed `transform::Transform`, which is judged against the app's
//! input contract, applied to the library content, admitted whole and saved
//! as an `adapted` work on the member's authoring app, in the background
//! after the response (`routes::Tailor`); `app.adapted` (or `app.kept`) on
//! the event stream tells the canvas to reshape (or to keep the defaults).
//!
//! Every path ends in the APP-010 storage operations unchanged (their own
//! prechecks, fences and audit events); model output exists only as a typed
//! definition after `admit_definition`, never as markup, SQL, FMIR or a
//! grant (AGENTS.md). Creating an app and granting on it stay operator
//! actions (ADR-007 D7.3): a member who owns no app with `edit_definition`
//! and `run` can retrieve but not generate. Logs carry identities, codes,
//! counts and durations; never the intent, a definition or the key.
pub mod ask;
pub mod commands;
pub mod labels;
pub mod lineage;
pub mod matcher;
pub mod model;
pub mod plan;
pub mod prompt;
pub mod routes;
pub mod shape;
pub mod talk;
pub mod transform;

use crate::storage::{LibraryApp, ReadableSource};
use model::{Model, ModelError, ModelRequest};
use plan::{Decision, Planned, Refusal};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::time::Instant;
use transform::Transform;

#[cfg(test)]
mod tests;

/// How a composed app came to be (ADR-011 §1: "retrieved, adapted or
/// generated, and labeled as such").
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    Retrieved,
    Adapted,
    Generated,
}
impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retrieved => "retrieved",
            Self::Adapted => "adapted",
            Self::Generated => "generated",
        }
    }
}

/// Generation attempts: the first and two retries carrying the code.
pub const MAX_GENERATION_ATTEMPTS: usize = 3;
/// The router call's budget (the fast path targets about 1.5 s).
pub const ROUTER_BUDGET: Duration = Duration::from_secs(12);
/// One generation attempt's budget (APP-093 review M5: the router, three
/// attempts and the storage reserve fit the route deadline).
pub const GENERATION_BUDGET: Duration = Duration::from_secs(32);
/// Time kept for draft, publish and save after the last model call, so the
/// route deadline never cuts between publish and save.
pub const STORAGE_RESERVE: Duration = Duration::from_secs(10);
/// A generation attempt is not started with less than this left for it.
pub const MIN_GENERATION_ATTEMPT: Duration = Duration::from_secs(5);
/// The whole route, every model call and storage operation included
/// (the member's request times out at this bound).
pub const COMPOSE_DEADLINE: Duration = Duration::from_secs(120);
/// ADR-011 demo mode (APP-098 demo shortcuts register; Victor, 2026-10-30:
/// the pitch is a concept-car demo): the organization ids (comma-separated
/// UUIDs) whose members' rehearsed sentences are tailored by a fixed
/// transform without a model call (`transform::rehearsed`, APP-098 DS-53;
/// APP-101 replaced the earlier canned router, `plan::demo_route`, with the
/// matcher every organization runs). Text that is not rehearsed goes to the
/// real model as everywhere else. Empty or unset by default: no
/// organization; the pitch deploy's stack names the synthetic demo
/// organization only. A rehearsed transform is judged and admitted like any
/// other; nothing bypasses admission, authorization, the audited command
/// path, receipts or live updates.
pub const DEMO_ORGANIZATIONS_ENV: &str = "GENUI_DEMO_ORGANIZATIONS";
const ROUTER_MAX_TOKENS: u32 = 300;
/// The transform call's output bound and budget (APP-101: the reshape
/// targets 1.5 s after the pick; the budget only caps a slow model).
const TRANSFORM_MAX_TOKENS: u32 = 400;
pub const TRANSFORM_BUDGET: Duration = Duration::from_secs(10);
const GENERATION_MAX_TOKENS: u32 = 8000;

/// The composer: a model (none: disabled) and the two model names.
#[derive(Clone)]
pub struct Composer {
    model: Option<Arc<dyn Model>>,
    router_model: String,
    generation_model: String,
    /// `DEMO_ORGANIZATIONS_ENV`: the demo organizations (module notes).
    demo: BTreeSet<String>,
    /// APP-101: the instant matcher runs before the router model
    /// (`matcher::MATCHER_ENV=off` turns it off: the pre-APP-101 behavior).
    matcher: bool,
}

/// Why generation ended without a definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationFailure {
    /// Every attempt was refused; the codes, in order (for the log).
    Refused(Vec<&'static str>),
    Model(ModelError),
    /// The route deadline left no room for an attempt (review M5): nothing
    /// was called, so nothing is cut between publish and save.
    OutOfTime,
}

/// Why a transform ended without a typed transform.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransformFailure {
    Model(ModelError),
    Refused(Refusal),
}

/// A generation that ended in a definition, with the codes of the refused
/// attempts before it.
#[derive(Clone, Debug)]
pub struct Generated {
    pub planned: Planned,
    pub refused: Vec<&'static str>,
}

impl Composer {
    pub fn new(model: Option<Arc<dyn Model>>, router_model: &str, generation_model: &str) -> Self {
        Self {
            model,
            router_model: router_model.to_owned(),
            generation_model: generation_model.to_owned(),
            demo: BTreeSet::new(),
            matcher: true,
        }
    }
    /// Turns the instant matcher on or off (module notes).
    pub fn with_matcher(mut self, on: bool) -> Self {
        self.matcher = on;
        self
    }
    /// The demo organizations (module notes; `DEMO_ORGANIZATIONS_ENV`).
    pub fn with_demo_organizations<I: IntoIterator<Item = String>>(mut self, ids: I) -> Self {
        self.demo = ids.into_iter().collect();
        self
    }
    /// The environment's composer: the key from `model::KEY_ENV` only (none
    /// or unusable: disabled; the rest of the service is unaffected), the
    /// model names, overridable, and the demo organizations
    /// (`DEMO_ORGANIZATIONS_ENV`: comma-separated lowercase UUIDs; anything
    /// else in the list is ignored).
    pub fn from_env() -> Self {
        let model = model::Anthropic::from_env().map(|client| Arc::new(client) as Arc<dyn Model>);
        let name = |variable: &str, default: &str| {
            std::env::var(variable)
                .ok()
                .filter(|value| model_name(value))
                .unwrap_or_else(|| default.to_owned())
        };
        Self::new(
            model,
            &name(model::ROUTER_MODEL_ENV, model::DEFAULT_ROUTER_MODEL),
            &name(model::GENERATION_MODEL_ENV, model::DEFAULT_GENERATION_MODEL),
        )
        .with_demo_organizations(demo_organizations(
            &std::env::var(DEMO_ORGANIZATIONS_ENV).unwrap_or_default(),
        ))
        .with_matcher(std::env::var(matcher::MATCHER_ENV).as_deref() != Ok("off"))
    }
    /// APP-122: the widget customizer over this composer's model and fast
    /// model name (`/shape <widget>` reaches the APP-103 Restyle mechanism).
    pub fn customizer(&self) -> crate::widgets::customize::Customizer {
        crate::widgets::customize::Customizer::new(self.model.clone(), &self.router_model)
    }
    /// A model is configured.
    pub fn enabled(&self) -> bool {
        self.model.is_some()
    }
    /// Whether this organization is a demo organization (module notes):
    /// listed here and by [`crate::demo::is_demo_org`], so `GENUI_DEMO_MODE`
    /// off turns every composer shortcut off.
    pub fn demo_for(&self, organization: &str) -> bool {
        self.demo.contains(organization) && crate::demo::is_demo_org(organization)
    }
    /// The route may compose for this organization: a model, the instant
    /// matcher (APP-101: a confident pick needs no model; without one the
    /// composition answers `composer_unavailable`), or the demo
    /// organization's rehearsed path.
    pub fn available_for(&self, organization: &str) -> bool {
        self.enabled() || self.matcher || self.demo_for(organization)
    }
    /// Whether the instant matcher runs.
    pub fn matcher_on(&self) -> bool {
        self.matcher
    }
    /// APP-108: one fast-model call (the router model) for a typed answer
    /// the caller judges (the box explanation); `Unavailable` without a model.
    pub async fn complete_fast(
        &self,
        system: &str,
        user: String,
        max_tokens: u32,
        budget: Duration,
    ) -> Result<model::ModelReply, ModelError> {
        let model = self.model.as_ref().ok_or(ModelError::Unavailable)?;
        model
            .complete(&ModelRequest {
                model: self.router_model.clone(),
                system: system.to_owned(),
                user,
                max_tokens,
                budget,
            })
            .await
    }
    /// APP-108: one fast-model call with PDF documents and citations.
    pub async fn complete_documents(
        &self,
        system: &str,
        user: String,
        documents: &[model::DocumentInput<'_>],
        max_tokens: u32,
        budget: Duration,
    ) -> Result<model::CitedReply, ModelError> {
        let model = self.model.as_ref().ok_or(ModelError::Unavailable)?;
        let request = ModelRequest {
            model: self.router_model.clone(),
            system: system.to_owned(),
            user,
            max_tokens,
            budget,
        };
        model.complete_with_documents(&request, documents).await
    }
    pub fn router_model(&self) -> &str {
        &self.router_model
    }
    pub fn generation_model(&self) -> &str {
        &self.generation_model
    }

    /// Step 1: the router's decision over the catalog, and its confidence
    /// word for the log. An empty library is `Generate` without a call.
    ///
    /// APP-101: the instant matcher first (`matcher::pick`; `exclude` is the
    /// member's authoring app, never a candidate): a confident pick is
    /// `Retrieve` with confidence `matched` and no model call; a near tie or
    /// no hit goes to the router model.
    pub async fn route(
        &self,
        intent: &str,
        library: &[LibraryApp],
        exclude: Option<&str>,
    ) -> Result<(Decision, &'static str), ModelError> {
        if library.is_empty() {
            return Ok((Decision::Generate, "empty_library"));
        }
        let mut nearest = None;
        if self.matcher {
            let found = matcher::pick(intent, library, exclude);
            if let Some(index) = found.pick {
                return Ok((Decision::Retrieve { index }, "matched"));
            }
            nearest = found.ranked.first().copied();
        }
        let Some(model) = self.model.as_ref() else {
            // No model to settle a near tie (APP-098 DS-03): the matcher's
            // best candidate, still only an app from the catalog, labeled
            // retrieved; with no candidate at all the caller is refused.
            return match nearest {
                Some(index) => Ok((Decision::Retrieve { index }, "nearest")),
                None => Err(ModelError::Unavailable),
            };
        };
        let request = ModelRequest {
            model: self.router_model.clone(),
            system: prompt::ROUTER_SYSTEM.to_owned(),
            user: prompt::router_user(intent, library),
            max_tokens: ROUTER_MAX_TOKENS,
            budget: ROUTER_BUDGET,
        };
        let reply = model.complete(&request).await?;
        Ok(plan::decide(&reply.text, library))
    }

    /// APP-101: the transform of a picked app. In a demo organization a
    /// rehearsed sentence is a fixed transform (`transform::rehearsed`,
    /// APP-098 DS-53, source `rehearsed`); otherwise the fast router model
    /// answers the closed shape (source `model`), parsed strictly. The
    /// caller judges and applies it (`transform::plan_transformed`).
    pub async fn transform(
        &self,
        intent: &str,
        library: &LibraryApp,
        demo: bool,
    ) -> Result<(Transform, &'static str), TransformFailure> {
        if demo {
            if let Some(rehearsed) = transform::rehearsed(intent) {
                return Ok((rehearsed, "rehearsed"));
            }
        }
        let model = self
            .model
            .as_ref()
            .ok_or(TransformFailure::Model(ModelError::Unavailable))?;
        let request = ModelRequest {
            model: self.router_model.clone(),
            system: transform::TRANSFORM_SYSTEM.to_owned(),
            user: transform::transform_user(intent, library),
            max_tokens: TRANSFORM_MAX_TOKENS,
            budget: TRANSFORM_BUDGET,
        };
        let reply = model
            .complete(&request)
            .await
            .map_err(TransformFailure::Model)?;
        transform::parse(&reply.text)
            .map(|parsed| (parsed, "model"))
            .map_err(TransformFailure::Refused)
    }

    /// Step 2: generation with at most two retries, each carrying only the
    /// previous refusal's code and its fixed hint.
    ///
    /// `deadline` is the route's (review M5): an attempt starts only while
    /// at least `MIN_GENERATION_ATTEMPT` remains after `STORAGE_RESERVE`, with
    /// a budget cut to what remains, so the route deadline never falls
    /// between publish and save.
    pub async fn generate(
        &self,
        intent: &str,
        sources: &[ReadableSource],
        organization: &str,
        app: &str,
        deadline: Instant,
    ) -> Result<Generated, GenerationFailure> {
        self.generate_with(intent, None, sources, organization, app, deadline)
            .await
    }

    /// APP-108: generation with a box's capped context in the user message
    /// (`prompt::generation_user_with`); `sources` are the box's registered
    /// revisions only, so every source input is pinned to one of them.
    pub async fn generate_with(
        &self,
        intent: &str,
        context: Option<&serde_json::Value>,
        sources: &[ReadableSource],
        organization: &str,
        app: &str,
        deadline: Instant,
    ) -> Result<Generated, GenerationFailure> {
        let model = self
            .model
            .as_ref()
            .ok_or(GenerationFailure::Model(ModelError::Unavailable))?;
        let contracts: Vec<serde_json::Value> = sources
            .iter()
            .map(|source| source.contract.clone())
            .collect();
        let mut refused: Vec<&'static str> = Vec::new();
        let mut previous: Option<Refusal> = None;
        for _ in 0..MAX_GENERATION_ATTEMPTS {
            let Some(budget) = attempt_budget(Instant::now(), deadline) else {
                return Err(if refused.is_empty() {
                    GenerationFailure::OutOfTime
                } else {
                    GenerationFailure::Refused(refused)
                });
            };
            let feedback = previous
                .as_ref()
                .map(|refusal| (refusal.code, prompt::hint(refusal.code)));
            let request = ModelRequest {
                model: self.generation_model.clone(),
                system: prompt::generation_system().to_owned(),
                user: prompt::generation_user_with(intent, &contracts, feedback, context),
                max_tokens: GENERATION_MAX_TOKENS,
                budget,
            };
            let reply = model
                .complete(&request)
                .await
                .map_err(GenerationFailure::Model)?;
            match plan::plan_generated(&reply.text, sources, organization, app) {
                Ok(planned) => return Ok(Generated { planned, refused }),
                Err(refusal) => {
                    refused.push(refusal.code);
                    previous = Some(refusal);
                }
            }
        }
        Err(GenerationFailure::Refused(refused))
    }
}

/// `DEMO_ORGANIZATIONS_ENV`'s value: the lowercase UUIDs it lists, nothing else.
pub fn demo_organizations(value: &str) -> BTreeSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| uuid_shaped(id))
        .map(str::to_owned)
        .collect()
}
fn uuid_shaped(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// The budget of a generation attempt started `now` under the route
/// `deadline`: `GENERATION_BUDGET`, cut to what remains before the storage
/// reserve; `None` when less than `MIN_GENERATION_ATTEMPT` remains.
pub fn attempt_budget(now: Instant, deadline: Instant) -> Option<Duration> {
    let remaining = deadline
        .checked_duration_since(now)?
        .checked_sub(STORAGE_RESERVE)?;
    (remaining >= MIN_GENERATION_ATTEMPT).then(|| remaining.min(GENERATION_BUDGET))
}

/// A model name the environment may set: a bounded identifier.
fn model_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
}
