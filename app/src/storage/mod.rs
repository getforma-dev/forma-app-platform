//! Tenant storage primitives, the APP-073 admission operations and the APP-010
//! definition, publication and saved-work operations. No session
//! adapter is implemented here; APP-009 constructs `VerifiedNonmember` inside
//! this module from a verified provider exchange.
//! A caller cannot construct a trusted request from headers or verifier strings.
mod admission;
mod authorization;
mod catalog;
mod definitions;
mod pool;
mod queries;
mod registry;

pub use admission::{
    code_digest, handle_digest, Admission, PendingAdmission, PendingReason, Reservation, SignOut,
    VerifiedNonmember,
};
pub use authorization::{Action, Publication, Record, TrustedRequest};
pub use definitions::{
    CurrentRevision, DefinitionDetail, DefinitionRevision, InputContract, PublicationDetail,
    PublishResult, RecordSchema, SavedWork, SavedWorkDetail, WorkBinding, MAX_CANONICAL_BYTES,
};
pub use pool::{Control, Data, Settings, Stores, Worker, Workspace};
pub use queries::{
    parse_parameters, query_digests, Parameters, QueryRun, RunSource, QUERY_HANDLER_DEADLINE,
};
pub use registry::RegistryIdentity;

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

#[cfg(test)]
mod admission_qualification;
#[cfg(test)]
mod definition_qualification;
#[cfg(test)]
mod managed_qualification;
#[cfg(test)]
mod qualification;
#[cfg(test)]
mod qualification_cases;
#[cfg(test)]
mod query_qualification;
#[cfg(test)]
mod tests;
