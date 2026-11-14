# Promote to beta

[APP-167](../../Tickets/4-done/APP-167.md) records the implementation and the sealed live acceptance. [ADR-017](../architecture/ADR-017-BETA-PROMOTION.md) records the decision. A configured environment or successful local test alone does not establish an operational pipeline; see the ticket's recorded live run before relying on it.

## Normal release

Merge a reviewed application pull request into `main`, the product trunk ([ADR-019](../architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)). The promotion workflow checks the exact merged commit's required push CI, builds and qualifies the service, qualification and operator images on ARM64, and retains those exact image bytes. PR builds have no AWS deployment authority.

The preparation job publishes the qualified images and prepares a runtime-only AWS change set. Its summary identifies the candidate, image digests and prepared release packet. Review that concrete packet, then approve **Promote to beta** in the run's **Review deployments** control. This is the one deployment approval. Victor can approve a run triggered by his own merge; administrator bypass is disabled.

Execution revalidates the approved packet, stops both existing services, proves they are stopped, applies the reviewed change set, and starts and verifies the new runtime. Expect a brief outage and existing sessions to sign out during the restart. A successful merge or image build is not evidence that beta deployed: the promotion must finish its live verification successfully.

The beta address stays [beta.getforma.dev](https://beta.getforma.dev/). Existing workspaces, trial data, accounts and invitation destinations are retained.

## Initial scope

The automated path covers compatible application and UI updates, with the existing bounded shutdown expiry. It refuses changes to the pinned infrastructure, migration and deployment contract, except for the one reviewed [schema release](#schema-releases) committed in source. Any other such change calls for a separately reviewed operator delivery; it must not be bypassed by editing a checksum or pretending that a migration is unnecessary. `main` remains governed by the existing repository landing decision.

Documentation-only changes need no runtime release. A superseded candidate, failed required check, wrong source/artifact, changed predecessor or unresolved earlier deployment must stop promotion rather than deploy an unverified mixture.

New image scan findings stop preparation unless an exact source-reviewed disposition applies. A disposition has one entry per image, each bound to that image's live predecessor manifest and its complete finding fingerprints, plus the deployment contract and an expiry; it is rechecked before execution. The accepted CVE names, severities, packages, roles and expiry are also pinned in `ops/promotion/registry.py`, so a JSON edit alone cannot widen them, and a CRITICAL finding among those read at preparation is never accepted. The approval summary reports introduced, reviewed and unreviewed counts, CVEs and the disposition hash/expiry. The baseline and raw findings are never rewritten or suppressed.

The scanner is read once, during preparation. Execution and its final checks recompute the delta from the findings in the packet and recheck the disposition file, its contract and predecessor binding and its expiry; they do not read the scanner again. A finding the scanner first reports after preparation, a CRITICAL included, is therefore promoted if the packet is approved: the approval binds the scan the packet shows ([APP-196 review](../../evidence/APP-196/review-2026-11-14.md), MINOR-2, decided: document it rather than re-read). Approve within hours of preparation, and never approve a packet whose preparation is more than a day old: abandon it instead, as for a [superseded release](#superseding-a-prepared-release), and merge again so that a fresh preparation reads the scanner.

The current disposition, [APP-175](../../evidence/APP-175/SCAN-DISPOSITION.md), was written for release A, the schema release that went live on November 14. Its source pin holds 16 findings after the 2026-11-14 renewal: the 14 Debian findings the scanner began reporting on November 12 for the unchanged images of that time (eleven in krb5, two in glibc and one in gcc-12), plus the re-vectored zlib CVE-2026-76844 and the dpkg CVE-2026-91094. Seven are HIGH and none CRITICAL. It is bound to release A's `to` contract and to the three predecessor images that release A replaced, so it has been inert since release A went live: every predecessor digest it names differs from the live registry, and it can admit nothing until APP-194 replaces the pin and the file. It also expires on November 20. Release A's findings are now ordinary baseline findings. It replaced the [APP-173 disposition](../../evidence/APP-173/SCAN-DISPOSITION.md), whose zlib CVE-2026-76844 and apt CVE-2026-109518 became baseline findings with the November 5 release (the apt fix is tracked in APP-089). APP-173 itself had replaced the first, operator-only [APP-167 disposition](../../evidence/APP-167/SCAN-DISPOSITION.md) of three MEDIUM gcc-12 findings.

Scanner data changes without any source change. On November 5 those two CVEs appeared overnight and refused preparation after the lock was taken; the documented abandon released it and `PROMOTION_ENABLED` was set to `false` while the disposition was reviewed. The [APP-173 record](../../evidence/APP-173/SCAN-DISPOSITION.md) has the timeline. Set the variable back to `true` immediately before merging the change that should promote.

The existing budget limits and requirement for a reconciliation basis less than 24 hours old still apply. A stale basis stops preparation before the approval stage. Budget maintenance must retain fresh billing evidence and the operator's reserve assessment in the canonical state; automation must not refresh the timestamp on an old assessment. Renewal or a larger budget is not implicitly authorized by a merge. The canonical policy also sets how many hours each release subtracts from the prior shutdown expiry. The first recovery changed that default from two to six hours. After a second preparation rejection, the reviewed recovery changed six to ten hours to retain both failed attempts' allowances within the original caps. This is a persistent conservative default, not an automatic reset after one release. The policy update changes no running schedule: the next approval displays the actual proposed expiry, November 30 at 04:00 UTC. Applying or later changing the policy requires an idle, hash-bound operator review and immutable receipt.

## Schema releases

A schema release carries migrations through the normal packet and the one approval, on the same qualified images ([APP-175](../../Tickets/4-done/APP-175.md), [ADR-017 amendment](../architecture/ADR-017-BETA-PROMOTION.md#amendment-2026-11-06-reviewed-schema-releases)). It replaces the by-hand schema 20 crossing of November 4.

**When it applies.** Only when `ops/promotion/schema-release.json` exists and every one of these holds; otherwise preparation refuses before the lock exactly as before:

- the checkout being promoted is the candidate commit itself;
- its `from` schema manifest and deployment contract digests equal the canonical baseline's, and its `from` version equals the policy schema version;
- its `to` digests equal the candidate's qualified source binding, and the candidate's `app/migrations/manifest.json` has those bytes and its `database_version` is the `to` version;
- the candidate's CDK source (`infra/src/*` and the four `infra/` project files) has the reviewed infrastructure digest, so the stack is unchanged;
- the current time is before its expiry.

Every value in the file is also pinned in `ops/promotion/schema_release.py`: the evidence path, both versions (20 and 22), all four digests, the infrastructure digest and the expiry (2026-11-20 00:00 UTC). A JSON-only edit therefore can neither widen nor retarget it; malformed, duplicated or retyped fields refuse. A unit test binds the committed file's `to` digests and infrastructure digest to `HEAD`, so CI fails if the tree drifts from the reviewed release. A promotion whose schema manifest and contract equal the baseline never reads the file. New scan findings in a schema release need a source-reviewed disposition exactly as in a normal promotion, but bound to the contract the release promotes (its `to` contract, which the approval summary shows), not the baseline's it replaces. Without one, preparation refuses before any change set. Every scan check (preparation, the final checks and execution) binds that same contract. The current release is recorded in [evidence/APP-175](../../evidence/APP-175/SCHEMA-RELEASE.md).

**What the approval shows.** The summary's `schema_release` is `{from_version: 20, to_version: 22, spec_sha256}`; `spec_sha256` is the hash of the committed file's exact bytes. `schema_version` stays the current policy version (20) and `preserved.schema` is `false`. The sequence starts with `snapshot database`, before `stop both services`, and adds `run reviewed migration` between `verify stopped boundaries` and `activate both services`. Approve only if those match the reviewed release.

**When to approve.** The database's automated backup window is 05:10–05:40 UTC, and RDS may refuse a manual snapshot while that backup runs. The snapshot starts within a few minutes of approval, so do not approve between about 05:00 and 05:45 UTC. A refused snapshot leaves beta serving; the release is then abandoned with the documented command (below) and merged again.

**Execution order.** Execution rechecks the release at its start, as it does the scan disposition, then records the approval and runs the final rechecks (reservation, live boundary, the exact change set and template, trunk, scan delta, release). Then:

1. while beta still serves, create a manual RDS snapshot, `forma-schema-release-<first 8 of the promotion id>-v22`, of the instance named by the stack's `DatabaseEndpoint` output; wait at most 12 minutes for `available`; record the observation;
2. run every final recheck again, and require at least 30 minutes of the execute budget left, so the outage starts immediately after a complete recheck;
3. stop both services, prove them stopped, execute the reviewed change set (which registers the migration task definition on the candidate service image), and record the stopped boundary;
4. run the stack's `MigrationTaskDefinition` once (Fargate 1.4.0, the two public task subnets and the task security group, as the operator steps did), wait at most 12 minutes for it to stop, and require the candidate image digest, `EssentialContainerExited` and exit code 0; record the observation;
5. prove the stopped boundary again, then activate and verify as for any release;
6. on success the canonical baseline takes the candidate's schema manifest and contract digests, the policy schema version becomes 22, and the completed record names the release hash, snapshot and migration task.

The snapshot and the task start are journaled mutations with immutable receipts; the requests name no tags, though the instance copies its own tags to the snapshot. Beta keeps accepting writes after the snapshot until both services' tasks have drained after the stop call, so the snapshot misses that interval: the final rechecks plus the drain. The instance's seven-day automated backups cover it. The migration task falls within the two extra task-hours every promotion window already reserves (`extra_task_hours`, priced at the migration task's 2 vCPU and 8 GiB); snapshot storage is not in the ledger.

**Time budget.** The budget bounds the runner step, from the start of execution through the health wait. The execute job now has 75 minutes, which also cover its checkout, artifact verification, venv, CI recheck and OIDC before that step. The role session lasts one hour from just before the step. The existing per-wait bounds already add up to 80 minutes (stop 15, stack update 20, settle 15, activation 15 plus health 15), so the step carries one 55-minute budget and shortens every wait to what remains. When the budget is spent the run refuses with the lock held instead of being killed mid-call. Neither the snapshot nor the migration starts unless its whole 12-minute bound remains, and no outage starts with less than 30 minutes left. That reserve covers the typical outage (the last normal execution took about 4 minutes) plus the migration bound, not the worst case. A slow but successful stack update can leave too little time to observe the migration, which then never starts: beta stays down with the database at the release's `from` version, recoverable with `restore-after-failed-migration` below. After the health wait, outside the budget, come one boundary read (about 17 AWS calls), six public probes and two S3 writes. These take seconds in practice. Their bounds are 65 seconds per AWS call and 15 per probe, so a run in which every call hung would outlast the session with the services already activated and the lock held.

**Failure and recovery.**

- *Snapshot refused, failed or too slow (step 1), or a recheck refuses after it (step 2).* No service has been touched: beta keeps serving the previous release and the phase stays `prepared`. This is deliberately the phase of a release that never started its outage, so the documented [prepared-release abandonment](#superseding-a-prepared-release) applies after its fence. `maintenance abandon` accepts exactly one `schema-snapshot` journal entry, checked against the packet's snapshot name and the live database. A confirmed entry's receipt must be intact. An unknown outcome (a refused, denied or unanswered CreateDBSnapshot) is read back once by its deterministic name with DescribeDBSnapshots; the observation, absent or present with its status, is recorded in the journal and an immutable receipt before the release is abandoned. Any snapshot is kept and never deleted, and no other RDS call is made. Two entries, a different name or instance, or any `schema-migrate` or `stop-` entry still refuse. The workflow does not retry; a fresh merge prepares the next attempt.
- *Migration failed or never started, after the outage began (steps 3–4).* Beta stays down: both services remain stopped, the phase stays `executing`, the lock is held, and nothing activates, retries or replays. The migration runner (`runtime.py migrate`) applies every migration of the release in one transaction and commits it explicitly. Its exit code says where the database is ([APP-196](../../evidence/APP-196/fixes-2026-11-14.md)):
  - **0**: committed; the database is at the `to` version.
  - **1**: refused or failed before the COMMIT was sent; the database is still at the `from` version. Recover with the operator command below.
  - **4**, reserved: commit outcome unknown. The COMMIT may have reached the server, and something failed from then on: a lost acknowledgement (an RDS failover or a network reset, for example), a failed close or a failed completion line. The database may be at either version. Follow the next item.
  - Any other code is not the program's own and proves nothing: a signal's 128+N (137, 143), or Python's 120 for a failed flush at shutdown. Follow the next item.
- *Commit outcome unknown (exit 4), any other outcome the restore command refuses, or a failure after a successful migration.* Fix forward:
  1. **Stay on the candidate.** Do not run `restore-after-failed-migration`; it refuses these outcomes anyway. Do not change the stack, and keep the lock. After a refused migration both services are still stopped on the candidate's task definitions.
  2. **Read the schema version, read-only.** After the executed change set, the stack's `OperatorTaskDefinition` runs the candidate's operator image, which carries the candidate's migrations and catalogs. Two documented overrides in `app/operator/overrides.json` read the version:
     - `verify-catalog`, the task definition's default command, proves that the database equals the latest reviewed catalog, the `to` version. On success it exits 0 and prints `{"catalog_version": <to>, "command": "verify-catalog", "read_only": true}`.
     - `verify-catalog-baseline`, with its `--version` set to the `from` version, proves that catalog instead.

     Each connects as `genui_migration` over verify-full TLS, proves the catalog and then the version row, and rolls back. When the catalog differs it exits 1 and prints `{"refused": "catalog_refused"}` on standard error. **No command runs these after the promotion cutover.** `infra/scripts/window_requests.py operator-task` composes the RunTask request from a `describe-stacks` output without calling AWS, but `ops/beta/run-step.sh`, which ran it, is fenced after the cutover, and no promotion command starts the operator task. Running the request is a separately reviewed operator action with Victor's approval of that action.
  3. **Act on the outcome:**
     - *At the `to` version:* the migration committed. Fix forward onto the candidate: activate both services, verify them, and record the release as a normal execution would (the baseline digests and the policy schema version move to `to`). No command does this after execution began: `release-rolled-back` and the restore both refuse once `schema-migrate` is journaled. It is a separately reviewed operator delivery. Keep the lock until the release is recorded.
     - *At the `from` version:* the COMMIT did not take effect and the database is unchanged. The restore command still refuses exit 4, because its proof is the exit code, not this read. Restoring the previous release, or running the migration again, is a separately reviewed operator delivery; neither has a command.
     - *Neither catalog matches:* the database is in no reviewed state. Activate nothing and keep the lock. Restoring the snapshot is the separately reviewed operation in the [deploy runbook](DEPLOY-RUNBOOK.md#rollback).

  The snapshot protects the data in every case. `release-rolled-back` refuses once `schema-migrate` is journaled, and no other command releases a lock after execution began.

**Restoring the previous release after a failed migration.** `maintenance restore-after-failed-migration` is an operator command; the workflow roles and a GitHub Actions environment are refused. It follows the abandonment and expiry-policy pattern: the reviewed state hash, the 65-minute fence after the original run ends, a read-only plan first, then `--apply`, a new private `--output` file that is never overwritten, immutable receipts and no automatic replay. It requires all of:

- phase `executing`, with the schema release in the packet and every journal entry confirmed;
- confirmed `schema-snapshot`, `stop-main`, `stop-broker` and `changeset-execute` entries, nothing after the migration, and no duplicates;
- either no `schema-migrate` entry (the migration never started), or a confirmed one whose start receipt is intact and names one task, and whose immutable observation shows that task STOPPED after it exited by itself (`stopCode` `EssentialContainerExited`) with exit code 1, the migration's own refusal before the COMMIT was sent. Everything else refuses and follows the fix-forward path above:
  - a missing observation, an unknown exit code, a start with failures or an unresolved start;
  - exit 0, which means the migration committed;
  - the reserved exit 4, commit outcome unknown;
  - a task stopped from outside or ended by a signal (137, 143), which can have stopped on either side of the commit;
  - any other code.
- the stack on the candidate's parameters with both services stopped and every runtime boundary exact, no pending change set, the promotion's template object equal to the baseline, the previous parameters differing only in images and expiry, and the previous release's expiry more than an hour away.

Plan first, into a new private output file, with an authorized non-workflow operator identity and the fresh state hash from `maintenance inspect`:

```sh
app/runtime/.venv/bin/python -m ops.promotion.maintenance restore-after-failed-migration \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --expected-state-sha256 "<fresh-reviewed-state-hash>" \
  --output "$PRIVATE_RECONCILIATION/restore-plan.json"
```

Standard output is one JSON object: `status` `planned`, `promotion_id`, `before_state_sha256`, `migration_started`, `migration_exit_code`, `restores_candidate_sha`, `previous_expires_at_utc` and `windows_preserved`. The output file holds the whole plan. Review both, then run the same command with `--apply` and another new output path. Apply prints `status` `restored` and adds `revision` and `state_sha256`. A refusal exits 1 with one generic line on standard error. The output file then holds `refused-or-outcome-unknown` and, when the code refused a precondition or a check, its fixed `reason`.

`--apply` repeats the plan, records it, takes the fenced lock (phase `restoring`), and creates and executes the change set `forma-restore-<promotion id>`. That change set puts the stack back on the baseline template with the previous release's parameters, run by the restricted CloudFormation role as in preparation. The command then proves both services stopped on the previous images, records the activation intent (phase `restore-activation-intended`), activates both services and requires them healthy, running the exact previous image digests with healthy load-balancer targets, and runs the public probes. The previous image's pool requires exactly its own schema version, the release's `from` version, on every connection. A healthy previous release therefore proves that the database is still there. On success it marks the failed window abandoned (its reservation is kept, and the previous window counts its whole reservation again), records the restored task definitions, leaves the baseline digests and schema policy unchanged, keeps the snapshot and releases the lock. Each attempt writes its own plan receipt, `restore-plan-<state hash prefix>-<attempt id>.json`, named in the phase write and later in the completed record. A phase write that failed without changing the state can therefore be retried with the same reviewed hash. Any failure keeps the lock, and nothing is repeated automatically. A failure from the restore change set through the `restored.json` receipt also records `restore-outcome.json`, including a previous release that never becomes healthy. A failure writing the plan receipt or the phase before that, or the final state write after it, keeps the lock without that receipt. The command restores only images and template, never data. A migration start that returned failures, or one whose outcome is unknown (an intent), needs manual reconciliation; "never started" above means no `schema-migrate` entry at all.

**A restored release that never becomes healthy.** Its database has probably moved, whatever the exit code said. Do not retry the restore (its phase refuses it anyway), and keep the lock. Read the schema version and act on it as in the fix-forward path above.

**Accepted limit: no pre-approval tenant check.** Migration 0021 copies every active provider-tenant binding into a new table keyed by `(issuer, provider_tenant_id)`. If two organizations already share one, the migration fails inside the outage: the transaction leaves the database at 20, beta stays down, and the restore command above brings the previous release back. No role in this design can query the database before the outage. The workflow roles have no database credentials and the promotion runs no database task before it. The schema campaign is the evidence for the upgrade itself. On the same migrations it upgrades a version-20 database seeded with organizations and active and inactive bindings to version 22, and it refuses whole, leaving version 20 and every byte unchanged, when two organizations share one active binding ([APP-169 report](../../evidence/APP-169/schema-campaign-review-fixes-2026-11-05.json)). It does not read beta's data.

**Before the first schema release.** Victor applies the bootstrap update once. The execute role gains only `ecs:RunTask` on the migration task family with the `ecs:cluster` condition, `iam:PassRole` of that task definition's two roles to ECS tasks, `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` on the database and `forma-schema-release-*` snapshots, and `rds:AddTagsToResource` on those snapshots alone, because the instance copies its tags to every snapshot. Regenerate the template from the reviewed private inventory with `python3 -m ops.promotion.bootstrap --inventory <private inventory> --output <private template>`, review the difference against the installed stack, apply it, and read the policy back, as for the APP-167 bootstrap fixes. Then merge the release change with `PROMOTION_ENABLED` set to `true`, outside the approval window above.

**After the release.** Remove `schema-release.json` in a follow-up pull request once the release succeeds, or once it expires. The source pin may stay as an inert record until the next release's values replace it: `test_scan.py` binds the committed disposition to it, and without the file it is never read (release A, 2026-11-14). CI enforces this: the release test binds the file to `HEAD`, so any later change to the deployment contract fails CI until the file is removed.

## Deployment state and failures

One private encrypted state store owns the complete budget ledger, active deployment lock, release packet and write-ahead journal. A known held-lock refusal reports `PROMOTION_LOCK_HELD` with operator inspection guidance. Other refusals and uncertain AWS outcomes remain generic to keep private exception data out of public logs. The original local ledger remains migration evidence. After cutover, legacy local mutation entrypoints refuse to create a competing ledger or deployment. GitHub concurrency complements this lock; it does not replace it.

The deployment lock has no automatic expiry and remains held while approval is pending. Failed or cancelled execution can have an unknown AWS outcome. Do not rerun mutation steps blindly, delete the lock or start a laptop deployment. Reconcile the journal against actual CloudFormation and ECS state, with the previous workflow fenced, before permitting another release. An activation intent cannot be abandoned as though activation never happened.

Whole-workflow reruns intentionally refuse. After safe abandonment, a fresh reviewed application merge creates the next promotion attempt. Interrupted execution needs operator reconciliation. A failed post-health Git tag job can be retried separately; it has no AWS permission and does not repeat deployment.

Recovery uses compatible qualified images through a reviewed release. This pipeline does not roll back the database. Private raw AWS receipts and state stay in private storage; public GitHub summaries contain only the information needed to identify and review the release.

`python -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET"` reports the state hash, active phase and unresolved operation count without printing private AWS configuration. A fully prepared release that never started execution can be abandoned with `maintenance abandon --expected-state-sha256 <reviewed-hash>` and the same state arguments. That command requires the original GitHub workflow to have been completed for at least 65 minutes, beyond the one-hour role session limit. It proves the existing runtime unchanged and healthy, deletes and confirms absence of the exact unused change set, and preserves the abandoned budget window before releasing the lock. A failed preparation before any template/change-set operation can also be abandoned if every journal entry is a confirmed, allowlisted ECR upload with an intact immutable receipt and a preserved reservation. It requires the same 65-minute fence, exact state hash, healthy unchanged runtime and complete absence of pending change sets; uploaded images and the abandoned window remain as evidence. Other partial preparations, unknown outcomes and any release that began execution still require explicit operator investigation; there is no force-unlock command.

One additional operator-only recovery uses `--reconcile-bootstrap-create-rejection` for the exact CDK bootstrap-version permission rejection observed in APP-167. It requires the same credential fence and reviewed state hash, verifies every upload receipt and actual template bytes, reads a unique matching AWS CloudTrail rejection for the run role/session/time, proves the exact change set absent twice, and checks the complete change-set inventory and healthy unchanged runtime. CloudTrail omitted request parameters in this incident: the record explicitly says the client token comes from the canonical intent, not an independently observed CloudTrail parameter. The original intent is preserved in the immutable reconciliation record and is never labeled a successful call or replayed. Other failures remain refused.

## Superseding a prepared release

If a candidate is awaiting its human approval and a source defect is found, do not approve it or merge a replacement expecting cancellation to clear the release lock. For a fully prepared release that has **never started execution**:

1. Cancel the whole old GitHub run, or reject its pending deployment and confirm the whole run is terminal. Record its actual terminal `updated_at` and candidate SHA.
2. Allow 65 minutes after that timestamp for the workflow credentials to expire. No agent needs to poll or remain active during this fence.
3. With the configured state bucket/prefix and an authorized nonworkflow AWS operator identity, use the repository's locked runtime environment:

   ```sh
   app/runtime/.venv/bin/python -m ops.promotion.maintenance inspect \
     --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   Verify the expected candidate, `prepared` phase and zero unresolved operations. Review the fresh returned state hash, then run:

   ```sh
   app/runtime/.venv/bin/python -m ops.promotion.maintenance abandon \
     --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<fresh-reviewed-state-hash>"
   ```

4. Inspect again and require `active: null`. The command preserves the reservation history and proves the prior runtime healthy and unchanged; it deletes only the unused reviewed change set. A failed check needs investigation, never a forced unlock or blind rerun.
5. Confirm a new reservation fits the retained budget. If another cancelled attempt exhausted the available allowance, use the separately reviewed [future-expiry policy procedure](#future-expiry-policy) while idle; do not delete reservation history or raise the caps. Then merge the reviewed, qualified fix PR. Its fresh run must pass its own preparation and present a new packet for approval. Approval of an older run never includes later fixes.

There is currently no GitHub-only abandonment workflow. Any started execution or unknown AWS outcome requires operator reconciliation instead of this prepared-only procedure. Required checks and time fences may remain pending at an agent handoff; record the next action rather than monitoring them continuously.

The November 2 spreadsheet follow-up records cancelled candidate `60fd6bd`, run [41443022905](https://github.com/getforma-dev/forma-app-platform/actions/runs/41443022905), and its **14:27:27 UTC** fence in [APP-108's handoff](../../evidence/APP-108/spreadsheet-review-followup-2026-11-02/README.md). It supersedes the older in-progress PR40–42 batch narrative below; it does not claim that abandonment or the replacement deployment has occurred.

The later [November 2 recovery](../../evidence/APP-167/recovery-2026-11-02/README.md) completed that abandonment and applied the reviewed 18-hour future-shortening policy. Canonical readback is idle at revision 1458 with all 20 windows preserved. Run41479098807 had passed qualification but encountered the earlier lease; it is not an approval target. Its replacement needs a fresh reviewed merge and its own checks/packet. No replacement runtime was deployed by this recovery.

## Future-expiry policy

`ops.promotion.expiry_policy` replaces the earlier incident-specific scripts for increasing the persistent number of hours subtracted from the live expiry when preparing a future release. It changes no current AWS schedule and creates no reservation. The next real packet displays its actual shutdown time for approval. The helper only allows a stricter default, bounded at 72 hours, with the proposed expiry still more than 24 hours away; extension/renewal requires separate review.

After safe abandonment, inspect the fresh idle state and use an authorized nonworkflow operator identity. Plan first, retaining a new private output file:

```sh
app/runtime/.venv/bin/python -m ops.promotion.expiry_policy \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --expected-state-sha256 "<fresh-reviewed-idle-state-hash>" --hours 18 \
  --output "$PRIVATE_RECONCILIATION/expiry-policy-plan.json"
```

The eighteen-hour value is the November 2 follow-up proposal, not a universal default. Review its proposed expiry and budget projection. Planning proves a healthy unchanged runtime, no pending change sets and a fresh canonical budget basis. It cannot refresh a stale assessment, reduce reservations, change the reservation ceiling (18000/21800 cents as of ADR-019; earlier packets were checked against 17000/20800), alter authority, or run while a deployment holds the lock. If the plan refuses, investigate its cause rather than increasing hours automatically.

Apply that reviewed value against the same idle hash using another new output path:

```sh
app/runtime/.venv/bin/python -m ops.promotion.expiry_policy \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --expected-state-sha256 "<same-reviewed-idle-state-hash>" --hours 18 \
  --output "$PRIVATE_RECONCILIATION/expiry-policy-result.json" --apply
```

Apply redoes the boundary/projection checks, writes immutable source-bound intent, performs one conditional policy-only update, and verifies exact readback. The ledger, live configuration, authority, schema and every reservation must remain identical; only shortening hours and state revision change. Unknown receipt, write or readback results stop without replay. Inspect private intent and canonical state before any recovery. The CLI reserves a private `0600` output before AWS work and never overwrites an earlier plan/result.

## Setup record

The `beta-plan` and `beta` GitHub environments allow only the branch `main` (ADR-019; they previously allowed only `pivot/pitch-mvp`). `beta` requires one approval from Victor-Villacis, with self-review allowed for the sole human owner and administrator bypass disabled. Read the branch-policy list back after any change.

The reviewed AWS bootstrap is installed and its policies, trust, OIDC issuer/audience and private bucket settings were read back exactly. Canonical-state cutover preserved all 16 windows and the original ledger hash; the local original is read-only and both new and old environment entrypoints refuse mutation. Actual AWS probes confirmed conditional writes and history protection. PR #46 merged as `07ea5aa`; all 12 required push workflows and ARM64 qualification passed. The [third promotion run](https://github.com/getforma-dev/forma-app-platform/actions/runs/41420932781) successfully prepared the immutable AWS change set. It was cancelled on November 2 at 10:22:51 UTC when the user requested the reviewed PR40–42 batch, so that obsolete packet must not be approved. Application execution and final live verification remain open in APP-167. GitHub uses short-lived OIDC credentials; do not add long-lived AWS access keys as repository secrets.

## Budget maintenance

Use an authorized operator identity with read access to Cost Explorer and write access to the canonical state; the workflow roles have no billing permission. Read `PROMOTION_STATE_BUCKET` and `PROMOTION_STATE_PREFIX` from the configured environment. Keep all output in a private directory and use the repository's locked runtime venv.

```bash
umask 077
app/runtime/.venv/bin/python -m ops.promotion.reconcile observe \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --output "$PRIVATE_RECONCILIATION/observation.json"
```

The observation contains a real account-scoped Cost Explorer response through the last completed UTC day and an assessment template. Cost Explorer data can lag and can include other workloads in the account. Inspect the response and copy `assessment_template` into a separate `assessment.json`. Write the reserve rationale, retain or increase each amount, and explicitly acknowledge coverage for billing lag and the query cost. The raw account total is evidence for the assessment, not an automatically attributed Forma expense or an extra charge added twice to the ledger.

Apply within one hour of observation:

```bash
app/runtime/.venv/bin/python -m ops.promotion.reconcile apply \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --observation "$PRIVATE_RECONCILIATION/observation.json" \
  --assessment "$PRIVATE_RECONCILIATION/assessment.json" \
  --output "$PRIVATE_RECONCILIATION/result.json"
```

The assessment binds to the observation and current ledger hashes. Apply reads Cost Explorer again and requires its scope and cost results to match the reviewed observation; an edited file or changed bill cannot refresh the basis. Application refuses during a deployment, on a stale/racing ledger, for reduced spending or reserves, or if the original budget validation fails. It preserves every window and all runtime settings. The private immutable receipt records both AWS observations and the assessment before the conditional ledger update. An unknown write result needs readback before retry. A month rollover, a lower reserve, a changed cap or an expiry renewal requires separate operator review; this command cannot grant them.

## Reviewed PR40–42 batch

PR41's drag handle has merged after all 28 GitHub checks and independent browser review. PR40's theme fixes and PR42's original spreadsheet viewer are being qualified together with PR44's reviewed test-fixture repair. Automatic preparation is temporarily disabled during the intermediate merges; it must be restored before the final reviewed application merge. This avoids preparing an incomplete batch while preserving the single human beta approval.

The cancelled third run never executed its prepared change set. The supported maintenance procedure may abandon it only after the credential fence at **2026-11-02 11:27:51 UTC**, proving the original runtime healthy and unchanged, deleting only the unused change set and preserving all 19 ledger windows. Its preparation evidence remains historical evidence, not an active approval request.

PR40 adds two explicitly reviewed frontend source files to the Docker context. The automatic contract guard remains unchanged. A separate operator procedure must prove the exact two allowlist lines and unchanged remaining protected files against immutable Git objects, retain an immutable receipt, and conditionally admit only the new context binding while idle. The scan disposition's three findings, predecessor and expiry remain unchanged. The reviewed budget projection also requires a separate ten-to-fourteen-hour shortening-policy adjustment that retains every allowance and the original caps. Neither operator operation nor a replacement AWS deployment is claimed by this source record; both require actual readback evidence. The final promotion still performs its full image/scan/health checks and waits for the human approval.
