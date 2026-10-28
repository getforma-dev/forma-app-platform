# APP-005 owned-broker independent review

Reviewed revision: `0595392e1d1c78a45c2de1815a503e33b5d5932d` (the broker replacement in `ea5d27c`, followed by the absolute frame-deadline repair). This reviewer did not edit APP-005 implementation files.

**Verdict: the prior blocking process-start finding is resolved; no additional blocking finding in this delta.** Exact-revision CI remains a separate completion requirement. This verdict approves the bounded query spike, not a managed application deployment or a phase gate. The original failing review and reproduction remain in [review-forkserver.md](review-forkserver.md).

## Inspected boundary

The controller now establishes an explicitly owned, data-free Linux broker before admission. Only that single-threaded broker forks; each query runs in a fresh child and creates its own engine. A private request file contains query arguments, while the broker receives only a constrained scratch-directory name. No connection, query or organization data is loaded in the preloader.

The five-second deadline starts before request serialization and transport. Connection, request send and response reads use the remaining absolute time; the broker additionally gives each request frame one absolute 100-ms budget. Worker results remain regular, non-symlink, byte-bounded JSON files read only after the broker acknowledges reaping that child. A final elapsed-time fence withholds late results; the application’s existing 1.8-second enqueue-based cutoff is unchanged.

On timeout or broken transport, one retirement owner kills the broker’s process group, waits for the broker and reaps adopted descendants before scratch cleanup. Other active callers join that bounded cleanup. Readiness must explicitly create a new broker; requests cannot silently restart one. Each forked child sets a parent-death signal and checks the parent race. The code uses group-specific `waitpid`, rather than reaping unrelated application children. This closes the original late-launch path; a detached starter thread would not have done so.

## Independent checks actually run

Recomputed all 11 artifacts in `container-broker-final/source-identity.json` against Git objects at the reviewed revision. They match, including the contract lock. Also verified all eight source/dependency hashes used by the independent focused run against that revision.

Ran the exact final `test_processes.py`, `processes.py`, `query.py`, fixture module and contract implementation in a separately built pinned Python 3.11.14/Linux image, with the real locked DuckDB dependency and no query-module stub. The temporary container had no network, a read-only root, UID65532, no capabilities/new privileges, two CPUs, a 1-GiB memory cap and private bounded temporary storage. All **12 supervision controls passed**; the container exited zero without OOM, and its container/image were removed. This focused run exercises process ownership and recovery, not analytical performance. [Runnable harness](review-broker-probe.py), [captured controls and source hashes](review-broker-controls.json).

- A stopped broker before acceptance is killed/reaped in **5.016 s**, with no delayed launch and successful explicit restart.
- A stopped broker plus stopped live child with two admitted callers reclaims both processes, scratch and slots; both callers fail closed and subsequent work succeeds.
- An ordinary stuck child is killed/reaped in **5.015 s**.
- Direct parent-death testing terminates the child before group cleanup, then confirms the adopted child is reaped.
- Trickle traffic cannot extend the request-frame deadline: the connection closes in **0.122 s**. Partial, malformed and oversized request/response frames are bounded.
- FIFO, symlink and oversized result files are refused with healthy recovery. A child’s nonzero exit is retained as a sanitized fixed exit code.

Read the complete actual-engine `container-broker-final` report: eleven groups, five cold samples and five six-view warm bursts pass. The warm bursts take **165.7–184.4 ms**; all 22 admitted requests return correct results, two excess requests are refused, and admission recovers. The full campaign’s stopped-broker and ordinary-child controls take **5.024 s** and **5.019 s** respectively. Its source hashes bind those results to this revision. The campaign’s three-GiB query container and the independent one-GiB process-only container are distinct qualification profiles.

## Limits retained

The broker is Linux-only. Startup/readiness is reported separately from warm requests; the cold fresh-interpreter path remains unchanged. Five samples are not p99 evidence. The full Northstar CIDR/six-view workload, managed authorization/storage/network overhead, production availability, and cloud acceptance remain outside this spike. Historical deadline misses and the failed fork-server-start control are retained rather than reclassified as passes. CI status is recorded separately in `ci.json`; this review does not infer an unseen CI result.
