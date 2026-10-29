# APP-075 local evidence: managed entry, qualification image and supervisor

**Author evidence, not acceptance.** Everything here ran on one Apple-silicon Mac with local Docker. Nothing touched AWS: no deployment, image push, registry, STS, RDS or public DNS. The [independent review](independent-review.md) requested changes (M1–M3, m1–m10); its dispositions are appended to it. No acceptance box is checked.

The bindings below were re-recorded after the review fixes, at `da3d203` (on the merged revision `e195ebf`). The first recording, at `5023eb9` (27 sources, a 41-file context, bundle `85381e18…`, executable `0f3be387…`, image `ff9f0fe0…`, seed paths at the top of `contracts/fixtures/`), is kept in Git history; it no longer describes the source.

## Identity

| Item | Value |
| --- | --- |
| Base | `e57ce89` (plan Task 4 naming the landed 0003); merged in `7e75d7c`, pin follow-up `f44ce72`, reviewed at `e195ebf` |
| Commits | `df109c3` B1 entry and tests · `faebc13` B3 image, supervisor, optional `QualificationTask` · `5023eb9` B2 harness and image checks · `da3d203` review fixes · plus the evidence commit that re-records this file |
| Checks ran at | `da3d2033197881197589a4ce06e5f5072e6df9da` (worktree clean apart from two untracked venv links and the evidence outputs being written; `app/admin/dist` and `node_modules` are ignored build outputs) |
| Compilers | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, `cargo 1.93.1`; floor `rustc 1.85.0 (4d91de4e4 2025-02-17)` |
| Host | Darwin arm64; Docker 29.8.2 linux/arm64; Python 3.11.0 (locked runtime venv); Node v22.21.0 |
| Qualification image | `sha256:fa35554b96140904b46987d0f30fcc05c1fcb00c783d69294e459e7cf9501d14`, linux/arm64, 489,200,192 bytes, user `65532:65532`, entrypoint `python3 /app/qualification_supervisor.py`, cmd `run`, volume `/run/genui` only, no exposed port. Local only and **removed after the checks**. |
| Test executable | `/app/qualification/genui-storage-tests`, sha256 `9cb8b4455b21ad4ab9f6f6500fde32e5a31140f678f4bc47d594bee1443e3836` (Cargo artifact `genui-df3b8685482dcc7f`, mode 0555), equal to the image's `build-identity.json` |
| Source bundle pin | `e35e62b8c432f3f499fbfcc3c926ed321d7a592da0a877f105633a0f6e1b39c9` (40 compile-time inputs: 27 Rust sources, the two Cargo files and the APP-006 `Cargo.toml`, the RDS CA and the 9 contract files the tests embed; the executable's reported value equals an independent recomputation from the exported build context) |
| Build context | 48 files, exactly the allowlist: the 40 bound inputs, `manifest.json`, `catalog-query.json`, `catalog-v4.json`, `runtime.py`, the supervisor, `requirements.lock`, the Dockerfile and its ignore file |
| Reviewed RDS CA | `app/certs/rds-global-bundle.pem` sha256 `90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b`, at `/etc/ssl/certs/rds-global-bundle.pem` mode 0444 in the image |
| Schema pins | database version 4; `catalog-v4.json` `336f870e…`; `manifest.json` `cc27fbac…`; `catalog-query.json` `3e3e744d…` |
| Seed pins | `fixtures/phase-one/v1/setup.json` `29221b7a…`, `contracts/fixtures/impl-2026-10-28.1/northstar.json` `45639ea3…`, `contracts/fixtures/impl-2026-10-28.1/spend-review.json` `4a4581f5…`, `app/runtime/tenant_seed.py` `ee307fb2…`, `contracts/validate.py` `c5ae11fe…` (the entry's `SEED_INPUTS` table, which the harness and image checks read) |
| Local positive run | run UUID `93ea7b2f-e294-4a26-9970-422a309663a8` (direct) and `6b3286d0-a066-40c0-bf0e-786bea67d150` (through the supervisor), harness label `genui-app075-259a8d637a`, 2026-10-29T22:50Z |

File SHA-256 at `da3d203`:

```
516b972c1999cba72915d99904232a8f4896eb5a596135d49134731ab9da12b3  app/src/storage/managed_qualification.rs
f32073eb285aa15e353e723b2983219d8f872308bd98acaf09ae1e0da98f43b1  app/src/storage/managed_qualification/tests.rs
26ce5ee32623243ebc89bd5163d6bdb519a441ba358db010b7f7d133754dccfd  app/runtime/qualification_supervisor.py
90431ae197af854b42cef873e3db1455e86c007ae55890a812583bb447146fd5  app/runtime/test_qualification_supervisor.py
0086034301cb3a77917a47d4a3164a400e0aba40ed1c3948a9b331cb5193c04b  app/qualification/Dockerfile
246291e89c7b04ae1784059b0edfabaf2917fcb7057136425b4174cc7ba55d4a  app/qualification/Dockerfile.dockerignore
708366eb61993f4bae9d797ba27a7c5763e0eea3a4f092d1a59be3ef87f467a9  app/qualification/local_harness.py
ecc01e2062ccfcb3539a57dce18e4ee033af5b173a87d605e3e736503ce641cf  app/qualification/check_image.py
8ed31b2cd6bbbd7a784e960814db3d3d8eeeb40ca69b95d91be3870f97dc8fb9  app/qualification/test_tools.py
ee307fb23c2ee0180f27510efa946fdba6269eee104ddb5e08feee56b174fd6b  app/runtime/tenant_seed.py
a45d74bfd2504fea402b91295761798e263e498f8e8087af3547a4899581c22d  infra/src/stack.ts
3ac984a0c5c3ab3c7506ba4f966de7cc51c727897d8dea4558f116a7b5ba0762  infra/test/security.test.ts
```

## Commands and results

Run from the repository root unless a directory is named; `CARGO_BUILD_JOBS=2` throughout. Outputs are in [checks/](checks/), exit codes in [checks/exit-codes.txt](checks/exit-codes.txt) (all 0).

| Command | Result |
| --- | --- |
| `cargo +1.93.1 fmt --check` (in `app/`) | clean |
| `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` (after `npm ci && npm run build` in `app/admin`, as CI does) | clean |
| `cargo +1.93.1 test --locked --lib storage:: -- --test-threads=1` | 28 passed, 5 ignored (the CI form) |
| `cargo +1.93.1 test --locked` | 69 + 1 + 2 passed, 6 ignored |
| `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests` | clean |
| `cargo +1.93.1 test --locked --lib storage::managed_qualification::tests::managed_cleanup_with_real_transports -- --ignored --exact --nocapture` | passed; [receipt](entry/cleanup-campaign.txt) |
| `… storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | not rerun after the review fixes (unchanged shared cases); its receipt is from `5023eb9` (36 checks) |
| `… storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | not rerun after the review fixes; its receipt is from `5023eb9` |
| `python3 scripts/check-schema-pins.py` | `agree: true`, version 4 everywhere |
| `app/runtime/.venv/bin/python -m unittest discover -s scripts -p 'test_*.py' -v` | 11 passed |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -v` | 104 passed, 1 skipped (includes the 18 supervisor tests) |
| `app/runtime/.venv/bin/python -m unittest discover -s app/qualification -v` | 6 passed (the proof tools' own tests; now in CI) |
| `npm run typecheck && npm test` (in `infra/`, after `npm ci`) | 19 of 19 template tests pass |
| `docker build --progress=plain -f app/qualification/Dockerfile -t genui-app075-qualification:dispositions .` | built; [log](image/build.txt) (the BuildKit cache mounts were warm; the library test profile compiled in 19 s) |
| `app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-app075-qualification:dispositions --base-revision e195ebf --output evidence/APP-075/image/check.json` | 27 of 27 expectations met; cleanup complete; [report](image/check.json) |
| `app/runtime/.venv/bin/python app/qualification/local_harness.py --image genui-app075-qualification:dispositions --output evidence/APP-075/local-positive/harness.json` | 10 of 10 expectations met; cleanup complete; [report](local-positive/harness.json) |
| `docker image rm genui-app075-qualification:dispositions` | removed; container, volume, network and image lists equal their pre-run state (apart from two containers another session removed meanwhile) |

The entry ([entry/README.md](entry/README.md)), the local positive ([local-positive/README.md](local-positive/README.md)) and the image ([image/README.md](image/README.md)) have their own notes.

## Acceptance items: what this evidence supports locally

| Ticket item | Local status |
| --- | --- |
| 1. Entry reviewed and tested: closed/duplicate-key parsing, selector/arming/UID, fixed target/organizations/pins, private-file refusals, deadline refusals (span over 600 s, remaining under 60 s), sanitized panic/timeout output, proven `Stores.close` and permit release on success, shared-case panic and timeout; a failure never emits a successful cleanup receipt | **Tested; reviewed with minors, now fixed (re-check pending).** 20 unit tests plus the Docker cleanup campaign (server-side session count 0 after each case). |
| 2. Confined local positive with production `Settings::new` TLS against the hostname alias as UID 65532, exact five arguments, `passed:true`, six checks; a fixture or insecure-TLS path cannot select the entry | **Passed locally at `da3d203`; re-check pending.** Source test forbids `Settings::fixture`, `NoTls` and similar in the entry; the untrusted-CA and wrong-hostname runs fail closed. |
| 3. Image and supervisor built from frozen source with exact context, executable, source and image bindings; reviewed RDS CA pinned; CA/registry/manifest mounts forbidden; service image and its context unchanged; supervisor refusals (wrong arguments, `serve`, operator password) tested | **Built and checked locally at `da3d203`; re-check pending.** The service context is **151 files**, byte-identical to `e195ebf` (it grew from 46 at `e57ce89` with the parser and import work, not APP-075). The ticket's "37-file" figure is stale. |
| 4. Optional `QualificationTask` (count zero, runtime role only, no migration secret) passes CDK template tests; independent review binds all of the above with no blocking finding | **Template tests pass. The review found no blocker; its M1–M3 are fixed here and need the reviewer's re-check.** |

## Limitations and open questions

- **The independent review requested changes** ([independent-review.md](independent-review.md)); the dispositions at its end record what `da3d203` fixed and what is left to APP-077/APP-078. A re-check of M1–M3 against these bindings is still needed.
- **Local only.** The synthetic CA, local SCRAM passwords, the `--network-alias` stand-in for the RDS hostname and the two labelled supervisor substitutions (refresh and CA pin) do not prove anything about IAM tokens, the real RDS certificate chain, STS refresh, Fargate mounts or CloudWatch. That is APP-078's job.
- **Filesystem stand-ins.** Most confined runs use a tmpfs `/run/genui` (0700, UID 65532) and a tmpfs `/tmp`. The CDK task has a read-only root, no `/tmp` and a Fargate task volume initialized from the image's `VOLUME`. `check_image.py` now runs one supervisor case that way (the image's own anonymous volume, no tmpfs, no `/tmp`): it passes every pre-launch check, including the run-directory check, and stops at `credential_refresh_unavailable`. The local positive still uses the tmpfs. Fargate's own volume behaviour is APP-078's to observe.
- **Seed code is now pinned.** `seed_program_sha256` (`app/runtime/tenant_seed.py`) and `seed_validator_sha256` (`contracts/validate.py`) join the manifest pins, so any edit to either file fails `seed_catalog_and_migration_pins_match_repository_files` until the declared hash in `SEED_INPUTS` is updated, and changes the operator's manifest.
- **Schema pins move with concurrent work.** Another worktree is adding migration 0005 and new `.rs` files. The entry computes every pin from embedded files, and the harness and image checks recompute them, so no constant needs editing. At merge, rerun `cargo test --lib storage::managed_qualification`, the Docker campaigns, `check_image.py` and `local_harness.py`; the catalog, manifest and source-bundle hashes recorded here will change. If a new compiled `.rs` file (or a new `include_str!`/`include_bytes!` target outside `app/migrations/`) lands, `source_bundle_covers_every_compiled_source` fails until it is added to `SOURCES` and to `app/qualification/Dockerfile.dockerignore`. A `catalog-v5` would also need the dockerignore entry.
- **CloudFormation behaviour of the disabled task is unproven.** With an empty `QualificationImageDigest`, the conditional execution-role policy contains `Fn::Select` over a split of the empty parameter. CloudFormation should not evaluate intrinsics of a resource whose condition is false, but only a real stack update (APP-078) can show it. The same is true of Fargate copying the image's 0700/65532 `/run/genui` into the task volume, which the service task already relies on.
- **`preflight.py` does not know `QualificationImageDigest` yet.** It neither checks nor reserves it. APP-078's window needs that, plus the operator's `run-task` override contract: `GENUI_APP008_MANAGED_QUALIFICATION=APP-008` and `GENUI_APP008_MANAGED_MANIFEST_JSON=<manifest>`, at most 8,192 bytes in total.
- **Leftover the author cannot attribute.** One anonymous Docker volume created at 2026-10-29T21:06:15Z (`878de9e6…`) is not from this work (no Docker activity at that time) and was left in place. The three anonymous volumes this harness leaked before its extract-container fix were removed. BuildKit cache mounts `genui-qualification-cargo-registry` and `genui-qualification-target-arm64` remain in the builder cache. They are not images or containers; prune them with `docker builder prune` if wanted. The re-recording left the volume, network and image lists exactly as it found them.
- **Venv links.** The runs used untracked `app/runtime/.venv` and `contracts/.venv` links to the main checkout's locked environments, which the fixtures need. The links were never committed and were removed afterwards.
