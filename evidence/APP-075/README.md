# APP-075 local evidence: managed entry, qualification image and supervisor

**Author evidence, not acceptance.** Everything here ran on one Apple-silicon Mac with local Docker. Nothing touched AWS: no deployment, image push, registry, STS, RDS or public DNS. The [independent review](independent-review.md) requested changes (M1–M3, m1–m10); its dispositions and the re-check of M1–M3 are appended to it. No acceptance box is checked.

The bindings below were re-recorded by the M1–M3 re-check at the merged tip `d6986c2`, after the review fixes merged in `3f918fb` together with the fixture provenance repair (`faf4b6c`), APP-010 (migration 0005, `definitions.rs`) and the APP-073 provider fixes. Earlier recordings, kept as history in Git and no longer describing the source:

- **Dispositions recording, at `da3d203`** (on `e195ebf`): 40 bound inputs (27 Rust sources), a 48-file context with `catalog-v4.json`, bundle `e35e62b8…`, executable `9cb8b445…`, image `fa35554b…`, database version 4, catalog `336f870e…`, manifest `cc27fbac…`, `setup.json` `29221b7a…`, `tenant_seed.py` `ee307fb2…`; positive runs `93ea7b2f…`/`6b3286d0…`.
- **First recording, at `5023eb9`**: 27 sources, a 41-file context, bundle `85381e18…`, executable `0f3be387…`, image `ff9f0fe0…`, seed paths at the top of `contracts/fixtures/`.

## Identity

| Item | Value |
| --- | --- |
| Base | `e57ce89` (plan Task 4 naming the landed 0003); merged in `7e75d7c`, pin follow-up `f44ce72`, reviewed at `e195ebf`, fixes merged in `3f918fb`, re-checked at `d6986c2` |
| Commits | `df109c3` B1 entry and tests · `faebc13` B3 image, supervisor, optional `QualificationTask` · `5023eb9` B2 harness and image checks · `da3d203` review fixes · `8001228` dispositions evidence · `3f918fb` merge (seed pins moved to the `faf4b6c` `setup.json`, the APP-010 `tenant_seed.py` and the APP-010 sources) · plus the re-check evidence commit that re-records this file |
| Checks ran at | `d6986c2ea474573d056c98b0704435358e5486cf` (worktree clean apart from two untracked venv links and the evidence outputs being written; `app/admin/dist` and `node_modules` are ignored build outputs). No source, pin or allowlist entry needed a change. |
| Compilers | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, `cargo 1.93.1`; floor `rustc 1.85.0 (4d91de4e4 2025-02-17)` |
| Host | Darwin arm64; Docker 29.8.2 linux/arm64; Python 3.11.0 (locked runtime venv); Node v22.21.0 |
| Qualification image | `sha256:d3af3ac012d54bfb11f2ba3f4483b3b897c85af66717b0100771e187e34d250e`, linux/arm64, 498,091,602 bytes, user `65532:65532`, entrypoint `python3 /app/qualification_supervisor.py`, cmd `run`, volume `/run/genui` only, no exposed port. Local only and **removed after the checks**. |
| Test executable | `/app/qualification/genui-storage-tests`, sha256 `b3ea2a49c18f7a5a8fef1afcbfdeecfa56f81a329af4819576e551f42e8f2255` (Cargo artifact `genui-df3b8685482dcc7f`, mode 0555), equal to the image's `build-identity.json` |
| Source bundle pin | `a885748970cdc0244f3d28aa1d88e409085bebc97ad0a0768855222518dbd2db` (42 compile-time inputs: 29 Rust sources, the two Cargo files and the APP-006 `Cargo.toml`, the RDS CA and the 9 contract files the tests embed; the executable's reported value equals an independent recomputation from the exported build context) |
| Build context | 50 files, exactly the allowlist: the 42 bound inputs, `manifest.json`, `catalog-query.json`, `catalog-v5.json`, `runtime.py`, the supervisor, `requirements.lock`, the Dockerfile and its ignore file |
| Reviewed RDS CA | `app/certs/rds-global-bundle.pem` sha256 `90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b`, at `/etc/ssl/certs/rds-global-bundle.pem` mode 0444 in the image |
| Schema pins | database version 5; `catalog-v5.json` `02e1c79b…`; `manifest.json` `b3cc8070…`; `catalog-query.json` `3e3e744d…` |
| Seed pins | `fixtures/phase-one/v1/setup.json` `74608644…`, `contracts/fixtures/impl-2026-10-28.1/northstar.json` `45639ea3…`, `contracts/fixtures/impl-2026-10-28.1/spend-review.json` `4a4581f5…`, `app/runtime/tenant_seed.py` `c9073aee…`, `contracts/validate.py` `c5ae11fe…` (the entry's `SEED_INPUTS` table, which the harness and image checks read) |
| Local positive run | run UUID `a3ea4642-b695-47f0-8006-33217f199a9a` (direct) and `e8d6e41a-3ccb-4f4b-8ddb-27eacd0c4979` (through the supervisor), harness label `genui-app075-00d9122039`, 2026-10-29T23:31Z; migrations 0001–0005, schema 5 |

File SHA-256 at `d6986c2` (changed since `da3d203`: `managed_qualification.rs`, `Dockerfile.dockerignore` and `tenant_seed.py`, all in the `3f918fb` merge):

```
e8a176624ae6c2fa3e8fdb40c4847b1cbc583456a4ac3a63c157484402c8bf50  app/src/storage/managed_qualification.rs
f32073eb285aa15e353e723b2983219d8f872308bd98acaf09ae1e0da98f43b1  app/src/storage/managed_qualification/tests.rs
26ce5ee32623243ebc89bd5163d6bdb519a441ba358db010b7f7d133754dccfd  app/runtime/qualification_supervisor.py
90431ae197af854b42cef873e3db1455e86c007ae55890a812583bb447146fd5  app/runtime/test_qualification_supervisor.py
0086034301cb3a77917a47d4a3164a400e0aba40ed1c3948a9b331cb5193c04b  app/qualification/Dockerfile
f015e183f76bdbcfe830964e3675e6ed83a730cb68f2b7fb1441afe7ab0169c7  app/qualification/Dockerfile.dockerignore
708366eb61993f4bae9d797ba27a7c5763e0eea3a4f092d1a59be3ef87f467a9  app/qualification/local_harness.py
ecc01e2062ccfcb3539a57dce18e4ee033af5b173a87d605e3e736503ce641cf  app/qualification/check_image.py
8ed31b2cd6bbbd7a784e960814db3d3d8eeeb40ca69b95d91be3870f97dc8fb9  app/qualification/test_tools.py
c9073aee151f25006219acb15d0e7ac35c01a76751ee3392c02f8f26aa50d4ad  app/runtime/tenant_seed.py
a45d74bfd2504fea402b91295761798e263e498f8e8087af3547a4899581c22d  infra/src/stack.ts
3ac984a0c5c3ab3c7506ba4f966de7cc51c727897d8dea4558f116a7b5ba0762  infra/test/security.test.ts
```

## Commands and results

Run from the repository root unless a directory is named; `CARGO_BUILD_JOBS=2` throughout. Outputs are in [checks/](checks/), exit codes in [checks/exit-codes.txt](checks/exit-codes.txt) (all 0).

| Command | Result |
| --- | --- |
| `cargo +1.93.1 fmt --check` (in `app/`) | clean |
| `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` (after `npm ci && npm run build` in `app/admin`, as CI does) | clean |
| `cargo +1.93.1 test --locked --lib storage:: -- --test-threads=1` | 31 passed, 6 ignored (the CI form) |
| `cargo +1.93.1 test --locked` | 74 + 1 + 2 passed, 7 ignored |
| `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests` | clean |
| `cargo +1.93.1 test --locked --lib storage::managed_qualification::tests::managed_cleanup_with_real_transports -- --ignored --exact --nocapture` | passed; [receipt](entry/cleanup-campaign.txt) |
| `… storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | passed, 36 checks; [receipt](checks/native-campaign.txt) |
| `… storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | passed; [receipt](checks/admission-campaign.txt) |
| `python3 scripts/check-schema-pins.py` | `agree: true`, version 5 everywhere |
| `app/runtime/.venv/bin/python -m unittest discover -s scripts -p 'test_*.py' -v` | 17 passed |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -v` | 115 passed, 1 skipped (includes the 18 supervisor tests) |
| `app/runtime/.venv/bin/python -m unittest discover -s app/qualification -v` | 6 passed (the proof tools' own tests; now in CI) |
| `npm run typecheck && npm test` (in `infra/`, after `npm ci`) | 19 of 19 template tests pass |
| `docker build --progress=plain -f app/qualification/Dockerfile -t genui-app075-qualification:recheck .` | built; [log](image/build.txt) (the BuildKit cache mounts were warm; the library test profile compiled in 33 s) |
| `app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-app075-qualification:recheck --base-revision d6986c2 --output evidence/APP-075/image/check.json` | 27 of 27 expectations met; cleanup complete; [report](image/check.json) |
| `app/runtime/.venv/bin/python app/qualification/local_harness.py --image genui-app075-qualification:recheck --output evidence/APP-075/local-positive/harness.json` | 10 of 10 expectations met; cleanup complete; [report](local-positive/harness.json) |
| `docker image rm genui-app075-qualification:recheck` | removed; container, volume, network and image lists equal their pre-run state |

The entry ([entry/README.md](entry/README.md)), the local positive ([local-positive/README.md](local-positive/README.md)) and the image ([image/README.md](image/README.md)) have their own notes.

## Acceptance items: what this evidence supports locally

| Ticket item | Local status |
| --- | --- |
| 1. Entry reviewed and tested: closed/duplicate-key parsing, selector/arming/UID, fixed target/organizations/pins, private-file refusals, deadline refusals (span over 600 s, remaining under 60 s), sanitized panic/timeout output, proven `Stores.close` and permit release on success, shared-case panic and timeout; a failure never emits a successful cleanup receipt | **Tested; reviewed, M1–M3 re-checked with no blocking finding.** 20 unit tests plus the Docker cleanup campaign (server-side session count 0 after each case), rerun at `d6986c2`. |
| 2. Confined local positive with production `Settings::new` TLS against the hostname alias as UID 65532, exact five arguments, `passed:true`, six checks; a fixture or insecure-TLS path cannot select the entry | **Passed locally at `d6986c2` (re-check).** Source test forbids `Settings::fixture`, `NoTls` and similar in the entry; the untrusted-CA and wrong-hostname runs fail closed. |
| 3. Image and supervisor built from frozen source with exact context, executable, source and image bindings; reviewed RDS CA pinned; CA/registry/manifest mounts forbidden; service image and its context unchanged; supervisor refusals (wrong arguments, `serve`, operator password) tested | **Built and checked locally at `d6986c2` (re-check).** The service context is **155 files** at `d6986c2` and holds no qualification input; none of its files is touched by the APP-075 fix commits (`e195ebf..8001228`). It was 151 at `e195ebf` and 46 at `e57ce89`; the growth is the parser, import and APP-010 work, not APP-075. The ticket's "37-file" figure is stale. |
| 4. Optional `QualificationTask` (count zero, runtime role only, no migration secret) passes CDK template tests; independent review binds all of the above with no blocking finding | **Template tests pass. The review found no blocker; the re-check of M1–M3 at `d6986c2` found no blocking finding.** |

## Limitations and open questions

- **The independent review requested changes** ([independent-review.md](independent-review.md)); the dispositions at its end record what `da3d203` fixed and what is left to APP-077/APP-078. Its re-check section records M1–M3 at `d6986c2` against these bindings.
- **Local only.** The synthetic CA, local SCRAM passwords, the `--network-alias` stand-in for the RDS hostname and the two labelled supervisor substitutions (refresh and CA pin) do not prove anything about IAM tokens, the real RDS certificate chain, STS refresh, Fargate mounts or CloudWatch. That is APP-078's job.
- **Filesystem stand-ins.** Most confined runs use a tmpfs `/run/genui` (0700, UID 65532) and a tmpfs `/tmp`. The CDK task has a read-only root, no `/tmp` and a Fargate task volume initialized from the image's `VOLUME`. `check_image.py` now runs one supervisor case that way (the image's own anonymous volume, no tmpfs, no `/tmp`): it passes every pre-launch check, including the run-directory check, and stops at `credential_refresh_unavailable`. The local positive still uses the tmpfs. Fargate's own volume behaviour is APP-078's to observe.
- **Seed code is now pinned.** `seed_program_sha256` (`app/runtime/tenant_seed.py`) and `seed_validator_sha256` (`contracts/validate.py`) join the manifest pins, so any edit to either file fails `seed_catalog_and_migration_pins_match_repository_files` until the declared hash in `SEED_INPUTS` is updated, and changes the operator's manifest.
- **Schema pins move with concurrent work.** Migration 0005 and the APP-010 `.rs` files landed and are bound here (database version 5, 29 Rust sources). The entry computes every pin from embedded files, and the harness and image checks recompute them, so no constant needs editing. At merge, rerun `cargo test --lib storage::managed_qualification`, the Docker campaigns, `check_image.py` and `local_harness.py`; the catalog, manifest and source-bundle hashes recorded here will change. If a new compiled `.rs` file (or a new `include_str!`/`include_bytes!` target outside `app/migrations/`) lands, `source_bundle_covers_every_compiled_source` fails until it is added to `SOURCES` and to `app/qualification/Dockerfile.dockerignore`. A `catalog-v6` would also need the dockerignore entry.
- **CloudFormation behaviour of the disabled task is unproven.** With an empty `QualificationImageDigest`, the conditional execution-role policy contains `Fn::Select` over a split of the empty parameter. CloudFormation should not evaluate intrinsics of a resource whose condition is false, but only a real stack update (APP-078) can show it. The same is true of Fargate copying the image's 0700/65532 `/run/genui` into the task volume, which the service task already relies on.
- **`preflight.py` does not know `QualificationImageDigest` yet.** It neither checks nor reserves it. APP-078's window needs that, plus the operator's `run-task` override contract: `GENUI_APP008_MANAGED_QUALIFICATION=APP-008` and `GENUI_APP008_MANAGED_MANIFEST_JSON=<manifest>`, at most 8,192 bytes in total.
- **Leftover the author cannot attribute.** One anonymous Docker volume created at 2026-10-29T21:06:15Z (`878de9e6…`) is not from this work (no Docker activity at that time) and was left in place. The three anonymous volumes this harness leaked before its extract-container fix were removed. BuildKit cache mounts `genui-qualification-cargo-registry` and `genui-qualification-target-arm64` remain in the builder cache. They are not images or containers; prune them with `docker builder prune` if wanted. The re-check left the container, volume, network and image lists exactly as it found them.
- **Venv links.** The runs used untracked `app/runtime/.venv` and `contracts/.venv` links to the main checkout's locked environments, which the fixtures need. The links were never committed and were removed afterwards.
