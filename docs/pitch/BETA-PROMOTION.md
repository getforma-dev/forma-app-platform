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

The existing budget limits and requirement for a reconciliation basis less than 24 hours old still apply. A stale basis stops preparation before the approval stage. Budget maintenance must retain fresh billing evidence and the operator's reserve assessment in the canonical state; automation must not refresh the timestamp on an old assessment. Renewal or a larger budget is not implicitly authorized by a merge.

## Deployment state and failures

One private encrypted state store owns the complete budget ledger, active deployment lock, release packet and write-ahead journal. The original local ledger remains migration evidence. After cutover, legacy local mutation entrypoints refuse to create a competing ledger or deployment. GitHub concurrency complements this lock; it does not replace it.

The deployment lock has no automatic expiry and remains held while approval is pending. Failed or cancelled execution can have an unknown AWS outcome. Do not rerun mutation steps blindly, delete the lock or start a laptop deployment. Reconcile the journal against actual CloudFormation and ECS state, with the previous workflow fenced, before permitting another release. An activation intent cannot be abandoned as though activation never happened.

Recovery uses compatible qualified images through a reviewed release. This pipeline does not roll back the database. Private raw AWS receipts and state stay in private storage; public GitHub summaries contain only the information needed to identify and review the release.

`python -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET"` reports the state hash, active phase and unresolved operation count without printing private AWS configuration. A fully prepared release that never started execution can be abandoned with `maintenance abandon --expected-state-sha256 <reviewed-hash>` and the same state arguments. That command requires the original GitHub workflow to have been completed for at least 65 minutes, beyond the one-hour role session limit. It proves the existing runtime unchanged and healthy, deletes and confirms absence of the exact unused change set, and preserves the abandoned budget window before releasing the lock. It refuses partial preparation, unknown outcomes and any release that began execution. Those require explicit operator investigation of the private journal; there is no force-unlock command.

## Setup record

The `beta-plan` and `beta` GitHub environments allow only the branch `pivot/pitch-mvp`. `beta` requires one approval from Victor-Villacis, with self-review allowed for the sole human owner and administrator bypass disabled. Both settings and branch-policy lists were read back after configuration.

AWS bootstrap, canonical-state cutover, restricted-role review and the first real promotion remain separate acceptance steps until their evidence is recorded in APP-167. GitHub uses short-lived OIDC credentials; do not add long-lived AWS access keys as repository secrets.

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
