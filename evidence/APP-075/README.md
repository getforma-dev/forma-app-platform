# APP-075 local evidence: managed entry, qualification image and supervisor

**Author evidence, not acceptance.** Everything here ran on one Apple-silicon Mac with local Docker. Nothing touched AWS: no deployment, image push, registry, STS, RDS or public DNS. The independent review the ticket requires has not happened, and no acceptance box is checked.

## Identity

| Item | Value |
| --- | --- |
| Base | `e57ce89` (plan Task 4 naming the landed 0003) |
| Commits | `df109c3` B1 entry and tests · `faebc13` B3 image, supervisor, optional `QualificationTask` · `5023eb9` B2 harness and image checks · plus the evidence commit that adds this file |
| Checks ran at | `5023eb94ad8d98da9fa1ab8907d145f395357664` (worktree clean apart from two untracked venv links) |
| Compilers | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, `cargo 1.93.1`; floor `rustc 1.85.0 (4d91de4e4 2025-02-17)` |
| Host | Darwin arm64; Docker 29.8.2 linux/arm64; Python 3.11.0 (locked runtime venv); Node v22.21.0 |
| Qualification image | `sha256:ff9f0fe0f55ed0cc115fe60e3e367a0b6b9e365d5824b44676897be70bf17e1d`, linux/arm64, 481,393,284 bytes, user `65532:65532`, entrypoint `python3 /app/qualification_supervisor.py`, cmd `run`, volume `/run/genui` only, no exposed port. Local only and **removed after the checks**. |
| Test executable | `/app/qualification/genui-storage-tests`, sha256 `0f3be387d802f62e70f3f3a71340bb0f6676e5bbf9ce6d6140e4c3f0c4556ef6` (Cargo artifact `genui-df3b8685482dcc7f`, mode 0555), equal to the image's `build-identity.json` |
| Source bundle pin | `85381e18f1a44cfc1b907893dbc60b71e13087007566e10091c3cf372aea0bcf` (27 compiled files; the executable's reported value equals an independent recomputation from the exported build context) |
| Reviewed RDS CA | `app/certs/rds-global-bundle.pem` sha256 `90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b`, at `/etc/ssl/certs/rds-global-bundle.pem` mode 0444 in the image |
| Schema pins | database version 4; `catalog-v4.json` `8f8e280b…`; `manifest.json` `a76c830b…`; `catalog-query.json` `3e3e744d…` |
| Seed pins | `fixtures/phase-one/v1/setup.json` `29221b7a…`, `contracts/fixtures/northstar.json` `45639ea3…`, `contracts/fixtures/spend-review.json` `4a4581f5…` |
| Local positive run | run UUID `2c46b30d-6194-495b-9884-565ca97e0d94` (direct) and `1e242233-3e68-4634-be91-52aaa383ea41` (through the supervisor), harness label `genui-app075-f816db25fc`, 2026-10-29T21:40Z |

File SHA-256 at `5023eb9`:

```
4f17b0729245e623b4e362ff932cd2345c30ad07efe24d0c88d45e40c973c04c  app/src/storage/managed_qualification.rs
c1ad20305bc50f5903bac880976493b24b06e26155566da7eba509963d5bfde1  app/src/storage/managed_qualification/tests.rs
32190011adad5b2864590d0bfd71d7a31c1252eff4cf3e863890a00a5fcc26a5  app/src/storage/mod.rs (unchanged by APP-075)
80e3fbbd31a16e5cb022882bd56107f6d01327d98026df4cf134445e3f3cf9d8  app/src/storage/qualification_cases.rs (unchanged)
d3edfa89056a3613cf20a97f95c45bec62927cbe5b0abe017499c0a3be5c1cf8  app/runtime/qualification_supervisor.py
1e22ef7004bcbc616710768ecb7672563e283b5db0f6ccd24228e553517a1ec1  app/runtime/test_qualification_supervisor.py
0086034301cb3a77917a47d4a3164a400e0aba40ed1c3948a9b331cb5193c04b  app/qualification/Dockerfile
67ed7939c66553910a365bbf0844a739e3850725fa0bba4997c434855670b0c1  app/qualification/Dockerfile.dockerignore
736b5e1304a3b6d20bf975217abfbf38e78103b98ec11cc6f50342f6eca57a94  app/qualification/local_harness.py
0d900a5f6cdb7452d55a0646c5421b5ee9d52be3c64ee7426e971106146548ac  app/qualification/check_image.py
a45d74bfd2504fea402b91295761798e263e498f8e8087af3547a4899581c22d  infra/src/stack.ts
3ac984a0c5c3ab3c7506ba4f966de7cc51c727897d8dea4558f116a7b5ba0762  infra/test/security.test.ts
```

## Commands and results

Run from the repository root unless a directory is named; `CARGO_BUILD_JOBS=2` throughout. Outputs are in [checks/](checks/), exit codes in [checks/exit-codes.txt](checks/exit-codes.txt) (all 0).

| Command | Result |
| --- | --- |
| `cargo +1.93.1 fmt --check` (in `app/`) | clean |
| `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | clean |
| `cargo +1.93.1 test --locked --lib storage:: -- --test-threads=1` | 26 passed, 5 ignored (the CI form) |
| `cargo +1.93.1 test --locked` | 49 + 1 + 2 passed, 5 ignored |
| `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests` | clean |
| `cargo +1.93.1 test --locked --lib storage::managed_qualification::tests::managed_cleanup_with_real_transports -- --ignored --exact --nocapture` | passed; [receipt](entry/cleanup-campaign.txt) |
| `… storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | passed, 36 checks (unchanged shared cases) |
| `… storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | passed |
| `python3 scripts/check-schema-pins.py` | `agree: true`, version 4 everywhere |
| `app/runtime/.venv/bin/python -m unittest discover -s scripts -p 'test_*.py' -v` | 5 passed |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -v` | 69 passed, 1 skipped (includes the 13 supervisor tests) |
| `npm run typecheck && npm test` (in `infra/`, after `npm ci`) | 19 of 19 template tests pass |
| `docker build --progress=plain -f app/qualification/Dockerfile -t genui-app075-qualification:local .` | built; [log](image/build.txt) (this final build reused the session's earlier layers; the first build compiled the executable in 69 s) |
| `app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-app075-qualification:local --base-revision e57ce89 --output evidence/APP-075/image/check.json` | 24 of 24 expectations met; cleanup complete; [report](image/check.json) |
| `app/runtime/.venv/bin/python app/qualification/local_harness.py --image genui-app075-qualification:local --output evidence/APP-075/local-positive/harness.json` | 10 of 10 expectations met; cleanup complete; [report](local-positive/harness.json) |
| `docker image rm genui-app075-qualification:local` | removed; no `app075` container, network or volume remains |

The entry ([entry/README.md](entry/README.md)), the local positive ([local-positive/README.md](local-positive/README.md)) and the image ([image/README.md](image/README.md)) have their own notes.

## Acceptance items: what this evidence supports locally

| Ticket item | Local status |
| --- | --- |
| 1. Entry reviewed and tested: closed/duplicate-key parsing, selector/arming/UID, fixed target/organizations/pins, private-file refusals, deadline refusals (span over 600 s, remaining under 60 s), sanitized panic/timeout output, proven `Stores.close` and permit release on success, shared-case panic and timeout; a failure never emits a successful cleanup receipt | **Tested; review pending.** 18 unit tests plus the Docker cleanup campaign (server-side session count 0 after each case). |
| 2. Confined local positive with production `Settings::new` TLS against the hostname alias as UID 65532, exact five arguments, `passed:true`, six checks; a fixture or insecure-TLS path cannot select the entry | **Passed locally; review pending.** Source test forbids `Settings::fixture`, `NoTls` and similar in the entry; the untrusted-CA and wrong-hostname runs fail closed. |
| 3. Image and supervisor built from frozen source with exact context, executable, source and image bindings; reviewed RDS CA pinned; CA/registry/manifest mounts forbidden; service image and its context unchanged; supervisor refusals (wrong arguments, `serve`, operator password) tested | **Built and checked locally; review pending.** The service context is **46 files**, byte-identical to `e57ce89`. The ticket's "37-file" figure predates APP-073's admission files and migrations 0003/0004. |
| 4. Optional `QualificationTask` (count zero, runtime role only, no migration secret) passes CDK template tests; independent review binds all of the above with no blocking finding | **Template tests pass; the independent review has not been done.** |

## Limitations and open questions

- **Independent review is outstanding** for B1–B3 (plan steps B1.5, B2 and B3). It should also close the open items in `evidence/APP-008/native/managed-entry/independent-handoff-review.md`. This author did not write a review file.
- **Local only.** The synthetic CA, local SCRAM passwords, the `--network-alias` stand-in for the RDS hostname and the two labelled supervisor substitutions (refresh and CA pin) do not prove anything about IAM tokens, the real RDS certificate chain, STS refresh, Fargate mounts or CloudWatch. That is APP-078's job.
- **Schema pins move with the concurrent 0003 work.** Another worktree is editing `0003_admission.sql`, `catalog-v3/v4.json` and `admission_operator.py`. The entry computes every pin from embedded files, and the harness and image checks recompute them, so no constant needs editing. At merge, rerun `cargo test --lib storage::managed_qualification`, the Docker campaigns, `check_image.py` and `local_harness.py`; the catalog, manifest and source-bundle hashes recorded here will change. If a new compiled `.rs` file lands, `source_bundle_covers_every_compiled_source` fails until it is added to `SOURCES` and to `app/qualification/Dockerfile.dockerignore`. A `catalog-v5` would also need the dockerignore entry.
- **CloudFormation behaviour of the disabled task is unproven.** With an empty `QualificationImageDigest`, the conditional execution-role policy contains `Fn::Select` over a split of the empty parameter. CloudFormation should not evaluate intrinsics of a resource whose condition is false, but only a real stack update (APP-078) can show it. The same is true of Fargate copying the image's 0700/65532 `/run/genui` into the task volume, which the service task already relies on.
- **`preflight.py` does not know `QualificationImageDigest` yet.** It neither checks nor reserves it. APP-078's window needs that, plus the operator's `run-task` override contract: `GENUI_APP008_MANAGED_QUALIFICATION=APP-008` and `GENUI_APP008_MANAGED_MANIFEST_JSON=<manifest>`, at most 8,192 bytes in total.
- **Leftover the author cannot attribute.** One anonymous Docker volume created at 2026-10-29T21:06:15Z (`878de9e6…`) is not from this work (no Docker activity at that time) and was left in place. The three anonymous volumes this harness leaked before its extract-container fix were removed. BuildKit cache mounts `genui-qualification-cargo-registry` and `genui-qualification-target-arm64` remain in the builder cache. They are not images or containers; prune them with `docker builder prune` if wanted.
- **Venv links.** The runs used untracked `app/runtime/.venv` and `contracts/.venv` links to the main checkout's locked environments, which the fixtures need. The links were never committed and were removed afterwards.
