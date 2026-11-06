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

New image scan findings stop preparation unless an exact source-reviewed disposition applies. A disposition has one entry per image, each bound to that image's live predecessor manifest and its complete finding fingerprints, plus the deployment contract and an expiry; it is rechecked before execution. The accepted CVE names, severities, packages, roles and expiry are also pinned in `ops/promotion/registry.py`, so a JSON edit alone cannot widen them, and a CRITICAL finding is never accepted. The approval summary reports introduced, reviewed and unreviewed counts, CVEs and the disposition hash/expiry. The baseline and raw findings are never rewritten or suppressed. The current disposition, [APP-173](../../evidence/APP-173/SCAN-DISPOSITION.md), lets a release introduce two HIGH Debian findings (zlib CVE-2026-76844, no Bookworm fix; apt CVE-2026-109518, fix tracked in APP-089) in all three images until November 12. Once a release succeeds they are ordinary baseline findings, with no end date, until APP-089 replaces the packages. It replaced the first, operator-only [APP-167 disposition](../../evidence/APP-167/SCAN-DISPOSITION.md) of three MEDIUM gcc-12 findings.

Scanner data changes without any source change. On November 5 those two CVEs appeared overnight and refused preparation after the lock was taken; the documented abandon released it and `PROMOTION_ENABLED` was set to `false` while the disposition was reviewed. The [APP-173 record](../../evidence/APP-173/SCAN-DISPOSITION.md) has the timeline. Set the variable back to `true` immediately before merging the change that should promote.

The existing budget limits and requirement for a reconciliation basis less than 24 hours old still apply. A stale basis stops preparation before the approval stage. Budget maintenance must retain fresh billing evidence and the operator's reserve assessment in the canonical state; automation must not refresh the timestamp on an old assessment. Renewal or a larger budget is not implicitly authorized by a merge. The canonical policy also sets how many hours each release subtracts from the prior shutdown expiry. The first recovery changed that default from two to six hours. After a second preparation rejection, the reviewed recovery changed six to ten hours to retain both failed attempts' allowances within the original caps. This is a persistent conservative default, not an automatic reset after one release. The policy update changes no running schedule: the next approval displays the actual proposed expiry, November 30 at 04:00 UTC. Applying or later changing the policy requires an idle, hash-bound operator review and immutable receipt.

## Schema releases

A schema release carries migrations through the normal packet and the one approval, on the same qualified images ([APP-175](../../Tickets/0-backlog/APP-175.md), [ADR-017 amendment](../architecture/ADR-017-BETA-PROMOTION.md#amendment-2026-11-06-reviewed-schema-releases)). It replaces the by-hand schema 20 crossing of November 4.

**When it applies.** Only when `ops/promotion/schema-release.json` exists and every one of these holds; otherwise preparation refuses before the lock exactly as before:

- its `from` schema manifest and deployment contract digests equal the canonical baseline's, and its `from` version equals the policy schema version;
- its `to` digests equal the candidate's qualified source binding, and the candidate's `app/migrations/manifest.json` has those bytes and its `database_version` is the `to` version;
- the candidate's CDK source (`infra/src/*` and the four `infra/` project files) has the reviewed infrastructure digest, so the stack is unchanged;
- the current time is before its expiry.

The evidence path, both versions (20 and 22) and the expiry (2026-11-20 00:00 UTC) are pinned in `ops/promotion/schema_release.py`, so a JSON-only edit cannot widen it; malformed, duplicated or retyped fields refuse. A unit test binds the committed file's `to` digests and infrastructure digest to `HEAD`, so CI fails if the tree drifts from the reviewed release. A promotion whose schema manifest and contract equal the baseline never reads the file. A schema release cannot also introduce scan findings: a disposition binds the baseline contract, which the release replaces, so preparation refuses before any change set. The current release is recorded in [evidence/APP-175](../../evidence/APP-175/SCHEMA-RELEASE.md).

**What the approval shows.** The summary's `schema_release` is `{from_version: 20, to_version: 22, spec_sha256}`; `spec_sha256` is the hash of the committed file's exact bytes. `schema_version` stays the current policy version (20), `preserved.schema` is `false`, and the sequence adds `snapshot database` and `run reviewed migration` between `verify stopped boundaries` and `activate both services`. Approve only if those match the reviewed release.

**Execution order.** Execution rechecks the release at its start and again immediately before stopping the services, as it does the scan disposition; a changed, removed or expired file refuses while beta still serves. Then:

1. stop both services, prove them stopped, execute the reviewed change set (which registers the migration task definition on the candidate service image), and record the stopped boundary;
2. create a manual RDS snapshot, `forma-schema-release-<first 8 of the promotion id>-v22`, of the instance named by the stack's `DatabaseEndpoint` output; wait at most 20 minutes for `available`; record the observation;
3. run the stack's `MigrationTaskDefinition` once (Fargate 1.4.0, the two public task subnets and the task security group, as the operator steps did), wait at most 30 minutes for it to stop, and require the candidate image digest, `EssentialContainerExited` and exit code 0; record the observation;
4. prove the stopped boundary again, then activate and verify as for any release;
5. on success the canonical baseline takes the candidate's schema manifest and contract digests, the policy schema version becomes 22, and the completed record names the release hash, snapshot and migration task.

The snapshot and the task start are journaled mutations with immutable receipts. Neither is tagged. The migration task falls within the two extra task-hours every promotion window already reserves (`extra_task_hours`, priced at the migration task's 2 vCPU and 8 GiB); snapshot storage is not in the ledger.

**Failure and recovery.** Any failure in steps 2–4 raises. Beta stays down: both services remain stopped, the phase stays `executing`, the lock is held, and nothing activates, retries or replays, the same posture as a failed stack update. The migration runner applies 21 and 22 in one transaction, so a migration that fails leaves the database at version 20, and one that exited 0 has committed version 22. A task stopped from outside, or one whose outcome was not observed, can have stopped on either side of the commit. To recover, an operator fences the original workflow (65 minutes after it ends, as in [Superseding a prepared release](#superseding-a-prepared-release)), inspects the state and journal (`schema-snapshot`, `schema-migrate`), reads the private receipts and the migration task's log, and establishes the database's recorded schema version before deciding. With the database still at 20, the way back to service is a reviewed rollback promotion of the prior live images, which serve schema 20. With the database at 22 the default is to fix forward; the snapshot protects the data, and restoring it is the separately reviewed operation in the [deploy runbook](DEPLOY-RUNBOOK.md#rollback). No maintenance command releases a lock after execution began; that reconciliation is explicit operator work, as for any started execution.

**Before the first schema release.** Victor applies the bootstrap update once: the execute role gains only `ecs:RunTask` on the migration task family with the `ecs:cluster` condition, `iam:PassRole` of that task definition's two roles to ECS tasks, and `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` on the database and `forma-schema-release-*` snapshots. Regenerate the template from the reviewed private inventory with `python3 -m ops.promotion.bootstrap --inventory <private inventory> --output <private template>`, review the difference against the installed stack, apply it, and read the policy back, as for the APP-167 bootstrap fixes. The stack leaves the instance's `copyTagsToSnapshot` at the CDK default, which is on; if IAM requires the caller to hold `rds:AddTagsToResource` for that, `CreateDBSnapshot` is denied after the services stop, so confirm it by policy simulation before enabling promotion. Then merge the release change with `PROMOTION_ENABLED` set to `true`. Remove `schema-release.json` in the next change after the release succeeds or expires; until then CI refuses any change to the deployment contract that the file does not name.

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
