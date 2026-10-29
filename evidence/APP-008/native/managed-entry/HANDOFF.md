# APP-008 slice2 handoff — draft, not accepted

**Stop point:** user requested wind down and transfer to Claude. Only the initial isolated-entry source and two narrow review repairs were completed. No behavioral, container, managed or release qualification has run for this entry. Do not mark it accepted or managed-ready. Root supplied starting HEAD `20a480b`; no Git operation was performed by this author.

## Files and current state

- New [managed_qualification.rs](../../../../app/src/storage/managed_qualification.rs): draft test-only entry, closed manifest loader, file checks, source/resource binding, deadline/watchdog, shared-case execution and bounded report.
- [storage/mod.rs](../../../../app/src/storage/mod.rs): only adds `#[cfg(test)] mod managed_qualification;`.
- Existing [qualification_cases.rs](../../../../app/src/storage/qualification_cases.rs), production storage/config, runtime, schemas and dependency files were not changed for this slice.
- [handoff-state.json](handoff-state.json) records final file hashes, focused command results and resource ownership. Logs are alongside it. The independent reviewer owns `independent-handoff-review.*`; preserve its incomplete disposition and findings.

Focused formatting and `cargo check --offline --locked --lib --tests` passed after the final two fixes. An earlier draft also passed strict Clippy, but that was before the source-binding/deadline repairs and is not final lint acceptance. **Zero entry behavior tests, zero new container tests and zero managed calls were run.** Historical slice1/R5 campaigns are unchanged and do not qualify this new source.

## Agreed interface implemented in draft

The ignored test name is `storage::managed_qualification::managed_readonly_entry`. It requires exact harness arguments, UID65532, `GENUI_APP008_MANAGED_QUALIFICATION=APP-008`, and owned0600 `/run/genui/app008-managed-run.json` under0700. All input files reject links/unsafe modes; no endpoint, role, SQL, CA-path or transport override is exposed.

The closed manifest binds schema/scope/arming, canonical v4 run UUID, original `issued_at`/`expires_at` spanning at most600 seconds, exact approved source RDS hostname/5432/`genui`, declared resource ID, exact two fixture organizations, the canonical synthetic registry SHA, fixed-path CA SHA, fixture-setup SHA, embedded catalog/migration hashes and an11-file compiled storage/entry source-bundle SHA. `source_bundle_sha256()` defines the exact ordered framing and file set; it includes the entry itself. It is not a whole-image or Git-revision attestation. The registry bytes must exactly equal `registry_bytes(run_id)` with the fixed synthetic issuer and two bindings.

Paths are fixed: credentials `/run/genui/credentials.json`, registry `/run/genui/app008-managed-registry.json`, CA `/etc/ssl/certs/rds-global-bundle.pem`. The manifest's CA hash proves file self-consistency only. Root explicitly approved this to permit an owned synthetic-CA/local-host-alias test. Any actual managed immutable image/launch must independently pin the reviewed RDS CA and prohibit mounts/overrides. Similarly, the RDS resource ID remains a declared control-plane binding; SQL cannot attest it.

The draft uses actual `Settings::new` and `Stores` with a private test-only `TrustedRequest` factory. Shared cases are application-data-read-only, not SQL READ ONLY: they include rollback-only denied DML and session-reset trials. A sanitized global panic hook precedes assertions; task panic payloads are discarded. The original absolute deadline reserves four seconds for cancellation/cleanup, with abort+await and `Stores.close`; a watchdog targets the original expiry. The final bounded `APP008_MANAGED_RESULT` JSON reports failure rather than raw exceptions. These behaviors are source-reviewed only, not exercised.

## Remaining work, in order

1. Finish independent review of this draft, especially panic/cancellation/abort-ack cleanup, watchdog/output blocking, immutable-input checks and the exact selector/arming boundary. The two initially identified issues were repaired: checked compiled source binding and a pre-parse monotonic sample that does not renew the original cutoff. No final approval was issued.
2. Add focused closed-manifest, pin, original-deadline, private-file/link/mode and sanitized-failure tests. Prove `Stores.close` and transport release on success, actual shared-case panic and timeout; a refusal before store construction is insufficient evidence.
3. Prepare the requested **local-only** positive execution: cached Linux test executable, owned PostgreSQL16.13 container on an internal Docker network, the exact accepted RDS hostname as an internal alias, and an owned CA cert for that hostname at the fixed CA path. Use actual migrations/validated synthetic seeds and distinct local role credentials. Do not use `Settings::fixture`, public DNS, shared RDS, insecure TLS or new production hooks. No such harness/image/network was created yet.
4. Re-run focused format/lint/compile/private-boundary checks and actual entry process tests, bind exact source/output hashes, then obtain independent review. Production Docker context/package/source/image binding remains root-owned and pending. Do not implement operator packaging, races, deployment or cloud execution as part of this slice.

## Reproduction and cleanup ownership

From the repository root, the completed safe checks are:

```sh
CARGO_BUILD_JOBS=2 cargo +1.93.1 fmt --manifest-path app/Cargo.toml -- --check
CARGO_BUILD_JOBS=2 cargo +1.93.1 check --offline --locked --manifest-path app/Cargo.toml --lib --tests
```

The eventual isolated executable invocation, **not run here**, is `<test-executable> --exact storage::managed_qualification::managed_readonly_entry --ignored --nocapture --test-threads=1`. It belongs only in the future reviewed local fixture or separately authorized managed launch. There is no implemented supervisor/refresh or manifest-generation utility for it yet.

No live process, container, Docker network, socket or private credential file is owned by this slice. The final compile process exited. Only cached image metadata was inspected; no image/container build, network creation, provider/cloud call, spending, staging, commit, ticket or schema action occurred. Root owns any separate auth/cloud resources; this handoff makes no statement that those are stopped.
