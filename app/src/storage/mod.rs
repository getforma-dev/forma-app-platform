//! Tenant storage primitives, the APP-073 admission operations, the APP-010
//! definition, publication and saved-work operations, the APP-009 session
//! records and the APP-013 record commands. `sessions` constructs
//! `VerifiedNonmember` from a verified provider exchange and is the only
//! constructor of a `TrustedRequest`, from the session row, the member's
//! current epoch and the registry identity. A caller cannot construct a
//! trusted request from headers or verifier strings.
mod admission;
mod authorization;
mod catalog;
mod commands;
mod definitions;
mod pool;
mod queries;
mod registry;
mod sessions;

pub use admission::{
    code_digest, handle_digest, Admission, PendingAdmission, PendingReason, Reservation, SignOut,
    VerifiedNonmember,
};
pub use authorization::{Action, Publication, Record, TrustedRequest};
pub use commands::{
    idempotency_key_shaped, CommandOutcome, CommandRequest, CommandResult, ConflictReason,
    RecordCommand, RecordDetail, RecordField, RecoveryView, COMMAND_COMMITTED,
    COMMAND_HANDLER_DEADLINE, MAX_COMMAND_VALUES, RETRY_HORIZON_SECONDS,
};
pub use definitions::{
    CurrentRevision, DefinitionDetail, DefinitionRevision, InputContract, PublicationDetail,
    PublishResult, RecordSchema, SavedWork, SavedWorkDetail, WorkBinding, MAX_CANONICAL_BYTES,
};
pub use pool::{Control, Data, Settings, Stores, Worker, Workspace};
pub use queries::{
    parse_parameters, query_digests, OpenableWork, Parameters, QueryRun, RunSource, WorkOutline,
    WorkView, QUERY_HANDLER_DEADLINE,
};
pub use registry::RegistryIdentity;
pub use sessions::{
    capability_organization, capability_shaped, AdmittedMember, Completion, Ended, FlowKind,
    FlowRecord, FlowStart, Lookup, PreAccess, PreAccessStart, SignedOut, PRELOGIN_SECONDS,
    PRE_ACCESS_ABSOLUTE_SECONDS, PRE_ACCESS_IDLE_SECONDS, SESSION_ABSOLUTE_SECONDS,
    SESSION_IDLE_SECONDS,
};

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Configuration,
    Denied,
    Unavailable,
    Deadline,
    UnknownOutcome,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "storage configuration refused",
            Self::Denied => "resource unavailable",
            Self::Unavailable => "storage unavailable",
            Self::Deadline => "storage deadline exceeded",
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
pub(crate) mod admission_qualification;
#[cfg(test)]
pub(crate) mod command_qualification;
#[cfg(test)]
mod definition_qualification;
#[cfg(test)]
mod managed_qualification;
#[cfg(test)]
pub(crate) mod qualification;
#[cfg(test)]
pub(crate) mod qualification_cases;
#[cfg(test)]
mod query_qualification;
#[cfg(test)]
mod race_qualification;
#[cfg(test)]
mod tests;
