# APP-008 native storage and readiness: independent review

Reviewer: `/root/app008_native_review`, 2026-10-29 UTC. The implementation owner is `/root/auth_readiness_answer`; root owns the main/readiness integration. This review made no implementation, ticket, Git, cloud or registry changes.

**Disposition: no open actionable findings in the source candidate identified in [independent-review.json](independent-review.json).** The independently executed final local PostgreSQL/TLS campaign passed all 29 checks with unchanged before/after source hashes. This is source/local qualification approval, not APP-008 completion, a managed RDS result or Phase 1 acceptance.

## Reviewed boundary

- **Authority construction and fixed operations.** `TrustedRequest` has no public constructor, deserializer, clone or production fixture path. Its fields are confined to the storage module; the negative public-construction doctest fails as intended. The public workspace/control/data methods expose fixed typed operations and bounded metadata results, not SQL, a client, row streams, transactions or cancellation handles. The worker has no business operation. Unknown action strings deny, and resource/action combinations are closed.
- **Registry and current authorization.** The separate registry enforces a bounded closed document, one organization per exact provider pair, canonical organization/version IDs, owned private regular-file/directory checks, no hard links or symlink components, and a retained version/digest. Empty bindings allow only readiness and refuse every synthetic resource request. Transactions check the registry identity and selected binding before acquisition, install tenant/principal/epoch only inside the transaction, read current organization/member state, and acquire the organization/member fence before rechecking permissions. Work operations require both app-run and work permission plus source visibility, and record reads lock the saved-work publication/version. The permit borrows its transaction and cannot escape it.
- **Identity integration limit.** Exact scoped provider-subject/principal confirmation currently exists in the Workspace publication path. Control/data rely on the future private trusted adapter, which does not exist in production yet. The schema plan expressly assigns provider binding/principal lookup to Workspace; this review does not treat an arbitrary verifier struct or the qualification's fixture constructor as that adapter. APP-009 must establish this transition before ordinary access. Real session, invitation and entitlement ownership remains APP-006/009/073/074.
- **Pool and transport ownership.** Four fixed roles share process-wide limits of two connecting/live transports each and eight total. Per-role admission bounds the queue. One absolute five-second acquisition deadline includes queueing, connect/TLS, role/catalog checks and the sole changed-generation retry. Driver permits outlive the transport until its destruction; an abort request alone does not free capacity. I also inspected the pinned Deadpool 0.13.1 `UnreadyObject` drop path: cancellation during creation/recycle detaches and destroys the unready connection, rather than returning it to the reusable pool.
- **Cancellation and reset.** Leases are non-reusable by default, and uncertain lease destruction takes the object out of Deadpool. Confirmed transaction termination is followed by standalone `DISCARD ALL`, baseline installation and explicit role/RLS/search-path/blank-scope validation. Only that successful path returns the object. Three-second transaction, credential-expiry margin and two-second cleanup bounds remain separate; uncertain commit does not trigger a business retry. Real backend IDs demonstrate successful reuse and replacement after uncertain/canceled operations.
- **TLS, role material and catalog.** New connections use the current fixed-role credential, exact deployment hostname/database and custom packaged trust roots, with PostgreSQL TLS required and hostname checking retained. Actual user/role flags, memberships, TLS and schema version are verified. New connections and recycle also compare the migration owner's embedded bounded read-only catalog query against the embedded expected JSON; no runtime SQL override is accepted. Read-only catalog transactions use `pg_catalog` search path and are rolled back before baseline reuse. The final reviewed query includes transitive role reachability and internal foreign-key trigger state.
- **Readiness integration.** Main treats absent registry configuration as an inert public shell with readiness unavailable, and an explicitly invalid registry as startup failure. When configured, readiness requires both native four-role success and the existing managed helper result bound to the same current credential generation. The test-only one-shot seams are absent from production builds. The positive local helper is explicitly synthetic; it supplies no S3/KMS acceptance evidence.

Source-deletion, publication-policy and work-adoption writers are not granted in this slice. Their future implementations must add the premise fences and delivery checks required by ADR-004; the current read tests do not authorize later writes or prove those future races. No session/admission/command/audit/delivery implementation is claimed here.

## Verification gap resolved during review

The first new missing-native-store unit check used an absent credential file. That case would also refuse under the old helper-only implementation, so it did not distinguish the new native gate. I requested composed checks using valid local credentials and real native Stores.

The owner/root added and I independently executed the meaningful combinations: native success plus helper failure remains unready; native failure plus helper success remains unready; both succeed; changing the credential generation invalidates the cached success until a fresh joint check. The root also strengthened its separately owned HTTP smoke to use a positive helper/marker for the missing-registry case. That binary/HTTP smoke remains root's separate validation; it is not counted among my executed commands below. No source bypass was demonstrated by the original weak test.

## Independently executed checks

From `app/`, with `CARGO_BUILD_JOBS=1`:

```sh
cargo +1.93.1 test --locked --lib
cargo +1.93.1 test --locked --doc
GENUI_NATIVE_REPORT=/temporary/private/review-result.json cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture
```

- 18 library tests passed; the separately invoked Docker qualification was the one ignored test.
- The public `TrustedRequest` construction compile-fail doctest passed.
- The final isolated PostgreSQL 16.13/TLS campaign passed **29 checks** in 29.28 seconds of test execution. Its exact command, output and full runtime receipt are embedded in `independent-review.json`; the wrapper took 31.548 seconds including the required rebuild.
- Coverage includes two-organization reads, all four roles' RLS/blank-scope/write refusal, current permission/epoch/binding denials, same-backend reset, cancellation during connect/recycle/BEGIN/finish/reset and a real query, uncertain commit, cleanup deadline, credential generation/expiry, source visibility/tombstones, both membership-revocation orders, eight-connection capacity and bounded waiters, shared retry deadline, empty-registry denial, catalog/role/schema drift, changed current password on a new connection, and wrong-host/CA/plaintext refusals. The campaign verifies removal of its own local database and private files.

The first independent run passed the same 29 cases, but its before/after guard detected `catalog-query.json` changing while the schema owner finalized it. That run and its hashes/output are retained as `prior_run_before_catalog_freeze`; it is not the final binding. I then rebuilt and reran the affected real campaign against the final resources, with **all recorded source hashes unchanged**. An initial inventory typo referring to the earlier proposed `schema_catalog.py` path stopped before any test execution and is also disclosed in the JSON; the actual comparison lives in `runtime.py`.

Final embedded catalog pins:

- Query: `028d7a84db3e7bbade26fd12c6fcc363fae4e93fe04777d011d07287085892cf`.
- Version 2 expected catalog: `82c01e831db712093f4ef86f98630f5d6e2a63936c723b351ea367bfa9069148`.

The JSON binds all inspected native sources, Cargo files, main/readiness/config, embedded resources and the runtime/seed inputs actually used. I inspected the owner's exact dependency/checksum evidence and Rust 1.85 floor result; I did not independently repeat the floor build. The local run uses macOS ARM64 and synthetic PostgreSQL SCRAM credentials with a throwaway CA, not RDS IAM or the packaged RDS CA against a managed endpoint.

## Remaining delivery gates

Root must retain final binary/HTTP smoke, exact image inputs/build, current source CI, the separate repaired schema review, and managed RDS isolation/reuse/race/recovery results. The runtime launcher's private empty-registry materialization is separately owned/reviewed alongside Python source. Hosted auth, real provider mapping, ordinary sessions/admission, application writes and complete Phase 1 behavior remain open. These limits do not invalidate this narrow source approval, and this review changes no ticket status.
