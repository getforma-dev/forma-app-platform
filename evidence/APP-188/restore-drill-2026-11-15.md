# APP-188: restore drill, 2026-11-15

Runbook [§11.2](../../docs/operations/TRIAL-RUNBOOK.md#112-the-restore-drill-the-operator-may-run-it-a-few-cents-of-rds), run by the operator (Claude) under Victor's operator permission, from `main` at `9f8a3883`, 08:33–08:48 UTC, outside the 05:00–05:45 UTC backup window, with no one signed in.

| Step | Result |
| --- | --- |
| Snapshot | `forma-restore-drill-20261115-snap`, `creating` 08:33:44, `available` 08:36:46 |
| Restore | `forma-restore-drill-20261115`, the live instance's class (`db.t4g.small`), subnet group, security group and parameter group; `available` 08:45:54 |
| Stats on the live database | task `361cf4613c5c45f9be27df75ff2a960f`, `EssentialContainerExited`, exit 0 |
| Stats on the copy (`PGHOST` override) | task `12ed00978991452a8262f18948bd7e4d`, `EssentialContainerExited`, exit 0 |
| Compare | `schema_version` 24, `audit_events` 200, five audit heads; **diff: identical** (`restore drill: identical`) |
| Clean-up | copy `deleting` then deleted, drill snapshot `deleted`; no `forma-restore-drill*` instance left; the manual snapshots are only the two `forma-schema-release-*` (v22, v24) |

The stats files, receipts and request files are in the operator's private trial folder. This record closes APP-188's drill item and the go/no-go's "restore drill's diff empty". The 2026-11-14 backup readback (APP-188 log) stays as the earlier, weaker evidence.
