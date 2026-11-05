# APP-078 local diagnosis — 2026-11-05

**Scope:** local Docker only. No AWS, no RDS, no registry push. This is step 1 of the re-scoped [APP-078](../../../Tickets/2-in-progress/APP-078.md): diagnose the managed qualification refusal of window `88d1b19d` (`case_refused`, `checks: []`, twice).

## Diagnostics on current main

- `app086/qualification-diagnostics` (`ba21ced`) was cherry-picked with `-x` onto `main` `7741e5a` as `a17c7d4`. The original branch is unchanged as the record.
- **Conflicts:**
  - `app/src/storage/pool.rs`: `main` expects schema 20 where the diagnostics expected 7. The version is kept and the trace hooks are used.
  - `app/qualification/test_tools.py`: an additive conflict; both sides' tests are kept.
- **Semantic fix** (`d3680d3`): `storage::Error` gained `ProjectionLimit` and `Busy` after the diagnostics were written, so the entry did not compile and the harness's closed error set would have read such a refusal as malformed. Both lists now carry the variants. A harness test compares the closed set with the entry's match.
- **Checks on `d3680d3`:** `just check admin clippy test py-qualification` all passed (test 100 s).
- **Images:** `just images d3680d3` built the service, qualification and operator images locally for ARM64. Both image checks passed and the parser manifests are equal ([build record](image-build-d3680d3.json)). Nothing was pushed.

## The CPU hypothesis

APP-086's lead hypothesis was that the unoptimized test-profile executable on 1 vCPU (the Fargate `QualificationTask` is `cpu: 1024`) misses the 5-second readiness `ACQUIRE` deadline. The local harness had run the entry with `--cpus=2`. It now takes `--cpus`, which defaults to the Fargate size (1), and records `entry_cpus`. A harness test pins that default to the CDK value.

The full harness ran against schema 20 with migrations, seed, TLS, four roles, positive and supervised runs, and the six negatives. The image was `genui-qualification:pitch-d3680d3`.

| `--cpus` | Result | Wall time | Healthy pools' full readiness (wrong-password run) |
| --- | --- | --- | --- |
| 2 | passed, cleanup complete | 18 s | 572 ms ([report](local-harness-cpus-2.json)) |
| 1 | passed, cleanup complete | 17 s | 441–442 ms ([report](local-harness-cpus-1.json)) |
| 0.5 | passed, cleanup complete | 18 s | 622 ms ([report](local-harness-cpus-0.5.json)) |
| 0.25 | passed, cleanup complete | 24 s | 1,005–1,006 ms ([report](local-harness-cpus-0.25.json)) |

Full readiness covers connect, identity, two 266 KB catalog comparisons and the schema version, for the pools that are not failed on purpose. It stays at least 5× under the deadline even at a quarter CPU. Apple Silicon cores are faster than Graviton vCPUs, but a quarter core is still well below one Fargate vCPU. **CPU starvation does not explain the refusal.**

## The diagnostics name real refusals

The negative runs at `--cpus=1` (same report) show the closed refusal for three failures:

| Failure | What the refusal names |
| --- | --- |
| Untrusted CA | every pool at `acquire`/`connect`, `cause: tls_unknown_issuer` |
| Wrong password for one role | `pool: control`, `stage: acquire`, `step: connect`, `error: Unavailable`, `sqlstate: 28P01`, `cause: db_error`; the other three pools `done` |
| CA without the hostname | every pool, `cause: tls_name_mismatch` |

Every run reports `secrets_absent_from_output: true`.

## What remains, and what the RDS run must answer

The local harness differs from the managed run in exactly two labeled substitutions:

1. Credential refresh writes synthetic local passwords instead of STS/IAM tokens.
2. The reviewed-CA pin is the synthetic CA's hash instead of the RDS CA.

Those two, together with RDS network latency, are what local runs cannot test. With these diagnostics, the next managed run names the failing pool, stage, step, storage error, cause and SQLSTATE instead of `checks: []`.

What Victor must approve for the qualification-only run on RDS (ticket acceptance items 2–3); none of this has been done:

- Push the reviewed qualification image built from the merged candidate to ECR `genui-qualification`. The 2026-10-30 diagnostic image `app078b-ba21ced-arm64` predates schema 20 and is not reusable.
- Reserve task hours for one qualification-only run under the canonical deployment state. APP-086 recorded that a second always-on window was refused for the month's budget, and that no qualification-only window kind exists in the reviewed procedure yet.
- Run the `QualificationTask` once, observe it STOPPED, confirm cleanup, and record the result line.
- No stack, service, image or schema change.
