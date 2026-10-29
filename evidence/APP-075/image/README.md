# APP-075 B3: qualification image, supervisor and optional task

Files: [Dockerfile](../../../app/qualification/Dockerfile), [Dockerfile.dockerignore](../../../app/qualification/Dockerfile.dockerignore), [qualification_supervisor.py](../../../app/runtime/qualification_supervisor.py) with [tests](../../../app/runtime/test_qualification_supervisor.py), [check_image.py](../../../app/qualification/check_image.py), [infra/src/stack.ts](../../../infra/src/stack.ts) and [infra/test/security.test.ts](../../../infra/test/security.test.ts). Build: [build.txt](build.txt). Checks: [check.json](check.json).

```sh
docker build --progress=plain -f app/qualification/Dockerfile -t genui-app075-qualification:local .
app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-app075-qualification:local \
  --base-revision e57ce89 --output evidence/APP-075/image/check.json
docker image rm genui-app075-qualification:local
```

## Image

`sha256:ff9f0fe0f55ed0cc115fe60e3e367a0b6b9e365d5824b44676897be70bf17e1d`, linux/arm64. Local only, never pushed, removed after the checks.

- **Stages.**
  - `inputs` (scratch) exists only to export the context.
  - `native` (`rust:1.93.1-bookworm@sha256:7c4ae649…`) runs `cargo test --no-run --locked --lib --message-format=json`. It selects exactly one library test-profile artifact, installs it with mode 0555 and writes `build-identity.json` (executable sha256, `rustc 1.93.1`, Cargo artifact name).
  - The final stage, `python:3.11.14-slim-bookworm@sha256:65a93d69…`, installs `requirements.lock` with hashes and adds the executable, identity, `runtime.py`, the supervisor and the reviewed CA, all read-only. `/run/genui` is 0700/65532 and is the only volume.
- **Configuration.** `USER 65532:65532`, `ENTRYPOINT python3 /app/qualification_supervisor.py`, `CMD run`. No port. The environment holds no credential, endpoint or arming value.
- **Context.** BuildKit's per-Dockerfile ignore file is an explicit 41-file allowlist: the 27 compiled sources, `manifest.json`, `catalog-query.json`, `catalog-v4.json`, the RDS CA, the 5 contract files the tests embed, `runtime.py`, the supervisor, `requirements.lock`, the Dockerfile and the ignore file. The exported context equals the allowlist exactly.
- **Service image unchanged.** The root `.dockerignore` context (46 files) is byte-identical to `e57ce89`, and it contains no qualification file.

## Confined checks (24 of 24)

All checks ran with `--network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges`, bounded resources and a tmpfs `/run/genui`.

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

  The last case passed every pre-launch check in the real image, wrote and then removed its private files, and printed only `credential_refresh_unavailable` on stderr.

## Supervisor unit tests (13)

[supervisor-tests.txt](../checks/supervisor-tests.txt):

- The registry bytes equal the Rust vector, and the reviewed-CA constant equals the packaged bundle.
- Refusals:
  - argument lists: `[]`, `serve`, `migrate`, `refresh`, `probe`, `run --exact`, `run run`, `RUN`
  - variables: `PGPASSWORD`/`PGHOST`/`PGPASSFILE`/`PGUSER`/`GENUI_MIGRATION_PASSWORD`/`db_password`, auth injection, a wrong or missing database host
  - identity: unarmed, wrong UID
  - manifests: absent, empty, array, invalid, duplicate key, oversized, bad UUID, v7 UUID, span 601, remaining under 60, `issued_at` too far ahead, string time, `armed:false`, wrong host, wrong CA pin
  - substitutions: changed CA bytes, a changed executable, mounts over `/etc/ssl`, `/etc/ssl/certs`, `/app`, the executable, Python's library and each run file, a planted run file (left untouched), a 0750 run directory

  Every refusal happens before any refresh or launch.
- **Exact launch.** The arguments are the five fixed values. The child's environment is `{GENUI_APP008_MANAGED_QUALIFICATION: APP-008}` only. The manifest, registry and credentials exist 0600 when the child starts and are removed afterwards.
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
