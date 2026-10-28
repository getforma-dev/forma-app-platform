# APP-007 migration harness TCP readiness — independent review

Reviewed 2026-10-28. **Accepted for the CI retry; no blocking findings.** The [structured record](tcp-readiness-review.json) binds the source, exact image, prior failure and all executed commands, observations and cleanup.

The retained [CI failure](ci-startup-race.txt) at source `45dd49f1314d9cd0405e4471dab110123a98185b` occurs on the first `SHOW server_version`, before migration. The old socket readiness check could accept the image's temporary initialization server, which subsequently stops. The repair only adds `--host=127.0.0.1` to the existing `pg_isready` call plus explanatory comments. Removing those additions exactly reproduces the committed file; Python parsing also passes. Timeout/poll limits, SQL, migration contents and cleanup are unchanged.

I reproduced the distinction in a named disposable container using the exact current pin `postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59` (PostgreSQL 16.13, local ARM64). The container had `--network=none`, no published ports, 512 MiB memory, one CPU and a 256 MiB ephemeral data directory. A temporary read-only initdb SQL fixture ran `SELECT pg_sleep(3)` between two synthetic markers.

| Observed phase | Unix-socket readiness | TCP loopback readiness | SQL observation |
| --- | --- | --- | --- |
| Temporary init server, about 1.074 s after inspection began | Exit 0 | Exit 2 | `listen_addresses` was empty |
| Final server, about 4.088 s | Exit 0 | Exit 0 | PostgreSQL `16.13 (Debian 16.13-1.pgdg13+1)`, `listen_addresses=*`; immediate socket `SELECT 1` succeeded |

The exact pinned image's entrypoint was inspected and contains the temporary-server override `-c listen_addresses=''`. Both initdb marker results were observed. This demonstrates that the new TCP gate rejects the temporary phase and admits the final server while retaining the harness's subsequent socket-based SQL. The upstream [official image entrypoint](https://github.com/docker-library/postgres/blob/master/docker-entrypoint.sh) documents the same initialization structure; the digest-bound local observation above is the controlling evidence.

Cleanup passed: forced container removal returned zero; a subsequent container inspection returned nonzero; the temporary fixture directory no longer exists. Only the two review evidence files were written. No application implementation edits, AWS calls or credential reads occurred.

This is a local test-fixture startup repair. It does not change or qualify production SQL, the application image, managed services or infrastructure. The deliberate sleep widened the race window rather than reproducing CI timing exactly. Root is separately running the full migration regression, and repaired x64 CI remains necessary; this local ARM64 check does not claim that CI has passed.
