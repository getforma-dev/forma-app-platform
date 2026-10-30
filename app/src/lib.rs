//! Foundation boundaries, admission, sessions and storage. Possessing a role
//! context does not authorize a request: protected routes resolve a live
//! workspace session into a storage-constructed `TrustedRequest`.
// ADR-011 scenarios: the action path (utterance -> typed draft -> confirm).
pub mod actions;
pub mod admission;
// APP-108 (ADR-011 pitch): folders (boxes): files, real data, apps built from them.
pub mod auth;
pub mod boxes;
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
// APP-099 (ADR-011 pitch): Forma as an MCP server; member-minted agent tokens.
pub mod mcp;
// APP-100 (DS-59): the channel a record version was committed through.
pub mod origins;
pub mod query;
pub mod readiness;
pub mod roles;
// APP-095 (ADR-011 pitch): share a work with a teammate; the members list.
pub mod sharing;
// APP-100 (ADR-011 pitch): Slack slash command, buttons and approver DMs.
pub mod slack;
pub mod storage;
pub mod supervise;
// APP-103 (ADR-011 pitch): canvas utility widgets (weather, customize in words).
pub mod widgets;
