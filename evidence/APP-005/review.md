# APP-005 independent review

Reviewed revision: `57caf8ab51c1ebc64da1c7c86a80e347dd8149f2` (`phase-one-spikes`).

**Verdict: no blocking findings in the declared synthetic qualification scope.** One P2 queue-deadline finding was reproduced, repaired and independently rechecked before this revision. This verdict covers APP-005 source, workflow, ticket and evidence; it does not independently review the unrelated GateWASM teardown records included in the same commit, or complete the managed application/phase gates.

## Inspected scope and evidence

Read the ticket, repository/contribution/review instructions, source-precedence guide, delivered platform §§5.2/6.3/13.2 and D4/N3, shared query/receipt contracts, P06 validation and S02 arithmetic. Inspected the compiler, fixture generator, worker, runner, Dockerfile, hash locks, CI workflow and retained failures. No implementation source was edited by this reviewer.

- The compiler accepts a bounded subset of the APP-002 typed grammar. Identifiers and operation tags are closed, literal values are bound, unsupported lookup/ratio/multi-input shapes fail, unavailable projected fields fail, and pagination cannot precede totals. Exact integer encoding, half-open time boundaries and independent Python fixture oracles support the reported numbers.
- Source selection resolves two trusted synthetic revision names, refuses symlinks/out-of-scope paths and checks actual byte digests. Each operation receives a private read-only database connection, two engine threads, 1 GiB engine memory and a 2 GiB spill limit; settings lock before any query runs.
- DuckDB's effective private database/WAL/spill allowlist additions are explicitly inspected, rather than falsely asserted absent. Sixteen direct-engine controls report configuration/read-only permission failures, including an existing image canary, and a legitimate query subsequently succeeds. The private scratch exception does not expose another organization directory.
- The runner copies a curated build context and mounts only the generated organization fixture, read-only. It does not mount the repository, fixture parent tree, home or credentials. The image's two outside-source canaries are generated synthetic controls. Host inspection and the inside-container checks verify no network, UID 65532, dropped capabilities, no new privileges, read-only root, 2 CPUs, 3 GiB total memory/no additional swap, 64 PIDs and bounded non-executable scratch.
- `container-reviewed` is the controlling campaign. Every recorded source hash matches the reviewed source, including the APP-002 implementation-lock identity. Its derived image is `sha256:f27c9156487751131767f98a0de3b2eebe854d349a1880eb6b9bb49cf382b138`; it exited zero without OOM. All nine check groups, five cold samples and five supervised six-view bursts passed. Earlier failed/intermediate campaigns remain labeled historical.

## Resolved P2 finding: include queue wait in the deadline

Before the repair, `Admission.submit` did not capture enqueue time and `supervised_query` created its default timer only when an executor slot opened. An independent control held both slots, queued the ordinary default call, waited 2.05 seconds, then released it. The query incorrectly returned `1140000` after 2.340762 seconds while reporting only 0.280108 seconds. Explicit starts in the warm benchmark had hidden the default-path defect.

The reviewed implementation stamps the earlier of caller origin and enqueue time in `query.py`'s `Admission.submit`, checks expiry before invoking the worker, and releases slots in `finally`. The added `queued_deadline_probe` checks no worker launch and subsequent recovery. The refreshed capacity trial records 22 admitted / 2 rejected, with 16 correct completed results and 6 deadline expirations; it does not describe queue admission as successful completion.

Independent recheck after the fix: the same held-queue control returned `Deadline` with zero expired-worker launches; the next query returned `1140000`, and scratch was empty (2.386082 seconds including the deliberate queue hold and recovery). An initial reviewer recheck sampled the active count before both held controls had joined; synchronizing those controls corrected that test-harness race, without any further application change.

## Additional focused checks run

Using `spikes/APP-005/.venv/bin/python` with inline reviewer controls, without rerunning the full campaign:

- Exercised the actual `supervised_query` backstop with its child launch replaced by a controlled sleeping Python child. The real supervision path killed and reaped it at 5.005403 seconds, returned `Deadline`, and removed its scratch. This supplements the retained forced-process control with a direct check of the ordinary supervisor function.
- Queried identical S02 data at filter epochs 1 and 2: rows stayed `1140000`, receipt digests differed. An SQL-looking bound literal returned the expected empty aggregate `null`, with no partial-result claim and no scratch leak.
- Recomputed every `container-reviewed/source-identity.json` hash; all matched. Inspected final Docker settings/results and verified `git diff --check` passed.

The existing heavy-query trial observes the actual interrupt and cleanup at 1.871 seconds and the child plus recovery at 2.095 seconds. Documentation correctly separates those observations from the 1.8-second interrupt request/result cutoff and five-second kill backstop.

## Interpretation and remaining gates

The measured ranges are 26.6–31.1 ms for the local engine path, 181–191 ms for the fresh-process cold sample, and 603–644 ms for six declared analytical views. Five samples are diagnostic, not p99 evidence. Cold means fresh process/engine with a local immutable revision; OS caches are not reset and digest validation itself reads the file.

There is no result cache, real-principal authorization, PostgreSQL record snapshot, S3/KMS cold fetch, browser delivery or full CIDR/Northstar workload here. Receipt identities are valid synthetic-preview artifacts, not proof of live grants or cache reauthorization. The runbook/ticket keep those application obligations and full managed N3 open. Unsupported shapes remain refused; the design's future receipt-preserving preaggregation fallback is not claimed implemented or measured.

The implementation uses the published, hash-locked DuckDB 2.0.0 and pinned Python base image. Primary [DuckDB configuration](https://duckdb.org/docs/2.0/configuration/overview), [security guidance](https://duckdb.org/docs/2.0/operations_manual/securing_duckdb/overview), and [Python API](https://duckdb.org/docs/2.0/clients/python/reference/) were checked during review. Engine settings remain defense in depth; the operating-system boundary is necessary.

## CI state at review

For the exact reviewed revision, [Planning integrity](https://github.com/getforma-dev/forma-app-platform/actions/runs/40756678200) passed. [Bounded query qualification](https://github.com/getforma-dev/forma-app-platform/actions/runs/40756678148), [Contract qualification](https://github.com/getforma-dev/forma-app-platform/actions/runs/40756678199) and [Released baseline](https://github.com/getforma-dev/forma-app-platform/actions/runs/40756678059) were still in progress when inspected. This review does not claim those runs passed. The lead must record their results and satisfy landing rules before moving APP-005 to Done.
