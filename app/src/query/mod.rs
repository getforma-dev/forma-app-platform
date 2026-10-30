//! APP-012 bounded typed query with receipt (plan Task 9; ADR-007 D7.10,
//! D7.9). Authorization lives in `storage::Data::run_query` and
//! `storage::Data::release_result`; this module executes an authorized
//! [`crate::storage::QueryRun`] in the confined engine child ([`worker`]) and
//! builds its receipt ([`receipt`]). Routes and the work page are the second
//! half of the ticket.
pub mod receipt;
pub mod worker;

pub use receipt::QueryReceipt;
pub use worker::{AdmissionStats, Engine, EngineSettings, Failure, QueryError, WorkerResult};

#[cfg(test)]
pub(crate) mod tests;
