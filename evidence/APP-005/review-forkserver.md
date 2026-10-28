# APP-005 fork-server delta review

Reviewed revision: `315c2929dfc5f72c45e47df6313f406f06dc7ab3`, relative to the earlier independently reviewed `57caf8a` candidate. This reviewer did not edit APP-005 implementation files.

**Verdict at this revision: one blocking P2 recovery finding.** The new normal campaigns and all four GitHub CI checks pass, but a separate launcher-stall fault control shows the five-second supervision boundary does not cover `Process.start()`. A repair and independent recheck are in progress; passing the prior campaign does not close this finding.

## Inspected delta

Read `processes.py`, the worker/runner/Dockerfile changes, query admission/deadline/engine setup, updated documentation, retained failed CI sample, and the replacement local and Linux CI evidence. The cold interpreter path remains unchanged. The warm path preloads trusted modules in a separate fork server before admission threads and database operations; query children are fresh and create/revalidate their own engine/source/query. Both measured environments reported one native fork-server thread. Source inspection found no top-level query, connection or organization-data load in the preloaded modules.

The process result path is byte-bounded JSON in private scratch, opened only after child exit with `O_NOFOLLOW | O_NONBLOCK`, checked as a regular file, capped at 1 MiB, and deleted during cleanup. This avoids a partial framed-pipe read extending the backstop. The 1.8-second enqueue-based cutoff and late-result withholding remain intact; capacity remains two running plus twenty queued.

[Python's process-start documentation](https://docs.python.org/3.11/library/multiprocessing.html#contexts-and-start-methods) supports the separate single-threaded fork-server mechanism. The measured startup is properly separated from warm admission and does not get relabeled as cold runtime performance.

## Blocking P2: process startup is outside killable supervision

At this revision, `processes.py` calls `process.start()` before `process.join(timeout=5)`. Starting a fork-server process includes synchronous IPC with the shared fork server. If that server stops responding, `start()` itself can block indefinitely; neither the five-second join timer nor the later 1.8-second result fence regains control. Admission slots can therefore remain occupied indefinitely without a query result. This is a recovery-boundary failure, not a claim that a late numeric result is returned to the application.

The independent [fault-control script](review-forkserver-probe.py) uses the exact reviewed supervisor and the pinned Python 3.11.14 image. It stubs only query imports, because no database query is needed to test process supervision. In an isolated, non-root/no-network/read-only container it warms a child, stops **its own** fork-server PID, and starts the next ordinary supervision call. The call remained blocked after **6.203098 seconds**, beyond the advertised five-second backstop. Resuming that owned server then caused the delayed child to launch and return successfully. The script resumes the server, joins the call, removes the owned container and deletes its temporary input directory. [Captured result](review-forkserver-start-stall.json).

A detached thread around `start()` alone would not close this finding: it could finish later and launch a child after timeout cleanup. The repair must own the entire start/run/response lifecycle within a killable boundary, prevent delayed child escape, reclaim scratch/admission, and prove subsequent healthy operation. A focused stalled-server regression plus a fresh complete campaign and exact-revision CI are required before acceptance.

## Successful evidence at the reviewed revision

Recomputed every `source-identity.json` artifact hash against Git objects at `315c292`; both `container-forkserver-reviewed` and `ci-forkserver` match, including the locked contracts. [Review evidence summary](review-forkserver-evidence.json).

- Local: ten groups, five cold samples and five six-view bursts pass; warm bursts **161.9–179.4 ms**, readiness **211.5 ms**, one native server thread. The ordinary already-started-child kill/reap control takes **5.028 s**. All 22 admitted load requests complete correctly, with two excess refusals, and subsequent admission succeeds.
- Linux x86 CI: the same groups and samples pass; warm bursts **219.4–224.4 ms**, readiness **264.8 ms**, one native server thread. The already-started-child control takes **5.010 s**. All 22 admitted requests complete correctly. The container exits 0 without OOM.
- The original Linux CI miss at **1.811 s** remains retained; the late result was withheld. The replacement does not raise the deadline or silently erase that failure.

All four checks for `315c292` are recorded as successful: [bounded query qualification](https://github.com/getforma-dev/forma-app-platform/actions/runs/40758758986), [contracts](https://github.com/getforma-dev/forma-app-platform/actions/runs/40758758764), [released baseline](https://github.com/getforma-dev/forma-app-platform/actions/runs/40758758830), and [planning](https://github.com/getforma-dev/forma-app-platform/actions/runs/40758758833). The downloaded query artifact agrees with the retained CI source hashes and results. These checks did not exercise a stalled fork-server start; the new independent fault result takes precedence for that recovery claim.

The original review remains historical in `review.md`. This delta review does not reclassify five samples as p99 data, qualify managed authorization/S3/KMS/network overhead, qualify the full Northstar CIDR/six-view workload, or complete any phase/cloud gate.
