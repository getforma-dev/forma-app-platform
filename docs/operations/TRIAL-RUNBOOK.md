# Real-user trial runbook (Victor's class, November 29–30)

The operator steps for the trial decided on 2026-11-14 (the plan's calendar; the charter is the ADR-008 amendment; the ceilings and the expiry are the [ADR-019 amendment](../architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md#amendment-2026-11-14-the-real-user-trial-its-expiry-and-its-ceilings)). Every step below is Victor's: an authorized non-workflow AWS identity, the repository's locked venv (`app/runtime/.venv/bin/python`, called `$PY` here), and private output under a 0700 directory. Agents prepare and check; they call no AWS API, send no email and approve nothing. Expected outputs are the exact JSON shapes the commands print; anything else stops the step.

```sh
export PY=app/runtime/.venv/bin/python
export REPO=getforma-dev/forma-app-platform
# PROMOTION_STATE_BUCKET, PROMOTION_STATE_PREFIX and PRIVATE_RECONCILIATION as in BETA-PROMOTION.md "Budget maintenance".
umask 077; mkdir -p "$PRIVATE_RECONCILIATION/trial"; export TRIAL="$PRIVATE_RECONCILIATION/trial"
# The trial organization's UUID (the clean-trial organization; the stack parameter CleanTrialOrganizationId):
export TRIAL_ORG=$(aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation \
  --query "Stacks[0].Parameters[?ParameterKey=='CleanTrialOrganizationId'].ParameterValue" --output text)
# Stack outputs by name (sections 6, 9 and 10 use these):
export OUT="Stacks[0].Outputs[?OutputKey=='%s'].OutputValue"
out() { aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation --query "$(printf "$OUT" "$1")" --output text; }
export CLUSTER=$(out ClusterName) SERVICE=$(out ServiceName)
```

Every `ops.promotion` command here takes `--expected-state-sha256`: read it fresh each time with `maintenance inspect` and paste it; a changed state refuses. Output files are created new (0600) and never overwritten: use a new name per attempt.

## 1. Release A: today's `main` (schema 20 → 22), Friday Nov 14 and Saturday Nov 15

Run 43157872671 is stuck on a stale ledger basis; its packet must not be approved. Promotion is paused (`PROMOTION_ENABLED=false`).

1. **Cancel the run and record its terminal time** (Nov 14):

   ```sh
   gh run cancel 43157872671 --repo $REPO
   gh run view 43157872671 --repo $REPO --json status,conclusion,updatedAt,headSha
   ```

   Expected: `"status": "completed"`, `"conclusion": "cancelled"`. Note `updatedAt`; the fence ends 65 minutes later. Nothing to watch during the fence.

2. **APP-175 bootstrap readback** (any time Nov 14; the execute role's new permissions for the schema release, [BETA-PROMOTION.md "Before the first schema release"](../pitch/BETA-PROMOTION.md#schema-releases)). Regenerate the template from the private inventory, diff it against the installed bootstrap stack, apply, then read the execute role's policy back and confirm it holds exactly: `ecs:RunTask` on the migration task family with the `ecs:cluster` condition, `iam:PassRole` of that task definition's two roles, `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` on the database and `forma-schema-release-*` snapshots, and `rds:AddTagsToResource` on those snapshots only.

   ```sh
   $PY -m ops.promotion.bootstrap --inventory "$PRIVATE_RECONCILIATION/inventory.json" --output "$TRIAL/bootstrap-template.json"
   # review the difference against the installed stack, apply it, then:
   aws iam list-role-policies --role-name <execute role name>
   aws iam get-role-policy --role-name <execute role name> --policy-name <the schema-release policy>
   ```

   If the readback differs from the list above, stop: release A's execution would fail at the snapshot or the migration task.

3. **After the fence: inspect and abandon** (Nov 15):

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   Expected: `{"state_sha256": "<64 hex>", "revision": N, "active": {"promotion_id": "...", "candidate_sha": "<the run's headSha>", "phase": "prepared", "unresolved_operations": 0}}`. Any other phase or a nonzero count: stop and investigate (BETA-PROMOTION "Deployment state and failures").

   ```sh
   $PY -m ops.promotion.maintenance abandon --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<state_sha256 from inspect>"
   ```

   Expected: `{"status": "abandoned", "promotion_id": "...", "windows_preserved": N}`. Inspect again: `"active": null`.

4. **Fresh billing basis** (within 24 hours of the approval; the stale basis is why the run was stuck):

   ```sh
   $PY -m ops.promotion.reconcile observe --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --output "$TRIAL/observe-a.json"            # prints {"status": "observed"}
   ```

   Copy `assessment_template` from the file into `$TRIAL/assessment-a.json`, write the single-line rationale (40–600 characters), keep or raise every amount, set `billing_lag_and_query_cost_covered` to `true`, and compare `account_total_usd` with the November ceiling ($210 GenUI / $248 project): the month-to-date total must be well below it, or stop and reassess before any release. Then, within the hour:

   ```sh
   $PY -m ops.promotion.reconcile apply --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --observation "$TRIAL/observe-a.json" --assessment "$TRIAL/assessment-a.json" --output "$TRIAL/reconcile-a.json"
   ```

   Expected: `{"status": "reconciled"}`.

5. **Re-enable promotion and merge.** Set the variable immediately before the merge that should promote:

   ```sh
   gh variable set PROMOTION_ENABLED --body true --repo $REPO
   gh variable list --repo $REPO | grep PROMOTION_ENABLED        # PROMOTION_ENABLED  true
   gh pr merge <the reviewed release-A pull request> --merge --repo $REPO
   gh run list --repo $REPO --workflow beta-promote.yml --limit 1
   ```

   The preparation summary must show `schema_release: {from_version: 20, to_version: 22, spec_sha256: ...}`, the sequence starting with `snapshot database`, the scan disposition hash `e60169964ed009b973dd24a342b10fe3a211e3dc297bb0ab411fc3b496562213` with its Nov 20 expiry, and a stop time (the proposed expiry) 18 hours before the current backstop. Approve only if it does.

6. **Approve** in the run's **Review deployments**, outside **05:00–05:45 UTC** (the RDS backup window; a refused snapshot leaves beta serving and needs a new merge) and within 24 hours of step 4. Execution: snapshot, stop, change set, migration 21–22, activate, health. Then:

   ```sh
   curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/health/ready     # 200
   gh run view <run id> --repo $REPO --json conclusion                                   # "success"
   ```

   The run's completed record names schema 22, the snapshot and the migration task. If the migration fails, beta stays down with the lock held: BETA-PROMOTION "Restoring the previous release after a failed migration"; nothing is retried by hand.

7. **Follow-up**: remove `ops/promotion/schema-release.json` and its pin in a pull request once the release succeeds (W1 replaces them with release B's).

## 2. The expiry and month renewal (Sunday Nov 23, before release B)

After release A the live backstop is **2026-11-24 14:00 UTC**: release A's packet took the canonical Nov 25 08:00 expiry minus 18 hours (`runner.py:222`), and on success that config became the live one, so the stack parameter, the schedule and the ledger window all read `2026-11-24T14:00:00Z`. Release B must **execute** (not merely merge) before that time; otherwise the schedule scales both services to zero and only the next successful release restarts them. Release B again subtracts 18 hours from the canonical expiry and refuses a stop within 24 hours, so the renewal below must precede it. Order matters and everything happens within 24 hours of the merge: November reconciled, December added, expiry renewed, then the merge.

**Learn early.** `reservation_cap`, `month_renewal` and `expiry_renewal` without `--apply` are read-only plans. Run all three right after release A's reconcile on Nov 15 (the month plan against the freshly reconciled ledger, the expiry plan after the month plan is *applied*, or expect `missing reconciled ledger month 2026-12`): the expiry plan reports November's fit under the caps a week before Nov 23. The cap raise itself needs the W3 merge first (the code caps are read from the checkout; `reservation_cap` refuses until they are 21000/24800).

1. **Reconcile November** exactly as in release A step 4 (`observe-b.json`, `assessment-b.json`, `reconcile-b.json`). Confirm the observed total against the $210 / $248 ceiling.
2. **Raise the canonical caps** to the ADR-019 amendment's pair, idle, once (it refuses a repeat):

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   $PY -m ops.promotion.reservation_cap --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" --expected-state-sha256 "<hash>"
   $PY -m ops.promotion.reservation_cap --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" --expected-state-sha256 "<hash>" --apply
   ```

   Expected plan: `{"status": "planned", "from": [18000, 21800], "to": [21000, 24800], ...}`; apply: `{"status": "caps-raised", "genui_cap_cents": 21000, "project_cap_cents": 24800, "revision": N+1, "state_sha256": ...}`. `expiry_renewal` refuses until the policy pair equals the code caps.
3. **Add December.** Write `$TRIAL/december.json`:

   ```json
   {"month": "2026-12", "ledger_sha256": "<the ledger_sha256 field of $TRIAL/reconcile-b.json>",
    "rationale": "December forward reservation for the real-user trial through the Dec 16 stop: no billing data yet; reserves cover probes, logs, secrets and billing lag.",
    "amounts": {"prior_extra_spend_cents": 0, "variable_reserve_cents": 1500, "auth_spent_or_reserved_cents": 3800, "unallocated_cents": 0}}
   ```

   The `ledger_sha256` is the digest of the ledger **after** `reconcile apply` rewrote it: `reconcile apply` writes it into its `--output` file (`reconcile-b.json`, field `ledger_sha256`). The assessment template's `ledger_sha256` is the pre-apply digest and `month_renewal` would refuse "assessment is bound to another ledger". Plan, read, then apply against the same inspected state hash:

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   $PY -m ops.promotion.month_renewal --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --assessment "$TRIAL/december.json" --output "$TRIAL/month-plan.json"
   $PY -m ops.promotion.month_renewal --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<same hash>" --assessment "$TRIAL/december.json" --output "$TRIAL/month-result.json" --apply
   ```

   Expected plan: `"status": "planned", "month": "2026-12", "created": true`, a `december_projection` with `retained_core_full_month_cents` about 5590, `release_compute_cents` about 4900 (release B from Dec 1 to its Dec 15 14:00 stop, service and broker), `genui_total_cents` those two plus the reserves, and `project_total_cents` 3800 more, beside `trial_ceiling_cents: {"genui": 8000, "project": 11800}` (the ceiling counts sixteen days of retained core, the projection the whole month); the apply prints `"status": "applied"` with the new `revision` and `state_sha256`. Running it again later re-assesses (new timestamp, never a lower amount); after Dec 1 it refuses and `reconcile` owns the month.

4. **Renew the expiry** against the state hash the month apply printed:

   ```sh
   $PY -m ops.promotion.expiry_renewal --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<state_sha256 from the month apply>" --output "$TRIAL/expiry-plan.json"
   $PY -m ops.promotion.expiry_renewal --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<same hash>" --output "$TRIAL/expiry-result.json" --apply
   ```

   Expected plan: `"before_expiry": "2026-11-24T14:00:00Z"`, `"after_expiry": "2026-12-16T08:00:00Z"`, `"live_stop_unchanged": "2026-11-24T14:00:00Z"`, `"next_release_expiry": "2026-12-15T14:00:00Z"`, `"genui_cap_cents": 21000`, `"project_cap_cents": 24800`, and `cost_by_month` for `2026-11` and `2026-12` each under those caps. The plan is release B's reservation projected: **if it still refuses with `budget exceeded for 2026-11`**, stop; the November reservation does not fit even the raised caps and a new decision is needed, not a different number here. The apply prints `"status": "applied"`.

   If release B then does not merge within 24 hours, either repeat steps 1, 3 and 4 the next day (the cap raise is done once), or `--revert` (same arguments plus `--revert`) to put the config back to the window's recorded expiry; `reconcile apply` refuses while the config is renewed and no release has consumed it.

## 3. Release B: migration 0023 and the trial capabilities (Sunday Nov 23)

W1 commits `ops/promotion/schema-release.json` for 22 → 23 with its pins; W3's operator changes (`--note`, `admin_policy`, `list-box-files`, `stats`) ride the same operator image. Within 24 hours of section 2:

1. If `main` carries new scanner findings, preparation refuses before approval: section 8 (the scan disposition renewal). Otherwise:
2. `gh variable set PROMOTION_ENABLED --body true --repo $REPO`, merge the reviewed release-B pull request, read the summary: `schema_release: {from_version: 22, to_version: 23}`, the stop time **2026-12-15T14:00:00Z**, the disposition (if any) with its Dec 16 expiry.
3. Approve outside 05:00–05:45 UTC. Then `/health/ready` → 200, `gh run view ... --json conclusion` → `"success"`.
4. **Verify from the back office** (the operator task now has `stats`):

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   The expiry is in the live parameters of the state the apply wrote; `aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation --query "Stacks[0].Parameters[?ParameterKey=='DeploymentExpiresAtUtc'].ParameterValue" --output text` prints `2026-12-15T14:00:00Z`, and `aws scheduler get-schedule --group-name <DeploymentExpiryGroup output> --name <schedule>` shows `at(2026-12-15T14:00:00)`.
5. **Deploy the alarms** (section 6), **set the Anthropic spend limit** (about $150 for the month, in the Anthropic console), **ask FormaStack to raise the fAuth tenant limit to at least 30**, and run the **restore drill** (section 9) while only Victor's account exists.
6. **Victor's admin row** (Monday Nov 24, before building the class apps; the Admin page requires it). Your principal id is in `admission list`'s redemption of the owner code:

   ```sh
   $PY -m ops.promotion.admission grant-admin --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --principal "<your principal id>" --output "$TRIAL/grant-admin.json"
   ```

   Expected: `{"status": "granted", "permission": "admin_policy", "changed": true, ...}`. A grant to the wrong member is undone with `revoke-admin` (same arguments) → `{"status": "revoked-admin", "changed": true, ...}`; it advances that member's epoch.

## 4. Codes by wave and the one announced restart (Friday Nov 28)

The back office is `ops/promotion/admission.py`. It mints the codes on your machine, writes the one-shot `OperatorInvitationInput` secret, runs one operator task per code (`admission issue --code-digest --provider-input-from-secret --authoring-only --note`), and **always empties the secret afterwards, even on failure**. The codes go only to the private `--output` file (0600). Nothing reaches the service until the restart: the service loads the capability secret once per task start (at most 128 entries), so one restart covers every code issued before it.

1. **Recipients.** `$TRIAL/wave-1.json`, one entry per student. The note is what you will see in `list`: **a seat number or initials, never a full name or an address**, because it travels in the RunTask override and so in CloudTrail's request parameters, `DescribeTasks`, the operator log and the receipt. The address stays in this file and the one-shot secret only:

   ```json
   {"recipients": [{"email": "student@example.org", "note": "Ada L."}, {"email": "...", "note": "..."}]}
   ```

   The identifiers file is FormaStack's private identifiers JSON used for the clean trial (`jwt_iss` and the trial tenant's `tenant_id`). Keep waves at 5–10 codes: each task takes a minute or two and a failed task ends its wave; the rest are reissued in a new wave with new entries.

2. **Issue a wave** (idle state; the hash from `maintenance inspect`):

   ```sh
   $PY -m ops.promotion.admission issue --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --count 8 --notes-file "$TRIAL/wave-1.json" \
     --identifiers "$PRIVATE_RECONCILIATION/identifiers.json" --expires-days 16 --output "$TRIAL/codes-wave-1.json"
   ```

   Expected: `{"status": "issued", "issued": 8, "failed": 0, "not_issued": 0, "capabilities_before": N, "capabilities_after": N+8, "capabilities_max": 128, "input_cleared": true, "code_digests": [...], "receipts": [...]}`. The file holds, per recipient, the code, its digest, `code_id`, `expires_at` and the provider invitation state (`confirmed`). Before migration 0023 only `--without-notes` works (the task refuses `note_unsupported` and writes nothing).

   Refusals, all before any write: a wrong account or a workflow identity; an organization that is not the stack's `CleanTrialOrganizationId`; a stale state hash or a held deployment lock; the input secret not empty (a previous run failed to clear it: read it with `aws secretsmanager get-secret-value --secret-id <OperatorInvitationInputSecretArn output>` and put `{}` back by hand, then investigate); more than 128 capabilities after this wave (revoke unused codes first); a stack without `OperatorTaskDefinition`, `OperatorImageDigest`, `ProviderManagementSecretArn` or `ProviderManagementUrl`. An `incomplete` status means a task failed: the file names each code's outcome; the secret is still cleared; issue the rest as a new wave.

3. **Review the codes**:

   ```sh
   $PY -m ops.promotion.admission list --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --output "$TRIAL/list-$(date -u +%Y%m%dT%H%M%S).json"
   ```

   Expected: `{"status": "listed", "count": 26, "by_status": {"active": 26}, ...}`; the file lists each code with its `note`, redemptions and provider invitation. A wrong code: `admission revoke --code <code_id>` (same arguments) → `{"status": "revoked", "changed": true, "capability_phase": "completed", "provider_phase": "completed", ...}`.

4. **Email the codes yourself** (D12), one per student, from your own mail; nothing in the app sends them.
5. **The one restart, in the evening, announced to the rehearsal members first** (every session signs out):

   ```sh
   $PY -m ops.promotion.admission restart --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --output "$TRIAL/restart-$(date -u +%Y%m%d).json" --announced
   ```

   Expected: `{"status": "restarted", "service": "<ServiceName>", "receipts": [...]}` after the forced deployment settles and the boundary (same images, template, parameters and schedules; healthy targets) is re-proved. Without `--announced` it refuses. Then `/health/ready` → 200 and one code tried on the invite page reaches the provider's sign-up. **No deploys after this.**

**Not available: renaming the organization.** The operator has only `provision --display-name`, which creates an organization; there is no command that changes an existing display name, so the trial organization keeps the name it was provisioned with unless W1/W2 add a rename.

## 5. Removing a member

1. On **`/admin`** (W2; requires your `admin_policy` row): the member's row, **Remove**, confirm. W1's `admin_remove_member` revokes the membership and entitlement, advances the epoch (their session ends at the next request) and audits; it refuses yourself and the last admin.
2. If their code was never redeemed, `admission revoke --code <code_id>` instead.
3. **Their uploads**, with your own identity (the task lists, you delete every S3 version and delete marker):

   ```sh
   $PY -m ops.promotion.admission delete-uploads --state-bucket ... --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" \
     --principal "<principal id>" --output "$TRIAL/delete-uploads-<principal>.json"
   ```

   Expected: `{"status": "deleted", "files": F, "versions": V, "keys": ["o/<org>/b/<box>/f/<file>", ...], ...}`. It refuses a key outside `o/<org>/b/`, a truncated listing (more than 100 files: inspect first) and leaves the `box_files` rows for the purge.

## 6. Alarms, the deny-all switch and rollback

**Alarms** (Nov 23, after release B). `ops/monitoring/alarms.json` is a standalone stack: an SNS topic with your email, alarms on load-balancer and target 5xx, unhealthy targets, no running task (the ECS metric sample count, since Container Insights is off) and database free storage under 2 GiB, plus an AWS Budget at the project ceiling. It references the beta's resources by identifier and changes none of them, so promotion's infrastructure digest is untouched.

```sh
ALB_DNS=$(out LoadBalancerDnsName)
ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
# The main service's target group is the one that is not port 3001 (the broker's).
TG_ARN=$(aws elbv2 describe-target-groups --load-balancer-arn "$ALB_ARN" --query "TargetGroups[?Port!=\`3001\`].TargetGroupArn" --output text)
aws cloudformation deploy --stack-name forma-beta-trial-alarms --template-file ops/monitoring/alarms.json --no-fail-on-empty-changeset \
  --parameter-overrides NotificationEmail=<your address> LoadBalancerFullName="${ALB_ARN#*loadbalancer/}" TargetGroupFullName="${TG_ARN##*:}" \
  ClusterName="$CLUSTER" ServiceName="$SERVICE" DBInstanceIdentifier=$(out DatabaseEndpoint | cut -d. -f1) \
  MonthlyBudgetUsd=248
```

**Expected alarms.** `UnhealthyTargets` and `NoRunningTask` treat missing data as breaching, so by design they fire during every promotion outage (the stop-both-services step, a few minutes) and at the scheduled stop (2026-11-24 14:00 UTC if release B has not executed; 2026-12-15 14:00 UTC after it), and recover when the service is back. On a class day there is no deploy, so any of these is real. `LoadBalancer5xx` also fires briefly while targets drain during a promotion.

Confirm the subscription from the email SNS sends, then the test email: `aws sns publish --topic-arn "$(aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query "$(printf "$OUT" AlarmTopicArn)" --output text)" --message "trial alarm test"` arrives within a minute. `aws cloudwatch describe-alarms --alarm-name-prefix forma-beta-trial-alarms --query "MetricAlarms[].{n:AlarmName,s:StateValue}"` shows every alarm `OK` once each has data (the free-storage and running-task alarms within ten minutes). On Dec 1 redeploy with `MonthlyBudgetUsd=118`; after the trial `aws cloudformation delete-stack --stack-name forma-beta-trial-alarms`.

**Deny-all** (a bad image or an exposed route; [DEPLOY-RUNBOOK "Rollback"](../pitch/DEPLOY-RUNBOOK.md#rollback)). It is the stack's own deny-all revision on the same image: every sign-in fails and every protected route answers 404, data untouched, and it is reversible in one call. The `just deploy rollback-deny` recipe is fenced since the promotion cutover; use the call directly:

```sh
aws ecs update-service --cluster "$CLUSTER" --service "$SERVICE" --task-definition "$(out RollbackTaskDefinition)"
aws ecs wait services-stable --cluster "$CLUSTER" --services "$SERVICE"
curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/canvas       # 404 (and sign-in fails)
```

Restore with the same call and the `TaskDefinition` output, then `services-stable` and `/health/ready` → 200. While the service is on the deny-all revision every promotion, renewal and back-office command refuses (the boundary check sees the drifted task definition): restore first. Tell the class in plain words what happened; fix forward through a release only in an announced window.

**Rollback of a release** is fixing forward: a new reviewed merge. The database is never rolled back by the pipeline; the pre-release snapshot protects the data and restoring it is the separately reviewed operation in the deploy runbook (the restore drill below rehearses the mechanics on a scratch instance).

## 7. Feedback

Feedback arrives at `feedback@getforma.dev` (the link in the app, D12). Read it daily, answer in a day, and keep three piles: *bug* (an agent files a ticket with the exact steps and the member's words, no address), *wish* (the backlog), *security or data* (a member sees another's entries, an export leaks, a session survives removal: deny-all first, then the fix). Between Dec 1 and 12 fixes land only in announced windows, announced on the day before by your own email and by a note in the app's feedback link text, and never on a class day. Keep `/admin` open during the class: consent, activity and removal are there.

## 8. Scan disposition renewal (if release B's preparation refuses on new findings)

The current disposition covers release A only (APP-175: contract 14a161a0…, 14 findings, expiry Nov 20). Once A lands those findings are baseline, and B needs a disposition only if the scanner introduces more. If B's preparation refuses with `new scan findings require separately reviewed disposition`:

1. `gh variable set PROMOTION_ENABLED --body false`; after the 65-minute fence, `maintenance abandon` (section 1 step 3; the pre-change-set case needs every journal entry to be a confirmed ECR upload, which this refusal leaves).
2. Extract, privately, each role's baseline findings from the canonical state (`live.registry.<role>` → `{"manifest_digest": ..., "findings": [...]}`) and fetch each candidate image's scan: `aws ecr describe-image-scan-findings --repository-name genui-synthetic --image-id imageDigest=<the summary's digest> > $TRIAL/scan-service.json` (and `genui-qualification`, `genui-operator`).
3. Generate the sanitized delta and the disposition with the renewal tool (no AWS call, nothing retyped):

   ```sh
   $PY -m ops.promotion.scan_renewal delta --contract <release B's "to" deployment contract sha256> \
     --baseline service=$TRIAL/baseline-service.json --baseline qualification=... --baseline operator=... \
     --current service=$TRIAL/scan-service.json --current qualification=... --current operator=... \
     --output evidence/APP-<ticket>/scan-delta-<date>.json
   $PY -m ops.promotion.scan_renewal generate --delta evidence/APP-<ticket>/scan-delta-<date>.json \
     --evidence evidence/APP-<ticket>/SCAN-DISPOSITION.md --expires-at-utc 2026-12-16T00:00:00Z --output ops/promotion/scan-disposition.json
   ```

   `--contract` is release B's `to` deployment contract: `schema_release.REVIEWED_SCHEMA_RELEASE['to']['deployment_contract_sha256']` (W1's pin, equal to `schema-release.json`'s `to`); the guard checks the disposition against the contract of the candidate being promoted. `scan_renewal check` binds the file to its delta but not to that pin; `test_scan.py`'s `ReleaseBindingTests` does (`test_contract_and_expiry_are_the_schema_releases`), so edit `ReleaseBindingTests.setUp` (`test_scan.py:186`) to read the new delta file and expect the new evidence path. The tool prints the finding names and the exact `REVIEWED_DISPOSITION` block for `ops/promotion/registry.py`. It refuses a CRITICAL finding and roles whose introduced sets differ. Paste the pin, write the evidence record in the APP-175 form (scope, method, each CVE's applicability, limits), and keep `schema-release.json`'s expiry equal to the disposition's (the tests bind them; W1 owns that file). `scan_renewal check --delta <the delta>` must print `"status": "bound"` and `python3 -m unittest ops.promotion.test_scan` must pass.
4. An independent review of the record, merge, `PROMOTION_ENABLED=true`, merge release B again.

If release A has **not** landed by Nov 19, the same `generate` with the APP-175 delta (`evidence/APP-175/scan-delta-2026-11-12.json`) and the new expiry extends A's disposition unchanged in scope; the schema release's expiry moves with it (W1). The committed file never changes until the coordinator says which case applies.

## 9. The restore drill (Nov 23, after release B, before any code goes out)

Prove that a snapshot restores and that the restored database is the live one: the same schema version and the same row counts for the organization. The comparison is the operator task's read-only `admission stats`, run once against the live database and once against the scratch instance through a one-off `PGHOST` environment override (the only time an override carries anything but the command; it points at a copy of the same data, under the same credentials). Do it while only your account exists, so the counts cannot move between the snapshot and the drill.

```sh
DB=$(out DatabaseEndpoint | cut -d. -f1)
SNAP=$(aws rds describe-db-snapshots --db-instance-identifier "$DB" --query "sort_by(DBSnapshots[?Status=='available'],&SnapshotCreateTime)[-1].DBSnapshotIdentifier" --output text)
aws rds describe-db-instances --db-instance-identifier "$DB" --query "DBInstances[0].{class:DBInstanceClass,subnets:DBSubnetGroup.DBSubnetGroupName,sg:VpcSecurityGroups[0].VpcSecurityGroupId}"
SCRATCH=forma-restore-drill-$(date -u +%Y%m%d)
aws rds restore-db-instance-from-db-snapshot --db-instance-identifier "$SCRATCH" --db-snapshot-identifier "$SNAP" \
  --db-instance-class <class> --db-subnet-group-name <subnets> --vpc-security-group-ids <sg> --no-publicly-accessible --no-multi-az --no-auto-minor-version-upgrade
aws rds wait db-instance-available --db-instance-identifier "$SCRATCH"            # 10–20 minutes
SCRATCH_HOST=$(aws rds describe-db-instances --db-instance-identifier "$SCRATCH" --query "DBInstances[0].Endpoint.Address" --output text)
```

Compose the two task requests locally and run them (`$W` is any private directory; `--window-id` is a fresh UUID for the client tokens):

```sh
aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation > "$TRIAL/stack.json"
W=$(python3 -c 'import uuid;print(uuid.uuid4())')
python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$W" --step drill-live \
  --override admission-stats --set=--organization="$TRIAL_ORG" --output "$TRIAL/drill-live.json"
python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$W" --step drill-scratch \
  --override admission-stats --set=--organization="$TRIAL_ORG" --output "$TRIAL/drill-scratch.json"
jq --arg h "$SCRATCH_HOST" '.overrides.containerOverrides[0].environment = [{"name": "PGHOST", "value": $h}]' "$TRIAL/drill-scratch.json" > "$TRIAL/drill-scratch.env.json"
for f in drill-live drill-scratch.env; do
  aws ecs run-task --cli-input-json "file://$TRIAL/$f.json" > "$TRIAL/$f.receipt.json"
  T=$(jq -r .tasks[0].taskArn "$TRIAL/$f.receipt.json"); aws ecs wait tasks-stopped --cluster "$CLUSTER" --tasks "$T"
  aws logs get-log-events --log-group-name <the operator task's awslogs group> --log-stream-name "genui-operator/operator/${T##*/}" --start-from-head \
    | jq -r '.events[].message' | grep '^{' | tail -1 > "$TRIAL/$f.stats.json"
done
diff <(jq -S . "$TRIAL/drill-live.stats.json") <(jq -S . "$TRIAL/drill-scratch.env.stats.json") && echo "restore drill: identical"
```

Expected: both print `{"organization_id": ..., "schema_version": 23, "counts": {...}, "audit_heads": {...}}` and the diff is empty. The operator task's log group is in its task definition (`aws ecs describe-task-definition --task-definition <OperatorTaskDefinition output> --query "taskDefinition.containerDefinitions[0].logConfiguration.options"`). Then delete the scratch instance; it is never left running:

```sh
aws rds delete-db-instance --db-instance-identifier "$SCRATCH" --skip-final-snapshot --delete-automated-backups
aws rds wait db-instance-deleted --db-instance-identifier "$SCRATCH"
```

Record the snapshot id, the two task ARNs and the diff result in the trial's private evidence. A real restore of the live database is the separate reviewed operation in the deploy runbook (new instance, service stopped, endpoint repointed, IAM `rds-db:connect` re-pointed); this drill proves the data, not that procedure.

## 10. The end-of-trial purge (by Sunday Dec 14)

The charter promises the organization's data is purged by Dec 14; the beta then stops at its Dec 15 14:00 UTC schedule. In order:

1. `admission list`; `admission revoke --code` for every code that is still `active` (so nobody can join after the class).
2. `admission delete-uploads --principal` for every member who uploaded (the Admin page's per-member upload count, W1's overview, tells you who): every S3 version of their files is gone.
3. On `/admin`, remove every member but yourself (the last admin cannot be removed).
4. `admission stats` (through the operator task, as in the drill, against the live database): the counts after steps 1–3 are the record of what remains.
5. **The organization's rows.** No operator command deletes an organization's rows today: memberships are revoked, not deleted; `box_files`, records, apps and audit chains stay. The purge of those rows is a Tier A operator command that does not exist yet (`admission purge-organization`, refusing a live member and requiring the organization inactive), or a reviewed one-off procedure under Victor's word. Decide before Dec 7; this runbook will carry the command once it is reviewed. Until then, step 4's `stats` output and the Dec 15 stop are the honest record, and the snapshots of the trial period are deleted when the charter's retention says so.
6. After the stop (Dec 15 14:00 UTC): both services at count 0 (`aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount}"` → 0 / 0); the alarms stack deleted (section 6); the retained database, keys and load balancer still cost money until a separate decision.
