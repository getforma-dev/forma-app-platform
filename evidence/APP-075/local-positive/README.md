# APP-075 B2: confined local positive

Harness: [local_harness.py](../../../app/qualification/local_harness.py). Report: [harness.json](harness.json). Label `genui-app075-259a8d637a`, run at 2026-10-29T22:50Z at `da3d203` against qualification image `sha256:fa35554b…` (arm64). The first recording (at `5023eb9`, image `ff9f0fe0…`) is in Git history.

```sh
app/runtime/.venv/bin/python app/qualification/local_harness.py \
  --image genui-app075-qualification:dispositions --output evidence/APP-075/local-positive/harness.json
```

## Setup

- **Executable.** Copied out of the qualification image, which compiled it in `rust:1.93.1-bookworm@sha256:7c4ae649…` with `cargo test --no-run --locked --lib --message-format=json`, taking the path from the artifact metadata. sha256 `9cb8b445…`, equal to the image's build identity.
- **PostgreSQL.** `postgres@sha256:5d143123…`, server 16.13. `ssl=on`, with a server certificate signed by a CA created for this run whose only SAN is `genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com`.
- **Migrations and seed.** `runtime.apply_migrations` for 0001–0004 (schema version 4) and `tenant_seed.seed(make_bundles())` through the locked runtime and contract environments.
- **Authentication.** The four application roles get distinct random local passwords. `pg_hba` allows `hostssl … scram-sha-256` only and rejects plaintext TCP. This is local password authentication, **not IAM**.
- **Network.** The server joins an `--internal` Docker network (no external route and no public DNS) under the exact hostname as a network alias. The run containers join only that network.
- **Run image.** A throwaway image derived from the qualification image differs only in `/etc/ssl/certs/rds-global-bundle.pem` (the synthetic CA, mode 0444). There are no bind mounts.
- **Run containers.** `--user 65532:65532 --read-only --cap-drop=ALL --security-opt=no-new-privileges`, bounded pids, memory and CPU, and a fresh tmpfs `/run/genui` with mode 0700 owned by the run UID. A loader writes the manifest, the canonical registry and the credential file with `O_EXCL` and mode 0600, records the UID and modes, then calls `execv` on the executable with the given arguments.

## Results (10 of 10 expectations met, cleanup complete)

| Run | Run UUID | Exit | Outcome |
| --- | --- | --- | --- |
| positive (direct, exact five arguments) | `93ea7b2f-e294-4a26-9970-422a309663a8` | 0 | `passed:true`, six checks, `cleanup_confirmed:true`, pins equal to the independent recomputation; euid 65532, `/run/genui` 0700/65532, all three files 0600/65532 |
| supervised positive | `6b3286d0-a066-40c0-bf0e-786bea67d150` | 0 | The supervisor launched the same executable and retained the result (`passed:true`, six checks, confirmed cleanup). Summary: one refresh, child exit 0, files removed. |
| unarmed | `b02d5bb6-…` | 101 | `configuration_refused`, cleanup not applicable |
| root UID (`0:0`, run directory owned by root) | `70ef69dd-…` | 101 | `configuration_refused`, cleanup not applicable |
| wrong arguments (no `--nocapture`) | `798d9ee0-…` | 101 | `configuration_refused`, cleanup not applicable |
| untrusted CA (manifest pinned to that CA) | `c2c745e0-…` | 101 | `case_refused`: TLS verification failed, cleanup confirmed |
| trusted CA, certificate for `localhost` only | `a687e623-…` | 101 | `case_refused`: hostname verification failed, cleanup confirmed |

The six checks:

1. aggregate fixed-role native readiness
2. TLS role identity and scoped two-organization reads
3. RLS, blank scope and direct-write refusal under all four roles
4. revocation, epoch, missing-grant and registry mismatch denials
5. the closed action matrix
6. same-backend reuse cleanup

The supervised run uses the actual `qualification_supervisor.supervise` with exactly two substitutions, both recorded in the report:

1. Credential refresh writes the synthetic local-password envelope instead of calling STS and generating IAM tokens.
2. The reviewed-CA pin is the synthetic CA's hash instead of `90a6f1a7…`.

Every other check ran unchanged: arguments, arming, the environment allowlist (the container's own Docker and image variables pass it), UID, manifest bounds, mounts, executable identity, private file creation, the child's reduced environment, pipe draining, result retention and file removal.

## Not proven here

IAM authentication and token refresh, the real RDS certificate chain, `rds.force_ssl`, Fargate's volume and mount layout, CloudWatch output and the managed matrix all belong to APP-078. The network alias stands in for DNS resolution of the real endpoint.
