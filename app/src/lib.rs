//! Foundation boundaries only. Application authorization and transactions have
//! their own tickets; possessing a role context does not authorize a request.
pub mod config;
pub mod readiness;
pub mod roles;
