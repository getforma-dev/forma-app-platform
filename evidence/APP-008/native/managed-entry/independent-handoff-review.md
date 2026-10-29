# APP-008 managed entry: incomplete independent handoff review

**INCOMPLETE — stopped at the user's request to wind down. No independent acceptance, managed proof or launch signal.** The [receipt](independent-handoff-review.json) records exact draft/dependency hashes and remaining work.

Draft source: [managed_qualification.rs](../../../../app/src/storage/managed_qualification.rs), SHA256 `57478ae92f0a7fdea62365c6fae4be650a1d63e44545edd249687d43df572dd7`. Its only declaration change is the `cfg(test)` module in [storage/mod.rs](../../../../app/src/storage/mod.rs), SHA256 `5c2c43fc192571fba6b3b16a3889f0be482f115b35adb7376ae46cb896ed1ac8`. The shared case source remains `80e3fbbd31a16e5cb022882bd56107f6d01327d98026df4cf134445e3f3cf9d8`.

The initial boundary review found the intended fixed target, private closed manifest, original at-most-ten-minute interval, synthetic two-org registry and private test identity factory. The draft calls production `Settings::new`, executes the existing shared cases, suppresses raw panic payloads and attempts abort/await followed by actual `Stores.close`. These observations are partial source inspection, not tested guarantees.

Two identified draft concerns received narrow source repairs before handoff: a checked eleven-file compiled source-bundle hash now accompanies the fixture/catalog/migration pins, and the monotonic deadline sample is captured before parsing/hashing. Neither repair was compiled or tested by this reviewer.

Resume with these checks:

- Finish the full frozen-source review and inspect any author tests. Verify closed/duplicate-key parsing, exact selector/arming/UID, fixed target/org/pins, private file ownership/modes/links/FIFO/size/encoding, and original deadline refusals.
- Prove actual pool/transport closure on success, case panic and timeout; verify cancellation is awaited, watchdog behavior is bounded, and failures cannot emit a successful cleanup receipt. Test output/panic privacy with synthetic sentinel values.
- Run a confined local positive with the accepted hostname and production `Settings::new` TLS path. A synthetic CA is local qualification only; no insecure TLS or fixture transport may select the managed entry.
- Review the separate root-owned package/supervisor, exact binary/source/image binding, fixed file paths, credential refresh, original deadline and cleanup. It must receive no operator password. The fixed-path CA hash checks bytes; the real package separately pins the reviewed RDS bundle without mounts/overrides. Declared ResourceId is not SQL attestation.

No build, test, container, cloud/provider call, Git operation or source/ticket edit was made by this reviewer. **This lane has no active local resources and needs no cleanup.** Author resource status was requested separately; do not infer it from this statement.

The later complete managed matrix remains required, including real IAM/TLS, per-role tenant/RLS/FK checks, same-connection reset, races, drift/refusal, HTTP and recovery. The historical36 local checks and these six shared groups do not pass managed acceptance.
