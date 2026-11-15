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

**Done on 2026-11-14; do not re-run.** Read back on 2026-11-14: the stack parameter `DeploymentExpiresAtUtc` and the expiry schedule both say `2026-12-15T14:00:00Z`, which is the renewed canonical expiry (`2026-12-16T08:00:00Z`) less release B's 18 hours, so the renewal below was applied before release B. Release C executed later on Nov 14 and subtracted another 18 hours: **the live stop is now `2026-12-14T20:00:00Z`** ([record](../../evidence/APP-209/release-2026-11-14.md)); `expiry_renewal` changes only the canonical input, so a later release is the only way to move it. The steps are kept as written, for reference.

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

**Steps 1–4 done on 2026-11-14; do not re-run.** Release B executed on Nov 14, not Nov 23: read back on 2026-11-14, the stack's expiry parameter and schedule say `2026-12-15T14:00:00Z` and the release snapshot `forma-schema-release-<8 hex>-v24` exists beside release A's `-v22`. The record is APP-194's close-out. Steps 5–8 remain; [section 11](#11-nov-23-operator-checklist) is their checklist (step 5's Anthropic spend limit is item 11.1a; step 6, rewritten 2026-11-15 as our own fAuth readback, and step 8, a helper as reviewer, stay here). The steps are kept as written, for reference.

The release-B pull request (APP-194) commits `ops/promotion/schema-release.json` for 22 → 24 (migrations 0023 and 0024) with its pins, expiring 2026-11-26 00:00 UTC ([record](../../evidence/APP-194/SCHEMA-RELEASE.md)); W3's operator changes (`--note`, `admin_policy`, `list-box-files`, `stats`) ride the same operator image. Within 24 hours of section 2:

1. If `main` carries new scanner findings, preparation refuses before approval: section 8 (the scan disposition renewal). Otherwise:
2. `gh variable set PROMOTION_ENABLED --body true --repo $REPO`, merge the reviewed release-B pull request, read the summary: `schema_release: {from_version: 22, to_version: 24}`, the stop time **2026-12-15T14:00:00Z**, the disposition (if any) with its 2026-11-26 expiry, the release file's.
3. Approve outside 05:00–05:45 UTC. Then `/health/ready` → 200, `gh run view ... --json conclusion` → `"success"`.
4. **Verify from the back office** (the operator task now has `stats`):

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   The expiry is in the live parameters of the state the apply wrote; `aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation --query "Stacks[0].Parameters[?ParameterKey=='DeploymentExpiresAtUtc'].ParameterValue" --output text` prints `2026-12-15T14:00:00Z`, and `aws scheduler get-schedule --group-name <DeploymentExpiryGroup output> --name <schedule>` shows `at(2026-12-15T14:00:00)`.
5. **Deploy the alarms** (section 6), **set the Anthropic spend limit** (item 11.1a: $1,000 a month, Victor's decision of 2026-11-15; it was "about $150"), and run the **restore drill** (section 9) while only Victor's account exists.
6. **Read back our own fAuth tenant** (rewritten 2026-11-15; it was "write to FormaStack"). FormaStack is us: Victor and Claude. fAuth is our `getforma-dev/gatewasm` service at `https://auth-staging.getforma.dev`, on one EC2 host in this same AWS account, so there is nobody to write to; the record of the commands below is the acknowledgement. The students' sign-in accounts and addresses live there, not in the beta's database; their deletion is section 10 step 6.

   **The user limit.** The trial tenant (`forma-trial-20261101`, created 2026-11-01) has no expiry, but its `max_users` is whatever the creating request set: the readback of 2026-11-15 found **10**, not the 1000 this step used to assume, so the eleventh registration would have been refused. One read confirms it; the key is fAuth's platform-admin secret (the one gatewasm's host template loads into `PLATFORM_ADMIN_KEY`), read into a variable and never printed:

   ```sh
   TEN=$(out CleanTrialProviderTenant)
   FAUTH=$(aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation \
     --query "Stacks[0].Parameters[?ParameterKey=='ProviderManagementUrl'].ParameterValue" --output text)
   PLAT_KEY=$(aws secretsmanager get-secret-value --secret-id "fauth-beta/platform-admin" --query SecretString --output text)
   curl -sS -H "Authorization: Bearer $PLAT_KEY" "$FAUTH/platform/tenants/$TEN" | tee "$TRIAL/fauth-tenant-before.json"
   # only if max_users < 30 (a write; reversible with the same call):
   # curl -sS -X PATCH -H "Authorization: Bearer $PLAT_KEY" -H 'content-type: application/json' -d '{"max_users": 50}' "$FAUTH/platform/tenants/$TEN" | tee "$TRIAL/fauth-tenant-after.json"
   unset PLAT_KEY
   ```

   Expected: JSON with `"max_users"` at least 30, `"status": "active"`, `"signup_mode": "invite_only"` and the current `user_count`. The file is the go/no-go's evidence (section 4).

   **Done 2026-11-15, about 08:35 UTC (the operator).** Before: `{"max_users": 10, "status": "active", "signup_mode": "invite_only", "user_count": 2}`. PATCHed to 50, then to **150** the same hour, and a second read agreed (`"max_users": 150`; the PATCH sets both `tenants` and `tenant_entitlements`, which the signup check reads as `COALESCE(e.max_users, t.max_users)`). Every guest a student shares with takes a seat in this tenant too (`auth/shared.rs`), so 25 students with two friends each plus about 28 members is about 78; 150 leaves room and still bounds a flood of guest accounts. `invite_only` keeps self-registration closed. The limit is tenant configuration, not a class constant: change it with the same PATCH. The before and after files are in the operator's private trial folder. Undo: the same PATCH with the old value.

   **Sign-in rate limits (done 2026-11-15; per client since about 03:00 UTC).** fAuth's limiters used to share one bucket for every client behind Caddy, so the first fix (about 01:50 UTC) raised the shared bursts to login 200, register 60 and token 120. The class is remote, every student on their own network, so the lasting fix keys each limiter on the client's own address instead: fAuth image `73b04ea` trusts the rightmost `X-Forwarded-For` only from `TRUSTED_PROXY_CIDRS` (Caddy on the host network). The per-client bursts are login 60, register 20 and token 120 (Victor's choice, 2026-11-15), in gatewasm's host template `render-template.py` and on the live host (`/opt/fauth/start.sh` and `runtime.env`). Readback: 61 failed sign-ins then 429 from one address while another address still got 401; `/health` ok; beta's sign-in page 200. A `FauthBetaHost` stack update that changes only UserData stops and starts the instance; it does not replace it.
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

9. **Close out release B** (after the run succeeds; APP-194). Release C (section 3a) starts from this state.

   ```sh
   $PY -m ops.promotion.maintenance inspect --state-bucket "$PROMOTION_STATE_BUCKET" --state-prefix "$PROMOTION_STATE_PREFIX"
   ```

   Expected: `"active": null`. The run's completed record names schema 24, the snapshot and the migration task. The canonical baseline now holds release B's `to`: contract `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356`, schema manifest `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` and policy schema version 24. Record the run, the candidate and the `beta/` tag in APP-194. Then remove `ops/promotion/schema-release.json` in a follow-up pull request; release C's own pull request may replace it instead. The pin in `schema_release.py` may stay as an inert record until release C's values replace it. Until the file is removed or replaced, CI fails any change to the deployment contract.

## 3a. Release C: a contract-only release (schema 24 → 24)

Release C changes the deployment contract (its qualification image context) and carries no migration. It ships as a reviewed contract-only release ([APP-210](../../Tickets/0-backlog/APP-210.md); [BETA-PROMOTION.md "Contract-only releases"](../pitch/BETA-PROMOTION.md#schema-releases)). It takes no snapshot and runs no migration, so the 05:00–05:45 UTC snapshot window does not apply. Everything else is a normal release: the fresh billing basis within 24 hours, the expiry (section 2), and section 8 if preparation refuses on new scanner findings (with `--contract` set to release C's `to` contract).

1. **Before the merge.** Section 3 step 9 is done and `maintenance inspect` shows `"active": null`. The release-C pull request commits `ops/promotion/schema-release.json` and its pin in `schema_release.py`, with an evidence record and an expiry. Its `from` is the live baseline above (version 24, schema manifest `176d68cb…`, contract `88d17cb5…`). Its `to` has the same version and schema manifest and release C's own contract, and its CI binds that `to` to the merged commit. `schema_release.load` refuses any other shape. The release's own review must confirm that `git diff --stat <live candidate> <release C candidate> -- app/migrations` is empty, and its evidence record (`evidence/APP-209/CONTRACT-RELEASE.md`) must say so: the promotion pins the manifest's digest and the CDK source, not the files under `app/migrations/`.
2. **Merge and read the summary.** `gh variable set PROMOTION_ENABLED --body true --repo $REPO`, merge the reviewed release-C pull request, and read the preparation summary. Approve only if every line holds, where the "reviewed record" is release C's evidence record (`evidence/APP-209/CONTRACT-RELEASE.md`, with a "file bytes" row and a `to.deployment_contract_sha256` row, as [release B's](../../evidence/APP-194/SCHEMA-RELEASE.md) has). Comparing the summary with the candidate's own committed `schema-release.json` proves nothing more: `validate_summary` already binds the summary to that file. The comparison that catches an unreviewed record is against the values the reviewed record states:
   - `schema_release: {from_version: 24, to_version: 24, spec_sha256}`, where `spec_sha256` equals the file-bytes SHA-256 the reviewed record states;
   - `schema_version: 24`;
   - `preserved`: `schema`, `template`, `trial`, `aliases` and `secrets` all `true`;
   - `sequence`: exactly the five normal steps, `stop both services`, `execute reviewed change set`, `verify stopped boundaries`, `activate both services`, `verify health and images`, with no `snapshot database` and no `run reviewed migration`;
   - `deployment_contract_sha256` equal to the `to.deployment_contract_sha256` the reviewed record states;
   - the stop time and the scan as for any release.
3. **Approve.** Then `/health/ready` → 200 and `gh run view ... --json conclusion` → `"success"`.
4. **Verify.** `maintenance inspect` shows `"active": null`. The completed record names the release's versions and hash, with no snapshot and no migration task. The baseline contract is the record's `to`, and the schema manifest digest and policy schema version 24 are unchanged. A failure after the outage began is a normal promotion's failure (BETA-PROMOTION "Deployment state and failures"), and `restore-after-failed-migration` refuses it. A prepared release that is not approved is abandoned as in section 1 step 3.
5. **Close out.** Remove `ops/promotion/schema-release.json` in a follow-up pull request, as for release B.

## 4. Codes by wave and the one announced restart (Friday Nov 28)

**Go/no-go before the first wave**, each with its evidence: release B executed and `/health/ready` 200; the caps, December month and expiry applied (section 2) and the stop reads 2026-12-14 02:00 UTC (the security release's, 2026-11-15); the alarms stack deployed and the test email received; the restore drill's diff empty (done 2026-11-15); the fAuth tenant readback showing `max_users` at least 30 (section 3 step 6; since 2026-11-15 our own read, not FormaStack's letter; done 2026-11-15, raised from 10 to 150); the purge path proven on the dev stand with no guest-linked share open, or the R1 decision taken (section 10); your `admin_policy` row granted and `/admin` opening; the rehearsal of Nov 26 done on Safari and a phone. Any "no" delays the codes, not the class date.

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

**Suspected stolen session (APP-215).** Signing out everywhere ends a member's sessions but no longer ends the guest shares they made. A member who suspects a compromise also revokes their guest shares in the share dialog. Removing them below also ends every share, but removal is final for that sign-in identity: they cannot be re-admitted with a new code under the same account (`already_member`). As a last resort, the operator can revoke their rows in `live_work_shares` with SQL, with Victor's word.

1. On **`/admin`** (W2; requires your `admin_policy` row): the member's row, **Remove**, confirm. W1's `admin_remove_member` revokes the membership and entitlement, advances the epoch (their session ends at the next request) and audits; it refuses yourself and the last admin.
   Reading the audit afterwards: a removal from `/admin` writes `membership.revoked` and `entitlement.revoked` on the **admission** chain with the actor being you (the admin) and the target's digest in the outcome; the operator channel's `revoke-membership` and `revoke-entitlement` write the same kinds on the **operator** chain with the actor being the target and no digest. Branch on `writer_role` before reading the actor. Your own membership was operator-seeded, so `/admin` shows no consent row for you; that is expected, every student has one.
2. If their code was never redeemed, `admission revoke --code <code_id>` instead.
   If they had shared an app **with everyone**, removal does not end that share; section 10 step 2 has the exact operator SQL that does (APP-215 interim, with Victor's word).
3. **Their uploads**, with your own identity (the task lists, you delete every S3 version and delete marker):

   ```sh
   $PY -m ops.promotion.admission delete-uploads --state-bucket ... --expected-state-sha256 "<hash>" --organization "$TRIAL_ORG" \
     --principal "<principal id>" --output "$TRIAL/delete-uploads-<principal>.json"
   ```

   Expected: `{"status": "deleted", "files": F, "versions": V, "keys": ["o/<org>/b/<box>/f/<file>", ...], ...}`. It refuses a key outside `o/<org>/b/`, a truncated listing (more than 100 files: inspect first) and leaves the `box_files` rows for the purge.

## 6. Alarms, the deny-all switch and rollback

**Alarms** (Nov 23, after release B). `ops/monitoring/alarms.json` is a standalone stack: an SNS topic with your email, alarms on load-balancer and target 5xx, unhealthy targets, no running task (the ECS metric sample count, since Container Insights is off) and database free storage under 2 GiB, plus an AWS Budget at the project ceiling. Since 2026-11-15 (infra review M1, Victor's decision) it also alarms on load-balancer and target 4xx spikes, the service's CPU and memory, the database's CPU, connections and freeable memory, the account's SES bounce and complaint rates, and the fAuth host's status check and CPU; the host is a new parameter, `FauthInstanceId`. It references the beta's resources by identifier and changes none of them, so promotion's infrastructure digest is untouched. Section 11.1 is the checklist, with the update that adds the M1 alarms to the live stack.

```sh
ALB_DNS=$(out LoadBalancerDnsName)
ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
# The main service's target group is the one that is not port 3001 (the broker's).
TG_ARN=$(aws elbv2 describe-target-groups --load-balancer-arn "$ALB_ARN" --query "TargetGroups[?Port!=\`3001\`].TargetGroupArn" --output text)
FAUTH_ID=$(aws cloudformation describe-stack-resources --stack-name FauthBetaHost --query "StackResources[?ResourceType=='AWS::EC2::Instance'].PhysicalResourceId" --output text)
aws cloudformation deploy --stack-name forma-beta-trial-alarms --template-file ops/monitoring/alarms.json --no-fail-on-empty-changeset \
  --parameter-overrides NotificationEmail=<your address> LoadBalancerFullName="${ALB_ARN#*loadbalancer/}" TargetGroupFullName="${TG_ARN##*:}" \
  ClusterName="$CLUSTER" ServiceName="$SERVICE" DBInstanceIdentifier=$(out DatabaseEndpoint | cut -d. -f1) FauthInstanceId="$FAUTH_ID" \
  MonthlyBudgetUsd=248
```

**Expected alarms.** `UnhealthyTargets` and `NoRunningTask` treat missing data as breaching, so by design they fire during every promotion outage (the stop-both-services step, a few minutes) and at the scheduled stop (2026-12-14 02:00 UTC since the security release of 2026-11-15; it was 2026-12-14 20:00 UTC after Release C and 2026-12-15 14:00 UTC after release B), and recover when the service is back. On a class day there is no deploy, so any of these is real. `LoadBalancer5xx` also fires briefly while targets drain during a promotion.

Confirm the subscription from the email SNS sends, then the test email: `aws sns publish --topic-arn "$(aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query "$(printf "$OUT" AlarmTopicArn)" --output text)" --message "trial alarm test"` arrives within a minute. `aws cloudwatch describe-alarms --alarm-name-prefix forma-beta-trial-alarms --query "MetricAlarms[].{n:AlarmName,s:StateValue}"` shows every alarm `OK` once each has data (the free-storage and running-task alarms within ten minutes). On Dec 1 redeploy with `MonthlyBudgetUsd=118`; after the trial `aws cloudformation delete-stack --stack-name forma-beta-trial-alarms`. To change `NotificationEmail` on a live stack, delete and recreate the stack; an in-place update rolls back (11.1, 2026-11-15).

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

The charter promises the organization's data is deleted by Dec 14; the beta stops at its schedule, **2026-12-14 02:00 UTC** since the security release of 2026-11-15 (each release subtracts 18 hours from the canonical 2026-12-16 08:00 UTC), so the purge must finish before that stop unless a later release renews it. The purge is two operator tasks and one object sweep, all through `admission.py`, after the memberships are gone. **What it keeps**, and what the charter and the notice must say: the organization row (suspended), its provider binding and sign-in routing, its record sequence, and the hash-chained audit heads and events (identifiers, kinds and hashes; no content, no name, no address). Everything else of the organization goes: members, principals, codes, apps, works, records, uploads, shares, sessions, cards, and every object version under `o/<org>/`. Manual snapshots are deleted the same day; automated backups lapse seven days later, so the bytes are fully gone about **Dec 21**.

**`genui_identity`, the guest side of live sharing.** Guest sign-in accounts and their sessions are never the organization's and stay. The organization's share sign-in flows (`shared_auth_flows`) are deleted with the purge. That includes any short-lived (10-minute) flow routed through the trial tenant as provider for another organization's share; such a flow simply has to be started again. A guest's `linked_apps` rows naming the organization's shares go with the purge at schema 24 or later: APP-212 built that follow-up once migration 0024 had dropped the `share_events` key (the gate is `GATED_PURGE_DELETIONS` in `admission_operator.py`, read from `genui_meta.schema_version` as `stats` reads it). The purge deletes them under the lifted FORCE, as `genui_owner`, in the same block as the flows and before the `live_work_shares` their key reaches, then restores FORCE and checks it. A guest's `share_events` rows (identifiers, kinds, versions and hashes; no content) are **kept at every version**, so each guest's hash chain still verifies; after deleting, the purge proves that no link of the organization remains (`purge_guest_links_remain`) and that its guests' `share_events` count is unchanged (`purge_guest_chains_changed`). Below schema 24 **the purge refuses `guest_linked_shares_present` and deletes nothing** while any guest has accepted or linked one of the organization's live shares; do not force it. Expect, under `identity`, the counts seen before deleting and the two outcomes, for example `"identity": {"shared_auth_flows_deleted": 3, "guest_rows": {"genui_identity.linked_apps": 2, "genui_identity.share_events": 5}, "linked_apps_deleted": 2, "share_events_kept": 5}`; `deleted` then carries `"genui_identity.linked_apps": 2`, counted in `rows_deleted`, and never a `share_events` entry. If a purge refuses with the generic `admission_operator_refused`, the likely cause is a statement that outran `connect()`'s 5-second statement timeout or 3-second lock timeout (the identity tables are briefly locked exclusively): nothing was deleted; run it again off-hours, with no one signed in. The purge also requires exactly the five audit heads an organization is provisioned with (`workspace`, `control`, `data`, `admission`, `operator`); the trial organization has them, an older hand-made one might not, and such a one refuses rather than guess which chains to keep.

1. **Codes.** `admission list`; `admission revoke --code` for every code still `active`.
2. **Members.** On `/admin`, remove every member but yourself; then your own membership through the operator channel, which the Admin page refuses for the last admin:

   ```sh
   # compose and run the documented override with your principal id, as in section 9's task loop:
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$(python3 -c 'import uuid;print(uuid.uuid4())')" \
     --step revoke-self --override admission-revoke-membership --set=--organization="$TRIAL_ORG" --set=--principal="<your principal id>" --output "$TRIAL/revoke-self.json"
   aws ecs run-task --cli-input-json "file://$TRIAL/revoke-self.json"
   ```

   Expected log line: `{"principal_id": ..., "changed": true, "membership_epoch": N}`. Every session is now dead.
   **A removed member's organization-wide share (APP-215 interim for the 2026-11-15 sharing review's M2; operator, with Victor's word).** Removal does not end a share the member made "with everyone" (`genui.work_shares`, `grantee_id IS NULL`): the app stays in every member's Library, and no operator override touches that table, so until the migration-bearing fix the operator ends it with this exact SQL on the beta database as the migration identity, Victor's words for the action appended to `$PRIV/approvals.md` first, with Victor's own principal id as `revoked_by` (the foreign key needs a member): `UPDATE genui.work_shares SET revoked_at=clock_timestamp(), revoked_by='<Victor's principal id>', revoke_event_id=gen_random_uuid() WHERE organization_id='<the organization id>' AND sharer_id='<the removed member's principal id>' AND grantee_id IS NULL AND revoked_at IS NULL;` Expected: `UPDATE 1` per shared app (`0` means they shared nothing with everyone); every member's Library drops the app at its next read. This interim appends no `permission.revoked` event; record the statement and its count with the approval. The same statement during the class, from section 5, is what withdraws a disruptive student's shared app.
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
6. **The sign-in accounts, in our own fAuth** (rewritten 2026-11-15; it was a request to FormaStack, which is us: section 3 step 6). The beta's `principals` rows (issuer, tenant, subject) and `provider_invitations` are gone with step 4; the accounts and addresses are in the fAuth tenant, and we delete them ourselves, after step 4 and before the stop (2026-12-14 02:00 UTC). `DELETE /manage/users/{usr_id}` is a hard delete: the row goes, and foreign-key cascades take the user's sessions, refresh tokens, credentials, codes and keys with it. It cannot be undone. `DELETE /platform/tenants/{id}` is only a soft delete (`status: deleted`, users and addresses left in place), so it does not count as deleting accounts; the tenant stays. The management key is the stack's attached management secret, JSON `{tenant_id, management_key}`; read the key into a variable, never print it.

   ```sh
   MGMT_ARN=$(aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation \
     --query "Stacks[0].Parameters[?ParameterKey=='ProviderManagementSecretArn'].ParameterValue" --output text)
   FAUTH=$(aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation \
     --query "Stacks[0].Parameters[?ParameterKey=='ProviderManagementUrl'].ParameterValue" --output text)
   MGMT=$(aws secretsmanager get-secret-value --secret-id "$MGMT_ARN" --query SecretString --output text | jq -r .management_key)
   H="Authorization: Bearer $MGMT"
   curl -sS -H "$H" "$FAUTH/manage/stats" | tee "$TRIAL/fauth-stats-before.json"                 # total_users, verified_users
   curl -sS -H "$H" "$FAUTH/manage/users?limit=100&offset=0" > "$TRIAL/fauth-users.json"          # ids and addresses: private, 0600
   # To keep your own account, remove its row from fauth-users.json first (match your address).
   for id in $(jq -r 'if type == "object" then .users else . end | .[].id' "$TRIAL/fauth-users.json"); do
     curl -sS -o /dev/null -w "$id revoke %{http_code}\n" -X POST -H "$H" "$FAUTH/manage/users/$id/revoke-sessions"
     curl -sS -o /dev/null -w "$id delete %{http_code}\n" -X DELETE -H "$H" "$FAUTH/manage/users/$id"
   done | tee "$TRIAL/fauth-deletions.log"
   curl -sS -H "$H" "$FAUTH/manage/stats" | tee "$TRIAL/fauth-stats-after.json"
   unset MGMT H
   ```

   Expected: `fauth-stats-before.json` with the count seen on `/admin` before step 2; one `delete 204` line per user in the log; `fauth-stats-after.json` with `"total_users": 0` (or 1, if you kept your own). More than 100 users would need a second page with `offset=100`; the class has at most about 30. Record the log and the after-stats with the purge receipt: they are the confirmation the notice's "deleted by December 14" rests on for the accounts.

   **Addresses outside the users table.** Two fAuth tables still hold student addresses after the users are gone: `tenant_user_invites.email` (the invitations the operator created; no delete endpoint) and `login_attempts.email` (swept by fAuth after 90 days). If the charter's promise is read to cover addresses, clear both with SQL on the fAuth database (an SSM session to the fAuth host, the database credentials from its host template's secret; gatewasm's `OPERATIONS.md` has the access steps): `DELETE FROM tenant_user_invites WHERE tenant_id = '<the tenant id, $(out CleanTrialProviderTenant)>'` and the same for `login_attempts`. fAuth's `audit_log` holds identifiers, kinds, times and IP addresses but no email, and matches the notice's "minimal security log"; keep it. Afterwards, on the GenUI side, clear `ProviderManagementSecretArn` and `ProviderManagementUrl` on the stack and schedule the management secret's deletion with a recovery window.
7. **The capability secret.** `revoke` removes a code's capability; redeemed and exhausted codes leave theirs until the operator's reconcile sweep removes every capability that can no longer be handed off. Run it once, after step 4, so the secret names no code of the purged organization (the override takes the attached management secret, never argv):

   ```sh
   python3 infra/scripts/window_requests.py operator-task --outputs "$TRIAL/stack.json" --window-id "$(python3 -c 'import uuid;print(uuid.uuid4())')" \
     --step reconcile-final --override admission-reconcile --set=--organization="$TRIAL_ORG" --output "$TRIAL/reconcile-final.json"
   aws ecs run-task --cli-input-json "file://$TRIAL/reconcile-final.json"      # then wait, read the log line as in section 9
   aws secretsmanager get-secret-value --secret-id "$(out ProviderCapabilitySecretArn)" --query SecretString --output text | python3 -c 'import json,sys; m=json.load(sys.stdin); print(len(m), "entries;", sum(k.startswith(sys.argv[1]) for k in m), "for the trial organization")' "$TRIAL_ORG"
   ```

   Expected: the task's log line names the capabilities it ended, and the second command prints `0 for the trial organization`.
8. **After the stop** (2026-12-14 02:00 UTC): both services at count 0 (`aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount}"` → 0 / 0); the alarms stack deleted (section 6); the retained database, keys and load balancer still cost money until a separate decision.

## 11. Nov 23 operator checklist

What is left of sections 2–10 after release B, as a checklist: for each item the commands, the output to expect, who runs it, how to verify it and how to undo it. Written 2026-11-14 from a read-only readback of the beta account and the code on `main` at `140a5a9f`; nothing was deployed, run or sent. Section 11.6 lists what it found stale or incomplete in sections 2–10.

**Standing on 2026-11-14 (read-only).** Release B executed on Nov 14: the stack parameter `DeploymentExpiresAtUtc` and the expiry schedule both read **2026-12-15T14:00:00Z** (Release C later moved them to **2026-12-14T20:00:00Z**), and the release's manual snapshot `forma-schema-release-<8 hex>-v24` exists beside release A's `-v22`, so sections 2 and 3 are done (their "Nov 23" headings are the plan's dates, kept as written). No alarms stack, SNS topic, CloudWatch alarm or AWS Budget exists in the account yet. The operator identity's permissions cover every step below. The operator task signs in to the database as `genui_migration` with the Secrets Manager password (`infra/src/stack.ts:609–617`), not an IAM token, so a restored copy accepts it unchanged; the application roles sign in with IAM tokens bound to the live instance's resource id (`app/runtime/runtime.py:704`), which matters only for item 3.

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

**Done 2026-11-15, about 01:21 UTC.** The stack was recreated with `NotificationEmail=victor@getforma.dev`. Victor confirmed the SNS subscription and the AWS User Notifications contact, and all five alarms read `OK`. **Gotcha:** changing `NotificationEmail` on a live stack makes CloudFormation try to replace the named budget, which AWS refuses ("same name … different internalId"), and the update rolls back. To change the address, delete the stack (the roll-back command below) and deploy it again. A budget-amount-only change, such as the Dec 1 redeploy, updates in place.

**M1 alarms, 2026-11-15 (APP-214).** Victor approved the attack-facing alarms of the infra review's M1 (about $0.80 a month: the account's 16 + 2 alarms less the 10 free ones). The template gained eleven alarms and one parameter, `FauthInstanceId`; `NotificationEmail`, the budget and its subscribers are byte-identical, and the test pins that no new resource references the address, so the update replaces nothing. Apply it as step 2a: an update that passes only the new parameter.

1. **Derive the parameters and look at them** before deploying. Each derivation was run against the live stack on 2026-11-14 and matches the template's `AllowedPattern`; the target group that is not port 3001 is the main service's on port 3000. The fAuth host is the one `AWS::EC2::Instance` of stack `FauthBetaHost`.

   ```sh
   ALB_DNS=$(out LoadBalancerDnsName)
   ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
   TG_ARN=$(aws elbv2 describe-target-groups --load-balancer-arn "$ALB_ARN" --query "TargetGroups[?Port!=\`3001\`].TargetGroupArn" --output text)
   FAUTH_ID=$(aws cloudformation describe-stack-resources --stack-name FauthBetaHost --query "StackResources[?ResourceType=='AWS::EC2::Instance'].PhysicalResourceId" --output text)
   printf '%s\n' "${ALB_ARN#*loadbalancer/}" "${TG_ARN##*:}" "$CLUSTER" "$SERVICE" "$DB" "$FAUTH_ID"
   ```

   Expected: six lines: `app/<name>/<16 hex>`, `targetgroup/<name>/<16 hex>`, the cluster name, the service name, the instance identifier, `i-<17 hex>`. Two target groups would print on one line separated by a tab: stop and pick the port-3000 one by hand. An empty sixth line means the fAuth stack has another name: `aws cloudformation list-stacks --stack-status-filter CREATE_COMPLETE UPDATE_COMPLETE --query 'StackSummaries[].StackName'` and pick it; the id must match `^i-[0-9a-f]{8,17}$`.

2. **Deploy** (a new stack):

   ```sh
   aws cloudformation deploy --stack-name forma-beta-trial-alarms --template-file ops/monitoring/alarms.json --no-fail-on-empty-changeset \
     --parameter-overrides NotificationEmail=<your address> LoadBalancerFullName="${ALB_ARN#*loadbalancer/}" TargetGroupFullName="${TG_ARN##*:}" \
     ClusterName="$CLUSTER" ServiceName="$SERVICE" DBInstanceIdentifier="$DB" FauthInstanceId="$FAUTH_ID" MonthlyBudgetUsd=248
   aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query 'Stacks[0].StackStatus' --output text
   ```

   Expected: `Waiting for changeset to be created..`, `Waiting for stack create/update to complete`, `Successfully created/updated stack - forma-beta-trial-alarms`; then `CREATE_COMPLETE`. The address becomes a stack parameter, readable by any identity with `cloudformation:DescribeStacks` in the account; it also lands in the shell history and in the budget's subscriber list. A `NoEcho` on `NotificationEmail` would be a one-line template change for APP-187, not for this checklist.

   **2a. Update the live stack with the M1 alarms** (the stack of 2026-11-15 exists; do not repeat `NotificationEmail`):

   ```sh
   aws cloudformation deploy --stack-name forma-beta-trial-alarms --template-file ops/monitoring/alarms.json --no-fail-on-empty-changeset \
     --parameter-overrides FauthInstanceId="$FAUTH_ID"
   aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query 'Stacks[0].StackStatus' --output text
   ```

   Expected: the same three lines, then `UPDATE_COMPLETE`. `deploy` passes every parameter it is not given as "use previous value", so the address, the identifiers and the budget amount stay as they are and the budget is not replaced. `UPDATE_ROLLBACK_COMPLETE` with a budget event in `describe-stack-events` means a parameter changed under it: compare `describe-stacks --query 'Stacks[0].Parameters'` with the values of step 1 and run again with only `FauthInstanceId`.

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

   Expected: sixteen alarms. The five of Nov 15, `forma-beta-trial-alarms-load-balancer-5xx`, `-target-5xx`, `-unhealthy-targets`, `-no-running-task` and `-database-free-storage`; and the eleven M1 alarms, `-load-balancer-4xx` (100 in 5 min; the balancer's own 4xx, which browsers never cause), `-target-4xx` (600 in 5 min, two a second: stuffing, scraping or many revoked pages; a class of 25 is tens, a revoked guest tab 150, the minute after an announced restart a few hundred), `-service-cpu` (80% for 10 min), `-service-memory` (85% for 10 min), `-database-cpu` (80% for 10 min), `-database-connections` (12 for 10 min: the service holds 8, two per role, an operator or migration task one or two more), `-database-freeable-memory` (under 256 MiB for 10 min), `-email-bounce-rate` (5%, where SES starts its review), `-email-complaint-rate` (0.1%, the same), `-fauth-status-check` (any failed check for 2 min; missing data is breaching, so a stopped or recreated host alarms) and `-fauth-cpu` (80% for 10 min). All `INSUFFICIENT_DATA` right after creation and `OK` within ten minutes; the two `-email-` alarms read `OK` without data (missing data is not breaching) and the `-fauth-status-check` needs the right instance id to leave `ALARM`. Then `{"limit": "248", "unit": "USD", "time": "MONTHLY", "type": "COST"}` (the amount may print as `248.0`); three notifications, `ACTUAL` 80, `ACTUAL` 100 and `FORECASTED` 100 (the API prints the thresholds as `80.0` and `100.0`, as it prints `248.0`; a forecast needs weeks of history and may stay silent in November).

**Verify.** Step 3's test email in the inbox and every alarm `OK` in step 4. An alarm that stays `ALARM` while `/health/ready` answers 200 has a wrong dimension: compare the six values of step 1 with the stack's parameters; `-fauth-status-check` in `ALARM` while `https://auth-staging.getforma.dev/` answers is a wrong or stale `FauthInstanceId` (the host's stack was recreated: derive it again and run step 2a). `-unhealthy-targets` and `-no-running-task` fire by design during a promotion outage and at the 2026-12-14 02:00 UTC stop (section 6); `-target-4xx` may fire once in the minute after an announced restart (section 4) if many pages were open, and recovers on its own.

**Roll back.** `aws cloudformation delete-stack --stack-name forma-beta-trial-alarms && aws cloudformation wait stack-delete-complete --stack-name forma-beta-trial-alarms` removes the topic, the subscription, the alarms and the budget and changes nothing of the beta. If the delete fails on a subscription that was never confirmed, repeat it with `--retain-resources EmailSubscription`; a pending subscription lapses by itself after three days. On Dec 1 the same deploy with `MonthlyBudgetUsd=118` is an update, and the confirmed subscription survives it.

### 11.1a The Anthropic spend limit (Victor; no command)

Section 3 step 5 and the plan's D10. **Victor's decision, 2026-11-15:** the organization's monthly spend limit in the Anthropic console is **$1,000**. November spend so far is about $6, Max gives a $200 API credit each month, and the app's own daily cap of 1,000 model calls is the working guard. This replaces the earlier "$150 for November and $100 for December with 50% alerts". He may add an email notification at about $100. What you see: the limit on the console's limits page. Nothing on AWS changes; no agent can do or read this. Record the date and the amount in the trial's private evidence.

### 11.2 The restore drill (the operator may run it; a few cents of RDS)

**Done 2026-11-15, 08:33–08:48 UTC (the operator): diff identical at schema 24**, copy and drill snapshot deleted ([record](../../evidence/APP-188/restore-drill-2026-11-15.md)).

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
6. **The fAuth accounts.** Victor runs section 10 step 6 (our own fAuth; irreversible): `stats` → list → revoke sessions → `DELETE /manage/users/{id}` each → `stats` with `total_users` 0. The deletions log and the after-stats go with the purge receipt. The operator may run the two reads (`stats` and the list) into `$TRIAL`.
7. **The capability secret.** Victor: the composed `admission-reconcile` override and `aws ecs run-task` (section 10 step 7), then the secret count → `… 0 for the trial organization`.
8. **After the stop**, 2026-12-14 02:00 UTC. The operator may read `aws ecs describe-services --cluster "$CLUSTER" --services "$SERVICE" --query "services[0].{desired:desiredCount,running:runningCount}"` → `0` and `0`. Victor deletes the alarms stack (11.1's roll-back command); `-no-running-task` and `-unhealthy-targets` are in `ALARM` from the stop until then, by design.

**Verify.** `counts_after` every `0` in the purge file, and no object version or delete marker left under the organization's prefix:

```sh
aws s3api list-object-versions --bucket "$(out EvidenceBucket)" --prefix "o/$TRIAL_ORG/" \
  --query '{versions: length(Versions || `[]`), markers: length(DeleteMarkers || `[]`)}'
```

Expected: `{"versions": 0, "markers": 0}`. Then the manual snapshot list empty, fAuth's after-stats reading `total_users` 0 and the capability count `0`. Record all of it in the trial's private evidence and a sanitized note in `evidence/APP-188/`.

**Roll back.** None after step 3 (the suspension needs a reviewed change) and none after step 4 (the charter promises the deletion; the last automated backup holding the data lapses about Dec 21). Before step 3, stop at any refusal and investigate: a revoked code is reissued, a removed member re-invited.

### 11.5 Victor's `admin_policy` row (the operator may run it; Victor signs in and opens `/admin`)

**Granted 2026-11-15, about 09:40 UTC (the operator)** to the organization's only member, Victor's Nov 1 account (his Gmail; fAuth user created 20:15:19, code redeemed 20:15:58): `{"status": "granted", "permission": "admin_policy", "changed": true, "membership_epoch": 14}`. Victor still has to sign in and open `/admin` to verify. **That account's entitlement ends 2026-12-01 20:15 UTC** (30 days from redemption, and nothing renews it; follow-up A19). Build the class apps from an account redeemed on or after Nov 15, and grant it admin with step 3.

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

- **Sections 2 and 3 are done**, executed on Nov 14 rather than Nov 23 (the expiry parameter and schedule read `2026-12-15T14:00:00Z` after release B `2026-12-14T20:00:00Z` after Release C and `2026-12-14T02:00:00Z` after the security release of 2026-11-15, the live stop; the `-v24` release snapshot exists). Their headings keep the plan's dates; the status lines at their tops say so. The release record is APP-194's close-out. Section 1's status line names release A's expiry; the live one is in section 2's.
- **Section 6 (alarms)**: every command and parameter is right, and `ops/promotion/test_monitoring_alarms.py` pins the template (5 tests; 6 since the M1 alarms of 2026-11-15, which also add `FauthInstanceId` to section 6's command). Missing: the subscription check before the test publish (11.1 step 3) and the budget readback (11.1 step 4).
- **Section 9 (drill)**: `window_requests.py operator-task --override admission-stats --set=--organization=…` exists exactly as written (`app/operator/overrides.json`, `admission-stats`; `infra/scripts/window_requests.py:490–497`); the log stream `genui-operator/operator/<task id>` matches the live task definition's `awslogs-stream-prefix` and container name; the restore and delete flags are valid. Incomplete: the expected stats line omits `audit_events`; the log group is a placeholder (derived in this section's setup); section 9 takes no snapshot although section 10 step 5 expects to delete "the drill snapshot" (11.2 takes one, so that the two counts are of the same bytes); section 9's restore names no parameter group, so the copy would get the engine default rather than the stack's `rds.force_ssl=1` group (11.2 passes it by name).
- **Section 10 (purge)**: `admission-revoke-membership`, `admission-reconcile`, `admission-suspend-organization` and `admission-purge-organization` exist, and `admission.py suspend` and `purge --confirm` print the shapes section 10 shows. Stale by omission: "the fix is a follow-up after migration 0024 is live" describes APP-212 (in review), not yet on `main`; the purge refuses `guest_linked_shares_present` at every schema version today (11.4).
- **Section 3 step 7 and section 4** (`grant-admin`, `issue`, `list`, `revoke`, `restart --announced`): as written; `--expires-days` accepts 1 to 30.
- **The header's** "agents call no AWS API" is relaxed for exactly the steps this section marks "operator".
- **The plan's scratch-RDS herd** is in TRIAL-25-READINESS, not in section 9, and has no runnable path on `main`; decided 2026-11-14: not attempted (11.3).

### 11.7 Security events and WAF (Victor; about $7 a month)

The infra review of 2026-11-14 (H2, no WAF; H3, no CloudTrail trail or detector) under Victor's decision of 2026-11-15: observability free or nearly free, so **no GuardDuty, Security Hub, Config or Inspector**; the WAF at about $7 a month; every alert by email to his address. Two standalone templates beside the alarms stack, pinned by `ops/promotion/test_monitoring_security.py` (APP-214 is the ticket). Neither creates an IAM resource (`validate-template` reports no capability), neither names an account id or an ARN, and neither touches `infra/src`: the beta is reached only through parameters, so promotion's infrastructure digest is unchanged.

**What you get.**

- `ops/monitoring/security-events.json`, about $0 a month (the first copy of management events is free, the bucket holds a few MB, rules on AWS events and email notices are free, the account Access Analyzer is free): one multi-region CloudTrail trail of management events, read and write, log-file validation on, into a private SSE-S3 bucket (public access blocked, TLS-only, versioned, Glacier Instant Retrieval after 30 days, expired after a year, retained when the stack is deleted); an SNS topic with your address; eleven EventBridge rules, each one email line naming who, what, when and from which address: root console sign-in; a root mutating API call (root's reads are left out on purpose, or a billing visit would mail dozens; the sign-in rule covers the visit); a console sign-in failure; a console sign-in without MFA by an IAM user or root; IAM changes (`CreateUser`, `CreateAccessKey`, `CreateLoginProfile`, Attach/Put/Detach of user, role and group policies, `UpdateAssumeRolePolicy`, `DeactivateMFADevice`, `DeleteVirtualMFADevice`); CloudTrail tampering (`StopLogging`, `DeleteTrail`, `UpdateTrail`, `PutEventSelectors`); `AuthorizeSecurityGroupIngress`; KMS `DisableKey` and `ScheduleKeyDeletion`; S3 `PutBucketPolicy`, `PutBucketAcl` and the bucket and account public-access-block puts and deletes; RDS `DeleteDBInstance`, `DeleteDBSnapshot` and `ModifyDBInstance` with `publiclyAccessible` true; a new active IAM Access Analyzer finding. No data events, Insights or CloudWatch Logs delivery. The promotion pipeline (`ops/promotion`: ECR pushes, ECS updates and tasks, change sets, state objects, a secret value, a schedule read) calls none of the operations the rules name, and the test pins that; a release whose change set touches the stack's roles or security groups mails one notice per changed resource through CloudFormation, which is right.
- `ops/monitoring/waf.json`, about $7.20 a month (web ACL $5, two rules $2, requests $0.60 per million where the class is far under one, two alarms $0.20): a REGIONAL web ACL associated with the beta's load balancer, default Allow, metrics and sampled requests on, no logging and no managed rule groups (both cost; the paid upgrade path is written in the template's `Metadata`). Rule 1 blocks an address past **2000 requests in five minutes** on any path; rule 2 blocks an address past **30 requests in five minutes** on the model-calling routes: `/v1/compose`, `/v1/ask`, `/v1/shape`, `/v1/act`, `/v1/mcp`, `/v1/widgets/customize`, `/v1/boxes/explain/…` and `/v1/boxes/<box>/apps`, `/brief` and `/ask` (Slack's callbacks are left out: they come from Slack's addresses). Both answer `429` with a plain one-line body and `Retry-After: 300`, and a block lasts until the address's rate falls under the limit. Both limits are parameters (`FloodLimit`, `ModelRouteLimit`). Two alarms on `BlockedRequests` (one blocked request in five minutes, one per rule) publish to the trial alarm topic.

  **Why those numbers hold for 25 students.** The load balancer's `/health/ready` checks go from the balancer to the targets and never pass the web ACL. A member tab is tens of requests in five minutes: one SSE stream per session shared across tabs with a 1 to 30 s reconnect backoff, a 30 s access check, and hint-driven refreshes spread over 0 to 3 s with one retry (APP-195). A guest tab on `/s/<share>` is the heaviest client, about 250 in five minutes (an access poll every 2 s and a document poll every 3 s), so one address reaches 2000 only with eight guest tabs open at once; the test pins these cadences against the client source. Static assets are tens of requests at the first load and served by the service worker afterwards. The model routes have per-member hourly limits in the app (compose 30, ask and shape 120, act 60, MCP 120 per token, widget customize 120, box explain 30); 30 per address per five minutes is six a minute, above any one member's use, and caps what several sessions behind one address can spend. A shared household or office of two or three students stays under both.

1. **Security events** (deploy in us-east-1, the beta's region, where the global IAM and console sign-in events arrive; the rules see this region's API calls, which is where every beta resource lives):

   ```sh
   aws cloudformation deploy --stack-name forma-security-events --template-file ops/monitoring/security-events.json \
     --no-fail-on-empty-changeset --parameter-overrides NotificationEmail=<your address>
   aws cloudformation describe-stacks --stack-name forma-security-events --query 'Stacks[0].StackStatus' --output text
   ```

   Expected: `Waiting for changeset to be created..`, `Waiting for stack create/update to complete`, `Successfully created/updated stack - forma-security-events`; then `CREATE_COMPLETE` within about three minutes (the trail waits for the bucket policy). Confirm the "AWS Notification - Subscription Confirmation" email, then read back:

   ```sh
   SEC_TOPIC=$(aws cloudformation describe-stacks --stack-name forma-security-events --query "Stacks[0].Outputs[?OutputKey=='SecurityTopicArn'].OutputValue" --output text)
   aws sns list-subscriptions-by-topic --topic-arn "$SEC_TOPIC" --query 'Subscriptions[].{p:Protocol,arn:SubscriptionArn}'
   aws cloudtrail get-trail-status --name forma-security-events-management --query '{logging:IsLogging,delivered:LatestDeliveryTime,digest:LatestDigestDeliveryTime}'
   aws events list-rules --name-prefix forma-security-events --query 'Rules[].{n:Name,s:State}' --output table
   aws accessanalyzer list-analyzers --type ACCOUNT --query 'analyzers[].{n:name,s:status}'
   ```

   Expected: one `email` subscription with a subscription ARN (`PendingConfirmation` means the click has not happened and nothing is delivered); `logging: true`, with a delivery time within fifteen minutes of creation and a digest time within the hour; eleven rules, all `ENABLED`; one analyzer, `ACTIVE`. The stack's own creation may mail up to three notices (`PutBucketPolicy`, `PutBucketPublicAccessBlock`, `PutEventSelectors`, if the rules came up before the bucket and the trail), and the analyzer's first scan mails one notice per externally reachable resource: expect the two GitHub OIDC roles (`forma-beta-promotion-prepare` and `-execute` trust GitHub's issuer by design); read them, then archive them in the Access Analyzer console so they do not return. End-to-end check that changes nothing: re-apply the trail's own selectors, and the "CloudTrail change: PutEventSelectors by `<your ARN>`" email arrives within about five minutes:

   ```sh
   aws cloudtrail put-event-selectors --trail-name forma-security-events-management \
     --event-selectors '[{"ReadWriteType":"All","IncludeManagementEvents":true}]' --query 'EventSelectors[0].ReadWriteType' --output text
   ```

   Expected notices in normal operation, so they are read correctly: one `PutEventSelectors` or `UpdateTrail` per update of this stack; `DeleteDBInstance` and `DeleteDBSnapshot` at the restore drill (11.2; seen 2026-11-15 08:48 UTC) and `DeleteDBSnapshot` at the purge (section 10, 11.4); three `ScheduleKeyDeletion` on 2026-11-15 at 11:05 UTC (the cost cleanup, follow-up A14); `PutRolePolicy`, `AttachRolePolicy` or `AuthorizeSecurityGroupIngress` through CloudFormation when a release changes the stack's roles or groups; `PutAccountPublicAccessBlock` when the review's L1 is applied; a console sign-in without MFA until H4's device is registered. Anything else is a question.

2. **WAF** (same identity; the alarms stack must exist, for `AlarmTopicArn`). Derive and look first:

   ```sh
   ALB_DNS=$(out LoadBalancerDnsName)
   ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
   TOPIC=$(aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query "$(printf "$OUT" AlarmTopicArn)" --output text)
   printf '%s\n' "$ALB_ARN" "$TOPIC"
   ```

   Expected: two lines, `arn:aws:elasticloadbalancing:us-east-1:<account>:loadbalancer/app/<name>/<16 hex>` and `arn:aws:sns:us-east-1:<account>:forma-beta-trial-alarms-AlarmTopic-<suffix>`; each matches its parameter's `AllowedPattern`. Then:

   ```sh
   aws cloudformation deploy --stack-name forma-beta-waf --template-file ops/monitoring/waf.json --no-fail-on-empty-changeset \
     --parameter-overrides LoadBalancerArn="$ALB_ARN" AlarmTopicArn="$TOPIC"
   aws wafv2 get-web-acl-for-resource --resource-arn "$ALB_ARN" --query 'WebACL.{name:Name,rules:Rules[].Name,default:DefaultAction}'
   curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/health/ready
   aws cloudwatch describe-alarms --alarm-name-prefix forma-beta-waf --query "MetricAlarms[].{n:AlarmName,s:StateValue}"
   ```

   Expected: `Successfully created/updated stack - forma-beta-waf`; `{"name": "forma-beta-waf-web-acl", "rules": ["per-address-flood", "per-address-model-routes"], "default": {"Allow": {}}}`; `200`; two alarms, `forma-beta-waf-flood-blocked` and `-model-routes-blocked`, `INSUFFICIENT_DATA` then `OK` within ten minutes. Then use the canvas and a shared page yourself for a few minutes: no `429`, and both alarms stay `OK`. When one fires, the sampled requests name the address and the paths:

   ```sh
   WEB_ACL=$(aws cloudformation describe-stacks --stack-name forma-beta-waf --query "Stacks[0].Outputs[?OutputKey=='WebAclArn'].OutputValue" --output text)
   aws wafv2 get-sampled-requests --web-acl-arn "$WEB_ACL" --rule-metric-name per-address-flood --scope REGIONAL --max-items 100 \
     --time-window StartTime=<ISO start>,EndTime=<ISO end> --query 'SampledRequests[].{ip:Request.ClientIP,path:Request.URI,at:Timestamp}'
   ```

   A student behind a busy shared network shows guest-page polls on `/v1/shared/…` from one address; a flood shows one path from one address. To loosen rather than remove, redeploy with `FloodLimit=4000` or `ModelRouteLimit=60`.

**Verify.** Step 1: the subscription confirmed, `logging: true`, eleven rules `ENABLED`, and the `PutEventSelectors` test email. Step 2: `/health/ready` answers `200` behind the web ACL, your own browsing shows no `429`, both alarms `OK`. Nothing in `GenuiSyntheticFoundation` changed: `aws cloudformation describe-stacks --stack-name GenuiSyntheticFoundation --query 'Stacks[0].LastUpdatedTime'` is unchanged, and the next `maintenance inspect` shows the same infrastructure digest.

**Roll back.** WAF, in an emergency (students answered `429` wrongly): `aws wafv2 disassociate-web-acl --resource-arn "$ALB_ARN"` takes effect within a minute and leaves the stack in place; then `aws cloudformation delete-stack --stack-name forma-beta-waf && aws cloudformation wait stack-delete-complete --stack-name forma-beta-waf` removes the web ACL and the two alarms (a `deploy` of the stack after a bare disassociation re-associates it). Security events: `aws cloudformation delete-stack --stack-name forma-security-events && aws cloudformation wait stack-delete-complete --stack-name forma-security-events` removes the trail, the rules, the topic and the analyzer; the bucket is retained with its logs (`DeletionPolicy: Retain`) and is emptied and deleted by hand when the record is no longer wanted. The trail's deletion mails one last `DeleteTrail` notice if its rule outlives it. Neither roll-back changes anything of the beta.

**Cost.** Security events about $0 (S3, cents). WAF about $7.20. The M1 alarms of 11.1 and these two make 18 alarms in the account, 10 free, so $0.80. About $8 a month in all, inside the ADR-019 ceilings; the budget parameter needs no change.

### 11.8 Uptime probes, SLO alarms and the golden-signals dashboard (about $3.60 a month)

The outside-in layer. The alarms of 11.1 watch the beta from inside AWS (the balancer's own counts, the task, the database); these ask what a student sees: can the app and sign-in be reached from outside, what share of answers fail, and how long an ordinary page takes. One standalone template, `ops/monitoring/observability.json` (stack `forma-beta-observability`), pinned by `ops/promotion/test_monitoring_observability.py`: two Route 53 HTTPS health checks that probe `https://beta.getforma.dev/health/ready` and `https://auth-staging.getforma.dev/health` every 30 s from us-east-1, us-west-2 and eu-west-1; an alarm on each; two SLO alarms on the load balancer; and the CloudWatch dashboard `forma-beta-golden-signals` with a row each for traffic, errors, latency, saturation, health and email under a one-line legend per row. It needs no app release, creates no IAM resource (`validate-template` reports no capability), names no account id or ARN and changes nothing of `GenuiSyntheticFoundation`: the beta is reached only through parameters, so promotion's infrastructure digest is unchanged. It publishes to the alarm topic of 11.1, so every notice reaches the same address. No string matching, fast interval or latency measurement on the probes, no CloudWatch Logs and no Synthetics canaries: those cost money. **Not deployed** (2026-11-15); Victor deploys it, agents prepare. Deploy it in **us-east-1**: Route 53 publishes health-check metrics only there, and the beta runs there.

**What each alarm means and the first thing to check.** Each one mails on `ALARM` and again on `OK`.

- `forma-beta-observability-app-unreachable`: Route 53's checkers got no healthy answer from `/health/ready` for three minutes in a row; missing data counts as down. First: `curl -sS -o /dev/null -w '%{http_code}\n' https://beta.getforma.dev/health/ready` from your machine. `503` means the service runs but is not ready (read the service log group); `502`, `504` or no answer means no healthy target, and `-unhealthy-targets` or `-no-running-task` of 11.1 says which (then the ECS service events). If `curl` answers `200` while the alarm holds, ask Route 53 what its checkers saw: `aws route53 get-health-check-status --health-check-id "$APP_HC"` (step 3) names each checker's last answer, such as a TLS or DNS failure seen only from outside. It fires by design at the 2026-12-14 02:00 UTC stop and during a promotion outage, like `-unhealthy-targets`.
- `forma-beta-observability-fauth-unreachable`: the same for fAuth's `/health`; nobody can sign in while it lasts. First: `curl` that address; then `-fauth-status-check` of 11.1. In `ALARM` too, the host itself is down (that alarm's description names the recreate procedure); `OK`, the host runs but fAuth or the proxy in front of it does not answer.
- `forma-beta-observability-error-rate`: more than 2% of the load balancer's requests were answered 5xx, by the service or by the balancer, in two of three five-minute periods; a period with fewer than 100 requests counts as 0%. First: the dashboard's Errors row says which count rose. Target 5xx: read the service log group for the failing route before anyone restarts anything. Balancer 5xx: no healthy target or a refused connection; read the Health row and the ECS service events.
- `forma-beta-observability-slow-median`: the median response time stayed above 2 s for fifteen minutes. It watches the median because model calls make p95 legitimately slow (tens of seconds); the dashboard's p99 line sitting there is normal. First: the Saturation row (service CPU and memory, database CPU and connections; 11.1's `-service-cpu`, `-service-memory`, `-database-cpu` and `-database-connections` alarm on the same metrics), then the Traffic row for a flood and the web ACL's blocked count.

**The probes' own requests count.** Each Route 53 checker asks every 30 s, and AWS's own figure is a request about every two seconds when all eight checker regions are used; three regions send fewer, roughly 50 to 60 in five minutes to `/health/ready` (an estimate: AWS publishes no per-region count). They pass the web ACL at about ten per checker address per five minutes, far under the 2000 flood limit of 11.7 (that limit's sizing note is about the balancer's own checks, which never pass the web ACL), and they show in the Traffic row. They also count in `RequestCount` and `TargetResponseTime`: so the error-rate gate is 100 requests, about twice what the probes alone send, and it measures real traffic: quiet hours stay silent, and a class working clears it easily. Their quick `200`s still pull the median down at quiet hours. With the class working, students' requests outnumber them. When the app is down the probes are answered 5xx by the balancer, so `-error-rate` fires together with `-app-unreachable`.

1. **Derive the parameters and look at them** (after the header's block and section 11's setup, which set `out`, `$OUT`, `$CLUSTER`, `$SERVICE` and `$DB`). The first six are 11.1 step 1's derivations; the topic is the alarms stack's output, as in 11.7 step 2. `WebAclName` is the `forma-beta-waf` stack's `WebAclName` output, `forma-beta-waf-web-acl`, passed as written.

   ```sh
   ALB_DNS=$(out LoadBalancerDnsName)
   ALB_ARN=$(aws elbv2 describe-load-balancers --query "LoadBalancers[?DNSName=='$ALB_DNS'].LoadBalancerArn" --output text)
   TG_ARN=$(aws elbv2 describe-target-groups --load-balancer-arn "$ALB_ARN" --query "TargetGroups[?Port!=\`3001\`].TargetGroupArn" --output text)
   FAUTH_ID=$(aws cloudformation describe-stack-resources --stack-name FauthBetaHost --query "StackResources[?ResourceType=='AWS::EC2::Instance'].PhysicalResourceId" --output text)
   TOPIC=$(aws cloudformation describe-stacks --stack-name forma-beta-trial-alarms --query "$(printf "$OUT" AlarmTopicArn)" --output text)
   printf '%s\n' "${ALB_ARN#*loadbalancer/}" "${TG_ARN##*:}" "$CLUSTER" "$SERVICE" "$DB" "$FAUTH_ID" "$TOPIC"
   ```

   Expected: seven lines: `app/<name>/<16 hex>`, `targetgroup/<name>/<16 hex>`, the cluster name, the service name, the instance identifier, `i-<17 hex>`, `arn:aws:sns:us-east-1:<account>:forma-beta-trial-alarms-AlarmTopic-<suffix>`. The stops of 11.1 step 1 apply: two target groups on one line, or an empty sixth line.

2. **Deploy** (a new stack; `AppHostname` and `FauthHostname` keep their defaults, `beta.getforma.dev` and `auth-staging.getforma.dev`):

   ```sh
   aws cloudformation deploy --region us-east-1 --stack-name forma-beta-observability --template-file ops/monitoring/observability.json \
     --no-fail-on-empty-changeset --parameter-overrides AlarmTopicArn="$TOPIC" LoadBalancerFullName="${ALB_ARN#*loadbalancer/}" \
     TargetGroupFullName="${TG_ARN##*:}" ClusterName="$CLUSTER" ServiceName="$SERVICE" DBInstanceIdentifier="$DB" FauthInstanceId="$FAUTH_ID" \
     WebAclName=forma-beta-waf-web-acl
   aws cloudformation describe-stacks --region us-east-1 --stack-name forma-beta-observability --query 'Stacks[0].StackStatus' --output text
   ```

   Expected: `Waiting for changeset to be created..`, `Waiting for stack create/update to complete`, `Successfully created/updated stack - forma-beta-observability`; then `CREATE_COMPLETE`.

3. **Read back**:

   ```sh
   obs() { aws cloudformation describe-stacks --region us-east-1 --stack-name forma-beta-observability --query "$(printf "$OUT" "$1")" --output text; }
   APP_HC=$(obs AppHealthCheckId) FAUTH_HC=$(obs FauthHealthCheckId)
   for hc in "$APP_HC" "$FAUTH_HC"; do
     aws route53 get-health-check-status --health-check-id "$hc" --query 'HealthCheckObservations[].{region:Region,status:StatusReport.Status}' --output table
   done
   aws cloudwatch describe-alarms --region us-east-1 --alarm-name-prefix forma-beta-observability --query "MetricAlarms[].{n:AlarmName,s:StateValue}"
   obs DashboardUrl
   ```

   Expected: for each probe, observations only from `us-east-1`, `us-west-2` and `eu-west-1` (several checkers each), every status beginning `Success: HTTP Status Code 200`. Four alarms, `forma-beta-observability-app-unreachable`, `-fauth-unreachable`, `-error-rate` and `-slow-median`: the two `-unreachable` read `OK` within about five minutes (Route 53 takes a minute or two to start publishing, and missing data counts as down, so each may mail one `ALARM` and its `OK` at creation); the two SLO alarms read `OK` within fifteen minutes. Then a console URL; open it: the legend, then the six rows. The Traffic row's web ACL lines are empty until `forma-beta-waf` (11.7) is deployed, and the Email row until SES sends.

**Verify.** Every probe observation `Success`, four alarms `OK`, and every row of the dashboard drawing data (but the two just named). Nothing in `GenuiSyntheticFoundation` changed: its `LastUpdatedTime` is unchanged and the next `maintenance inspect` shows the same infrastructure digest.

**Roll back.** `aws cloudformation delete-stack --region us-east-1 --stack-name forma-beta-observability && aws cloudformation wait stack-delete-complete --region us-east-1 --stack-name forma-beta-observability` removes the two health checks, the four alarms and the dashboard. The topic and its subscription belong to the alarms stack and stay; nothing of the beta changes. To quiet one alarm through a planned outage without removing anything: `aws cloudwatch disable-alarm-actions --region us-east-1 --alarm-names forma-beta-observability-error-rate`, and `enable-alarm-actions` afterwards.

**Cost.** Route 53 lists each HTTPS health check on an AWS endpoint at $0.50 basic plus $1.00 for HTTPS, so $3.00 for the two. The same pricing page lets an account create up to 50 health checks on its own AWS endpoints free, which would leave only the HTTPS feature, $2.00; the first bill shows which applies (a non-AWS endpoint would be $0.75 plus $2.00 each). The alarms are six alarm metrics at $0.10: the two probe alarms and the median alarm one each, the error-rate alarm three, because CloudWatch bills every metric in a math expression. The account is already past its 10 free alarm metrics (11.7), so $0.60. The dashboard is free as one of the account's first three custom dashboards (up to 50 metrics each; this one uses about twenty), $3 a month otherwise. About $3.60 a month at list price, $2.60 with the free health-check allowance; with 11.1 and 11.7, about $12 a month in all, inside the ADR-019 ceilings; the budget parameter needs no change.
