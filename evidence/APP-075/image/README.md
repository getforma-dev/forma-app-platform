# APP-075 B3: qualification image, supervisor and optional task

Files: [Dockerfile](../../../app/qualification/Dockerfile), [Dockerfile.dockerignore](../../../app/qualification/Dockerfile.dockerignore), [qualification_supervisor.py](../../../app/runtime/qualification_supervisor.py) with [tests](../../../app/runtime/test_qualification_supervisor.py), [check_image.py](../../../app/qualification/check_image.py), [infra/src/stack.ts](../../../infra/src/stack.ts) and [infra/test/security.test.ts](../../../infra/test/security.test.ts). Build: [build.txt](build.txt). Checks: [check.json](check.json).

```sh
docker build --progress=plain -f app/qualification/Dockerfile -t genui-app075-qualification:recheck .
app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-app075-qualification:recheck \
  --base-revision d6986c2 --output evidence/APP-075/image/check.json
docker image rm genui-app075-qualification:recheck
```

## Image

`sha256:d3af3ac012d54bfb11f2ba3f4483b3b897c85af66717b0100771e187e34d250e`, linux/arm64, 498,091,602 bytes, built at the merged tip `d6986c2` for the M1–M3 re-check. Executable sha256 `b3ea2a49c18f7a5a8fef1afcbfdeecfa56f81a329af4819576e551f42e8f2255`, equal to its `build-identity.json`. Source bundle `a885748970cdc0244f3d28aa1d88e409085bebc97ad0a0768855222518dbd2db`. Local only, never pushed, removed after the checks. History (see Git): at `da3d203`, image `fa35554b…`, executable `9cb8b445…`, bundle `e35e62b8…`; at `5023eb9`, image `ff9f0fe0…`, executable `0f3be387…`, bundle `85381e18…`.

- **Stages.**
  - `inputs` (scratch) exists only to export the context.
  - `native` (`rust:1.93.1-bookworm@sha256:7c4ae649…`) runs `cargo test --no-run --locked --lib --message-format=json`. It selects exactly one library test-profile artifact, installs it with mode 0555 and writes `build-identity.json` (executable sha256, `rustc 1.93.1`, Cargo artifact name).
  - The final stage, `python:3.11.14-slim-bookworm@sha256:65a93d69…`, installs `requirements.lock` with hashes and adds the executable, identity, `runtime.py`, the supervisor and the reviewed CA, all read-only. `/run/genui` is 0700/65532 and is the only volume.
- **Configuration.** `USER 65532:65532`, `ENTRYPOINT python3 /app/qualification_supervisor.py`, `CMD run`. No port. The environment holds no credential, endpoint or arming value.
- **Context.** BuildKit's per-Dockerfile ignore file is an explicit 50-file allowlist: the 42 compile-time inputs the source bundle binds (29 Rust sources, `app/Cargo.toml`, `app/Cargo.lock`, the APP-006 `Cargo.toml`, the RDS CA and the 9 contract files the tests embed), `manifest.json`, `catalog-query.json`, `catalog-v5.json`, `runtime.py`, the supervisor, `requirements.lock`, the Dockerfile and the ignore file. The exported context equals the allowlist exactly. `check_image.allowlist()` skips the two `!dir/` re-include lines and refuses one not followed at once by its `dir/**` exclusion.
- **Service image unchanged.** The root `.dockerignore` context is 155 files at `d6986c2` (the check compares it with `d6986c2` itself) and contains no qualification file. None of its files is touched by the APP-075 fix commits `e195ebf..8001228`; at `da3d203` it was 151 files, byte-identical to `e195ebf`.
- **Pins.** The unarmed executable reports database version 5, `catalog-v5.json` `02e1c79b…`, `manifest.json` `b3cc8070…`, `catalog-query.json` `3e3e744d…`, and the five seed pins (`setup.json` `74608644…`, edition `impl-2026-10-28.1` `northstar.json` `45639ea3…` and `spend-review.json` `4a4581f5…`, `tenant_seed.py` `c9073aee…`, `contracts/validate.py` `c5ae11fe…`), all equal to the repository recomputation. (At `da3d203`: version 4, `336f870e…`, `cc27fbac…`, `setup.json` `29221b7a…`, `tenant_seed.py` `ee307fb2…`.)

## Confined checks (27 of 27)

All checks ran with `--network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges` and bounded resources. All but one used a tmpfs `/run/genui` and a tmpfs `/tmp`; the image-volume case used neither (see below).

- **Probe.**
  - Ran as UID/GID 65532.
  - The CA at the fixed path has sha256 `90a6f1a7…` and mode 0444.
  - The executable matches its identity with mode 0555.
  - The supervisor and `runtime.py` equal the repository bytes.
  - There is no set-id file under `/app`.
  - `/run/genui` is 0700/65532.
- **Executable.**
  - The exact five arguments, unarmed, give `configuration_refused` with cleanup not applicable.
  - The reported `resource_pins` equal the recomputation from the exported context and from the repository.
  - `--list --ignored` shows the managed entry.
- **Supervisor in the real image.** Each refusal below launched nothing, removed its files and never echoed the password value.

  | Case | Exit | Error |
  | --- | --- | --- |
  | `serve` | 2 | `configuration_refused` |
  | `run --exact` | 2 | `configuration_refused` |
  | the default command, unarmed | 2 | `configuration_refused` |
  | `PGPASSWORD` present | 2 | `configuration_refused` |
  | a volume mounted over `/etc/ssl/certs` | 2 | `configuration_refused` |
  | a tmpfs over `/app/qualification` | 2 | `configuration_refused` |
  | a valid manifest with the reviewed CA but no credential source | 3 | `credential_refresh_unavailable` (the summary also carries this code) |
  | the same on the image's own anonymous `/run/genui` volume, no tmpfs and no `/tmp` (as in the CDK task) | 3 | `credential_refresh_unavailable` |

  The last two cases passed every pre-launch check in the real image (including the 0700/65532 run-directory check, which the image-volume case meets with the ownership Docker copied from the image), wrote and then removed their private files, and printed only `credential_refresh_unavailable` on stderr. The image-volume container was inspected (its only mount is that volume) and removed with `--volumes`; the volume was then confirmed gone.

## Supervisor unit tests (18)

[supervisor-tests.txt](../checks/supervisor-tests.txt):

- The registry bytes equal the Rust vector, and the reviewed-CA constant equals the packaged bundle.
- Refusals:
  - argument lists: `[]`, `serve`, `migrate`, `refresh`, `probe`, `run --exact`, `run run`, `RUN`
  - variables: `PGPASSWORD`/`PGHOST`/`PGPASSFILE`/`PGUSER`/`GENUI_MIGRATION_PASSWORD`/`db_password`, auth injection, a wrong or missing database host, and any name outside the allowlist (endpoint, CA-bundle, proxy, `PYTHON*`, `LD_*`, `AWS_CONTAINER_CREDENTIALS_FULL_URI`, profile and key variables) or an AWS config file other than `/dev/null`. The allowlist is the task's 8 variables, the two run-task overrides, the ECS/Fargate-injected names and the image's own; a test runs a full Fargate-like environment through to a pass.
  - identity: unarmed, wrong UID
  - manifests: absent, empty, array, invalid, duplicate key, oversized, bad UUID, v7 UUID, span 601, remaining under 60, `issued_at` too far ahead, string time, `armed:false`, wrong host, wrong CA pin
  - substitutions: changed CA bytes, a changed executable, mounts over `/etc/ssl`, `/etc/ssl/certs`, `/app`, the executable, Python's library and each run file, a planted run file (left untouched), a 0750 run directory

  Every refusal happens before any refresh or launch.
- **Exact launch.** The arguments are the five fixed values. The child's environment is `{GENUI_APP008_MANAGED_QUALIFICATION: APP-008}` only. The manifest, registry and credentials exist 0600 when the child starts and are removed afterwards.
- A run file whose write or fsync fails after `O_EXCL` creation is removed.
- SIGTERM (as ECS StopTask sends to PID 1) kills the child, removes the files, prints the summary with `error:"terminated"` and returns 143; the image entry point installs this handler.
- A pass needs exactly six passed checks; five, a failed one or none gives `checks_incomplete`.
- Refreshes repeat until the child exits.
- Refresh exceptions print only `credential_refresh_unavailable`.
- Missing, duplicated or failed results never pass.
- The child is killed at the original `expires_at`.
- 4 MiB of stderr is drained and not forwarded.

## Optional `QualificationTask` (CDK)

The new `QualificationImageDigest` parameter defaults to empty and accepts only an empty value or an ECR digest. The `QualificationTaskEnabled` condition gates three resources and one output:

- The task definition: 1 vCPU/2 GiB, the `ImageArchitecture` platform, a read-only root, UID 65532, `python3 /app/qualification_supervisor.py run`, one `/run/genui` task volume, `awslogs` prefix `genui-qualification`, and the refresh environment only. It has no secrets, ports or health check.
- Its execution role, which can only pull that repository and write logs.
- That role's default policy.
- The `QualificationTaskDefinition` output.

The task role is the existing runtime `TaskRole`: the four database identities and two tenant roles. No migration role or secret is involved, and no secret reader includes the qualification execution role. No service, schedule or other resource references the task, so its count is zero. Two new template tests and two adjusted ones cover this; the suite has 19 tests and all pass ([infra-tests.txt](../checks/infra-tests.txt)).
