//! Whole-output admission of typed app definitions (APP-010; design section
//! 6.2, C02, C04, C13; ADR-002).
//!
//! [`canonical`] is the restricted forma-json-v1 encoding,
//! [`digest`](mod@digest) the domain-separated SHA-256 identities and
//! [`validate`] the closed `app_definition` contract of edition
//! impl-2026-10-28.1, reproducing `contracts/schema.py` and
//! `contracts/validate.py`. Nothing here stores, renders or authorizes: an
//! admitted digest is content identity, never a grant, and organization scope
//! always comes from the trusted request.
use std::fmt;

pub mod canonical;
pub mod digest;
pub mod validate;

pub use canonical::{decode, encode};
pub use digest::digest;
pub use validate::{admit_definition, AdmittedDefinition};

/// Why a document was refused. Each variant has a stable [`code`](Self::code).
/// Text payloads name a location such as `$.content.views[0].kind`, plus a
/// fixed rule for [`Encoding`](Self::Encoding); they never echo submitted values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// A key outside the contract's closed field set.
    UnknownField(String),
    /// Script, markup, SQL, URL or path syntax where the contract declares no
    /// free text, or a key that names such content.
    ExecutableContent(String),
    /// More views than [`validate::MAX_VIEWS`].
    TooManyViews(usize),
    /// More operations in one view's query than [`validate::MAX_OPERATIONS`].
    TooManyOperations(usize),
    /// Bytes beyond the 1 MiB document or 256 KiB definition bound.
    TooLarge(usize),
    /// A renderer kind or renderer version outside the registry.
    UnregisteredRenderer(String),
    /// An operator, measure, comparison or lookup rule, or an operator version,
    /// outside the registry.
    UnregisteredOperator(String),
    /// A client-supplied grant, permission, membership, role, principal or
    /// tenant claim.
    ClientGrant(String),
    /// Not a forma-json-v1 encoding of the contract: syntax, a refused scalar,
    /// depth or key, a declared field of the wrong type, format or range, a
    /// broken reference, or a digest that does not match the content.
    Encoding(String),
}

impl AdmissionError {
    /// Stable snake_case reason code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownField(_) => "unknown_field",
            Self::ExecutableContent(_) => "executable_content",
            Self::TooManyViews(_) => "too_many_views",
            Self::TooManyOperations(_) => "too_many_operations",
            Self::TooLarge(_) => "too_large",
            Self::UnregisteredRenderer(_) => "unregistered_renderer",
            Self::UnregisteredOperator(_) => "unregistered_operator",
            Self::ClientGrant(_) => "client_grant",
            Self::Encoding(_) => "encoding",
        }
    }

    /// Lower is more severe. A document breaking several rules reports its most
    /// severe class, so an authority claim or executable content is never
    /// masked by an incidental shape error elsewhere in the same output.
    fn severity(&self) -> u8 {
        match self {
            Self::ClientGrant(_) => 0,
            Self::ExecutableContent(_) => 1,
            Self::UnknownField(_) => 2,
            Self::UnregisteredRenderer(_) => 3,
            Self::UnregisteredOperator(_) => 4,
            Self::TooManyViews(_) => 5,
            Self::TooManyOperations(_) => 6,
            Self::TooLarge(_) => 7,
            Self::Encoding(_) => 8,
        }
    }
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code();
        match self {
            Self::TooManyViews(count) => {
                write!(f, "{code}: {count} views, at most {}", validate::MAX_VIEWS)
            }
            Self::TooManyOperations(count) => write!(
                f,
                "{code}: {count} operations in one view, at most {}",
                validate::MAX_OPERATIONS
            ),
            Self::TooLarge(bytes) => write!(f, "{code}: {bytes} bytes"),
            Self::UnknownField(detail)
            | Self::ExecutableContent(detail)
            | Self::UnregisteredRenderer(detail)
            | Self::UnregisteredOperator(detail)
            | Self::ClientGrant(detail)
            | Self::Encoding(detail) => write!(f, "{code}: {detail}"),
        }
    }
}

impl std::error::Error for AdmissionError {}

#[cfg(test)]
mod tests;
