//! Foundation boundaries only. Application authorization and transactions have
//! their own tickets; possessing a role context does not authorize a request.
pub mod admission;
pub mod auth_qualification;
pub mod config;
pub mod imports;
pub mod readiness;
pub mod roles;
pub mod storage;
pub mod supervise;
