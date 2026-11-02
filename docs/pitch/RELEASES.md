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
| 2026-11-01 20:13:41 | `711d87d` | `beta/20261101-711d87d` | `1608eea9` | TD:13 | 18 | `sha256:6d3f93f1ebd71c673a793faac8350efec00aef6f6e7ac6bcad86ed8369591362` | `sha256:abba554d213803d31abb1d44f7d31a0ff7803324949e49fa79e982c20ef29886` | `sha256:5b02040c4b359aeb405ced0b3f14b5644b33b131b3f98e9a4d797b0b657ab2cf` | via `just deploy` |
| 2026-11-01 21:53:16 | `1d88470` | `beta/20261101-1d88470` | `a58e69cd` | TD:14 | 18 | `sha256:e86e338bd55eaf50f3244a727ec1f86bf6e88f76dd75d8d8fb1b7e6d6088d8d6` | `sha256:89c833dddc68575e19595b1b0f785875de4f4e7f5f04a3b0545df5e7798e6141` | `sha256:7756df219fe58c6b69e8108ef2f990817143105a94d24f85b3cf6f3f15396b56` | via `just deploy` |
| 2026-11-02 00:40:47 | `9ee4a01` | `beta/20261102-9ee4a01` | `f4449af8` | TD:15 | 19 | `sha256:b75f5b98da630e771df2703c91f1a4c7edf38c13923de83b0452c9d89103bfd4` | `sha256:91d2ca888db7a05e5a824c78845e3a38fc69605d8203d9c176838025a31b93bf` | `sha256:85ec6029eb0a27a9b29cb59351afda02fdb9926c45a2d95254f7162ccc950bd4` | via `just deploy` |
| 2026-11-02 03:29:22 | `7c3ab12` | `beta/20261102-7c3ab12` | `5b239cd0` | TD:16 | 19 | `sha256:8b469c07c5501b2c86d49287ae640e0bb45ebe791f325ca4ad920914b94c191f` | `sha256:c3df74ce5d0ec7b9a3ffe8a67c7c4f2c7ec883dff1391dab586d2cb11709990e` | `sha256:d93266735e2ae8f8f091767efe277e87ec336238b18e1d46bfa8b3e488498e15` | via `just deploy` |
