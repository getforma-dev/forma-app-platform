# APP-006 local consumer evidence

**Blocked acceptance; locally qualified candidate.** No deployed auth service, environment client, browser login, app session, app membership or live rollback was exercised. The reusable verifier and its limits are documented in [the spike](../../spikes/APP-006/README.md). All source fixtures are synthetic or public-only; ephemeral private keys and signed test tokens exist only in test memory.

## Current reviewed source

Implementation `49389c6` is on `main`. [Independent review](review.md) found no blocking local-consumer issue, and all eight [source CI checks](ci.json) passed. The [Linux run](ci-49389c6/qualification.json) passed the same 23 tests and matched all 13 input hashes; it explicitly reports no upstream checkout available there. Local producer and reviewer checks independently verified the 21 pinned upstream blobs. These results leave all hosted/member/session acceptance blocked.

## Recorded local run

[local/qualification.json](local/qualification.json) identifies the exact input SHA-256 values, registry dependency versions/checksums/license metadata, base app HEAD, platform and toolchain. The run used macOS ARM64, Rust 1.93.1 and two Cargo jobs. It rechecked two public fixture hashes and twenty-one immutable upstream Git blobs against the independently available GateWasm repository. An app HEAD is context only: candidate file hashes identify these uncommitted implementation inputs.

| Check | Actual result |
| --- | --- |
| `cargo fmt --all --check` | Passed; [output](local/01-fmt.log) |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed; [output](local/02-clippy.log) |
| `cargo test --locked -- --test-threads=2 --nocapture` | **23 passed**, zero failed; [output](local/03-test.log) |
| Slow HTTP body / held cache mutex | Refused at observed 2004 ms / 2003 ms; no identity delivered. These are local observations, not service latency guarantees. |
| Concurrent unknown keys | Twenty-four callers caused one authoritative HTTP attempt, one unknown-key refusal and twenty-three cooldown refusals; valid known-key recovery then succeeded. |
| Unknown-key outage/cancellation | Attempt budget retained; prior authority cleared; no stale-grace sign-in; an allowed successful refresh restored verification. |
| Source-bound 248 access-only profile | Ten unique access keys admitted; oversized/duplicate/corrupt sets refused atomically; old/new/next overlap and cross-purpose signature refusals passed. |
| Key freshness at release | Expired key lease refused on the real verify path; an acquired lease refused after actual authoritative replacement. |

The two-second deadline includes final cache-lock wait and uses a final monotonic identity-release fence. The cache-generation/freshness fence was added after independent review preflight correctly identified that retaining raw key bytes alone could outlive current authority. The fixed implementation rechecks token expiry, deadline and key authority after the final awaited lock, while holding that cache guard through the release decision.

[development-compile-failure](development-compile-failure/qualification.json) retains a failed development lint/compile attempt while adding those regressions: two test-only `MutexGuard` arguments needed explicit dereferencing. No tests ran in that failed campaign. The final run above supersedes it; failure output is retained rather than described as a passing run.

## Reproduce

From the repository root:

```sh
python3 spikes/APP-006/qualify.py --output evidence/APP-006/local \
  --upstream-checkout /Users/victorvillacis/dev/Forma/work/FORMA-247/gatewasm
```

Omit the upstream argument on hosts without that independent repository. The result then explicitly records zero upstream blobs rechecked; it does not turn retained source hashes into proof of upstream availability. Output files are replaced on rerun; retain a previous failed campaign under a distinct evidence directory before rerunning. The workflow `.github/workflows/auth-contract.yml` runs the same local mock checks on Ubuntu and uploads exact logs/identities. **No Linux CI success is claimed by this local record.**

## What remains blocked

- FormaStack supplies FORMA-247's deployed revision/image/health and approved callback/client/tenant contract, with real PKCE/state, single-use code, backend token exchange, tenant isolation and rollback evidence. The former GateWasm AWS environment was deleted; local fixtures do not replace it.
- GenUI verifies observed issuer/audience/lifetime/JWKS against that receipt, then qualifies the real consumer path and unknown-app-principal denial. The crate returns only a cryptographically verified external identity; it does not know invitations, app roles or membership epochs.
- Historical FORMA-248 candidate `22652f8e` lacked a public access-vs-SSF selector. The owner supplied `dfe7896826c4e1e3a616246fe6022d311cb38519` with access-only `/auth/jwks` and separate `/ssf/jwks`; exact source hashes and the earlier ambiguity are retained. The consumer now locally qualifies an explicit bounded access-only profile, including prepublished next keys, overlap, retirement after observed replacement and genuine SSF-signed `typ=JWT` refusal. This does not prove upstream CI, KMS custody, deployed endpoint separation or live rotation. A fresh cached key can remain usable until expiry or authoritative replacement; no immediate remote-retirement claim is made.
- Existing-session continuity, cookie/CSRF policy, membership rechecks, key-alert/audit integration, fleet-wide admission and deployed outage/rollback remain their owning tickets' work. No provider downgrade or fake organization picker is introduced.

[APP-006](../../Tickets/9-blocked/APP-006.md) stays Blocked with all acceptance boxes open. This preparation does not pass the [Phase 1 gate](../../Tickets/phases/PH01.md) or start dependent Phase 2 work.
