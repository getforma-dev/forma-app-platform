# Promote to beta

[APP-167](../../Tickets/2-in-progress/APP-167.md) tracks implementation and live acceptance. [ADR-017](../architecture/ADR-017-BETA-PROMOTION.md) records the decision. A configured environment or successful local test alone does not establish an operational pipeline; see the ticket's recorded live run before relying on it.

## Normal release

Merge a reviewed application pull request into `pivot/pitch-mvp`, the current product trunk. The promotion workflow checks the exact merged commit's required push CI, builds and qualifies the service, qualification and operator images on ARM64, and retains those exact image bytes. PR builds have no AWS deployment authority.

The preparation job publishes the qualified images and prepares a runtime-only AWS change set. Its summary identifies the candidate, image digests and prepared release packet. Review that concrete packet, then approve **Promote to beta** in the run's **Review deployments** control. This is the one deployment approval. Victor can approve a run triggered by his own merge; administrator bypass is disabled.

Execution revalidates the approved packet, stops both existing services, proves they are stopped, applies the reviewed change set, and starts and verifies the new runtime. Expect a brief outage and existing sessions to sign out during the restart. A successful merge or image build is not evidence that beta deployed: the promotion must finish its live verification successfully.

The beta address stays [beta.getforma.dev](https://beta.getforma.dev/). Existing workspaces, trial data, accounts and invitation destinations are retained.

## Initial scope

The automated path covers compatible application and UI updates, with the existing bounded shutdown expiry. It refuses changes to the pinned infrastructure, migration and deployment contract. Such a refusal calls for a separately reviewed operator delivery; it must not be bypassed by editing a checksum or pretending that a migration is unnecessary. `main` remains governed by the existing repository landing decision.

Documentation-only changes need no runtime release. A superseded candidate, failed required check, wrong source/artifact, changed predecessor or unresolved earlier deployment must stop promotion rather than deploy an unverified mixture.

New image scan findings stop preparation unless an exact source-reviewed disposition applies. The initial operator-only disposition is bound to its predecessor image, deployment contract and three complete medium-finding fingerprints; it expires November 9 and is rechecked before execution. The approval summary reports introduced, reviewed and unreviewed counts, CVEs and the disposition hash/expiry. The baseline and raw findings are never rewritten or suppressed. See [the scan review](../../evidence/APP-167/SCAN-DISPOSITION.md).

The existing budget limits and requirement for a reconciliation basis less than 24 hours old still apply. A stale basis stops preparation before the approval stage. Budget maintenance must retain fresh billing evidence and the operator's reserve assessment in the canonical state; automation must not refresh the timestamp on an old assessment. Renewal or a larger budget is not implicitly authorized by a merge. The canonical policy also sets how many hours each release subtracts from the prior shutdown expiry. The first recovery changed that default from two to six hours. After a second preparation rejection, the reviewed recovery changed six to ten hours to retain both failed attempts' allowances within the original caps. This is a persistent conservative default, not an automatic reset after one release. The policy update changes no running schedule: the next approval displays the actual proposed expiry, November 30 at 04:00 UTC. Applying or later changing the policy requires an idle, hash-bound operator review and immutable receipt.

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

The eighteen-hour value is the November 2 follow-up proposal, not a universal default. Review its proposed expiry and budget projection. Planning proves a healthy unchanged runtime, no pending change sets and a fresh canonical budget basis. It cannot refresh a stale assessment, reduce reservations, change either original cap (17000/20800 cents), alter authority, or run while a deployment holds the lock. If the plan refuses, investigate its cause rather than increasing hours automatically.

Apply that reviewed value against the same idle hash using another new output path:

```sh
app/runtime/.venv/bin/python -m ops.promotion.expiry_policy \
  --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
  --expected-state-sha256 "<same-reviewed-idle-state-hash>" --hours 18 \
  --output "$PRIVATE_RECONCILIATION/expiry-policy-result.json" --apply
```

Apply redoes the boundary/projection checks, writes immutable source-bound intent, performs one conditional policy-only update, and verifies exact readback. The ledger, live configuration, authority, schema and every reservation must remain identical; only shortening hours and state revision change. Unknown receipt, write or readback results stop without replay. Inspect private intent and canonical state before any recovery. The CLI reserves a private `0600` output before AWS work and never overwrites an earlier plan/result.

## Setup record

The `beta-plan` and `beta` GitHub environments allow only the branch `pivot/pitch-mvp`. `beta` requires one approval from Victor-Villacis, with self-review allowed for the sole human owner and administrator bypass disabled. Both settings and branch-policy lists were read back after configuration.

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
