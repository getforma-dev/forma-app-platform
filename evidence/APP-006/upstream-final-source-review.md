# Independent final auth source-delta review

**Result: source compatibility accepted; APP-006 remains Blocked.** No consumer source, original evidence pin, acceptance box or external gate was changed. No runtime test, deployment, hosted acceptance or KMS operation ran for this review.

Reviewer: `/root/auth_final_handoff_review`. Observation: `2026-10-28T21:20:44.869573+00:00`. The [machine-readable review](upstream-final-source-review.json) retains exact command arguments, working directories, exit codes, immutable-object hashes, the full reviewed patch and GitHub observations.

## Source identity

[Auth PR #17](https://github.com/getforma-dev/gatewasm/pull/17) is merged at **`737c73f88f2b89e8712e4cd1f5dbf40163ab6081`**, with all **eight checks successful** at final CI head **`c469efedde35bb16fff72ef11ac2ddb5fa8358e5`**. GitHub reports merge time `2026-10-28T21:17:44Z`.

The merge commit object is **not available in the inspected local auth object database**: `git cat-file -e 737c73f88f2b89e8712e4cd1f5dbf40163ab6081^{commit}` returned 128. The review instead read that exact immutable commit through GitHub's Git commit API. Its tree is **`fa0860eeb39613e05f1eb5b53299b072bd583b24`**, which exactly equals `git rev-parse c469efedde35bb16fff72ef11ac2ddb5fa8358e5^{tree}` from the locally available final head. This establishes complete final-tree equivalence without claiming a local merge-object inspection or fetching into the other team's checkout.

## Immutable consumer binding

The existing [source contract](../../spikes/APP-006/source-contract.json) was left unchanged. All **21 original pinned blobs** were independently read with `git show COMMIT:PATH`; every byte count and SHA-256 matched the manifest. All **nine** files in its access-only profile at `dfe7896826c4e1e3a616246fe6022d311cb38519` also remain byte-identical at the final CI head, and therefore in the observed merged tree:

- `crates/server/src/jwt/signing.rs`
- `crates/server/src/jwt/keys.rs`
- `crates/server/src/jwt/kms.rs`
- `crates/server/src/handlers/jwks.rs`
- `crates/server/src/handlers/mod.rs`
- `crates/server/src/types.rs`
- `crates/server/tests/integration.rs`
- `docs/handoff/FORMA-248.md`
- `handoff/forma-248/README.md`

Every individual expected/observed hash and byte count is retained in the JSON review. No JWT/JWK shape, issuer/audience/tenant claim, access-key purpose, endpoint, cache header or consumer profile changed in this delta.

## Complete subsequent change

`git diff --name-status dfe7896826c4e1e3a616246fe6022d311cb38519 c469efedde35bb16fff72ef11ac2ddb5fa8358e5` lists exactly two modified files:

| File | Reviewed change |
|---|---|
| `crates/server/src/main.rs` | Invalid or absent required KMS configuration and conflicting process-readable signing keys now call an explicit refusal helper that logs a reason and exits with code 1, replacing panic/expect. The refusal conditions and successful key-selection path are unchanged. |
| `.github/workflows/ci.yml` | The pinned Rust toolchain setup installs Clippy; the warning threshold is unchanged. |

The locally qualified image remains bound to source **`e828404462a44f806b0b22fe6f4c04552cc1370b`**. Its complete delta to final CI head is only `.github/workflows/ci.yml`; this review does not relabel that image as a different source commit or establish a registry digest.

## Disposition and limits

No APP-006 wire/claims consumer change or rerun of unchanged consumer tests is required for this source-only delta. Preserve the original qualification campaign and pins, and retain this final-merge equivalence receipt alongside them.

Hosted image/revision identity, registered client/callback and real PKCE exchange, unknown app principal/current-membership denial, managed key operation/rotation and hosted rollback evidence remain required. This review clears none of those gates and does not complete [APP-006](../../Tickets/9-blocked/APP-006.md) or Phase 1. The [existing consumer review](review.md) remains the independent implementation review; this document only reviews its upstream source delta.
