# Independent review: canonical verifier diagnostics and private hosted-key campaign

Reviewer: `/root/app011_fixtures`, independently reviewing root-authored APP-006 implementation. Review date: 2026-10-29 UTC. Source is the uncommitted candidate identified below. No implementation edits, Cargo builds, hosted requests, credentials, callback, membership, session, cloud or registry operations were performed by this reviewer.

**Disposition: no open actionable source findings in the reviewed candidate.** One process-deadline finding was raised, repaired by the implementation owner, and verified independently with the actual process boundary. This review approves the narrow local source candidate; it does not pass APP-006/FORMA-248 hosted acceptance or establish an executed positive hosted campaign.

## Finding and disposition

**R1, P2 — overall process deadline omitted synchronous hash/output work. Resolved.** The first reviewed example (`135b78e17cb58f2f6fff3c87f6492c65f26e620678301c486117a5c90d6ba5c2`) wrapped only the asynchronous campaign with a Tokio timeout. `executable_digest()` and `stdout.write_all()/flush()` could remain blocked outside that timeout, retaining the process after its claimed bound. Returning from the two-second input read did not fix a later blocked output write.

The owner added a fixed process watchdog at `main` entry, armed before input/hash work. It has no argument, environment or private-envelope clock/deadline override. It exits 124 at the 90-second monotonic deadline; failure to arm exits 125. The Tokio timeout remains for graceful fixed-code reporting, and `runtime.shutdown_timeout(Duration::ZERO)` prevents a canceled resolver's blocking task from holding runtime destruction. Missing/partial JSON or either watchdog exit is failure, never success.

The boundary harness now fills a real stdout pipe before launch and leaves it undrained. During the independent run, process sampling showed the main thread blocked in the stdout write and the separate watchdog sleeping. The child then exited **124 after 90.010 seconds**; the pipe retained only the harness fill bytes, with no partial report or stderr. This directly covers the reported failure path. Timers remain ordinary OS-scheduled deadlines, not a real-time scheduling guarantee.

## Reviewed behavior

- **Authentication/cache policy preserved.** The library diff adds counters and a read-only snapshot method. Attempt accounting is incremented under the same mutex, after the existing cooldown decision and before I/O. Accepted-keyset accounting occurs only after a complete validated replacement. Saturating diagnostic counters do not govern authorization, freshness, retirement or refresh admission. Existing checked generation advancement and identity-release fences are unchanged. Observing diagnostics cannot fetch, reset the attempt time, refresh expiry or expose a token, key ID, issuer URL or identity. Its mutex wait is bounded by the existing verification deadline.
- **Concurrency and cancellation.** Snapshots take the same mutex as refresh state, so they cannot observe a partially installed set. Canceling a reserved refresh preserves the attempt and empty authority state. The added tests check successful unknown-key refresh, immediate/repeated throttling, failed and canceled attempts, cache preservation, and a blocked diagnostic read. Cancellation does not add a detached refresh that can later install keys. Existing verifier concurrency tests remain in the owner's full 26-test run.
- **Private input.** The executable rejects arguments, terminal/regular-file input, more than 16 KiB, missing EOF beyond two seconds, malformed/duplicate/unknown fields, foreign endpoints, expired and nonsynthetic envelopes. Only a Unix FIFO descriptor is accepted. The exact staging access-JWKS URL, issuer, audience and 900-second lifetime contract cannot be redirected or weakened by the envelope. No public listener or token-bearing endpoint is added.
- **Campaign sequence.** One production `Verifier::new` instance and the real clock serve all five steps. The original token must verify for the expected tenant and subject with adequate remaining lifetime. An unknown-kid control preserves signed claims/signature, must throttle immediately, waits at least 61 real monotonic seconds, then must produce one accepted keyset refresh followed by `UnknownKey`, another cooldown refusal and successful verification of the original identity. A random collision with an already published key fails the trial instead of proving an unknown-key fetch. No retries, cache clearing, clock overrides or process replacement can manufacture a pass.
- **Evidence and redaction.** Output is limited to fixed cases/codes/counters, permitted run metadata, source/executable hashes and durations. Neither token, token hash, claims, expected provider IDs, keys nor HTTP response bodies are emitted. Input source pins are explicitly declarations, and the disk executable hash is explicitly not process attestation. Counters distinguish reserved attempts from accepted keysets and do not pretend to be provider/proxy access-log counts.
- **Build/API compatibility.** Public diagnostics are additive; `Verifier::new`, `verify`, refusal variants and identity semantics are unchanged. Cargo manifest/lock bytes are unchanged. New standard-library APIs predate the declared Rust 1.85 floor; the reviewer did not separately compile with Rust 1.85. The example intentionally uses Unix fd/FIFO APIs and is scoped to the current macOS/Linux execution targets. Owner qualification used Rust 1.93.1. No Windows example or new framework release claim is made.
- **Runner and image source.** The normal qualification runner now tests all targets, builds the example, and runs its actual process-boundary harness. Its build concurrency accepts only one or two jobs. The new Dockerfile uses existing pinned base digests, release/locked compilation, one build job, a nonroot final user, and a one-shot entrypoint. The six-file allowlist contains only Docker/Cargo files, the canonical verifier and the example. The actual context-export receipt matches these reviewed bytes. Runtime confinement flags and HTTPS egress remain operator execution responsibilities; Docker source review is not an image/deployment acceptance result.

## Checks and limits

Independently executed from repository root:

```sh
python3 spikes/APP-006/qualify_hosted_key_boundary.py --executable spikes/APP-006/target/debug/examples/hosted_key_qualification
```

Result: **13/13 passed**, no hosted request or successful token. The withheld-EOF input failed after 2.033 seconds; the full-output-pipe trial exited 124 after 90.010 seconds. The other input cases failed with their expected fixed codes and no private-marker disclosure. Executable SHA-256: `d1ead9ccb87413b8f12cb9d2bfd64df0e1536f3c97d93bdb35db002369819661`.

A separate empty-pipe invocation returned the expected exit 1. Its compiled verifier/campaign hashes matched the current reviewed files, binding the independently exercised binary to this candidate. The owner’s [local qualification](local-1/qualification.json) records successful format, strict Clippy, all-target tests, example build and the same full process-boundary campaign. The reviewer inspected that report and [context export](context.json); these owner checks are distinct from the independently executed boundary run.

The actual 61-second **positive hosted** sequence was not executed. Valid hosted issuer/subject, actual deployment/image/architecture binding, mailbox/callback/session/membership/entitlement, outage recovery, KMS custody, rotation, fleet retirement, Linux image execution and source CI/release status must retain their separate evidence. No source review or local mock/negative result substitutes for them. Root owns integration, CI, ticket status and any authorized hosted coordination.

## Exact reviewed SHA-256 values

| Path | SHA-256 |
| --- | --- |
| `spikes/APP-006/src/lib.rs` | `4fbfa5be174bde8f1a5410f66e4b5d53abae03d8860f7b02921b1eb99e68e39e` |
| `spikes/APP-006/src/tests.rs` | `d30a302586f4b5097345bdcc702f7787fa36ed4b4838084350b7039f336c553d` |
| `spikes/APP-006/examples/hosted_key_qualification.rs` | `4875cd8a222437eeddac1f26605ffc3f8f635a1af874adb241460e2d2e6d0095` |
| `spikes/APP-006/qualify_hosted_key_boundary.py` | `0004a26d30a58ddf9093cf788d5a2f02f9ebf2b250f4c98ecb9edba9de9c8fa0` |
| `spikes/APP-006/qualify.py` | `b9b96e7806c18931269de7f2485ea2d70ffc0dfa3f6ffd44f4f57c97b268f1d3` |
| `spikes/APP-006/Dockerfile` | `eff94a1d093d95215ebe292a110207f0bbd5205176739ef87943b44ccfc133de` |
| `spikes/APP-006/.dockerignore` | `3a8eebc3fcd544add374ba2c5a0ce1c4f4f8dd44d93e3b2c6a74868f1412e1e7` |
| `spikes/APP-006/Cargo.toml` | `9ca03539a0f419a2481d6d03e2a2e9eebc78c7e5b03b6c6e68800bf7c923d30b` |
| `spikes/APP-006/Cargo.lock` | `caec3dcdcb3228c1e98b7ef177517408485ecc3ac5af5b48927d81e87e050b8d` |
| `evidence/APP-006/hosted-key-qualification/README.md` | `d195e1d9554cd5429e8067fb5713d26a581b1a0be2e51d70e5ae87e3de6e3159` |

## Follow-up: image helper, CI wiring and documentation

Reviewed the added `qualify_hosted_key_image.py`, four-line workflow addition, appended spike README section and campaign README spacing corrections. The canonical verifier, diagnostic tests, example, private-pipe harness, runner, Cargo manifests and six Docker-context inputs retain the exact hashes recorded above. The campaign README hash below supersedes its earlier documentation hash.

**Disposition: no new actionable findings.** The helper resolves the supplied image once and uses its immutable image ID for every container. It verifies the configured nonroot user, exact one-shot entrypoint and absence of exposed ports. Each fresh named container has no network, a read-only root, all capabilities dropped, no new privileges, no mounts, and bounded process/memory/CPU settings. Inspected settings, actual exit state, fixed refusal codes, no stderr/private-marker output and compiled source hashes are checked before success. Containers created successfully are forcibly removed in `finally`; a failed test or cleanup cannot become a passed report. Existing evidence output paths are refused. This helper contains no valid hosted envelope or network-enabled path.

CI builds the image from the existing allowlisted `spikes/APP-006` context after canonical qualification, then invokes the helper against that build. The existing read-only repository permissions, pinned actions, Rust toolchain file, job timeout and always-upload evidence step remain intact. The workflow does not publish an image, supply credentials or launch a hosted campaign. The documentation additions accurately distinguish this preparation from hosted/callback acceptance.

The reviewer independently ran the three image cases against **`sha256:5ee5ad165bca1e37bc2efe13f2fb727912c008cbe721b18f452a98482404cedc`**, with a fresh temporary output file and no build:

```sh
python3 spikes/APP-006/qualify_hosted_key_image.py --image sha256:5ee5ad165bca1e37bc2efe13f2fb727912c008cbe721b18f452a98482404cedc --output <fresh-temporary-result.json>
```

**3/3 passed on Linux ARM64/aarch64:** empty input (`input_size`, 0.170 s), malformed synthetic marker (`input_schema`, 0.158 s), oversized input (`input_size`, 0.136 s). All three containers were removed. No host directory, secret or network access was supplied. The reviewer also compiled the Python helper and verified every source hash in the owner's durable [image-local-1.json](image-local-1.json) against the current files. That receipt's SHA-256 is `5ada06f4fe8b7e0e84c30051fcc87ef15a843bd270509d8a5dcdb401fce6c14f`.

This follow-up establishes the three Linux image boundary negatives only. It does not repeat the 90-second watchdog test inside the image, run the positive hosted sequence, verify another CPU architecture, pass remote CI or publish a registry artifact. Root is separately rerunning full source-bound qualification and Rust 1.85 checks; those results are not assumed by this review.

| Follow-up path | SHA-256 |
| --- | --- |
| `spikes/APP-006/qualify_hosted_key_image.py` | `3f427c76e321b7a9a87ea7050dbb8580486590582981be046b793907a62f57ea` |
| `.github/workflows/auth-contract.yml` | `8297741c84aa9618544274b9ab35ee50a262c04bc7dacde8c9c081d1e9958e40` |
| `spikes/APP-006/README.md` | `7f35130bd8ce3ef4a34f0184ef628933613bb0581825b9f836248f719ef27d6a` |
| `evidence/APP-006/hosted-key-qualification/README.md` | `9d4936518000b4f2ff6757a0de82e3607fec7a2e7b26687ac0819ce300308856` |
