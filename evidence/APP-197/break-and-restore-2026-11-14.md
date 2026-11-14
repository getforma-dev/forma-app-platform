# APP-197: break-and-restore proofs (2026-11-14)

By Claude (Opus 5.5), on branch `trial/app-197-catalog-cadence`. Each guard below was broken in `app/src/storage/pool.rs`, and `just campaign postgres` (the APP-008 native campaign) was run and failed. The file was then restored, and its SHA-256 equalled the committed `pool.rs` of `e22027dd`: `5dcc24d084215b64a21b83057de89ad6874e1b92b950cc6262cc87766b5b07ae`. After the restores the campaign passed: `test result: ok. 1 passed` (46.3 s), 37 checks.

The implementation came first; the qualification was written after it, and these breaks show that it catches each rule.

| Guard | Break | Where the campaign failed |
| --- | --- | --- |
| A5-1 The window exists: no comparison on a reuse inside 30 s | `CATALOG_RECHECK` set to 0 s | The A5 check's first reuse under catalog drift was refused (`Lease::acquire(...).unwrap()` on `Unavailable`): the drift was no longer tolerated inside the window |
| A5-2 A reuse after the window compares again | the recheck condition forced to `false` | The metadata-view check: the reused connections ran the trap view, and the marker counted `4` instead of `0` |
| A5-3 A new backend is compared at creation | `connect` ran the checks without the comparison | The metadata-view check: the new connections ran the trap view (marker `12` instead of `0`) |
| A5-4 Readiness always compares | readiness ran the checks without the comparison | The canonical-catalog check: `assertion failed: empty.readiness().await.is_err()` (disabled RLS was not refused) |
| A5-5 Identity on every reuse | `verify_identity` only when comparing | The A5 check: a role with `CREATEDB` was admitted on reuse inside the window |
| A5-6 The schema version on every reuse | `verify_schema_version` only when comparing | The A5 check: a reused connection was admitted with the version row set to 1 |
| A5-7 The session baseline on every reuse | the `verify_baseline` call removed from `recycle` | The A5 check: `assertion left != right failed: the poisoned connection is never reused` (a leftover organization scope was reused) |

Notes:

- The first version of the baseline sub-check poisoned the idle connection's `search_path`. `verify_identity` also checks `search_path`, so that version did not isolate the baseline rule: with A5-7 applied, the campaign failed only later, at the temporary-schema check (`Deadline` at readiness). The sub-check now leaves an organization scope behind, which only the baseline check sees. A5-7 was re-run against it, with the result above.
- A5-1 to A5-6 were run before that sub-check changed. Their failures are at assertions that the change did not touch, but they were not re-run against the final test file.
- "A failed recheck discards the connection" was not broken on its own: deadpool discards any object whose `recycle` returns an error, and the A5 check's count (two comparisons for a refused reuse: the reused connection, then the new backend the pool creates in its place) shows the discard.
- The harness was a scratch script outside the repository. It applied each break, ran the campaign, wrote the original bytes back and compared digests.
