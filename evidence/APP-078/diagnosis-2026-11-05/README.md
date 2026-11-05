# APP-078 local diagnosis — 2026-11-05

**Scope:** local Docker only. No AWS, no RDS, no registry push. This is step 1 of the re-scoped [APP-078](../../../Tickets/2-in-progress/APP-078.md): diagnose the managed qualification refusal of window `88d1b19d` (`case_refused`, `checks: []`, twice).

## Diagnostics on current main

- `app086/qualification-diagnostics` (`ba21ced`) was cherry-picked with `-x` onto `main` `7741e5a` as `a17c7d4`. The original branch is unchanged as the record.
- **Conflicts:**
  - `app/src/storage/pool.rs`: `main` expects schema 20 where the diagnostics expected 7. The version is kept and the trace hooks are used.
  - `app/qualification/test_tools.py`: an additive conflict; both sides' tests are kept.
- **Semantic fix** (`d3680d3`): `storage::Error` gained `ProjectionLimit` and `Busy` after the diagnostics were written. Every closed error list now carries them: the entry's mapping, the harness's set, and (`cd17f93`) the Rust test's list. A harness test compares the harness set with the entry.
- **Bisect note:** `a17c7d4` alone does not compile its test target; `d3680d3` fixes that. Squash-merging this branch keeps `main` bisectable.
- **Checks:** `just check fmt clippy test py-qualification` passes at `cd17f93`.
- **Images:** `just images cd17f93` built the service, qualification and operator images locally for ARM64. Both image checks passed and the parser manifests are equal ([build record](image-build-cd17f93.json)). Nothing was pushed.

## The CPU hypothesis

APP-086's lead hypothesis was that the unoptimized test-profile executable on 1 vCPU (the Fargate `QualificationTask` is `cpu: 1024`) misses the 5-second readiness `ACQUIRE` deadline. The local harness had run the entry with `--cpus=2`. It now takes `--cpus`, which must be a positive number, defaults to the Fargate size (1), and is recorded as `entry_cpus`. A harness test pins that default to the CDK value.

The full harness ran against schema 20 with migrations, seed, TLS, four roles, positive and supervised runs, and the six negatives. The image was `genui-qualification:pitch-cd17f93`.

| `--cpus` | Result | Wall time | Healthy pools' full readiness (wrong-password run) |
| --- | --- | --- | --- |
| 2 | passed, cleanup complete | 18 s | 372 ms ([report](local-harness-cpus-2.json)) |
| 1 | passed, cleanup complete | 16 s | 380–466 ms ([report](local-harness-cpus-1.json)) |
| 0.5 | passed, cleanup complete | 18 s | 554–555 ms ([report](local-harness-cpus-0.5.json)) |
| 0.25 | passed, cleanup complete | 25 s | 1,199 ms ([report](local-harness-cpus-0.25.json)) |

Full readiness covers connect, identity, two comparisons of the schema-20 catalog (`catalog-v20.json`, about 466 KB) and the schema version, for the pools that are not failed on purpose. It stays about 4× under the deadline even at a quarter CPU, and about 10× under at 1 CPU. Apple Silicon cores are faster than Graviton vCPUs, but a quarter core is still well below one Fargate vCPU. **CPU starvation does not explain the refusal.**

## The diagnostics name real refusals

The negative runs at `--cpus=1` (same report) show the closed refusal for three failures:

| Failure | What the refusal names |
| --- | --- |
| Untrusted CA | every pool at `acquire`/`connect`, `cause: tls_unknown_issuer` |
| Wrong password for one role | `pool: control`, `stage: acquire`, `step: connect`, `error: Unavailable`, `sqlstate: 28P01`, `cause: db_error`; the other three pools `done` |
| CA without the hostname | every pool, `cause: tls_name_mismatch` |

Every run reports `secrets_absent_from_output: true`. That includes the supervised run, which exercises credential refresh; the harness now scans it too and requires the field on every run.

## What remains, and what the RDS run must answer

The deployed service passes the same store readiness on RDS ([window record](../window/README.md), step 13). That narrows the question to the qualification task's own inputs, which local runs replace:

1. The supervisor's IAM-token credential refresh (the local supervised run writes synthetic passwords instead).
2. The reviewed-CA pin (the synthetic CA's hash locally).
3. The task role.

The environment also differs: local PostgreSQL against the RDS engine, and the Fargate network against a local Docker network.

With these diagnostics, a refusal in the managed readiness stage names the failing pool, stage, step, storage error, cause and SQLSTATE. A failure in a later case, including the second, aggregate readiness case, still reports only its `case` and `reason: panic`.

What Victor must approve for the qualification-only run on RDS (ticket acceptance items 2–3); none of this has been done:

1. **Approve the beta promotion of the merged candidate** once this branch is merged. That promotion publishes all three images, including this qualification image, and sets `QualificationImageDigest`, so the `QualificationTaskDefinition` uses it (`ops/promotion/runner.py`). It also reserves `extra_task_hours=2` for the window. The 2026-10-30 diagnostic image `app078b-ba21ced-arm64` predates schema 20 and is not reusable.
2. **Review a qualification-only RunTask procedure before the run,** as the ticket's §Verification requires. No such procedure exists under the canonical deployment state yet. It should say whether the promotion's 2 extra task-hours cover the run or a separate reservation is needed.
3. **Run the `QualificationTask` once,** observe it STOPPED, confirm cleanup, and record the result line. The run itself makes no stack, service, image or schema change. The image change is the promotion in item 1.

For history: APP-086 recorded that the legacy preflight refused a second always-on window for the month's budget. That refusal came from the pre-APP-167 local recipes, not from the current canonical state.
