# APP-075 independent review: managed qualification entry, image and supervisor (plan B1–B3)

| Item | Value |
| --- | --- |
| Reviewer | Independent reviewer (Claude), not the implementer. Read-only apart from this file; nothing committed. |
| Date | 2026-10-29 |
| Revision reviewed | `phase-one-spikes` at `e195ebfdc605dbec9b7e5d895ecc4b2ebdf02819`. APP-075 commits `df109c3`, `faebc13`, `5023eb9`, `bde2667`, merged in `7e75d7c`, plus the pin follow-up `f44ce72`. |
| Controlling text | `Tickets/0-backlog/APP-075.md` acceptance; `docs/planning/PHASE-1-EXECUTION-PLAN.md` Track B1–B3; ADR-004 §Identity registry and qualification seam; ADR-005; ADR-008; `evidence/APP-008/managed-plan.md`; `evidence/APP-008/managed-harness-preparation.md` slice 2; the open items in `evidence/APP-008/native/managed-entry/independent-handoff-review.md` |
| Author evidence | `evidence/APP-075/README.md` and its `entry/`, `image/`, `local-positive/` and `checks/` (all recorded at `5023eb9`) |
| AWS | Not touched. No push, registry, STS, RDS or DNS. |

File SHA-256 values at the reviewed revision:

```
bcf8e1a30638ec61923ae8ed90e0ada22e241fbd9ad69551bfde5ecfe27e0838  app/src/storage/managed_qualification.rs
3e97f6f12f1728cbbee2fc47caabc4679c5eb3934b03ea0b2ca805bfc6808c47  app/src/storage/managed_qualification/tests.rs
d3edfa89056a3613cf20a97f95c45bec62927cbe5b0abe017499c0a3be5c1cf8  app/runtime/qualification_supervisor.py
1e22ef7004bcbc616710768ecb7672563e283b5db0f6ccd24228e553517a1ec1  app/runtime/test_qualification_supervisor.py
0086034301cb3a77917a47d4a3164a400e0aba40ed1c3948a9b331cb5193c04b  app/qualification/Dockerfile
246291e89c7b04ae1784059b0edfabaf2917fcb7057136425b4174cc7ba55d4a  app/qualification/Dockerfile.dockerignore
736b5e1304a3b6d20bf975217abfbf38e78103b98ec11cc6f50342f6eca57a94  app/qualification/local_harness.py   (unchanged since 5023eb9)
0d900a5f6cdb7452d55a0646c5421b5ee9d52be3c64ee7426e971106146548ac  app/qualification/check_image.py     (unchanged since 5023eb9)
a45d74bfd2504fea402b91295761798e263e498f8e8087af3547a4899581c22d  infra/src/stack.ts
3ac984a0c5c3ab3c7506ba4f966de7cc51c727897d8dea4558f116a7b5ba0762  infra/test/security.test.ts
ee307fb23c2ee0180f27510efa946fdba6269eee104ddb5e08feee56b174fd6b  app/runtime/tenant_seed.py
```

## Verdict

**Changes requested.** There are no blockers and no security defects in the entry, supervisor, image or CDK task. Three majors stop acceptance. `f44ce72` moved the entry's seed pins and allowlist, but it did not update the two local proof tools. As merged, `check_image.py` fails 2 of its 24 expectations and `local_harness.py` cannot produce a passing positive run. The recorded bindings (27 sources, 41-file context, bundle `85381e18…`, executable `0f3be387…`) describe `5023eb9`, not the merged source.

With the two tool fixes applied in memory (no repository edit), the merged source passes 24 of 24 image checks and 10 of 10 harness expectations. That shows the fix is sufficient, not just plausible.

## What the reviewer ran (merged revision, local only)

| Command | Result |
| --- | --- |
| `cd app && CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib` | 67 passed, 0 failed, 6 ignored. All 18 non-ignored `storage::managed_qualification::tests::*` pass in a parallel run. |
| `cargo +1.93.1 fmt --check` | clean |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -v` | 99 run, OK (1 skipped), including the 13 supervisor tests |
| `cd infra && npm run typecheck && npm test` (node_modules present) | typecheck clean; 19 of 19 template tests pass |
| `cargo +1.93.1 test --locked --lib storage::managed_qualification::tests::managed_cleanup_with_real_transports -- --ignored --exact --nocapture` | passed on PostgreSQL 16.13/TLS. Success with 6 checks; case panic holding a transaction and a lease; timeout behind the `begin` barrier (4,002 ms). All three end with cleanup confirmed, slots 0 and server sessions 0. |
| `docker build --target inputs --output type=local` of `app/qualification/Dockerfile` | 48 files, exactly the allowlist: 30 sources, 3 migration resources, the CA, 9 contract files, `runtime.py`, the supervisor, `requirements.lock`, the Dockerfile and its ignore file |
| `docker build -f app/qualification/Dockerfile -t genui-app075-review:local .` | built, linux/arm64. Image `sha256:0ac5958d…`. Executable `5c3e5e79b3cd7a5c84132b89a5d898c6cf9b4440a477b01af1ac153415fa6968` equals its `build-identity.json`. Source bundle `22ef30bc7d8296d70ba90934db752cd2acc5821899cac9814f53718bf7150994`. |
| `check_image.py --image … --base-revision HEAD` as merged | **22 of 24.** The context-allowlist and resource-pin expectations fail (M1, M2). Cleanup complete. |
| the same, with the M1+M2 fixes applied in memory by a scratch wrapper | 24 of 24, cleanup complete |
| `local_harness.py --image …` with the M1 fix applied in memory | 10 of 10 expectations met, cleanup complete. Direct positive run `2d144d1b-98c2-4f50-b5d1-35887079fd26` and supervised run `aea3889a-b039-4fb1-bbcc-205969f84fda`: both `passed:true`, 6 checks, cleanup confirmed. Unarmed, root UID and wrong arguments give `configuration_refused`; untrusted CA and hostname mismatch give `case_refused`. Migrations 0001–0004, schema 4. |
| teardown | The review image was removed. No `app075` image, container or network remains, and the volume count is unchanged (21). |

## Findings

### Blockers

None.

### Major

**M1. The harness recomputes the seed pins from the wrong edition, so the merged B2/B3 proofs fail.**

- **Where:** `app/qualification/local_harness.py:108-109`.
- **Problem:** `expected_pins()` still hashes `contracts/fixtures/northstar.json` and `spend-review.json`. Since edition `impl-2026-10-29.2` (`90c7d3b`), those files hash to `d9acfd8f…` and `186162da…`. The entry (`managed_qualification.rs:89-91`) correctly pins the edition-one bytes `45639ea3…` and `4a4581f5…`, which `tenant_seed.make_bundles` now reads (`tenant_seed.py:25-27`).
- **Consequence:** `local_harness.py` builds a manifest whose pins the entry refuses, so the positive run reports `configuration_refused`. `check_image.py` fails "executable's resource pins equal the repository recomputation". `f44ce72` left both tools unchanged (their hashes equal `5023eb9`).
- **Fix:** read `contracts/fixtures/impl-2026-10-28.1/northstar.json` and `contracts/fixtures/impl-2026-10-28.1/spend-review.json`. Better, derive the directory once from `tenant_seed.py`, or share one constant with the Rust test at `tests.rs:908-915` so the two cannot drift again.

**M2. `check_image.allowlist()` counts directory re-include lines as files.**

- **Where:** `app/qualification/check_image.py:89-91`.
- **Problem:** `f44ce72` added the `!app/src/imports/` and `!contracts/fixtures/impl-2026-10-28.1/` re-include idiom (the root `.dockerignore` already uses it). `allowlist()` keeps every line that starts with `!`, so the expected set contains two directories that `files_under()` can never return.
- **Consequence:** "qualification context is exactly the explicit allowlist" fails with `missing: ['app/src/imports/', 'contracts/fixtures/impl-2026-10-28.1/']`, even though the exported context is exactly right.
- **Fix:** `return sorted(line[1:] for line in lines if line.startswith("!") and not line.endswith("/"))`. Also assert that every `!dir/` line is followed by its `dir/**` exclusion, so the idiom cannot silently admit a whole directory.

**M3. The acceptance evidence is bound to `5023eb9`, not to the merged source.**

- **Where:** `evidence/APP-075/README.md:16` and `:19`, `evidence/APP-075/image/README.md:21`, `evidence/APP-075/entry/README.md:15`.
- **Problem:** these record 27 sources, a 41-file context, bundle `85381e18…`, executable `0f3be387…`, image `ff9f0fe0…` and seed paths `contracts/fixtures/northstar.json`. At the merged revision the values are 30 sources, 48 files, bundle `22ef30bc…` and executable `5c3e5e79…`. The catalog and manifest pins also changed (`336f870e…`, `cc27fbac…`), and the seed path is the `impl-2026-10-28.1/` one.
- **Why it blocks acceptance:** acceptance item 3 asks for "exact Docker-context, executable, source and image bindings". Item 2's positive run must run on the source that will be accepted. `f44ce72` defers this to "the next image build", but as merged that build fails its checks (M1, M2).
- **Fix:** after M1 and M2, rerun `check_image.py`, `local_harness.py` and the cleanup campaign at the merged revision. Replace the identity table and the image and entry notes, and keep the `5023eb9` receipts as history.

### Minor

**m1. Seed provenance is pinned by fixture bytes only.**

- **Where:** `app/src/storage/managed_qualification/tests.rs:931-932`.
- **Problem:** the guard checks the bundle tuple, not the directory `tenant_seed.py` reads. A future edition switch in the seed would not fail this test. The seeded rows also depend on `tenant_seed.py` itself (APP-073's `ENTITLEMENT_WINDOW`, member entitlements) and on `contracts/validate.py` (`digest`), and neither is pinned.
- **Fix:** also assert `CONTRACTS / "fixtures" / "impl-2026-10-28.1" / (name + ".json")` is present in `tenant_seed.py`. Either add `seed_code_sha256` pins for `app/runtime/tenant_seed.py` and `contracts/validate.py`, or state at `managed_qualification.rs:84-87` that APP-077's operator-image digest binds the seed code.

**m2. "Every source compiled" omits the files the tests embed at compile time.**

- **Where:** `managed_qualification.rs:108-110`; commit `df109c3` message.
- **Problem:** `admission/tests.rs:17-51` embeds `contracts/implementation-lock.json` and 8 fixture files, and `managed_qualification/tests.rs:20` embeds `app/certs/rds-global-bundle.pem`. These compiled inputs are in the image context but not in `source_bundle_sha256`. The managed path's behavior does not depend on them, and the image digest binds them.
- **Fix:** add them to `SOURCES` (the source test can derive them by scanning for `include_str!`/`include_bytes!`), or change the wording to "every Rust source and Cargo input".

**m3. `passed:true` does not require the six named checks.**

- **Where:** `managed_qualification.rs:584`; `qualification_supervisor.py:306-308`.
- **Problem:** a success is any `Ok(checks)` with confirmed cleanup. The count is structural today, because each case pushes a check only after its assertions and the case source is bound. Still, neither the entry nor the supervisor rejects a result with zero or five checks.
- **Fix:** have `report` require `checks.len() == 6` and the fixed name list. Have the supervisor require `len(result["checks"]) == 6` and every `passed` true.

**m4. The `entry_panic` path drops the watchdog before writing its result.**

- **Where:** `managed_qualification.rs:653-661`.
- **Problem:** unwinding drops the `Watchdog` (its `Drop` joins the thread) before `emit`. A blocked stdout on that path is then bounded only by the supervisor's kill, not by the entry's status 124 at the original expiry.
- **Fix:** record the validated deadline in a `OnceLock<Instant>` after `load`. In the `catch_unwind` fallback, call `Watchdog::start(deadline)` again before returning.

**m5. The oversized-report fallback loses the cleanup state.**

- **Where:** `managed_qualification.rs:668-672`.
- **Problem:** it always emits `cleanup_applicable:true` and `cleanup_confirmed:false`. This is conservative, but it misreports a not-applicable refusal.
- **Fix:** copy `result["cleanup_applicable"]` and `result["cleanup_confirmed"]` (booleans only) into the fallback.

**m6. A partly written run file is never removed.**

- **Where:** `qualification_supervisor.py:243-246`.
- **Problem:** if `write_new_private` fails after `O_CREAT|O_EXCL` succeeded (a write or fsync error), the path was never appended to `written`, so the file survives.
- **Fix:** append the path to `written` immediately after a successful `os.open`. `O_EXCL` proves the file is the supervisor's own.

**m7. "Run files are removed on every path" does not hold for ECS StopTask.**

- **Where:** `qualification_supervisor.py:26` and `:330-336`.
- **Problem:** the supervisor is PID 1 (exec-form `ENTRYPOINT`) and installs no SIGTERM handler, so the kernel ignores SIGTERM. StopTask ends in SIGKILL after `stopTimeout`: no `finally`, no summary, and the credential envelope is not unlinked. It disappears only with the task's ephemeral volume.
- **Fix:** before `supervise`, install a handler that raises `SystemExit(143)` (a `def` that raises, registered with `signal.signal(signal.SIGTERM, …)`), so `finally` kills the child, removes files and prints the summary. Add a unit test that sends SIGTERM to a supervised fake child.

**m8. The environment check is a denylist.**

- **Where:** `qualification_supervisor.py:93`.
- **Problem:** run-task overrides can still set `AWS_ENDPOINT_URL_STS`, `AWS_CA_BUNDLE`, `SSL_CERT_FILE`, `PYTHON*` or `LD_*`. None of these reaches the operator password, which is not in this task definition and not readable by its execution role. None weakens database TLS: the child gets only the arming variable, and rustls uses the fixed CA. They can, however, redirect or break the refresh path's trust.
- **Fix:** refuse any name outside an explicit allowlist. That list is the eight task variables, the arming variable, `GENUI_APP008_MANAGED_MANIFEST_JSON`, the ECS/Fargate-injected `AWS_CONTAINER_*`/`ECS_*`/`AWS_EXECUTION_ENV`/`AWS_DEFAULT_REGION`, and the base image's `PATH`, `HOME`, `HOSTNAME`, `LANG`, `GPG_KEY`, `PYTHON_VERSION`, `PYTHON_SHA256` and `PYTHON*` values from the Dockerfile.

**m9. The confined checks do not mirror the CDK task's filesystem.**

- **Where:** `check_image.py:36-38`; `local_harness.py:66-67`.
- **Problem:** both mount a tmpfs `/run/genui` (uid 65532, 0700) and a tmpfs `/tmp`. The task has a read-only root, no `/tmp`, and a Fargate bind-mount volume initialized from the image's `VOLUME`.
- **Fix:** add one supervisor case that uses the image's own anonymous volume and no `/tmp` mount, removed with `--volumes`. It should reach `credential_refresh_unavailable` with `check_run_directory` passing. Fargate itself remains APP-078's to observe.

**m10. The disabled-state template evaluates `Fn::Select` over an empty parameter.**

- **Where:** `infra/src/stack.ts:225`.
- **Problem:** with an empty `QualificationImageDigest`, the conditional execution-role policy contains that expression. CloudFormation should skip intrinsics of a false-condition resource, but nothing here proves it (the author notes this too).
- **Fix:** remove the doubt with `Fn.conditionIf(qualificationTaskEnabled.logicalId, repositoryOf(qualificationImage.valueAsString), repositoryArn).toString()` as the pull resource. Keep the template test that resolves both states.

## Specific questions put to the reviewer

**Is `f44ce72`'s seed-pin move right, given `tenant_seed.py`?**

Yes for the entry. Since `90c7d3b`, `tenant_seed.make_bundles` reads `CONTRACTS/fixtures/impl-2026-10-28.1/{northstar,spend-review}.json` (`tenant_seed.py:25-27`). Those files hash to `45639ea3…` and `4a4581f5…`, the same bytes as the top-level files at `5023eb9` (verified with `git show 5023eb9:contracts/fixtures/northstar.json | shasum`). The constants therefore stayed correct, and the test now hashes the right paths.

Adding `imports/{mod,sandbox}.rs` and `supervise.rs` to `SOURCES` is also correct and necessary. They are compiled into the library (`lib.rs:6,10`), and `source_bundle_covers_every_compiled_source` would otherwise fail. The `impl-2026-10-28.1` allowlist lines are needed because `admission/tests.rs:23-35` embeds those files at compile time.

What `f44ce72` missed is the Python tooling (M1, M2) and the evidence (M3).

**Is the service image context unchanged?**

Yes, as far as APP-075 is concerned. No APP-075 commit, including `f44ce72`, touches `.dockerignore`, `app/Dockerfile` or any file in the root context (`git diff --name-only e57ce89 bde2667`, plus `f44ce72`'s file list). The root context exported at the merged revision has 151 files and contains no qualification input. It grew from 46 files because of the parser and import work, not APP-075.

The ticket's "37-file" figure is stale twice over. The author's 46-file figure is right for `e57ce89`.

## Acceptance items

| # | Ticket item | Reviewer finding | Status |
| --- | --- | --- | --- |
| 1 | Entry reviewed and tested: closed/duplicate-key manifest, exact selector/arming/UID, fixed target/organizations/pins, private-file ownership/mode/link/size refusals, deadline refusals (>600 s span, <60 s remaining), sanitized panic/timeout output, `Stores.close` and permit release on success, case panic and timeout; no failure emits a successful cleanup receipt | 18 unit tests reviewed and rerun, plus the real-transport campaign rerun (3 cases, server sessions 0). `deny_unknown_fields` and serde duplicate-field detection apply at both levels; invalid UTF-8, FIFO, hard and soft links, symlinked ancestors, `..`, 0750/0755 directories and 0640/0604/0400/0700 files are all refused. Cleanup is confirmed only when the task finished, `Stores::close` succeeded (it itself waits for 8 free permits) and `slots().0 == 0`; an unacknowledged abort yields `cleanup_unconfirmed`. A sentinel panic stays out of both pipes. | **Met**, with minors m3–m5 |
| 2 | Confined local positive with production `Settings::new` TLS, hostname alias, UID 65532, exact five arguments, `passed:true` and six checks; no fixture or insecure-TLS path selects the entry | The entry calls `Settings::new` once with fixed constants. `Settings::fixture` is `cfg(test)` and is source-tested absent from the entry. rustls uses only the fixed CA, `SslMode::Require` with webpki hostname verification, and no custom verifier exists. The reviewer reran the positive at the merged revision on an internal network: direct and supervised runs pass, and the untrusted CA and wrong hostname fail closed. As merged, the harness cannot do this (M1). | **Not met at the merged revision** until M1 and M3 |
| 3 | Image and supervisor built from frozen source with exact bindings; reviewed RDS CA pinned; CA/registry/manifest mounts forbidden; service image and context unchanged; supervisor refusals (wrong arguments, `serve`, operator password) tested | Image rebuilt at the merged revision: 48-file context equal to the allowlist, the CA at the fixed path (`90a6f1a7…`, 0444, root-owned), executable equal to its identity, UID 65532, no port. In the real image, the supervisor refuses `serve`, extra arguments, unarmed runs, `PGPASSWORD`, a volume over `/etc/ssl/certs` and a tmpfs over `/app/qualification`; it refuses mounts under `/run/genui` in unit tests. Service context untouched by APP-075. The bindings are not yet recorded for the merged revision, and `check_image.py` fails as merged (M2, M1). | **Not met at the merged revision** until M1–M3 |
| 4 | Optional `QualificationTask` (count zero, runtime role only, no migration secret) passes the CDK tests; independent review binds all of the above with no blocking finding | 19 of 19 template tests rerun. The empty digest gates the task, its execution role and policy, and the output. Nothing references the task. The task role is the runtime `TaskRole`. The execution role can pull only the selected repository and write logs, and reads no secret. No `Secrets`, port or health check exists. This review has no blockers but requests changes (M1–M3). | **CDK part met**; review binding pending the M1–M3 fixes and a re-check |

## Open items from the APP-008 handoff review

| Handoff item | Status |
| --- | --- |
| Full frozen-source review: closed/duplicate-key parsing, selector/arming/UID, target/org/pins, private file ownership/modes/links/FIFO/size/encoding, original-deadline refusals | **Closed.** Reviewed and tested, as in acceptance item 1. |
| Prove pool/transport closure on success, case panic and timeout; cancellation awaited; watchdog bounded; failures cannot emit a successful cleanup receipt; output privacy with sentinels | **Closed.** Covered by the unit tests and the real-transport campaign, which the reviewer reran. The watchdog's exit 124 is tested. The only residual gap is m4 (the `entry_panic` emit). |
| Confined local positive with the accepted hostname and production `Settings::new` TLS; no insecure TLS or fixture transport | **Closed for the code** (the reviewer reran it at the merged revision). **Open for the recorded evidence** (M1, M3). |
| Separate root-owned package and supervisor, exact binary/source/image binding, fixed paths, credential refresh, original deadline, cleanup, no operator password; CA pinned without mounts or overrides; declared ResourceId is not SQL attestation | **Mostly closed.** Image files are root-owned 0444/0555 on a read-only root. The supervisor pins the reviewed CA in the manifest and on disk, refreshes through `runtime.refresh`/`load` until the child exits, kills at the original expiry, and gives the child only the arming variable. The task definition carries no migration secret, and ECS overrides cannot add secrets or change `entryPoint`. Residual: m6–m8. The ResourceId remains `declared_control_plane_resource_id`, labeled as needing a control-plane receipt (APP-078). |

## Claims the reviewer tried and failed to refute

1. **No fixture path, insecure TLS or wrong CA reaches the managed path through the supervisor.** The supervisor requires the manifest's `ca_sha256` to equal the reviewed pin, and the on-disk CA to hash to it (`qualification_supervisor.py:125`, `:238`). The entry re-hashes the CA against the manifest before and after the cases (`managed_qualification.rs:459`). The CA lives on a read-only image layer, and mounts at or under `/etc/ssl`, `/app`, `/usr`, `/bin`, `/sbin`, `/lib`, `/opt` and under `/run/genui` are refused. A mount at `/etc` or `/run` is accepted, but it can supply only the identical CA bytes and a directory that must be 0700 and owned by 65532 with no pre-existing run files. Running the executable directly needs an `entryPoint` override, which ECS run-task does not offer. The entry by itself accepts any CA its manifest names, and reports it only as `ca_sha256_self_consistency_only`; that is by design.
2. **The entry cannot be selected by any other invocation.** The argument vector must be exactly the five values, compared before any file read (`managed_qualification.rs:604`). Reordered, truncated, extended, renamed and `--test-threads=2` variants are refused. The supervisor launches only that vector, with `env={ARM: ARM_VALUE}`, `cwd=/` and stdin `/dev/null`.
3. **The deadline is the manifest's original one.** Wall and monotonic clocks are sampled together before any read. The span is at most 600 s, `issued_at` at most 5 s ahead, and at least 60 s must remain. The case, abort and close reserves are 4, 3 and 1 s, and the watchdog exits 124 at expiry. The supervisor rechecks the same bounds and kills at `expires_at`.
4. **A failure never claims cleanup.** `Cleanup::Confirmed` requires all three conditions in `supervise` (`:546`). `passed` also requires the post-case input recheck. An `entry_panic` reports cleanup unconfirmed, and a refusal before store construction reports cleanup not applicable.
5. **The service image is unaffected.** `managed_qualification` and its siblings are `#[cfg(test)]` (`storage/mod.rs:68-77`). Neither the supervisor nor any qualification file is in the root `.dockerignore` allowlist, and the service `ENTRYPOINT` is still `runtime.py serve`.
6. **The qualification allowlist admits exactly the compiled inputs.** The merged-revision export equals the 48 allowlisted files: every `app/src/**/*.rs` except `main.rs`, the Cargo files, the APP-006 crate's `Cargo.toml`/`lib.rs` (its `tests.rs` is not compiled for a dependency), the three migration resources, the CA, the nine embedded contract files, `runtime.py`, the supervisor, the lock, the Dockerfile and its ignore file. It contains no harness, check script, evidence or credentials.
7. **The QualificationTask is count zero and cannot reach the migration password.** It has no service, schedule or reference. `Secrets` is undefined. Its execution role has only ECR pull for the selected repository and log writes, and it is absent from every `secretsmanager:GetSecretValue` policy. Its task role is the runtime role, which assumes only the four database roles and two tenant roles.

## Not checked or left to later tickets

- Fargate behaviour (APP-078): `/proc/self/mountinfo` contents, the volume taking the image's 0700/65532 ownership, the read-only root without `/tmp`, IAM tokens, the real RDS certificate chain, STS refresh, CloudWatch output, and the disabled-template intrinsic (m10).
- `clippy -D warnings` and the `cargo +1.85.0` floor were not rerun by the reviewer (the author's receipts are at `5023eb9`).
- `infra/scripts/preflight.py` does not yet know `QualificationImageDigest`. The author records this as APP-078 work.
- Operator packaging and seed code identity are APP-077's (m1).

## Dispositions (2026-10-29)

Recorded by the implementer, not the reviewer. Fixes are in `da3d203` (on `e195ebf`); the re-recorded bindings are in the evidence commit that adds this section. Tests were written before each code change; the supervisor and tool tests were run and seen failing first, and the Rust tests were written before the entry changes they exercise. The reviewer's re-check of M1–M3 is still needed.

| Finding | Disposition | Where |
| --- | --- | --- |
| M1 harness seed pins from the wrong edition | **Fixed.** The entry's new `SEED_INPUTS` table (pin field, repository path, declared SHA-256) is the one path list. The Rust test checks every declared hash against the repository file, and the edition folder against `tenant_seed.py`. `local_harness.seed_inputs()` reads the same table from the entry source, and `expected_pins()` recomputes each pin from those paths; `check_image.py` uses `expected_pins()`. New `app/qualification/test_tools.py` (now in the tenant-foundation workflow) asserts that the tools read the edition `impl-2026-10-28.1` files and match the declared pins. | `managed_qualification.rs` `SEED_INPUTS`; `tests.rs` `seed_catalog_and_migration_pins_match_repository_files`; `local_harness.py`; `test_tools.py` |
| M2 allowlist counted directory re-includes | **Fixed** as proposed. `allowlist()` skips `!dir/` lines and raises when one is not followed at once by its `dir/**` exclusion. Unit-tested on the real ignore file and on synthetic unclosed cases. | `check_image.py` `allowlist`; `test_tools.py` |
| M3 evidence bound to `5023eb9` | **Fixed.** Reran the cleanup campaign, a local image build, `check_image.py` (27 of 27) and `local_harness.py` (10 of 10) at `da3d203`, and replaced the identity, image, entry and local-positive tables: 40 bound inputs, 48-file context, bundle `e35e62b8…`, executable `9cb8b445…`, image `fa35554b…`, catalog `336f870e…`, manifest `cc27fbac…`, seed paths under `impl-2026-10-28.1/`. The `5023eb9` values are named as history and kept in Git. The native and admission campaigns were not rerun (unchanged shared cases); the README says so. | `evidence/APP-075/README.md`, `image/`, `entry/`, `local-positive/`, `checks/` |
| m1 seed provenance by fixture bytes only | **Fixed.** `seed_program_sha256` (`app/runtime/tenant_seed.py`) and `seed_validator_sha256` (`contracts/validate.py`) join the closed `pins`; the test asserts `CONTRACTS / "fixtures" / "<edition>" / (name + ".json")` is in `tenant_seed.py`. The doc comment says APP-077's operator-image digest binds the rest of the seed environment. Consequence: any edit to either file now fails that test until its declared hash is updated, and changes the operator's manifest. | `managed_qualification.rs`; `tests.rs` |
| m2 compiled inputs missing from `SOURCES` | **Fixed** by binding them (the entry documents `SOURCES` as compiled inputs). `SOURCES` now has 40 entries: the RDS CA and the 9 contract files the tests embed were added. `source_bundle_covers_every_compiled_source` scans every bound `.rs` for `include_str!`/`include_bytes!` and requires each target outside `app/migrations/` (pinned separately) to be bound. All were already in the qualification allowlist. | `managed_qualification.rs` `SOURCES`; `tests.rs` |
| m3 `passed:true` without the six named checks | **Fixed.** `report` passes only with exactly `CHECKS` (the six names, in order, each `{"name":…,"passed":true}`); otherwise `case_refused` (cleanup failure still dominates). A test checks each name occurs once in `qualification_cases.rs`, and the Docker campaign asserts the exact list. The supervisor requires six checks, each a named object with `passed:true`, else `checks_incomplete`. | `managed_qualification.rs` `CHECKS`/`complete`; `qualification_supervisor.py` `complete_checks` |
| m4 `entry_panic` emits without the watchdog | **Fixed** with a per-run `OnceLock<Instant>` rather than a static: `run` records the validated deadline in it, and `guarded`'s panic fallback re-arms `Watchdog::start(deadline)`, which `emit` drops only after writing. A new process test panics after recording a 1 s deadline and then stalls; it exits 124 with no result line. | `managed_qualification.rs` `guarded`/`run`; `tests.rs` `an_entry_panic_after_load_keeps_the_original_deadline_watchdog` |
| m5 oversized fallback loses cleanup state | **Fixed.** The `report_refused` fallback copies `cleanup_applicable`/`cleanup_confirmed` only when they are booleans, else true/false. | `managed_qualification.rs` `emit` |
| m6 partly written run file survives | **Fixed.** `write_new_private` appends the path to `written` right after a successful `O_EXCL` open. Tested with an fsync failure on the manifest and on the registry. | `qualification_supervisor.py` |
| m7 SIGTERM ignored as PID 1 | **Fixed.** The image entry point calls `supervise(..., handle_sigterm=True)`, which installs a handler raising `Terminated` (a `BaseException`, so no refusal or refresh handler swallows it). The run then kills the child, removes the files, prints the summary with `error:"terminated"` and returns 143. A second SIGTERM during cleanup is ignored, and the previous handler is restored. A unit test sends SIGTERM to a supervisor running a hanging fake child. Only the SIGKILL after `stopTimeout` skips cleanup. Fargate behaviour is APP-078's to observe. | `qualification_supervisor.py`; `test_qualification_supervisor.py` |
| m8 environment denylist | **Fixed.** The allowlist holds the task's 8 variables, the arming and manifest overrides, exact ECS/Fargate names (`AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`, `AWS_DEFAULT_REGION`, `AWS_EXECUTION_ENV`, `ECS_CONTAINER_METADATA_URI`, `ECS_CONTAINER_METADATA_URI_V4`, `ECS_AGENT_URI`), and Docker's and the image's `PATH`, `HOME`, `HOSTNAME`, `LANG`, `GPG_KEY`, `PYTHON_VERSION`, `PYTHON_SHA256`, `PYTHONDONTWRITEBYTECODE`, `PYTHONUNBUFFERED`, `AWS_CONFIG_FILE` and `AWS_SHARED_CREDENTIALS_FILE`. The last two must be `/dev/null`. It deliberately uses exact names, not `AWS_CONTAINER_*`/`ECS_*` prefixes, so `AWS_CONTAINER_CREDENTIALS_FULL_URI` cannot redirect the credential source. If Fargate injects a name not listed, the run fails closed and APP-078 must add it. The local supervised positive passed under the allowlist inside the container. | `qualification_supervisor.py` `ALLOWED_ENVIRONMENT` |
| m9 confined checks use tmpfs | **Fixed for the image checks; noted for the harness.** `check_image.py` adds an `image-volume-no-tmp` supervisor case: no tmpfs, no `/tmp`, the image's own anonymous `/run/genui` volume (inspected as the only mount, then removed with `--volumes` and confirmed gone). It reaches `credential_refresh_unavailable`, so the run-directory check passed. The local positive keeps its tmpfs, and the README states the difference. | `check_image.py`; `evidence/APP-075/README.md` limitations |
| m10 disabled-state `Fn::Select` | **Declined here; left to APP-078.** `infra/src/stack.ts:225` is unchanged. APP-078 owns the CloudFormation and Fargate behaviour and can apply the `Fn.conditionIf` pull resource with the stack update that proves it. | — |

Not changed: `app/migrations`, the storage definition files, `Tickets/` and `docs/`.
