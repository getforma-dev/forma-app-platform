//! Import boundary. The parser runs only as a separate `parser-host`
//! executable, one process per job, built from `parser/` with its own
//! lockfile and never linked into this binary (ADR-009 D9.1).
pub mod sandbox;
