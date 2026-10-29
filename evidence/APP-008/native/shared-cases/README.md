# Shared native qualification cases — slice1

Only the first slice of the [managed harness preparation](../../managed-harness-preparation.md) is implemented. Six existing result groups now live in [qualification_cases.rs](../../../../app/src/storage/qualification_cases.rs): fixed-role readiness; two-organization publication/record reads; all-role RLS/blank-scope/write refusal; revoked/epoch/grant/registry negatives; the closed action matrix; and same-backend commit/error-rollback cleanup. Fixed fixture IDs and the existing PID/result helpers moved with them.

The [local campaign](../../../../app/src/storage/qualification.rs) still owns Docker, local credentials/TLS, setup/seeding, operator mutations, lifecycle/race cases and cleanup. It supplies its existing private request factory to the extracted functions. SQL statements, assertions, result names and ordering are preserved. The shared functions accept an existing `Stores` and the test-owned request factory; they do not choose an endpoint or transport, run an operator command, read an environment variable, or expose an arbitrary SQL argument.

The sole declaration change in [storage/mod.rs](../../../../app/src/storage/mod.rs) is a sibling module guarded by `cfg(test)`. The shared functions and constants remain private to the storage test implementation. There is no new production constructor, exported raw client, runtime flag, dependency, authenticated route or test hook. The module has read-only application operations, expected write-denial attempts that roll back, and session-poison/reset checks; it creates no persistent fixture state.

[Author checks](author-checks.json) retain source hashes before/after verification, exact commands and separate logs. With `CARGO_BUILD_JOBS=2`, Cargo offline and the existing cached PostgreSQL16.13 image:

- Strict Clippy, formatting and production library compilation passed.
- Four storage unit tests and the compile-fail doctest for private `TrustedRequest` construction passed.
- The existing complete native PostgreSQL/TLS campaign passed **36 checks** in38.86 seconds, including actual owned-container/private-file cleanup. [The new receipt](local-postgres-tls.json) has exactly the same ordered names and passing results as the historical R5 receipt.
- All recorded production/runtime/schema/resource/dependency files remained byte-identical except the `cfg(test)` module declaration; only the original qualification source and new shared test source otherwise changed. Inputs stayed unchanged during the checks.

This is local source preparation, not RDS/IAM or managed acceptance. No managed entry, separate image, supervisor, race coordinator, operator package or cloud action was implemented. Historical R5 evidence remains intact. The new test file and changed module declaration affect image build inputs even though production behavior is unchanged; root must separately review the exact Docker allowlist and source/context/image binding before calling any image current.
