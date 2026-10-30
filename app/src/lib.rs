//! Foundation boundaries, admission, sessions and storage. Possessing a role
//! context does not authorize a request: protected routes resolve a live
//! workspace session into a storage-constructed `TrustedRequest`.
pub mod admission;
pub mod auth;
// ADR-007 D7.13: compiled only with the `qualification-harness` feature (the
// isolated qualification executable); release builds never contain it.
pub mod auth_qualification;
// APP-013: the declared record command route, payload digest and record page.
pub mod commands;
pub mod composer;
pub mod config;
pub mod http;
pub mod imports;
pub mod query;
pub mod readiness;
pub mod roles;
pub mod storage;
pub mod supervise;
