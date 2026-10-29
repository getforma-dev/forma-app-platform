# Independent review: contract edition impl-2026-10-29.2

**Reviewer:** a separate general-purpose agent, given the code but not the implementer's conclusions. It made no file changes and no git writes.

**Scope:** contract drift against ADR-009 D9.6 and ADR-002, retention of the prior edition's vectors, parity between Rust, Python and JavaScript, and validation holes.

**Diff reviewed:** the uncommitted change against `623fcd5`, before the fixes below.

**What the reviewer verified:**
- The retained files are byte-identical to `git show 623fcd5:…` (identities 45639ea3, 4a4581f5, 33b46ff5 and 089e9a30).
- Every edition-one vector reproduces in Python, JavaScript and Rust.
- All 57 edition-one negative vectors keep their verdicts in Python, and the 19 that touch a definition keep them in Rust.
- Edition-one definition digests and input-contract digests still admit.
- `make_fixtures.py` reproduces the edition-two fixtures byte for byte.
- Rust and Python agree on: coverage, the requirement that records be complete, `window_minutes` (including a window across a leap day), unregistered versions, `exact_rational` and the streamed row digest.

## Findings and dispositions

| # | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| 1 | major | `lock.py check` failed because README.md was edited after `lock.py write`. | **Fixed.** The lock was rewritten after the final README edit, and `lock.py check` passes (`green.txt`). |
| 2 | major | Bundles that mix editions passed every edition-two check: an edition-two definition with an edition-one publication or receipt, including a receipt claiming `partial: false` over partial coverage. | **Fixed.** `validate_bundle` now requires an edition-two definition to come with an edition-two publication and receipt. An edition-one definition may still be published in either edition. Tests in `test_receipt_operator_and_renderer_identity_lists` cover both cases. |
| 3 | major | The `job_state_v2` rules refused states D9.4 can reach: a claimed job reads `queued` until its start commits, has no position, and may already carry a cancel request. "1 = next to be claimed" was over-decided. | **Fixed.** Queue fields travel together and only on a queued job; they may be absent. A queued job may carry `cancel_requested`. `queue_position` is now defined as D9.4's organization position. Tests are updated. |
| 4 | minor | JavaScript checked the 38-digit bound after reduction, while Python and Rust check it before, and `BigInt` accepted `' 12'` and `'0x10'`. | **Fixed.** `verify_vectors.mjs` now requires canonical decimal strings of at most 38 digits before reduction, with refusal cases added. |
| 5 | minor | Pinning per-operator and per-renderer versions to the registry edition makes the lists derivable. `ratio` gained `window_minutes` without a version change. | **Kept and documented** in the contracts README. Every registered operator and renderer is at version 1 in this edition. `window_minutes` is admitted only under definition schema version 2, so no version-1 definition changes meaning. The lists carry information once a later edition raises a single version. |
| 6 | minor | The problem body invents `limit_exceeded` and makes the 429 rule global. Floats bypassed forma-json-v1. Queue names are shared between the job and query queues. | **Partly fixed.** `validate_problem` now runs `canonical()` first and requires integer members. The README scopes the 429 rule to this edition's bodies, not the ADR-006 rate limits, and says the route tells which queue is meant. The code `limit_exceeded` is kept as "decided here". |
| 7 | minor | The normalized-rows framing is an array, not lines. Rows are not validated. There is no unknown-value variant for out-of-int64 text. | **Kept.** The array framing is recorded as "decided here" so that the identity is an ordinary value digest. Row grammar and unknown values are left to APP-016/018 and listed in the evidence README. |
| 8 | minor | The evidence README was missing. | **Fixed.** `README.md` in this directory, including the erratum. |
| 9 | nit | The retained vector "unknown version refused" now fails for a different reason. A comment overstated what an unregistered version reports. | **Accepted and documented.** The comment is corrected. |
| 10 | nit | The edition date will roll over at 00:00 UTC. | Committed on 2026-10-29 UTC. |
| 11 | nit | The old lock file's identity is recorded but not retained. ADR-009 is now locked. `failed` is accepted with `failure_code: "cancelled"`. | **Accepted.** The identity is recorded for traceability. Locking ADR-009 is deliberate, since it controls the edition. `failure_code` vocabulary is left open (see the README). |

## Found after the review

The implementer found one more problem while checking consumers: `app/runtime/tenant_seed.py` seeds PH01 tenants from `contracts/fixtures/{northstar,spend-review}.json`. After the edition change, it would have stored edition-two content under schema version 1.

**Fixed.** The seed now reads the retained edition-one fixtures. Its three bundles validate at schema version 1, with their original definition digests (`green.txt`). The Docker-backed native campaign that uses the seed was not run.
