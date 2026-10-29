# APP-008 native storage author evidence

The original 29-check evidence and review below are preserved. Later follow-ups cover [R3 replication mode](r3/README.md), [R4 admission ordering](r4/README.md) and [R5 temporary-schema ordering](r5/README.md). Final source acceptance requires the latest follow-up's independent supplement.

This slice implements four fixed native PostgreSQL role pools, private transaction authorization, and database readiness. It does not implement a production identity/session adapter, admission, entitlements, application writes, or provider provisioning. All real identities remain unavailable until APP-006/009/073/074 establish their own contracts. The initial ordinary routes remain denied.

## Integration contract

Construct `Settings::new(credentials_path, registry_path, packaged_ca_path, exact_rds_hostname)`, then `Stores::new(settings)`. The paths and hostname are deployment configuration, never request input. The production constructor accepts the fixed us-east-1 RDS hostname suffix and port 5432; the canonical credential file must name that same host and database `genui`. `GENUI_RDS_CA_FILE` can supply the already packaged CA path. Root owns `GENUI_IDENTITY_REGISTRY_FILE` delivery and the main/readiness wiring.

`Stores::readiness().await` checks all four native identities concurrently within one aggregate five-second deadline. It checks current credentials, TLS, role flags/memberships, schema version, the full embedded logical catalog, and blank session context; successful leases are reset before return. It returns only a fixed `storage::Error`, never PostgreSQL details. This database result must be combined with the existing managed S3/KMS helper result for the same credential generation. The local composed-readiness test uses a clearly synthetic helper; it does not prove S3/KMS delivery.

The separate registry requires schema_version 1, a canonical UUID registry_version, and 0–32 exact issuer/provider_tenant_id/organization_id bindings. It is an owned 0600 regular file in an owned 0700 directory, has no links, rejects unknown fields and duplicate provider pairs, and is bounded 16 KiB. Its version/SHA-256 are retained for the process lifetime. An empty binding list supports deny-all startup and database readiness; it cannot authorize even a synthetic request. Replacement requires process restart and future session reconciliation. It never extends either existing credential JSON or callback JSON.

The module exposes no production `TrustedRequest` constructor, deserializer, fixture switch or header. A compile-fail public-boundary check proves callers cannot initialize its private fields. Future APP-009 must construct requests only after actual verification, private mapping and workspace binding/principal confirmation. Control/data operations rely on that future private adapter; a publicly constructible verifier result is insufficient.

## Implemented bounds and authorization

- Process-wide limits are two connecting/live transports per role and eight total. Each role admits at most eight acquisitions including active leases; excess requests refuse immediately. The same five-second acquisition deadline covers queueing, connection/TLS, catalog/identity checks and at most one changed-generation retry. Each new connection obtains its role material from the current canonical file.
- A lease is dirty by default. Cancellation or uncertain cleanup permanently detaches its Deadpool object. The owned driver drops its transport before releasing capacity; requesting abort alone cannot release capacity. A focused lifecycle test checks that order, and real PostgreSQL tests check changed backends after cancellation.
- Transactions are READ COMMITTED/READ WRITE for lock-capable business reads, bounded three seconds and ending before file expiry minus five seconds. Trusted org/principal/epoch are bound into transaction-local settings before a nonfinal current precheck. Only then does the operation take the organization/member fence, recheck permissions and, for record reads, lock the exact saved-work publication/version. No membership lock is taken at checkout; APP-013 retains its future outcome-before-final-fence slot.
- The closed ten-action vocabulary uses current member/epoch and resource relationships. Record visibility additionally requires app run and every source dependency's current visibility/tombstone status. Current typed operations return bounded publication or record metadata. Worker gains no job authorization. No business DML, outcome, audit or delivery mechanism is claimed.
- Reuse requires awaited COMMIT or ROLLBACK, standalone DISCARD ALL, baseline reinstallation, and exact user/role, row_security, search_path and blank-scope checks. Cleanup has a separate two-second bound. Ambiguous commit returns UnknownOutcome and is never automatically retried. Same-backend tests cover role/settings poisoning, listeners, advisory locks and explicit rollback after a SQL error.
- New connections and every recycle use the migration owner's embedded `catalog-query.json` and `catalog-v2.json`. The single read-only query runs in a private READ ONLY transaction with local pg_catalog search path. Its nullable text is bounded 262,144 bytes, parsed as JSON and compared to the expected JSON value. No runtime SQL override exists. Disabled RLS, extra UPDATE and renamed/missing relation tests all refuse before reuse.

## Reproduction and receipts

`dependency-verification.json` records exact released versions, declared MSRVs, non-yanked registry entries and matching downloaded archive checksums. `tokio-postgres-rustls 0.14.0` declares no MSRV; compatibility is established by the actual locked build rather than inferred from its edition. The consumer rust-version remains 1.85. `author-checks.json` records source hashes, commands, toolchains, logs and before/after equality. The floor qualification is on the recorded local target, not every supported Rust target.

From `app/`, with the existing locked runtime and contract-validator Python environments installed:

```sh
CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib
CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --doc
CARGO_BUILD_JOBS=2 GENUI_NATIVE_REPORT=/absolute/separate/output.json cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture
```

`local-postgres-tls.json` records 29 actual local checks. The harness uses unique Docker names, a random loopback-only port, the pinned PostgreSQL 16.13 image, a new private CA, SCRAM role passwords, the canonical migration runner, and revalidated canonical fixture seeds. It proves both revocation orderings, current role material on a new connection, bounded waiter/capacity behavior, cleanup/connection cancellation, strict TLS refusals, registry denials, catalog drift and composed readiness. The positive helper in the composed cases emits only the synthetic file's generation/hash/expiry/check time. No AWS, hosted callback, managed secret, real mailbox, import or business-write claim follows.

The isolated campaign reserves a 16 MiB test-thread stack for its many paused futures; production request tasks are unchanged. During development a read-only fixture reset and the campaign's initial default-stack limit were corrected. The one container/private directory left by that aborted test process was explicitly removed. The final campaign confirms removal of its own database and private files. Abrupt process termination still requires operator cleanup; no destructor can promise cleanup after SIGKILL.

[Independent review](independent-review.md) reports no open actionable source findings and independently repeats all 29 local PostgreSQL/TLS checks, the 18 library tests and public-construction doctest. Its final before/after hashes are unchanged against the finalized catalog. Root's combined image, CI and managed qualification remain separate gates. No files were staged or committed by this author, and no cloud or registry action was performed.
