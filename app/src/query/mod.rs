//! APP-012 bounded typed query with receipt (plan Task 9; ADR-007 D7.10,
//! D7.9). Authorization lives in `storage::Data::run_query` and
//! `storage::Data::release_result`; this module executes an authorized
//! [`crate::storage::QueryRun`] in the confined engine child ([`worker`]),
//! builds its receipt ([`receipt`]), serves the JSON query route and the
//! work page ([`routes`]) and turns each view's outcome into what the page
//! shows ([`page`]).
pub mod canvas;
pub mod canvas_display;
pub mod page;
pub mod receipt;
pub mod routes;
pub mod worker;

pub use receipt::QueryReceipt;
pub use routes::{Executor, Work};
pub use worker::{AdmissionStats, Engine, EngineSettings, Failure, QueryError, WorkerResult};

#[cfg(test)]
pub(crate) mod tests;
