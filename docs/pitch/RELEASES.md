# Beta releases

The deploy history of beta.getforma.dev (the `GenuiSyntheticFoundation` stack), one row per activated window ([ADR-013](../architecture/ADR-013-REPOSITORY-AND-DELIVERY.md) D13.5). From the first window run through `just deploy`, `just deploy activate` appends the row itself, after the target is healthy, and tags the candidate `beta/<yyyymmdd>-<sha7>` (D13.2); the row is then committed on `pivot/pitch-mvp`. The full record of each window, with the approvals it ran under, is the private `$PRIV/windows/<window-id>/release.json` ([DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md), "The private directory"; schema in `ops/beta/release.py`).

Non-secret facts only: the candidate commit, the window by its first 8 hex, the task definition by its revision number, the database schema version and the three image digests (service `genui-synthetic`, `genui-qualification`, `genui-operator`). Never an account id, ARN, secret or personal name.

## History before `just deploy` (reconstructed)

Rebuilt on 2026-10-31 from the tracked record only: the APP-078 window record (`evidence/APP-078/window/README.md`), the APP-076 trial (`evidence/APP-076/trial/README.md`), and the beta logs of APP-096, APP-100, APP-105, APP-113 and APP-117. Times are UTC. Where a log names only the day, the time column holds the day. The digests of the pitch windows were kept only in the other clone's private directory, so they show as "not recorded in Git". A value marked *(inferred)* is not stated in a log but follows from the migrations in the candidate's tree. No tags were made for these windows.

| Activated (UTC) | Candidate | Window | Task definition | Schema | Service digest | Qualification digest | Operator digest | What shipped; notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-30 11:15 | `aa220b3` | `88d1b19d` | TD:4 | 7 | `sha256:90d12ba89969bf4781b71fe6321951e27babbfb14e5ac8210411cab4f5e80ff4` | `sha256:1b3a128aea3aa93d64f3505abd82e143af9bfba8bc46109b024b69868bb6cb99` | `sha256:ed92d54b044b3909815e6bfa8e73f7f524076dbd817d73306f1238a5568acca6` | APP-078 go-live (Phase 1, managed RDS), the APP-076 invite-first trial. The 11:15 is the second change set's `UPDATE_COMPLETE`. The first change set rolled back over ECS AZ rebalancing, which `aa220b3` turns off. The `559b11b` images had 10 CRITICAL CVEs and were rebuilt (`2679102`, `aa220b3`). The deny-all `RollbackTaskDefinition:2` was drilled for 3 minutes and then restored. |
| 2026-10-30 | `dceca31` | `10e6ecc5` | TD:5 | 10 | not recorded in Git | not recorded in Git | not recorded in Git | First pitch build, `pitch-dceca31-arm64` (APP-096, ADR-011; P1: Northstar is the demo organization). Activation was refused first and fixed in `287ad5a`. Two secret-only redeploys stayed on TD:5: the composer key (APP-096) and the Slack token and signing secret (APP-100). |
| 2026-10-30 | `de88266` | `3b63b1f9` | TD:6 | 10 *(inferred)* | not recorded in Git | not recorded in Git | not recorded in Git | Library pane (APP-104), `pitch-de88266-arm64`. |
| 2026-10-30 | `0c3261b` | `5742ecc3` | TD:7 | 10 *(inferred)* | not recorded in Git | not recorded in Git | not recorded in Git | Focus and unite (APP-106), workflows (APP-107), notes (APP-109); library seeded with 20 apps. |
| 2026-10-30 | `422d332` | `34e746fa` | TD:8 | 11 | not recorded in Git | not recorded in Git | not recorded in Git | Folders (APP-108). A pre-migration snapshot was taken at schema 10. |
| 2026-10-30 | `11857ba` | `65d9cebd` | TD:9 | 12 | not recorded in Git | not recorded in Git | not recorded in Git | APP-113 review fixes (APP-105 log). The operator role lost its composer and Slack secret access. |
| 2026-10-31 | `6f060a0` | `b0c6ebda` | TD:10 | 13 | not recorded in Git | not recorded in Git | not recorded in Git | Canvas 503 hotfix (long deadline, paired reads, timing log), APP-115 governance quick wins. |
| 2026-10-31 | `ab19994` | not recorded in Git | TD:11 | 13 *(inferred)* | not recorded in Git | not recorded in Git | not recorded in Git | Batched canvas reads, the `/signin/northstar` alias (APP-117), polish. Found live: an existence leak on unknown aliases. |
| 2026-10-31 | `3901992` | not recorded in Git | TD:12 | 13 *(inferred)* | not recorded in Git | not recorded in Git | not recorded in Git | APP-117 alias-leak fix. The last deploy in the record. |

Each window superseded the one in the row above it (the recorded chain `88d1b19d` → `10e6ecc5` → `3b63b1f9` → `5742ecc3` → `34e746fa`, then `65d9cebd` and `b0c6ebda` per APP-105). Through 2026-10-31 there were eight pitch activations, TD:5 to TD:12. The runbook's recorded-state summary now points here; its observe step must establish the live state before another deployment.

## Releases through `just deploy`

| Activated (UTC) | Candidate | Tag | Window | Task definition | Schema | Service digest | Qualification digest | Operator digest | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
