# APP-005 qualified query evidence

Date: 2026-10-28. The final local Docker/Linux ARM64 qualification is [container-reviewed/results.json](container-reviewed/results.json), with effective [container controls](container-reviewed/container-state.json), [source/image identity](container-reviewed/source-identity.json), and a compact [summary](summary.json). The image exited successfully, was removed by the runner and was not deployed.

**Outcome:** the bounded single-input typed-query path passes its synthetic correctness, refusal, confinement, admission and cleanup checks. This qualifies the declared spike scope, not full Northstar/CIDR behavior or the managed application latency target. [The runbook](../../spikes/APP-005/README.md) states the supported subset and every remaining cost/behavior boundary.

Five fresh engine samples over **1,000,000 rows** returned the independently computed **500,500,000-byte** total. The measured engine path was **26.6–31.1 ms**, with **181–191 ms** fresh-process wall time. Five six-view bursts using separate supervised processes, with two active slots, took **603–644 ms**. These six views are total, count, destination totals, minute totals, minimum and maximum. They are diagnostic ranges, not p99 estimates and not a substitute for the full application six-view workload.

All nine check groups passed:

- Container identity, read-only root, dropped capabilities, no new privileges, blocked network, no cloud credentials, and host-inspected CPU/memory/PID/tmpfs limits.
- Closed typed input, identifier/source path/URL and SQL-looking literal controls; unsupported CIDR and unbounded output fail explicitly.
- Sixteen direct-engine hostile file/URL/extension/configuration controls, followed by a successful legitimate query. Effective settings include only the authorized revision and DuckDB's automatically added private database/WAL/spill paths.
- Two running/twenty waiting admission, 22 accepted and two rejected requests under a held control: 16 completed with correct results, six expired without releasing results, and subsequent admission succeeded. Acceptance into a queue is not a promise that work will finish before its deadline.
- A query held in the queue for 2.05 seconds expired before its worker launched; all slots recovered and the next query succeeded.
- An actual interrupt request at the 1.8-second deadline, with discarded result, process exit, scratch reclamation and successful recovery query. The observed engine interruption plus cleanup took **1.871 seconds**; the whole child process including recovery took **2.095 seconds**. These cleanup timings are distinct from the result-delivery deadline. An independent five-second worker-process backstop remains available.
- Real observed engine spill under a tighter 16 MiB diagnostic, bounded memory refusal or interruption/completion, and complete scratch removal. Ordinary query policy remains 1 GiB.
- A deliberately nonresponsive process was killed and reaped by the supervisor control.
- S02's 1,140,000 total, 900,000 selected-destination total, five included rows, half-open upper-boundary exclusion, timeline, page truncation state and stable/changed receipt identities.

The container ran with two CPUs, 3 GiB memory and no additional swap, 64 PIDs, a 2 GiB non-executable tmpfs and only the synthetic organization revision directory mounted read-only. Each operation uses a distinct read-only DuckDB database and 1 GiB engine/2 GiB spill limits. DuckDB settings are defense in depth; the container is the operating-system boundary. A synthetic canary in the test image exists solely to prove direct-engine outside-source refusal; no real organization data or credentials enter the image.

## Retained failures and dispositions

- Initial trusted connection setup tried setting runtime-only `enable_logging`/allowlist controls at connection construction. DuckDB 2.0 refused them. They now run in the required trusted initialization order, before external access is disabled and configuration is locked. No untrusted query executes in that interval.
- [local-initial.json](local-initial.json) retained an observability failure: enumerating installed extensions tries to read the extension directory and is correctly denied by the locked filesystem policy. The qualifier now verifies INSTALL/LOAD refusal directly instead of weakening the allowlist for enumeration.
- [container-initial/failure.txt](container-initial/failure.txt) records an image-layout bug: Python eagerly evaluated a default repository-relative path even though the container supplied the contract path. Explicit conditional path selection fixed it.
- [container-second/results.json](container-second/results.json) retained a failed assertion that `allowed_directories` stayed empty. DuckDB automatically authorizes its private spill directory and database/WAL files. The final assertion checks the effective scope is precisely those per-query resources plus the selected Parquet file; it does not broaden access.
- The first 16 MiB spill diagnostic reached its memory limit after creating real spill files. Memory refusal is an expected safe outcome for this deliberately tighter resource control. The final test requires nonzero observed spill, no released partial number and removed scratch; it does not raise the ordinary limit to make the control complete.
- [container-final/results.json](container-final/results.json) was an earlier passing campaign before ordinary scheduled queries gained independent process supervision. Its faster warm timings are retained historical evidence only. It includes process startup in measured warm bursts, but it is not the final evidence.
- Independent review then found a queue-deadline defect: the generic submission path started its deadline only after receiving an executor slot. A held default submission returned successfully after more than 2.3 seconds while reporting only its execution time; the explicit burst start in `container-qualified` had masked this. Admission now stamps enqueue time, enforces it before worker launch and propagates it to the operation. The new queued-expiry regression and refreshed **container-reviewed** campaign pass; that directory is the current evidence.

## Limits and follow-through

The cold source is already local; OS caches are not flushed, and source digest validation reads the file. Authorization is a synthetic trusted fixture context, with no RDS round trips. S3/KMS fetch/decryption, network latency, actual browser delivery, CIDR/multi-input operations and complete application receipts over a real record snapshot remain unmeasured. APP-012 and the later journey/capacity tickets must validate those actual costs and shapes. No full N3 gate is closed here, no preaggregation speedup is asserted, and no deadline/limit was raised based on these measurements.

The current disposition is to retain the measured single-input limits and reject unsupported shapes. If the managed workload misses its target, reduce admitted workload or propose versioned receipt-preserving preaggregation with independent measurements. Previous results and source revisions must remain reproducible.

Exact DuckDB release/license/wheel hashes are retained in [dependency.json](dependency.json); the reproducible harness locks every package and its base image. Focused CI is configured in `.github/workflows/query.yml`; this local report does not claim the GitHub run passed. Root review/CI evidence is recorded separately when available.
