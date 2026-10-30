//! Foundation boundaries, admission, sessions and storage. Possessing a role
//! context does not authorize a request: protected routes resolve a live
//! workspace session into a storage-constructed `TrustedRequest`.
// ADR-011 scenarios: the action path (utterance -> typed draft -> confirm).
pub mod actions;
pub mod admission;
pub mod auth;
// ADR-007 D7.13: compiled only with the `qualification-harness` feature (the
// isolated qualification executable); release builds never contain it.
pub mod auth_qualification;
// APP-013: the declared record command route, payload digest and record page.
pub mod commands;
pub mod composer;
pub mod config;
// ADR-011 scenarios: demo switches scoped to synthetic organizations; display names.
pub mod demo;
pub mod directory;
// APP-095 (ADR-011 pitch): the in-process event hub and the SSE route.
pub mod events;
pub mod http;
pub mod imports;
pub mod query;
pub mod readiness;
pub mod roles;
// APP-095 (ADR-011 pitch): share a work with a teammate; the members list.
pub mod sharing;
pub mod storage;
pub mod supervise;
