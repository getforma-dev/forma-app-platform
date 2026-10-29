# Independent managed-runtime negative review

No blocking finding in the helper or prepared registration request. [The JSON receipt](independent-review.json) records source hashes, exact payload comparison and independent checks.

Reviewed helper **`26cc81d7eb0dfc3eb5ec75e5b25c6522d34b0e9c0e79cdd6423bfbbc057199df`**, tests **`fb54e1115c068d11ab34ab57aca4f8b84ab81594282f861bca7b5a054d5cfb31`**. The pinned runtime hash matches deployed source 8f8c34b; all shared runtime functions used by the local fixture are unchanged from that source.

The runner creates one private isolated directory, refreshes one baseline, and changes only its active copy for missing-file, wrong-region, expired-file and unreachable-database cases. It restores the original document after every case and verifies real helper positives for four database roles, two storage prefixes and two KMS keys. The pinned helper uses read-only SQL, object listing and in-memory KMS operations; it writes no shared store data or grants. The unreachable case reserves a non-listening loopback port before presenting any credentials.

Time limits are explicit: 110 seconds of work plus 10 seconds for cleanup, bounded startup/HTTP/readiness waits and process-group termination. Output contains only fixed classifications and assertions. Native diagnostics, credentials, endpoints and file contents are not exported.

**14 boundary tests passed independently.** A separate focused check against the exact local 8f image confirmed that `RUST_LOG=off` yields the expected first listener-marker line; no source repair was needed. Its native process, private directory and network-disabled container were removed. Author Linux fixture evidence also passed 14 tests and confirmed reserved-port refusal on the target image.

The prepared request hash is **`d19a98edc8c5b846fc7a55d06858847b2425e806bc8546c4f3eee1ed98d74732`**. It matches the existing task except trial family, entry point/full reviewed source, explicit APP-007 opt-in and removal of the service health check. Image, roles, network mode, volume, sizing, UID and read-only setting are unchanged; no secret entries exist.

Native 503 proves configuration invalidation; restored native 200 may reuse a valid baseline cache, while fresh direct probes establish managed restoration. The process cleanup flag confirms the reaped leader after group termination, not a descendant census. Local checks are not managed acceptance. Root alone registers/runs the task, supervises its 120-second envelope, records STOPPED/exit0 and cleanup, and deregisters the exact temporary definition. No cloud calls, credential reads, implementation edits or staging occurred in this review.
