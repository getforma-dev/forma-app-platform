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
//! Every path ends in the APP-010 storage operations unchanged (their own
//! prechecks, fences and audit events); model output exists only as a typed
//! definition after `admit_definition`, never as markup, SQL, FMIR or a
//! grant (AGENTS.md). Creating an app and granting on it stay operator
//! actions (ADR-007 D7.3): a member who owns no app with `edit_definition`
//! and `run` can retrieve but not generate. Logs carry identities, codes,
//! counts and durations; never the intent, a definition or the key.
pub mod model;
pub mod plan;
pub mod prompt;
pub mod routes;

use crate::storage::{LibraryApp, ReadableSource};
use model::{Model, ModelError, ModelRequest};
use plan::{Decision, Planned, Refusal};
use std::{sync::Arc, time::Duration};

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
/// One generation attempt's budget.
pub const GENERATION_BUDGET: Duration = Duration::from_secs(60);
/// The whole route, every model call and storage operation included
/// (the member's request times out at this bound).
pub const COMPOSE_DEADLINE: Duration = Duration::from_secs(120);
const ROUTER_MAX_TOKENS: u32 = 300;
const GENERATION_MAX_TOKENS: u32 = 8000;

/// The composer: a model (none: disabled) and the two model names.
pub struct Composer {
    model: Option<Arc<dyn Model>>,
    router_model: String,
    generation_model: String,
}

/// Why generation ended without a definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerationFailure {
    /// Every attempt was refused; the codes, in order (for the log).
    Refused(Vec<&'static str>),
    Model(ModelError),
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
        }
    }
    /// The environment's composer: the key from `model::KEY_ENV` only (none
    /// or unusable: disabled; the rest of the service is unaffected) and the
    /// model names, overridable.
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
    }
    pub fn enabled(&self) -> bool {
        self.model.is_some()
    }
    pub fn router_model(&self) -> &str {
        &self.router_model
    }
    pub fn generation_model(&self) -> &str {
        &self.generation_model
    }

    /// Step 1: the router's decision over the catalog, and its confidence
    /// word for the log. An empty library is `Generate` without a call.
    pub async fn route(
        &self,
        intent: &str,
        library: &[LibraryApp],
    ) -> Result<(Decision, &'static str), ModelError> {
        if library.is_empty() {
            return Ok((Decision::Generate, "empty_library"));
        }
        let model = self.model.as_ref().ok_or(ModelError::Unavailable)?;
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

    /// Step 2: generation with at most two retries, each carrying only the
    /// previous refusal's code and its fixed hint.
    pub async fn generate(
        &self,
        intent: &str,
        sources: &[ReadableSource],
        organization: &str,
        app: &str,
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
            let feedback = previous
                .as_ref()
                .map(|refusal| (refusal.code, prompt::hint(refusal.code)));
            let request = ModelRequest {
                model: self.generation_model.clone(),
                system: prompt::generation_system().to_owned(),
                user: prompt::generation_user(intent, &contracts, feedback),
                max_tokens: GENERATION_MAX_TOKENS,
                budget: GENERATION_BUDGET,
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

/// A model name the environment may set: a bounded identifier.
fn model_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_')
}
