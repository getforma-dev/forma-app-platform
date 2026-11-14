# Real-user trial runbook (Victor's class, November 29–30)

The operator steps for the trial decided on 2026-11-14 (the plan's calendar; the charter is the ADR-008 amendment; the ceilings and the expiry are the [ADR-019 amendment](../architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md#amendment-2026-11-14-the-real-user-trial-its-expiry-and-its-ceilings)). Every step below is Victor's: an authorized non-workflow AWS identity, the repository's locked venv (`app/runtime/.venv/bin/python`, called `$PY` here), and private output under a 0700 directory. Agents prepare and check; they call no AWS API, send no email and approve nothing. Expected outputs are the exact JSON shapes the commands print; anything else stops the step. Section 11 is the checklist of what remains after release B; it names the steps the operator agent may run under Victor's permission, and only those.

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

**Done on 2026-11-14 at 07:17 UTC; do not re-run.** Run 43564578519 (candidate `511d97e3`) went live and was sealed as `beta/20261114-511d97e`. The live expiry is 2026-11-24T14:00Z. The record is [`evidence/APP-175/release-2026-11-14.md`](../../evidence/APP-175/release-2026-11-14.md). The steps below are kept as written, for reference.

Run 43157872671 was stuck on a stale ledger basis, and its packet was never approved. Promotion was paused (`PROMOTION_ENABLED=false`).

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

   The preparation summary must show `schema_release: {from_version: 20, to_version: 22, spec_sha256: ...}`, the sequence starting with `snapshot database`, the scan disposition hash `f00adb9a047ce880535966ef5551d543b611be31c144be08efbc3e0345b5025b` (the 2026-11-14 renewal; 16 findings per role) with its Nov 20 expiry, and a stop time (the proposed expiry) 18 hours before the current backstop. Approve only if it does.

6. **Approve** in the run's **Review deployments**, outside **05:00–05:45 UTC** (the RDS backup window; a refused snapshot leaves beta serving and needs a new merge) and within 24 hours of step 4. Execution: snapshot, stop, change set, migration 21–22, activate, health. Then:

   ```sh
   curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/health/ready     # 200
   gh run view <run id> --repo $REPO --json conclusion                                   # "success"
   ```

   The run's completed record names schema 22, the snapshot and the migration task. If the migration fails, beta stays down with the lock held: BETA-PROMOTION "Restoring the previous release after a failed migration"; nothing is retried by hand.

7. **Follow-up**: remove `ops/promotion/schema-release.json` and its pin in a pull request once the release succeeds (W1 replaces them with release B's).

## 2. The expiry and month renewal (Sunday Nov 23, before release B)

**Done on 2026-11-14; do not re-run.** Read back on 2026-11-14: the stack parameter `DeploymentExpiresAtUtc` and the expiry schedule both say `2026-12-15T14:00:00Z`, which is the renewed canonical expiry (`2026-12-16T08:00:00Z`) less release B's 18 hours, so the renewal below was applied before release B. The steps are kept as written, for reference.

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

## 3. Release B: migrations 0023 and 0024 and the trial capabilities (Sunday Nov 23)

**Steps 1–4 done on 2026-11-14; do not re-run.** Release B executed on Nov 14, not Nov 23: read back on 2026-11-14, the stack's expiry parameter and schedule say `2026-12-15T14:00:00Z` and the release snapshot `forma-schema-release-<8 hex>-v24` exists beside release A's `-v22`. The record is APP-194's close-out. Steps 5–8 remain; [section 11](#11-nov-23-operator-checklist) is their checklist (step 5's Anthropic spend limit is item 11.1a; step 6, the FormaStack email, and step 8, a helper as reviewer, stay as written here). The steps are kept as written, for reference.

The release-B pull request (APP-194) commits `ops/promotion/schema-release.json` for 22 → 24 (migrations 0023 and 0024) with its pins, expiring 2026-11-26 00:00 UTC ([record](../../evidence/APP-194/SCHEMA-RELEASE.md)); W3's operator changes (`--note`, `admin_policy`, `list-box-files`, `stats`) ride the same operator image. Within 24 hours of section 2:

1. If `main` carries new scanner findings, preparation refuses before approval: section 8 (the scan disposition renewal). Otherwise:
2. `gh variable set PROMOTION_ENABLED --body true --repo $REPO`, merge the reviewed release-B pull request, read the summary: `schema_release: {from_version: 22, to_version: 24}`, the stop time **2026-12-15T14:00:00Z**, the disposition (if any) with its 2026-11-26 expiry, the release file's.
3. Approve outside 05:00–05:45 UTC. Then `/health/ready` → 200, `gh run view ... --json conclusion` → `"success"`.
4. **Verify from the back office** (the operator task now has `stats`):

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   The expiry is in the live parameters of the state the apply wrote; `aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation --query "Stacks[0].Parameters[?ParameterKey=='DeploymentExpiresAtUtc'].ParameterValue" --output text` prints `2026-12-15T14:00:00Z`, and `aws scheduler get-schedule --group-name <DeploymentExpiryGroup output> --name <schedule>` shows `at(2026-12-15T14:00:00)`.
5. **Deploy the alarms** (section 6), **set the Anthropic spend limit** (about $150 for the month, in the Anthropic console), and run the **restore drill** (section 9) while only Victor's account exists.
6. **Write to FormaStack** (one email, kept with the trial's private evidence): raise the fAuth tenant's user limit to at least 30 by Nov 28, and **delete the tenant's users (or the tenant) by Dec 14** on your word, because the students' sign-in accounts and addresses live there, not in the beta's database (section 10 step 6). Ask for a written acknowledgement now and a written confirmation after the deletion; record both. Without the acknowledgement the notice's "deleted by December 14" is not true for the sign-in accounts, and the Nov 28 go/no-go says no.
7. **Victor's admin row** (Monday Nov 24, before building the class apps; the Admin page requires it). Your principal id is in `admission list`'s redemption of the owner code:

   ```sh
   $PY -m ops.promotion.admission grant-admin --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --principal "<your principal id>" --output "$TRIAL/grant-admin.json"
   ```

   Expected: `{"status": "granted", "permission": "admin_policy", "changed": true, ...}`. A grant to the wrong member is undone with `revoke-admin` (same arguments) → `{"status": "revoked-admin", "changed": true, ...}`; it advances that member's epoch.
8. **A helper as reviewer of one app's work**, and taking it back. A reviewer sees and changes every entry of the work's own-visibility collections (D7, W4). The app and work ids are in the app's URL and the share dialog; the member's principal id is in `admission list`'s redemptions.

   ```sh
   $PY -m ops.promotion.admission grant-reviewer --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --principal "<helper principal id>" --app "<app id>" --work "<work id>" --output "$TRIAL/grant-reviewer-<work>.json"
   $PY -m ops.promotion.admission revoke-reviewer --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --principal "<helper principal id>" --app "<app id>" --work "<work id>" --output "$TRIAL/revoke-reviewer-<work>.json"
   ```

   Expected: `{"status": "granted-reviewer", "permission": "record_review", "changed": true, ...}` and `{"status": "revoked-reviewer", "permission": "record_review", "changed": true, "membership_epoch": N+1, ...}`. The grant needs release B's migration 0024 in the database (before it the task refuses `permission_unsupported` and writes nothing); the revoke works at any schema, and a revoke of a row that does not exist reports `"changed": false`. The revoke advances the member's epoch, so their next request re-reads their authority. The sharing dialog's own reviewer revoke is W4's; this is the operator's path for any personal `record_review` grant. The operator's revoke removes a personal `record_review` row only. It does not end a reviewer share with everyone in the organization, which the app's owner stops in the share dialog ("Stop sharing with everyone"), and the dialog shows each person's access from live permissions.

## 4. Codes by wave and the one announced restart (Friday Nov 28)

**Go/no-go before the first wave**, each with its evidence: release B executed and `/health/ready` 200; the caps, December month and expiry applied (section 2) and the stop reads 2026-12-15 14:00 UTC; the alarms stack deployed and the test email received; the restore drill's diff empty; FormaStack's written acknowledgement of the user-limit raise and the Dec 14 deletion (section 3 step 6); the purge path proven on the dev stand with no guest-linked share open, or the R1 decision taken (section 10); your `admin_policy` row granted and `/admin` opening; the rehearsal of Nov 26 done on Safari and a phone. Any "no" delays the codes, not the class date.

The back office is `ops/promotion/admission.py`. It mints the codes on your machine, writes the one-shot `OperatorInvitationInput` secret, runs one operator task per code (`admission issue --code-digest --provider-input-from-secret --authoring-only --note`), and **always empties the secret afterwards, even on failure**. The codes go only to the private `--output` file (0600). Nothing reaches the service until the restart: the service loads the capability secret once per task start (at most 128 entries), so one restart covers every code issued before it.

1. **Recipients.** `$TRIAL/wave-1.json`, one entry per student. The note is what you will see in `list` and what every organization admin sees beside the code on `/admin`: **a seat number or initials, never a full name or an address**, because it travels in the RunTask override and so in CloudTrail's request parameters, `DescribeTasks`, the operator log and the receipt. The address stays in this file and the one-shot secret only:

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
   Reading the audit afterwards: a removal from `/admin` writes `membership.revoked` and `entitlement.revoked` on the **admission** chain with the actor being you (the admin) and the target's digest in the outcome; the operator channel's `revoke-membership` and `revoke-entitlement` write the same kinds on the **operator** chain with the actor being the target and no digest. Branch on `writer_role` before reading the actor. Your own membership was operator-seeded, so `/admin` shows no consent row for you; that is expected, every student has one.
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

## 8. Scan disposition renewal (if a preparation refuses on new findings)

No disposition is committed. Release B's (APP-194: CVE-2025-6020, pam, one per role, `e35d6b45…`, expiry Nov 26) was made by this section on 2026-11-14, after release B's first preparation refused, and was spent when release B went live the same day; it was removed in release B's close-out ([record](../../evidence/APP-194/release-2026-11-14.md)). Release A's (APP-175: 16 findings per role, `f00adb9a…`, expiry Nov 20) was spent the same way. Their findings are baseline now, so a later promotion needs a disposition only if the scanner introduces more. If a preparation refuses with `new scan findings require separately reviewed disposition`:

1. `gh variable set PROMOTION_ENABLED --body false`; after the 65-minute fence, `maintenance abandon` (section 1 step 3; the pre-change-set case needs every journal entry to be a confirmed ECR upload, which this refusal leaves).
2. Extract, privately, each role's baseline findings from the canonical state (`live.registry.<role>` → `{"manifest_digest": ..., "findings": [...]}`) and fetch each candidate image's scan: `aws ecr describe-image-scan-findings --repository-name genui-synthetic --image-id imageDigest=<the summary's digest> > $TRIAL/scan-service.json` (and `genui-qualification`, `genui-operator`).
3. Generate the sanitized delta and the disposition with the renewal tool (no AWS call, nothing retyped):

   ```sh
   $PY -m ops.promotion.scan_renewal delta --contract <the promoted candidate's deployment contract sha256> \
     --baseline service=$TRIAL/baseline-service.json --baseline qualification=... --baseline operator=... \
     --current service=$TRIAL/scan-service.json --current qualification=... --current operator=... \
     --output evidence/APP-<ticket>/scan-delta-<date>.json
   $PY -m ops.promotion.scan_renewal generate --delta evidence/APP-<ticket>/scan-delta-<date>.json \
     --evidence evidence/APP-<ticket>/SCAN-DISPOSITION.md --expires-at-utc <expiry> --output ops/promotion/scan-disposition.json
   ```

   `--contract` is the contract of the candidate being promoted, which the guard checks: for a schema release its `to` contract (`schema_release.REVIEWED_SCHEMA_RELEASE['to']['deployment_contract_sha256']`; release B's was `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356`), and for a normal promotion the baseline's, which the approval summary shows and which a normal promotion never changes. `scan_renewal check` binds the file to its delta. `test_scan.py`'s `ReleaseBindingTests` run whenever a disposition is committed: they bind it to its evidence record and the one `scan-delta-*.json` beside it (keep exactly one delta there), and, only while `schema-release.json` is committed, to that release's `to` contract and expiry, so the disposition's expiry must then equal the release file's (release B's was 2026-11-26T00:00:00Z). With no release committed, the inert source pin binds nothing and the reviewer chooses the expiry: after the promotion's planned approval and before the deployment's stop (integration review MINOR-1). The tool prints the finding names and the exact `REVIEWED_DISPOSITION` block for `ops/promotion/registry.py`. It refuses a CRITICAL finding and roles whose introduced sets differ. Replace `REVIEWED_DISPOSITION = None` with the printed pin (the file and its pin land together, and a test checks it) and write the evidence record in the APP-175 form (scope, method, each CVE's applicability, limits). `scan_renewal check --delta <the delta>` must print `"status": "bound"` and `python3 -m unittest ops.promotion.test_scan` must pass.
4. An independent review of the record, merge, `PROMOTION_ENABLED=true`, merge the change to promote again. Once that release is live, remove the file and its pin in its close-out.

(Release A landed on 2026-11-14, so the earlier "if release A has not landed by Nov 19" extension no longer applies.)

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

Expected: both print `{"organization_id": ..., "schema_version": 24, "counts": {...}, "audit_heads": {...}}` and the diff is empty. The operator task's log group is in its task definition (`aws ecs describe-task-definition --task-definition <OperatorTaskDefinition output> --query "taskDefinition.containerDefinitions[0].logConfiguration.options"`). Then delete the scratch instance; it is never left running:

```sh
aws rds delete-db-instance --db-instance-identifier "$SCRATCH" --skip-final-snapshot --delete-automated-backups
aws rds wait db-instance-deleted --db-instance-identifier "$SCRATCH"
```

Record the snapshot id, the two task ARNs and the diff result in the trial's private evidence. A real restore of the live database is the separate reviewed operation in the deploy runbook (new instance, service stopped, endpoint repointed, IAM `rds-db:connect` re-pointed); this drill proves the data, not that procedure.

## 10. The end-of-trial purge (by Sunday Dec 14)

The charter promises the organization's data is deleted by Dec 14; the beta then stops at its Dec 15 14:00 UTC schedule. The purge is two operator tasks and one object sweep, all through `admission.py`, after the memberships are gone. **What it keeps**, and what the charter and the notice must say: the organization row (suspended), its provider binding and sign-in routing, its record sequence, and the hash-chained audit heads and events (identifiers, kinds and hashes; no content, no name, no address). Everything else of the organization goes: members, principals, codes, apps, works, records, uploads, shares, sessions, cards, and every object version under `o/<org>/`. Manual snapshots are deleted the same day; automated backups lapse seven days later, so the bytes are fully gone about **Dec 21**.

**`genui_identity`, the guest side of live sharing.** Guest sign-in accounts and their sessions are never the organization's and stay. The organization's share sign-in flows (`shared_auth_flows`) are deleted with the purge. That includes any short-lived (10-minute) flow routed through the trial tenant as provider for another organization's share; such a flow simply has to be started again. A guest's `linked_apps` and `share_events` rows (identifiers, kinds, versions and hashes; no content) name the organization's shares with keys the purge cannot cut, so **the purge refuses `guest_linked_shares_present` and deletes nothing** while any guest has accepted or linked one of the organization's live shares. Do not force it. **Revoking the shares does not clear the refusal**, because removed links and chain rows remain. The fix is a follow-up after migration 0024 is live, which drops the `share_events` key: at schema 24 or later the purge deletes the organization's `linked_apps` rows under the lifted FORCE and keeps the identifier-only `share_events`, as the charter (ADR-008) says; below 24 it refuses as now. That follow-up must land and be reviewed before Dec 14 (APP-188). Until then, expect the refusal if any guest ever accepted one of the organization's shares. If a purge refuses with the generic `admission_operator_refused`, the likely cause is a statement that outran `connect()`'s 5-second statement timeout or 3-second lock timeout (the identity tables are briefly locked exclusively): nothing was deleted; run it again off-hours, with no one signed in. The purge also requires exactly the five audit heads an organization is provisioned with (`workspace`, `control`, `data`, `admission`, `operator`); the trial organization has them, an older hand-made one might not, and such a one refuses rather than guess which chains to keep.

1. **Codes.** `admission list`; `admission revoke --code` for every code still `active`.
2. **Members.** On `/admin`, remove every member but yourself; then your own membership through the operator channel, which the Admin page refuses for the last admin:

   ```sh
   # compose and run the documented override with your principal id, as in section 9's task loop:
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$(python3 -c 'import uuid;print(uuid.uuid4())')" \
     --step revoke-self --override admission-revoke-membership --set=--organization="$TRIAL_ORG" --set=--principal="<your principal id>" --output "$TRIAL/revoke-self.json"
   aws ecs run-task --cli-input-json "file://$TRIAL/revoke-self.json"
   ```

   Expected log line: `{"principal_id": ..., "changed": true, "membership_epoch": N}`. Every session is now dead.
3. **Suspend** (no sign-in, no share reads; reversible only by a reviewed change):

   ```sh
   $PY -m ops.promotion.admission suspend --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --output "$TRIAL/suspend.json"
   ```

   Expected: `{"organization_id": ..., "status": "suspended", "changed": true, "audit_event": null, ...}`. There is no `organization.suspended` audit kind (D7.8); the receipt is the record.
4. **Purge**, typing the organization id a second time:

   ```sh
   $PY -m ops.promotion.admission purge --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --confirm "$TRIAL_ORG" --output "$TRIAL/purge.json"
   ```

   It runs `admission stats` (the counts before), `admission purge-organization` (refuses `organization_not_suspended` or `active_memberships_remain`; otherwise one transaction, children before parents over every `genui` table with an `organization_id` except the five kept ones, plus the organization's `code_index` rows; refuses `purge_incomplete` or `purge_audit_changed` and rolls back if anything remains or the chains moved), `admission stats` again (every count `0`, or it stops before touching an object), then lists every version and delete marker under `o/<org>/` (box files `b/…/f/…` and source objects `s/…`; any other key refuses), writes the intent receipt, deletes in batches of 1000 and lists again. Expected: `{"status": "purged", "rows_deleted": R, "tables": T, "audit_events_kept": E, "counts_after": {all 0}, "objects_deleted": O, "next": "Delete the manual snapshots ...", ...}`. The file keeps the counts before and after. A repeat is harmless (`rows_deleted: 0`, `objects_deleted: 0`).
5. **Snapshots.** Delete every manual snapshot of the instance the same day; the automated ones expire with the seven-day retention:

   ```sh
   aws rds describe-db-snapshots --db-instance-identifier "$DB" --snapshot-type manual --query "DBSnapshots[].DBSnapshotIdentifier" --output text
   for s in <each forma-schema-release-* and drill snapshot listed>; do aws rds delete-db-snapshot --db-snapshot-identifier "$s"; done
   aws rds describe-db-instances --db-instance-identifier "$DB" --query "DBInstances[0].BackupRetentionPeriod"     # 7
   ```

   Record the snapshot ids and the date; seven days later the last automated backup containing the data is gone.
6. **FormaStack.** Send the deletion request agreed on Nov 23 (section 3 step 6): delete the trial tenant's users, or the tenant, by Dec 14. The beta's `principals` rows (issuer, tenant, subject) and `provider_invitations` are gone with step 4; the sign-in accounts and addresses are FormaStack's. Record their written confirmation with the purge receipt; until it arrives the notice's "deleted by December 14" is not true for the accounts.
7. **The capability secret.** `revoke` removes a code's capability; redeemed and exhausted codes leave theirs until the operator's reconcile sweep removes every capability that can no longer be handed off. Run it once, after step 4, so the secret names no code of the purged organization (the override takes the attached management secret, never argv):

   ```sh
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$(python3 -c 'import uuid;print(uuid.uuid4())')" \
     --step reconcile-final --override admission-reconcile --set=--organization="$TRIAL_ORG" --output "$TRIAL/reconcile-final.json"
   aws ecs run-task --cli-input-json "file://$TRIAL/reconcile-final.json"      # then wait, read the log line as in section 9
   aws secretsmanager get-secret-value --secret-id "$(out ProviderCapabilitySecretArn)" --query SecretString --output text | python3 -c 'import json,sys; m=json.load(sys.stdin); print(len(m), "entries;", sum(k.startswith(sys.argv[1]) for k in m), "for the trial organization")' "$TRIAL_ORG"
   ```

   Expected: the task's log line names the capabilities it ended, and the second command prints `0 for the trial organization`.
8. **After the stop** (Dec 15 14:00 UTC): both services at count 0 (`aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount}"` → 0 / 0); the alarms stack deleted (section 6); the retained database, keys and load balancer still cost money until a separate decision.

## 11. Nov 23 operator checklist

What is left of sections 2–10 after release B, as a checklist: for each item the commands, the output to expect, who runs it, how to verify it and how to undo it. Written 2026-11-14 from a read-only readback of the beta account and the code on `main` at `140a5a9f`; nothing was deployed, run or sent. Section 11.6 lists what it found stale or incomplete in sections 2–10.

**Standing on 2026-11-14 (read-only).** Release B executed on Nov 14: the stack parameter `DeploymentExpiresAtUtc` and the expiry schedule both read **2026-12-15T14:00:00Z**, and the release's manual snapshot `forma-schema-release-<8 hex>-v24` exists beside release A's `-v22`, so sections 2 and 3 are done (their "Nov 23" headings are the plan's dates, kept as written). No alarms stack, SNS topic, CloudWatch alarm or AWS Budget exists in the account yet. The operator identity's permissions cover every step below. The operator task signs in to the database as `genui_migration` with the Secrets Manager password (`infra/src/stack.ts:609–617`), not an IAM token, so a restored copy accepts it unchanged; the application roles sign in with IAM tokens bound to the live instance's resource id (`app/runtime/runtime.py:704`), which matters only for item 3.

**Who runs what.** Victor's word or Victor's spending: **item 1** (a new stack in his account with his address: a topic, five alarms and a budget), **item 1a** (his Anthropic console) and **item 4** (irreversible); **item 3** is decided and needs nothing from him (11.3). The operator agent may run **items 2 and 5** under Victor's existing operator permission (his identity, the locked venv and `$TRIAL` of this file's header), and may prepare item 4's request files and read-only listings; the header's "agents call no AWS API" is relaxed for exactly those steps. Every `ops.promotion` command takes a fresh `--expected-state-sha256` from `maintenance inspect`. An agent that must not block replaces every `aws … wait` below with one `describe` call and a later return.

Setup, after the header's block:

```sh
DB=$(out DatabaseEndpoint | cut -d. -f1)
ACCOUNT=$(aws sts get-caller-identity --query Account --output text)
aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation > "$TRIAL/stack.json"
LOG_GROUP=$(aws ecs describe-task-definition --task-definition "$(out OperatorTaskDefinition)" \
  --query 'taskDefinition.containerDefinitions[0].logConfiguration.options."awslogs-group"' --output text)
```

### 11.1 Alarms and the budget (Victor)

Section 6's stack, with the checks it lacks. The template creates nothing inside the beta's stack and no IAM resource (`validate-template` reports no capability), and `ops/promotion/test_monitoring_alarms.py` pins it.

1. **Derive the parameters and look at them** before deploying. Each derivation was run against the live stack on 2026-11-14 and matches the template's `AllowedPattern`; the target group that is not port 3001 is the main service's on port 3000.

   ```sh
   ALB_DNS=$(out LoadBalancerDnsName)
   ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
   TG_ARN=$(aws elbv2 describe-target-groups --load-balancer-arn "$ALB_ARN" --query "TargetGroups[?Port!=\`3001\`].TargetGroupArn" --output text)
   printf '%s\n' "${ALB_ARN#*loadbalancer/}" "${TG_ARN##*:}" "$CLUSTER" "$SERVICE" "$DB"
   ```

   Expected: five lines: `app/<name>/<16 hex>`, `targetgroup/<name>/<16 hex>`, the cluster name, the service name, the instance identifier. Two target groups would print on one line separated by a tab: stop and pick the port-3000 one by hand.

2. **Deploy**:

   ```sh
   aws cloudformation deploy --stack-name forma-beta-trial-alarms --template-file ops/monitoring/alarms.json --no-fail-on-empty-changeset \
     --parameter-overrides NotificationEmail=<your address> LoadBalancerFullName="${ALB_ARN#*loadbalancer/}" TargetGroupFullName="${TG_ARN##*:}" \
     ClusterName="$CLUSTER" ServiceName="$SERVICE" DBInstanceIdentifier="$DB" MonthlyBudgetUsd=248
   aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query 'Stacks[0].StackStatus' --output text
   ```

   Expected: `Waiting for changeset to be created..`, `Waiting for stack create/update to complete`, `Successfully created/updated stack - forma-beta-trial-alarms`; then `CREATE_COMPLETE`. The address becomes a stack parameter, readable by any identity with `cloudformation:DescribeStacks` in the account; it also lands in the shell history and in the budget's subscriber list. A `NoEcho` on `NotificationEmail` would be a one-line template change for APP-187, not for this checklist.

3. **Confirm the subscription** from the "AWS Notification - Subscription Confirmation" email SNS sends, then check it and send the test:

   ```sh
   TOPIC=$(aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query "$(printf "$OUT" AlarmTopicArn)" --output text)
   aws sns list-subscriptions-by-topic --topic-arn "$TOPIC" --query 'Subscriptions[].{p:Protocol,arn:SubscriptionArn}'
   aws sns publish --topic-arn "$TOPIC" --message "trial alarm test $(date -u +%FT%TZ)"
   ```

   Expected: one `email` subscription whose `arn` is a subscription ARN; `PendingConfirmation` means the click has not happened, and a publish then delivers nothing. `publish` prints `{"MessageId": "<uuid>"}` and the email arrives within a minute.

4. **Alarms and the budget**:

   ```sh
   aws cloudwatch describe-alarms --alarm-name-prefix forma-beta-trial-alarms --query "MetricAlarms[].{n:AlarmName,s:StateValue}"
   aws budgets describe-budget --account-id "$ACCOUNT" --budget-name forma-beta-trial-alarms-monthly-cost \
     --query 'Budget.{limit:BudgetLimit.Amount,unit:BudgetLimit.Unit,time:TimeUnit,type:BudgetType}'
   aws budgets describe-notifications-for-budget --account-id "$ACCOUNT" --budget-name forma-beta-trial-alarms-monthly-cost \
     --query 'Notifications[].{t:NotificationType,pct:Threshold}'
   ```

   Expected: five alarms, `forma-beta-trial-alarms-load-balancer-5xx`, `-target-5xx`, `-unhealthy-targets`, `-no-running-task` and `-database-free-storage`, `INSUFFICIENT_DATA` right after creation and `OK` within ten minutes; `{"limit": "248", "unit": "USD", "time": "MONTHLY", "type": "COST"}` (the amount may print as `248.0`); three notifications, `ACTUAL` 80, `ACTUAL` 100 and `FORECASTED` 100 (the API prints the thresholds as `80.0` and `100.0`, as it prints `248.0`; a forecast needs weeks of history and may stay silent in November).

**Verify.** Step 3's test email in the inbox and every alarm `OK` in step 4. An alarm that stays `ALARM` while `/health/ready` answers 200 has a wrong dimension: compare the five values of step 1 with the stack's parameters. `-unhealthy-targets` and `-no-running-task` fire by design during a promotion outage and at the Dec 15 stop (section 6).

**Roll back.** `aws cloudformation delete-stack --stack-name forma-beta-trial-alarms && aws cloudformation wait stack-delete-complete --stack-name forma-beta-trial-alarms` removes the topic, the subscription, the alarms and the budget and changes nothing of the beta. If the delete fails on a subscription that was never confirmed, repeat it with `--retain-resources EmailSubscription`; a pending subscription lapses by itself after three days. On Dec 1 the same deploy with `MonthlyBudgetUsd=118` is an update, and the confirmed subscription survives it.

### 11.1a The Anthropic spend limit (Victor; no command)

Section 3 step 5 and the plan's D10. In the Anthropic console, set the organization's monthly spend limit to **$150 for November** with the 50% alert on, and on Dec 1 to **$100**; raise November to $200 if Nov 29 spends over $60. What you see: the limit and the alert address on the console's limits page. Nothing on AWS changes; no agent can do or read this. Record the date and the two amounts in the trial's private evidence.

### 11.2 The restore drill (the operator may run it; a few cents of RDS)

Section 9 with three additions: a fresh snapshot, so that the two counts are of the same bytes; the live instance's class, subnet group and security group read into variables rather than retyped; and the log group derived. Nothing writes to the live database. Run it while only Victor's account exists, with Victor signed out from the snapshot until both stats tasks have stopped (a sign-in writes `sessions` and audit rows and the diff is then not empty), and outside 05:00–05:45 UTC: inside the backup window `create-db-snapshot` refuses `InvalidDBInstanceState`, and a manual snapshot of this single-AZ instance suspends I/O for a few seconds. The copy costs its instance-hours (db.t4g.small, a few cents an hour) and is deleted in the same hour. A second attempt the same day needs step 4 first, or the names collide (`DBSnapshotAlreadyExists`, `DBInstanceAlreadyExists`).

1. **Snapshot**:

   ```sh
   DRILL_SNAP=forma-restore-drill-$(date -u +%Y%m%d)-snap
   aws rds create-db-snapshot --db-instance-identifier "$DB" --db-snapshot-identifier "$DRILL_SNAP" --query 'DBSnapshot.{id:DBSnapshotIdentifier,status:Status}'
   aws rds wait db-snapshot-available --db-snapshot-identifier "$DRILL_SNAP"      # minutes; or one describe-db-snapshots ... Status later
   ```

   Expected: `{"id": "forma-restore-drill-<date>-snap", "status": "creating"}`, then `available`.

2. **Restore a copy** with the live instance's shape. The task security group is admitted by the database's security group, so the operator task reaches the copy; the live parameter group (the stack's, `rds.force_ssl=1`, `infra/src/stack.ts:222`) is applied by name; the copy inherits the snapshot's key and engine version and gets no deletion protection.

   ```sh
   read -r CLASS SUBNETS SG PG <<<"$(aws rds describe-db-instances --db-instance-identifier "$DB" \
     --query 'DBInstances[0].[DBInstanceClass,DBSubnetGroup.DBSubnetGroupName,VpcSecurityGroups[0].VpcSecurityGroupId,DBParameterGroups[0].DBParameterGroupName]' --output text)"
   SCRATCH=forma-restore-drill-$(date -u +%Y%m%d)
   aws rds restore-db-instance-from-db-snapshot --db-instance-identifier "$SCRATCH" --db-snapshot-identifier "$DRILL_SNAP" \
     --db-instance-class "$CLASS" --db-subnet-group-name "$SUBNETS" --vpc-security-group-ids "$SG" \
     --db-parameter-group-name "$PG" --no-publicly-accessible --no-multi-az --no-auto-minor-version-upgrade --no-deletion-protection \
     --query 'DBInstance.{id:DBInstanceIdentifier,status:DBInstanceStatus}'
   aws rds wait db-instance-available --db-instance-identifier "$SCRATCH"         # 10–20 minutes; or one describe-db-instances ... DBInstanceStatus later
   SCRATCH_HOST=$(aws rds describe-db-instances --db-instance-identifier "$SCRATCH" --query "DBInstances[0].Endpoint.Address" --output text)
   ```

   Expected: `{"id": "forma-restore-drill-<date>", "status": "creating"}`, then `available`; `$SCRATCH_HOST` is `forma-restore-drill-<date>.<id>.us-east-1.rds.amazonaws.com`.

3. **Compose both requests locally, point the second at the copy, run both, read each result line.** The `PGHOST` environment override on the second request is the one documented exception to command-only overrides (section 9); `operator.py` accepts it because the value matches its `HOSTNAME` rule and the CA pin is untouched.

   ```sh
   W=$(python3 -c 'import uuid;print(uuid.uuid4())')
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$W" --step drill-live \
     --override admission-stats --set=--organization="$TRIAL_ORG" --output "$TRIAL/drill-live.json"
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$W" --step drill-scratch \
     --override admission-stats --set=--organization="$TRIAL_ORG" --output "$TRIAL/drill-scratch.json"
   jq --arg h "$SCRATCH_HOST" '.overrides.containerOverrides[0].environment = [{"name": "PGHOST", "value": $h}]' "$TRIAL/drill-scratch.json" > "$TRIAL/drill-scratch.env.json"
   for f in drill-live drill-scratch.env; do
     aws ecs run-task --cli-input-json "file://$TRIAL/$f.json" > "$TRIAL/$f.receipt.json"
     T=$(jq -r '.tasks[0].taskArn' "$TRIAL/$f.receipt.json"); jq -c '{failures, status: .tasks[0].lastStatus}' "$TRIAL/$f.receipt.json"
     aws ecs wait tasks-stopped --cluster "$CLUSTER" --tasks "$T"                 # one to two minutes
     aws ecs describe-tasks --cluster "$CLUSTER" --tasks "$T" --query 'tasks[0].{stop:stopCode,exit:containers[0].exitCode}'
     aws logs get-log-events --log-group-name "$LOG_GROUP" --log-stream-name "genui-operator/operator/${T##*/}" --start-from-head \
       | jq -r '.events[].message' | grep '^{' | tail -1 > "$TRIAL/$f.stats.json"
   done
   test -s "$TRIAL/drill-live.stats.json" -a -s "$TRIAL/drill-scratch.env.stats.json" || echo "a stats file is empty: run get-log-events again"
   diff <(jq -S . "$TRIAL/drill-live.stats.json") <(jq -S . "$TRIAL/drill-scratch.env.stats.json") && echo "restore drill: identical"
   ```

   Expected: each compose prints `{"command": ["admission", "stats", "--organization", "<org>"], "override_bytes": N, "request": "<path>", "sha256": "<64 hex>"}`; each run `{"failures": [], "status": "PROVISIONING"}`, then `{"stop": "EssentialContainerExited", "exit": 0}`; each stats file one line, `{"organization_id": "<org>", "schema_version": 24, "counts": {"principals": N, "memberships": N, …, "sessions": N}, "audit_heads": {"admission": N, "control": N, "data": N, "operator": N, "workspace": N}, "audit_events": N}` (the 23 `counts` keys are `STATS_TABLES`, `app/runtime/admission_operator.py:1025`); the diff empty and `restore drill: identical`. `exit` 1 in the second task alone points at the copy's host or security group, not at the data. A non-empty diff confined to `sessions` means a sign-in or the service's idle-session sweep (`app/src/storage/sessions.rs:1176`, rows idle over an hour), not a bad restore.

4. **Delete the copy and the drill snapshot**, the same hour:

   ```sh
   aws rds delete-db-instance --db-instance-identifier "$SCRATCH" --skip-final-snapshot --delete-automated-backups --query 'DBInstance.DBInstanceStatus'
   aws rds wait db-instance-deleted --db-instance-identifier "$SCRATCH"
   aws rds delete-db-snapshot --db-snapshot-identifier "$DRILL_SNAP" --query 'DBSnapshot.Status'
   aws rds describe-db-instances --db-instance-identifier "$SCRATCH"
   ```

   Expected: `"deleting"`, then `"deleted"` for the snapshot, then an error `DBInstanceNotFound` for the copy.

**Verify.** The `restore drill: identical` line, `exit` 0 twice, the copy gone, and `aws rds describe-db-snapshots --db-instance-identifier "$DB" --snapshot-type manual --query "DBSnapshots[].DBSnapshotIdentifier"` listing only `forma-schema-release-*`. Record privately the snapshot id, both task ARNs, both receipts and both stats files; record publicly, in `evidence/APP-188/restore-drill-<date>.md`, the date, the snapshot id, both task ids (the UUID after the last `/`, never the ARN), `schema_version` and "diff: identical". That record closes APP-188's drill item and the go/no-go's "restore drill's diff empty".

**Roll back.** Step 4, at any point. Nothing in the live account changes except the snapshot and the copy, and both are deleted; a copy left running costs its hours until it is, so end with `aws rds describe-db-instances --query "DBInstances[?starts_with(DBInstanceIdentifier,'forma-restore-drill')].DBInstanceIdentifier"` printing an empty list. To run a task again, recompose it with a new `--window-id`; the same request file returns the same task.

### 11.3 The scratch-RDS herd measurement (decided: not attempted)

The plan recommends the 26-page `refresh-herd` against the restored copy during the drill, on Victor's word, as "an IAM policy for the restored instance and one task run, under $2" ([TRIAL-25-READINESS](../planning/TRIAL-25-READINESS.md); it is also APP-197's A6 trigger). On `main` at `140a5a9f` it cannot be run as described, for four reasons:

- `window_requests.py qualification-task` composes a RunTask for the `QualificationTaskDefinition` whose manifest names `local_harness.HOST`, the live endpoint, and refuses any `--rds-resource-id` but `local_harness.RESOURCE_ID`, the live instance's (`infra/scripts/window_requests.py:387–392`: "live RDS resource id differs from the pinned qualification target; stop").
- The qualification image runs exactly one test, `storage::managed_qualification::managed_readonly_entry`, APP-008's six read-only checks (`app/runtime/qualification_supervisor.py`, `ARGUMENTS`), never `storage::qualification::refresh_herd`.
- The herd campaign starts its own Docker PostgreSQL and has no external-database mode (`app/src/storage/qualification.rs`, module `refresh_herd`), and the copy is not publicly reachable.
- The application roles sign in with IAM tokens for the live resource id; a copy restored as in 11.2 has IAM authentication off (the restore's default) and a new resource id.

What it would take, as a reviewed change with Victor's word for each part: a scratch-target manifest (host and resource id) that the supervisor and the composer accept; the restore with `--enable-iam-database-authentication`; an `rds-db:connect` statement for the copy's resource id (`aws rds describe-db-instances --db-instance-identifier "$SCRATCH" --query 'DBInstances[0].DbiResourceId'`) on the qualification task role; and a herd entry in the qualification image that runs the service against an external database. The run would then cost the copy's extra hour and one 2-vCPU task for a quarter of an hour, under the plan's $2. Decided 2026-11-14 (the coordinator, under the plan's capacity re-decision): the scratch-RDS herd is not attempted. The class's evidence is the 15-page `refresh-herd` CI gate and the 25-way submission race (W4); the 26-page release-profile measurement is the local quiet-host run in the capacity statement. Item 3 is closed; the drill (11.2) runs without it, and APP-197's A6 stays an RDS-only option that beta evidence alone can trigger. Recorded in APP-188's log.

### 11.4 The end-of-test purge (Victor; by Dec 14)

Section 10 as written, with the order, who runs each step and what the operator may prepare. Step 3 is reversible only by a reviewed change; step 4 is irreversible.

**Before Dec 14, on `main`.** APP-212 (in review) implements the follow-up APP-188's log requires; on `main` at `140a5a9f` it is not yet merged. `purge_organization` refuses `guest_linked_shares_present` whenever any `genui_identity.linked_apps` or `share_events` row names the organization (`app/runtime/admission_operator.py:1142`), at every schema version; migration 0024 made the fix possible and did not make it. No read-only probe counts those rows (`admission stats` reads `genui` tables only), so the first signal is the purge's own refusal, which rolls its transaction back and deletes nothing. If any guest ever accepted or linked one of the organization's shares, the purge of Dec 14 requires APP-212 merged and promoted first (its release in an announced window); until then `guest_linked_shares_present` is the expected refusal. Settle it at the Nov 28 go/no-go: either APP-212 is on beta, or Victor records that no guest share was ever opened for the organization ("Stop sharing with everyone" in the share dialog does not clear the refusal).

Steps (a fresh `--expected-state-sha256` for each `ops.promotion` command; section 10 has steps 2–7 in full; the `list` and `revoke` forms are section 4 step 3):

1. **Codes.** The operator may run `admission list …` → `{"status": "listed", "count": N, "by_status": {…}, "truncated": false, "receipts": […]}`. Victor runs `admission revoke --code <code_id> …` for every code still `active` → `{"status": "revoked", "code_id": "<id>", "changed": true, "capability_phase": "completed", "provider_phase": "completed", …}`.
2. **Members.** Victor removes every member but himself on `/admin`, then his own membership through the composed `admission-revoke-membership` override (section 10 step 2; the operator may compose the file, Victor runs `aws ecs run-task`). Log line: `{"principal_id": "<his>", "changed": true, "membership_epoch": N}`. Every session is dead from here.
3. **Suspend.** Victor: `admission suspend …` → `{"organization_id": "<org>", "status": "suspended", "changed": true, "audit_event": null, "receipts": […]}`. Reversible only by a reviewed change.
4. **Purge.** Victor: `admission purge … --confirm "$TRIAL_ORG"` → `{"status": "purged", "organization_id": "<org>", "rows_deleted": R, "tables": T, "audit_events_kept": E, "kept": […], "counts_before": {…}, "counts_after": {every count 0}, "objects_deleted": O, "receipts": […], "next": "Delete the manual snapshots …"}`. Refusals, all before any object is deleted: `organization_unknown`, `organization_not_suspended`, `active_memberships_remain`, `purge_tables_differ`, `purge_foreign_children_unknown`, `purge_rls_state_refused`, `purge_rls_not_restored`, `guest_linked_shares_present` (before APP-212; after it, at schema 24, the organization's `linked_apps` rows are deleted and `share_events` kept), `purge_incomplete`, `purge_audit_changed`, or the generic `admission_operator_refused` (a statement outran the 5-second timeout; run it again off-hours). A repeat is harmless.
5. **Snapshots.** Victor: `aws rds describe-db-snapshots --db-instance-identifier "$DB" --snapshot-type manual --query "DBSnapshots[].DBSnapshotIdentifier" --output text`, then `aws rds delete-db-snapshot --db-snapshot-identifier <each> --query 'DBSnapshot.Status'` → `"deleted"`; `aws rds describe-db-instances --db-instance-identifier "$DB" --query "DBInstances[0].BackupRetentionPeriod"` → `7`. Today the manual snapshots are `forma-schema-release-<8 hex>-v22` and `-v24`; the drill snapshot is gone with 11.2.
6. **FormaStack.** Victor sends the deletion request agreed on Nov 23 (section 3 step 6); the written confirmation goes with the purge receipt.
7. **The capability secret.** Victor: the composed `admission-reconcile` override and `aws ecs run-task` (section 10 step 7), then the secret count → `… 0 for the trial organization`.
8. **After the stop**, Dec 15 14:00 UTC. The operator may read `aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount}"` → `0` and `0`. Victor deletes the alarms stack (11.1's roll-back command); `-no-running-task` and `-unhealthy-targets` are in `ALARM` from the stop until then, by design.

**Verify.** `counts_after` every `0` in the purge file, and no object version or delete marker left under the organization's prefix:

```sh
aws s3api list-object-versions --bucket "$(out EvidenceBucket)" --prefix "o/$TRIAL_ORG/" \
  --query '{versions: length(Versions || `[]`), markers: length(DeleteMarkers || `[]`)}'
```

Expected: `{"versions": 0, "markers": 0}`. Then the manual snapshot list empty, FormaStack's written confirmation in hand and the capability count `0`. Record all of it in the trial's private evidence and a sanitized note in `evidence/APP-188/`.

**Roll back.** None after step 3 (the suspension needs a reviewed change) and none after step 4 (the charter promises the deletion; the last automated backup holding the data lapses about Dec 21). Before step 3, stop at any refusal and investigate: a revoked code is reissued, a removed member re-invited.

### 11.5 Victor's `admin_policy` row (the operator may run it; Victor signs in and opens `/admin`)

Section 3 step 7. The Admin page needs the row; the grant needs Victor's principal id, which `admission list` shows as the redemption of his code.

1. **List** (operator):

   ```sh
   LIST="$TRIAL/list-$(date -u +%Y%m%dT%H%M%S).json"
   $PY -m ops.promotion.admission list --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --output "$LIST"
   jq '.codes[] | select(.redemptions | length > 0) | {code_id, note, redemptions: [.redemptions[] | {principal_id, membership_status, redeemed_at}]}' "$LIST"
   ```

   Expected: `{"status": "listed", "count": N, "by_status": {…}, "truncated": false, "receipts": […]}` on standard output (the codes are only in the file); the `jq` shows one redemption with `"membership_status": "active"`, whose `principal_id` is Victor's.

2. **If no redemption exists**, Victor's account does not exist yet. The operator issues one code to Victor's own address and restarts the service (required: the service loads the capability secret at task start, and no one is signed in yet to be signed out); Victor reads the code from the private file and redeems it at `https://beta.getforma.dev/invite`. No email is sent.

   ```sh
   printf '{"recipients": [{"email": "<your address>", "note": "owner"}]}\n' > "$TRIAL/wave-0.json"
   $PY -m ops.promotion.admission issue --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --count 1 --notes-file "$TRIAL/wave-0.json" \
     --identifiers "$PRIVATE_RECONCILIATION/identifiers.json" --expires-days 30 --output "$TRIAL/codes-wave-0.json"
   $PY -m ops.promotion.admission restart --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --output "$TRIAL/restart-$(date -u +%Y%m%dT%H%M%S).json" --announced
   curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/health/ready
   ```

   Expected: `{"status": "issued", "issued": 1, "failed": 0, "not_issued": 0, "capabilities_before": N, "capabilities_after": N+1, "capabilities_max": 128, "input_cleared": true, …}`; `{"status": "restarted", "service": "<ServiceName>", "receipts": […]}` once the forced deployment has settled; `200`. Then step 1 again. The students' Nov 28 restart (section 4 step 5) is still needed for their codes.

3. **Grant** (operator):

   ```sh
   $PY -m ops.promotion.admission grant-admin --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX" \
     --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" --principal "<Victor's principal id>" --output "$TRIAL/grant-admin.json"
   ```

   Expected: `{"status": "granted", "permission": "admin_policy", "principal_id": "<id>", "changed": true, "membership_epoch": N, "receipts": […]}`. `"changed": false` means the row already existed (and is not audited again). A refusal (`member_unknown`, `member_inactive`) means the id is not an active member's; `admission.py` prints "grant-admin task did not succeed".

**Verify** (Victor): `https://beta.getforma.dev/admin` opens with the members list; before the grant it answers the same "not found" page as any unknown address (a 404), by design. This is the go/no-go's "your `admin_policy` row granted and `/admin` opening".

**Roll back.** `revoke-admin` with the same arguments → `{"status": "revoked-admin", "permission": "admin_policy", "changed": true, "membership_epoch": N+1, …}`; it advances his epoch, so an open session re-reads its authority at its next request.

### 11.6 Checked against the code: what in sections 2–10 is stale or incomplete

Every command named below was compared with `main` at `140a5a9f` on 2026-11-14, and every AWS derivation with the live account, read-only.

- **Sections 2 and 3 are done**, executed on Nov 14 rather than Nov 23 (the live expiry parameter and schedule read `2026-12-15T14:00:00Z`; the `-v24` release snapshot exists). Their headings keep the plan's dates; the status lines at their tops say so. The release record is APP-194's close-out. Section 1's status line names release A's expiry; the live one is in section 2's.
- **Section 6 (alarms)**: every command and parameter is right, and `ops/promotion/test_monitoring_alarms.py` pins the template (5 tests). Missing: the subscription check before the test publish (11.1 step 3) and the budget readback (11.1 step 4).
- **Section 9 (drill)**: `window_requests.py operator-task --override admission-stats --set=--organization=…` exists exactly as written (`app/operator/overrides.json`, `admission-stats`; `infra/scripts/window_requests.py:490–497`); the log stream `genui-operator/operator/<task id>` matches the live task definition's `awslogs-stream-prefix` and container name; the restore and delete flags are valid. Incomplete: the expected stats line omits `audit_events`; the log group is a placeholder (derived in this section's setup); section 9 takes no snapshot although section 10 step 5 expects to delete "the drill snapshot" (11.2 takes one, so that the two counts are of the same bytes); section 9's restore names no parameter group, so the copy would get the engine default rather than the stack's `rds.force_ssl=1` group (11.2 passes it by name).
- **Section 10 (purge)**: `admission-revoke-membership`, `admission-reconcile`, `admission-suspend-organization` and `admission-purge-organization` exist, and `admission.py suspend` and `purge --confirm` print the shapes section 10 shows. Stale by omission: "the fix is a follow-up after migration 0024 is live" describes APP-212 (in review), not yet on `main`; the purge refuses `guest_linked_shares_present` at every schema version today (11.4).
- **Section 3 step 7 and section 4** (`grant-admin`, `issue`, `list`, `revoke`, `restart --announced`): as written; `--expires-days` accepts 1 to 30.
- **The header's** "agents call no AWS API" is relaxed for exactly the steps this section marks "operator".
- **The plan's scratch-RDS herd** is in TRIAL-25-READINESS, not in section 9, and has no runnable path on `main`; decided 2026-11-14: not attempted (11.3).
