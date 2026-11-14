//! Tenant storage primitives, the APP-073 admission operations, the APP-010
//! definition, publication and saved-work operations, the APP-009 session
//! records and the APP-013 record commands. `sessions` constructs
//! `VerifiedNonmember` from a verified provider exchange and is the only
//! constructor of a `TrustedRequest`, from the session row, the member's
//! current epoch and the registry identity. A caller cannot construct a
//! trusted request from headers or verifier strings.
mod actions;
mod admission;
mod agents;
mod artifact_kind;
pub(crate) use artifact_kind::parse_artifact_kind;
mod authorization;
// APP-108 (ADR-011 pitch): boxes, files, registered data, derivations.
mod boxes;
// APP-123 (ADR-011 pitch): cards made by you and pinned answers.
mod cards;
mod catalog;
mod commands;
mod composer;
mod data_copy;
mod definitions;
mod governance;
mod labels;
// Trial plan D4/D6 (ADR-008 amendment 2026-11-14): organization admins and
// member display names.
mod org_admin;
mod pool;
mod queries;
mod record_components;
mod record_query;
mod registry;
pub mod rules;
mod sessions;
mod shared_access;
mod shared_identity;
mod shared_reads;
mod sharing;
mod work_labels;
mod work_presentation;
#[cfg(test)]
mod work_presentation_checks;

pub use actions::{
    CollectionSummary, DryRun, RecordRow, RecordsListing, WorkReads, READ_ONLY_WORKS, RECORDS_LIMIT,
};
pub use admission::{
    code_digest, handle_digest, Admission, PendingAdmission, PendingReason, Reservation, SignOut,
    VerifiedNonmember,
};
pub use agents::{
    agent_label_shaped, AgentAttribution, AgentToken, MintedAgentToken, AGENT_TOKEN_DAYS,
    MAX_LIVE_AGENT_TOKENS,
};
pub use authorization::{Action, Publication, Record, TrustedRequest};
pub use boxes::{
    box_name, BoxFile, BoxRow, BoxView, Derivation, FileData, Lineage, NewFile, NewRegistration,
    MAX_BOX_FILES,
};
pub use cards::{Carded, MemberCard, NewCard, ReceiptKey, MAX_MEMBER_CARDS, MAX_SPEC_BYTES};
pub use commands::{
    idempotency_key_shaped, CommandOutcome, CommandRequest, CommandResult, ConflictReason,
    RecordCommand, RecordDetail, RecordField, RecoveryView, COMMAND_COMMITTED,
    COMMAND_HANDLER_DEADLINE, MAX_COMMAND_VALUES, RETRY_HORIZON_SECONDS,
};
pub use composer::{
    bindings as composer_bindings, AuthoringApp, ComposerContext, LibraryApp, LibraryView,
    PublishedInput, ReadableSource, CONTEXT_DEADLINE as COMPOSER_CONTEXT_DEADLINE,
};
pub use data_copy::{
    editable_definition, DataCopyInit, DataCopyInput, DataCopyReceipt, DataCopyRows,
    DATA_COPY_BYTES, DATA_COPY_ROWS, DATA_COPY_TRANSPORT_BYTES,
};
pub use definitions::{
    CurrentRevision, DefinitionDetail, DefinitionRevision, InputContract, PublicationDetail,
    PublishResult, RecordSchema, SavedWork, SavedWorkDetail, WorkBinding, MAX_CANONICAL_BYTES,
};
pub use governance::{
    AccessEntry, AccessRow, AgentScope, AgentSummary, ACTIVITY_LIMIT, MAX_SCOPE_APPS, MAX_TOUCHED,
    SCOPE_ACTIONS, SCOPE_SPACES,
};
pub use labels::{
    address_shaped, card_shaped, label_description, label_name, merge_label, shortcut_shaped,
    Label, LabelChange, Labeled, Labels, MemberLabel, NameRules, MAX_LABEL_ALIASES,
};
pub use org_admin::{MemberRow, RemoveOutcome};
pub use pool::{Control, Data, Settings, Stores, Worker, Workspace};
pub use queries::QueryActor;
pub use queries::{
    command_slug, events_fields, field_type, library_sample_sources, library_sidecar,
    parse_parameters, query_digests, LibraryEntry, NamedWork, OpenableWork, Parameters, QueryRun,
    RunSource, WorkOutline, WorkPart, WorkView, COMMAND_MARKER, QUALIFIED_OPERATORS,
    QUERY_HANDLER_DEADLINE,
};
pub use record_components::{
    record_component_digest, RecordComponentsRun, RECORD_COMPONENT_BYTES, RECORD_COMPONENT_ROWS,
};
pub use registry::RegistryIdentity;
pub use sessions::{
    agent_token_organization, capability_organization, capability_shaped, slack_id, AdmittedMember,
    AgentCaller, Completion, Ended, FlowKind, FlowRecord, FlowStart, Lookup, PreAccess,
    PreAccessStart, SignedOut, AGENT_TOKEN_PREFIX, PRELOGIN_SECONDS, PRE_ACCESS_ABSOLUTE_SECONDS,
    PRE_ACCESS_IDLE_SECONDS, SESSION_ABSOLUTE_SECONDS, SESSION_IDLE_SECONDS,
};
pub use shared_access::{
    normalize_share_email, GuestAccess, LinkedApp, LiveShare, NewLiveShare, SharedAuthTarget,
};
pub use shared_identity::*;
pub use shared_reads::GuestQueryRun;
pub use sharing::{
    member_label, Audience, Granted, Member, Openable, ShareEntry, ShareRole, ShareStanding,
    Shared, Standing, WorkSharing, JUDGMENT_LIMIT, SHARE_HANDLER_DEADLINE,
};
pub use work_labels::{WorkLabel, WorkLabelResult};
pub use work_presentation::WorkPresentation;

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Configuration,
    Denied,
    Unavailable,
    Deadline,
    ProjectionLimit,
    Busy,
    UnknownOutcome,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "storage configuration refused",
            Self::Denied => "resource unavailable",
            Self::Unavailable => "storage unavailable",
            Self::Deadline => "storage deadline exceeded",
            Self::ProjectionLimit => "record projection bound exceeded",
            Self::Busy => "record snapshot capacity reached",
            Self::UnknownOutcome => "transaction outcome unknown",
        })
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Denied)
    }
}

/// A canonical contract organization identifier (configuration validation).
pub fn valid_organization(value: &str) -> bool {
    valid_id(value)
}
/// A canonical contract identifier in a route path (shape only; no lookup).
pub fn valid_identifier(value: &str) -> bool {
    valid_id(value)
}
/// A lowercase local identifier in a route path (`genui.local_id`).
pub fn valid_local_id(value: &str) -> bool {
    definitions::local_id(value)
}

/// Canonical contract UUIDs: lowercase, RFC variant, version4 or7.
fn valid_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
        && matches!(value.as_bytes()[14], b'4' | b'7')
        && matches!(value.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
}

// The Docker fixtures are crate-visible to the APP-009 session campaign
// (`auth::tests`); still compiled only into the test executable.
#[cfg(test)]
mod actions_campaign;
// APP-108: the folders campaign (Docker; upload → parse → register → view →
// derived app → share → isolation).
#[cfg(test)]
pub(crate) mod admission_qualification;
#[cfg(test)]
mod box_campaign;
#[cfg(test)]
pub(crate) mod command_qualification;
#[cfg(test)]
mod composer_qualification;
#[cfg(test)]
mod definition_qualification;
#[cfg(test)]
mod managed_qualification;
#[cfg(test)]
mod mcp_campaign;
// Trial plan D4/D5/D6: consent, names and the organization admin definers.
#[cfg(test)]
mod org_admin_campaign;
// Trial plan D4/D6: the /admin and /me/name routes over real rows (W2).
#[cfg(test)]
mod org_admin_routes_campaign;
#[cfg(test)]
mod private_entries_campaign;
#[cfg(test)]
pub(crate) mod qualification;
#[cfg(test)]
pub(crate) mod qualification_cases;
#[cfg(test)]
mod query_qualification;
#[cfg(test)]
mod race_qualification;
#[cfg(test)]
mod record_components_campaign;
#[cfg(test)]
mod record_query_campaign;
// APP-192 (D9): the share routes with roles and the organization-wide share.
#[cfg(test)]
mod share_routes_campaign;
#[cfg(test)]
mod shared_campaign;
#[cfg(test)]
mod sharing_qualification;
#[cfg(test)]
mod slack_campaign;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod workflows_campaign;
