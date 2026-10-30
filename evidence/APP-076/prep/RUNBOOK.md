# APP-076 go-live trial runbook — Nov 2, ~13:00–18:00 UTC

**Preparation, not evidence.** Nothing below has run against `beta.getforma.dev`. Prepared 2026-10-30 at `89c6ec1` (`phase-one-spikes`). It follows the APP-078 window ([RUNBOOK](../../APP-078/prep/RUNBOOK.md), [APPROVALS](../../APP-078/prep/APPROVALS.md)), which must have completed first. Every AWS mutation is marked **[MUTATION · Tx]** with the trial approval below that covers it; read-only calls are marked [read]. Steps are marked **V** (Victor, installed Chrome, second network) or **O** (the orchestrator, operator machine). Times are UTC.

**Stop rules.** Stop at: a precondition that does not hold; any not_found row that differs in status, body or headers (except the known findings G7/G8 if the orchestrator has dispositioned them in writing before 13:00); any number that differs from 1,140,000 / 900,000; a command outcome other than the one stated; a session or Set-Cookie minted by a refusal; a secret-shaped string found by the scan in anything bound for `evidence/`. On a stop the service stays RUNNING on the recorded digest (it is the beta) unless a protected route is exposed, in which case go straight to §8 (rollback) and leave it rolled back. A failed or unknown AWS call is reconciled, never retried blindly. No fAuth deployment operation overlaps the trial (FormaStack confirms). No LLM call is made.

**Retention rule.** Nothing retained, pasted into chat or committed may contain an access code, a session or pre-access cookie value, a fAuth token, a provider invitation token, an authorization code, a `request_uri` or a full callback URL. Retained means sanitized: statuses, header names and non-secret values, body SHA-256s, operator JSON lines without code fields, Victor's yes/no observations with the minute, screenshots of pages whose address bar shows only `/w/…`, `/work/…`, `/workspace`, `/invite`, `/signin/…` or `/auth/pending`. Private material lives under `$PRIV` (gitignored, `0700`), and `trial_checks.py scan` runs over every file before it is copied to `evidence/APP-076/trial/`.

## Fixed names

```sh
export AWS_REGION=us-east-1 AWS_PAGER= AWS_MAX_ATTEMPTS=1
REPO=/Users/victorvillacis/dev/FormaBuiltApps/forma-app-platform        # main checkout at the candidate tip
PRIV=$REPO/.cache/operations/beta/app-076-trial                       # gitignored; mkdir -m 700
W78=$REPO/.cache/operations/beta/app-078-window                       # APP-078's private record (stack-after.json, codes/)
TRIAL=91de7f07-2fec-435e-b066-775e6f85068e                            # proposed trial UUID (T1); the run-task clientToken seed
BASE=https://beta.getforma.dev
CLUSTER=GenuiSyntheticFoundation-ClusterEB0386A7-HMlpmHwGMVSN
SERVICE=GenuiSyntheticFoundation-ServiceD69D759B-pNHHjoez1RWF
LOGS=GenuiSyntheticFoundation-Logs6819BB44-ujo9czITOJoW
WR="python3 infra/scripts/window_requests.py"                          # from $REPO
TC="python3 -I evidence/APP-076/prep/trial_checks.py"                  # from $REPO

# Seeded identifiers (app/runtime/tenant_seed.py; evidence/APP-009/handles.json). Non-secret.
NORTHSTAR_ORG=00000000-0000-4000-8000-000000000001   CANARY_ORG=00000000-0000-4000-8000-000000000002
NS_APP=00000000-0000-4000-8000-000000000065          NS_SEEDED_WORK=00000000-0000-4000-8000-000000000069
NS_EVENTS=00000000-0000-4000-8000-00000000006e       NS_INVENTORY=00000000-0000-4000-8000-000000000070
BOUND_WORK=<BOUND_WORK_ID>                           # APP-078 create-bound-work (G4); proposed 00000000-0000-4000-8000-0000000000f2
SR_APP=00000000-0000-4000-8000-0000000000c9          SR_WORK=00000000-0000-4000-8000-0000000000cd
SR_COLL=00000000-0000-4000-8000-0000000000ce         SR_SOURCE=00000000-0000-4000-8000-0000000000d2
SR_RECORD=00000000-0000-4000-8000-0000000013b1       # S05 record; spares …13b2, …13b3 (see "Record ids")
NS_HANDLE=se45m37vabqrfvbh3gbwn22qkm                 CANARY_HANDLE=z33gibxjspwxcnopbsdxqtijey
```

The full URLs (since G9, `895f530`, `/workspace` links each work Victor may open and, with `record_write`, a fresh record of its collection; the record ids below are still typed so that the trial's ids stay the recorded ones):

| What | URL |
| --- | --- |
| Invitation entry | `https://beta.getforma.dev/invite` |
| Northstar sign-in | `https://beta.getforma.dev/signin/se45m37vabqrfvbh3gbwn22qkm` |
| Canary sign-in | `https://beta.getforma.dev/signin/z33gibxjspwxcnopbsdxqtijey` |
| Northstar work (bound) | `https://beta.getforma.dev/w/00000000-0000-4000-8000-000000000065/<BOUND_WORK_ID>` |
| Spend Review record | `https://beta.getforma.dev/work/00000000-0000-4000-8000-0000000000c9/00000000-0000-4000-8000-0000000000cd/records/00000000-0000-4000-8000-0000000000ce/00000000-0000-4000-8000-0000000013b1` |

**Where Spend Review runs (decision D1, prepared default).** On RDS, Spend Review exists only in the second fixture organization `…0002` (the "Canary" organization of `handles.json`): the seed places Northstar in `…0001` and both a Northstar copy and Spend Review in `…0002`. The APP-013/014 campaigns created Spend Review in Northstar through `create-app` plus the workspace's definition admission, publication and `create_work`, which have no operator command and no route (G3). The prepared default is therefore: Victor holds two memberships, Northstar (`…0001`, the Northstar work) and Canary (`…0002`, Spend Review and the canary replay of Northstar identifiers). The canary property is unchanged: `…0002` shares Northstar's app id, but Victor's `…0002` grants name only Spend Review, the bound work exists only in `…0001`, and RLS scopes every lookup to the session's organization. If the orchestrator prefers Spend Review in Northstar, G3's operator channel is needed first.

**Record ids.** The S05 command `record_decision` is a `create` whose one declared transition is `needs_context → reviewed`; since the APP-013 M1 fix (contract edition `impl-2026-10-29.3`) a record admits exactly two commits: the create at version 0 (→ 1, `needs_context`) and the transition (→ 2, `reviewed`). Then the page offers no form. A record therefore cannot be reused for a repeat. The trial uses `…13b1`; `…13b2` and `…13b3` are spares for a restarted sequence (each a fresh version-0 record: substitute the last URL segment). The seeded `…00f0` (EXP-002 in the campaigns) is already at version 1 on RDS from the seed and is **not** used. `…13a1`–`…13a5` are the campaigns' race records and are avoided.

## Approvals for this trial (Victor, in chat, before 13:00)

| # | Decision | Default prepared | Steps |
| --- | --- | --- | --- |
| T1 | Trial slot Nov 2 ~13:00–18:00 UTC, Victor present ~45 min from a second network, no fAuth operation in it; trial UUID above. | As stated (ADR-010 §8 item 9). | all |
| T2 | Three codes on RDS through APP-078's managed channel (`mint-code` + `issue --code-digest`): **NS** (`…0001`, grants `discover:$NS_APP`, `run:$NS_APP`, `record_read:$NS_APP:$BOUND_WORK`, `source_read:$NS_EVENTS`, `source_read:$NS_INVENTORY`), **CANARY** (`…0002`, `discover:$SR_APP`, `run:$SR_APP`, `record_read:$SR_APP:$SR_WORK`, `record_write:$SR_APP:$SR_WORK`, `source_read:$SR_SOURCE`), **SPARE** (`…0001`, `discover:$NS_APP`, revoked before use); 14 days, 1 redemption; private delivery channel of his choice. This refines APP-078 A9 (which named work `…0069`, the placeholder-bound seeded work). | As stated. If APP-078 15.3 already issued codes with other grants, those are revoked and these issued. | 0.6, 2, 6.1 |
| T3 | Service restart (`update-service --force-new-deployment`, stop-then-start, ~2–3 min outage; every session and stored refresh token ends, by ADR-006/D7.11). | Yes. | 5.1 |
| T4 | Rollback as a redeploy of `PREVIOUS_TD` (deny-all, G11), then restore to the candidate revision (two short outages; CloudFormation drift between them). | Yes. | 8 |
| T5 | Revoke Victor's Canary entitlement at the end (his Canary identity then gets the neutral denial; re-admission is by a new code). | Yes. Alternative: skip, and record the revoked class as not run. | 6.4 |
| T6 | About ten operator tasks on Nov 2 (list, revoke, verify ×2, verify-catalog, optional qualification re-run) under the November reservation (`extra_task_hours: 3`) if APP-078's window left hours, else a new preflight reservation first. | Yes. | 0.5, 0.6, 6.1, 6.4, 7 |
| T7 | The opt-in forwarded-IP check: eleven sign-in starts from the operator address in one minute (it meets the 10/minute limit on purpose). | Yes. | 1.3 |
| T8 | Exporting session cookie values from Chrome into a 0600 file on the operator machine for the scripted member and canary rows (values never leave the file; the sessions end at the restart / revocation). | Optional. Without it those rows are recorded "not run" and the Chrome observations stand alone. | 3, 6.2 |

## 0. Preconditions (O, ~12:00–12:55, read-only unless marked)

- [ ] 0.1 APP-078 receipts exist and are reviewed: `evidence/APP-078/window/` with migrations once and again (catalog version = `app/migrations/manifest.json`), seed twice, backfill both organizations, both handles installed, `upload-sources` twice, `register-source-revision` (`9161c1e4-27c0-4cbb-addf-eb7ce504bf45`), **`create-bound-work` → `$BOUND_WORK`** (G4), qualification `passed: true`, snapshot retained. APP-078 Tier A review recorded. Otherwise stop.
- [ ] 0.2 Candidate identity: `CANDIDATE` (PR 4 tip or the contingency branch), its exact-source CI run ids, and `SERVICE_DIGEST` from APP-078 step 3. The APP-012 review majors M1–M4 (M4: a lock wait at session resolution must stay the uniform 404, not 503) are fixed and re-checked in the candidate.
- [ ] 0.3 [read] `aws ecs describe-services --cluster $CLUSTER --services $SERVICE > $PRIV/service-before.json` → `ACTIVE`, desired/running **1/1**, one deployment `PRIMARY` `COMPLETED`, `deploymentConfiguration` 0/100. `aws ecs describe-tasks` on the running task → `lastStatus RUNNING`, container image digest = `SERVICE_DIGEST`. Record `CANDIDATE_TD` (the service's task-definition ARN) and `PREVIOUS_TD` (G11). `aws elbv2 describe-target-health` → one `healthy`.
- [ ] 0.4 [read] Hosted sign-in wiring present (G1, G2): the running task definition supplies the auth configuration, the identity registry binding both organizations to the deployed fAuth issuer and their `ten_` ids, and the provider invitation capabilities for the NS and CANARY codes. FormaStack confirms: issuer, tenants and clients `northstar-platform` / `canary-platform` unchanged; callback `https://beta.getforma.dev/auth/callback`; `victor@getforma.dev` has no existing account in either tenant (or the invited flow is confirmed to work for it); SES sandbox recipient verified. Without G1 and G2 resolved the trial cannot start (§Gaps).
- [ ] 0.5 [MUTATION · T6] Operator `verify-catalog` (run pattern of APP-078: compose with `$WR operator-task --outputs $W78/stack-after.json --window-id $TRIAL --step verify-catalog --override verify-catalog --output $PRIV/run-verify-catalog.json`, then `aws ecs run-task --cli-input-json file://…`, wait, describe, read the log) → `catalog_version N`, `read_only true`. Item 1: if the candidate carries a migration APP-078 did not apply, run steps `migrate-1`/`migrate-2` (override `migrate`) here and require one history row per new migration and a byte-identical repeat; otherwise record "no unapplied migration" with N.
- [ ] 0.6 Codes (T2). If APP-078 15.3 did not issue exactly NS, CANARY and SPARE: for each label `$WR mint-code --organization <org> --label <label> --out-dir $PRIV/codes` (prints only the digest), then step `issue-<label>` with override `admission-issue-preminted`, `--set=--organization=<org> --set=--code-digest=<digest> --set=--issued-by=app076-operator --set=--grant=<first> --add=--grant=<each further>` [MUTATION · T6]. Log line: `code_digest`, `code_id`, `expires_at`, never a code. Then `admission-list-northstar` and the same override with `--set=--organization=$CANARY_ORG` → the codes `active`, zero redemptions. Deliver NS and CANARY to Victor once, privately (T2 channel), from `$PRIV/codes/access-code-<label>.json`. SPARE is delivered too (it is revoked in 6.3 before he types it).
- [ ] 0.7 `mkdir -m 700 -p $PRIV/trial $PRIV/https`; `umask 077; echo '{}' > $PRIV/trial-cookies.json`. `$TC scan --root $PRIV/trial --self-test` → `self_test.passed true` (the planted code, invitation token, cookie and callback URL are all detected).
- [ ] 0.8 [read] `aws logs` start marker: record the time; every log read in this runbook is from this time on.

## 1. Baseline (O, 13:00–13:15)

- [ ] 1.1 TLS and build receipts: `curl -sS -o /dev/null -w '%{http_code} %{ssl_verify_result}\n' $BASE/health/live` → `200 0`; `openssl s_client -connect beta.getforma.dev:443 -servername beta.getforma.dev </dev/null 2>/dev/null | openssl x509 -noout -subject -issuer -enddate > $PRIV/trial/tls.txt` (subject `beta.getforma.dev`, the ACM certificate of APP-078 snapshot, valid to 2027-05-13). `$BASE/` → 200 landing linking `/invite`; `/health/ready` → 200; `/signin/$NS_HANDLE` → 200 sign-in page (any well-formed handle renders it; G12).
- [ ] 1.2 Outsider matrix (item 9): `$TC matrix --base-url $BASE --bound-work $BOUND_WORK --check matrix --label baseline --output $PRIV/trial/matrix-baseline.json`. Expected: every row passes; classes `not_found` (404, `{"code":"not_found","status":404}`, one header set), `cross_site` (403, `{"code":"cross_site","status":403}`), `callback` (400 neutral page, no session cookie) each `uniform: true`. On `89c6ec1` the local binary failed exactly the G7 rows (undecodable segment on the command route and record page: 400 text/plain) and the G8 rows (`Allow` header on a method mismatch). Both are fixed (`dc78707`): on that tip the local binary passes every row, 298 rows, every class uniform (`not_found` 211 rows, one body, one header set); see [local-binary-matrix.json](local-binary-matrix.json). A candidate image older than `dc78707` still stops item 9.
- [ ] 1.3 [T7] `$TC matrix … --check forwarded-ip --label forwarded-ip --output $PRIV/trial/forwarded-ip.json` → ten `400` then `429` with `Retry-After`: a forged leftmost `X-Forwarded-For` does not create new address classes behind the ALB (D7.14). The operator address is limited for a minute; Victor's network is not.

## 2. Victor: Northstar (V, ~13:15–13:50; O watches the logs)

**Victor's list, part 1** (Chrome on the phone hotspot; nothing to paste into chat; tell the orchestrator only "done" and what you saw):

1. Open `https://beta.getforma.dev/invite`. Using only the keyboard (Tab, type, Enter), type `WRONG` and press Enter: the same page says the code was not accepted. Now type the **NS** code and press Enter.
2. You are on fAuth's sign-up page. Before typing, glance at the address bar: it shows `auth-staging.getforma.dev/…/authorize?client_id=…&request_uri=…` and nothing else (no email, no role, no invite token). Sign up with `victor@getforma.dev` and a new password of your own. Never share the password.
3. You land on **Confirm your email address**. Click **Request a new confirmation message**: it says a new message was requested. The page now counts down ("request another message in … seconds"): that is the 60-second cooldown.
4. (24-hour limit, about 5 minutes; skippable, see below.) Each time the countdown ends, click again. After the fifth accepted request in total, the button does not come back (the page keeps the countdown or says it could not request a message). Tell the orchestrator how many were accepted.
5. Open the **newest** confirmation mail at `victor@getforma.dev` (older links no longer work) and click its link. fAuth says the address is confirmed. Note the minute the mail arrived.
6. Back in the GenUI tab, click **Request a new confirmation message** once more: it sends you to the sign-in page (already confirmed). Click **Continue to sign in**. You land on the workspace. (If the tab is gone: type `https://beta.getforma.dev/signin/se45m37vabqrfvbh3gbwn22qkm`.)
7. Type `https://beta.getforma.dev/w/00000000-0000-4000-8000-000000000065/<BOUND_WORK_ID>`. Window start `2026-10-22 10:00`, end `2026-10-22 10:10`, destination empty, **Run views**: `total` shows **1,140,000** with its receipt; `timeline` shows minute rows with a receipt. Run again with destination `198.51.100.7`: `total` shows **900,000**. Screenshot both.
8. Open DevTools (Cmd-Option-I) → Application → Local storage and Session storage for `beta.getforma.dev`: both empty. Cookies: only `__Host-genui_…` names, all HttpOnly. Tell the orchestrator "Northstar done".

If step 4 is skipped, item 4's 5-per-24-hour limit is recorded as proven locally only (APP-009 session campaign) and item 4 stays open for the reviewer's disposition.

O during part 1 (item 3, 4, 5): [read] CloudWatch `aws logs filter-log-events --log-group-name $LOGS --start-time <0.8> --filter-pattern '"invite.validate"'` → one `refused` (the WRONG code) then one `handoff`; `auth.resend` outcomes in Victor's order (`requested`, `genui_cooldown`, …); the callback and admission without `auth.admission_denied`. Save to `$PRIV/trial/logs-part1.json`. Then `admission-list-northstar` [T6] → NS `redeemed_count 1`, no pending reservation left. Victor's observations (steps 1–8, with minutes) go to `$PRIV/trial/victor-part1.md`.

## 3. Member scripted checks (O, optional T8, ~13:50–14:00)

- [ ] 3.1 Victor (if T8): in Chrome DevTools → Application → Cookies → `https://beta.getforma.dev` → select `__Host-genui_session`, copy its value; in his terminal on the operator machine run `pbpaste | python3 -c 'import json,os,sys; p=sys.argv[1]; d=json.load(open(p)); d[sys.argv[2]]=sys.stdin.read().strip(); f=os.open(p,os.O_WRONLY|os.O_TRUNC); os.write(f,json.dumps(d).encode()); os.close(f)' "$PRIV/trial-cookies.json" member_session`, then copies something else to clear the clipboard. The value is never typed, printed or pasted elsewhere.
- [ ] 3.2 `$TC matrix --base-url $BASE --bound-work $BOUND_WORK --private-input $PRIV/trial-cookies.json --check matrix,controls --label member --output $PRIV/trial/matrix-member.json`. Expected: the controls pass (`/workspace` 200, work page 200, `total` `1140000`, destination `900000`, `timeline` 200 with receipt digests, a query string on the JSON route 422 `invalid_parameters`); the `real_member` rows (foreign Spend Review ids of `…0002`, nonexistent ids, malformed ids, query strings on the work page, an unused outcome key) are byte-identical to the anonymous 404; cross-site posts with the real cookie are the uniform 403 and the session survives (`/workspace` still 200 afterwards). Item 2: the same query the outsiders were refused succeeds for the admitted member. The matrix sends no sign-out, resend or same-origin command with a real session.

## 4. Victor: Canary admission and the Spend Review record (V, ~14:00–14:35)

**Victor's list, part 2:**

9. Open `https://beta.getforma.dev/invite`, type the **CANARY** code, Enter. Sign up again with `victor@getforma.dev` (this is a different organization and a different fAuth tenant). Confirm the newest mail, return as in step 6 (or type `https://beta.getforma.dev/signin/z33gibxjspwxcnopbsdxqtijey`), **Continue to sign in**. This ends your Northstar session; that is expected (one workspace session per browser).
10. Type the Spend Review record URL (the last row of the URL table). It says "No record committed yet (version 0)".
11. Open DevTools → Console, paste the **S05 snippet** below, press Enter. It prints three lines: `1 ik-1 first : committed`, `2 ik-1 retry : replayed`, `3 stale edit : 409 conflict … current version 1`.
12. Reload the page: version 1, status `needs_context`. In the status box type `reviewed`, press **Submit record_decision**: "The command was committed", version 2, status `reviewed`. The page no longer shows a form. Screenshot. Tell the orchestrator "record done".

S05 snippet (form posts twice with the page's own key, which is `ik-1`; then one JSON edit with a new key still expecting version 0; it reads nothing secret and prints no cookie):

```js
(async () => {
  const form = document.querySelector('form.command');
  if (!form || form.elements['expected_record_version'].value !== '0') { console.log('stop: not a version-0 record page'); return; }
  form.elements['value.status'].value = 'needs_context';
  form.elements['value.note'].value = 'Confirm the billing period';
  const body = new URLSearchParams(new FormData(form)).toString();
  const post = () => fetch(form.action, {method: 'POST', body, headers: {'Content-Type': 'application/x-www-form-urlencoded'}});
  const notice = (r) => new URL(r.url).searchParams.get('notice');
  const first = await post();            // ik-1 at version 0 -> version 1
  const retry = await post();            // the identical request after a "lost" answer -> the stored outcome
  console.log('1 ik-1 first :', notice(first));
  console.log('2 ik-1 retry :', notice(retry));
  const f = Object.fromEntries(new FormData(form));
  const [, , , app, work, , command] = new URL(form.action).pathname.split('/');
  const key = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('');
  const stale = await fetch(form.action, {method: 'POST', headers: {'Content-Type': 'application/json', 'Idempotency-Key': key},
    body: JSON.stringify({kind: 'command_request', schema_version: 1, app_id: app, work_id: work, publication_id: f.publication_id,
      command_id: command, schema_id: f.schema_id, schema_version_expected: Number(f.schema_version_expected), record_id: f.record_id,
      expected_record_version: 0, expected_work_version: Number(f.expected_work_version), idempotency_key: key,
      values: [{field_id: 'status', value: {type: 'enum', value: 'needs_context'}},
               {field_id: 'note', value: {type: 'text', value: 'A second edit from a stale page'}}]})});
  const problem = await stale.json().catch(() => ({}));
  console.log('3 stale edit :', stale.status, problem.code, problem.recovery_view && problem.recovery_view.reason,
              'current version', problem.recovery_view && problem.recovery_view.current_record_version);
})();
```

Expected database effect (item 6): two commits on `…13b1` (versions 1 and 2), two outcomes (keys `ik-1` = the first page's key, `ik-2` = the reload's key), two `command.committed` events on `…0002`'s data chain; the replay and the conflict write nothing. If line 1 is not `committed` or line 2 not `replayed`, stop the record part; a rerun uses spare `…13b2` (step 10 with the last segment changed). If line 3 is not `409 conflict expected_version`, record it and continue to step 12 only on the orchestrator's word.

O after part 2: [read] logs `"command.submit"` → `committed`, `replayed`, `conflict`, `committed` in that order; save `$PRIV/trial/logs-part2.json`.

## 5. Restart and reopen (O then V, ~14:35–14:50)

- [ ] 5.1 [MUTATION · T3] `aws ecs update-service --cluster $CLUSTER --service $SERVICE --force-new-deployment > $PRIV/trial/restart.json`; [read] `aws ecs wait services-stable …`; `describe-services` → 1/1 on `CANDIDATE_TD`, the new task's image digest = `SERVICE_DIGEST`, target healthy. Record stop and start times. Tell Victor "restarted".

**Victor's list, part 3:**

13. Reload the record page: it now says not found (the restart ended every session; by design). Type `https://beta.getforma.dev/signin/z33gibxjspwxcnopbsdxqtijey`, **Continue to sign in** (fAuth may not ask for the password again), then type the Spend Review record URL again: **version 2, `reviewed`, note "Confirm the billing period"**, no form. Screenshot.

## 6. Canary replay, revoked code, revocation (V and O, ~14:50–15:20)

**Victor's list, part 4:**

14. Still signed in to Canary, type the Northstar work URL (step 7): Chrome shows `{"code":"not_found","status":404}`. Tell the orchestrator "replay done".
15. Wait for "spare revoked", then on `https://beta.getforma.dev/invite` type the **SPARE** code: the same "not accepted" page as `WRONG`.
16. (T8) Copy the `__Host-genui_session` value again as in 3.1, with `canary_session` as the last word of the command.

- [ ] 6.1 Before step 15: [MUTATION · T6] step `revoke-spare`, override `admission-revoke` with `--set=--code-id=<SPARE code_id>`; `admission-list-northstar` → SPARE `revoked`, NS still `active` with one redemption.
- [ ] 6.2 (T8) `$TC matrix --base-url $BASE --bound-work $BOUND_WORK --private-input $PRIV/trial-cookies.json --check canary --label canary --output $PRIV/trial/matrix-canary.json` → the `canary_replay` rows (Northstar's bound work page, `total`/`timeline` queries, the run form, the record page and outcome lookup, and the demo's command post with Northstar's work id, EXP-002's id and a fresh key) equal the anonymous not_found bytes; `canary_replay.control_workspace` 200. Logs: no `command.submit` line for these posts (the command route refuses at the session/precheck before storage, `outcome = denied` at most).
- [ ] 6.3 Victor's Canary principal id (G5): read it from the database through a reviewed operator path; until G5 exists, 6.4 cannot run and the revoked class is recorded "not run".
- [ ] 6.4 [MUTATION · T5] step `revoke-canary-entitlement`, override `admission-revoke-entitlement` `--set=--organization=$CANARY_ORG --set=--principal=<principal>` → revoked. Then `$TC matrix … --check canary --canary-revoked --label canary-revoked --output $PRIV/trial/matrix-canary-revoked.json` → every canary row **and** `/workspace` are the uniform 404 (revoked entitlement, item 2). Victor (optional): reload the record page → not found.

Not run live, by design (item 2): an entitlement revocation *while a query is queued or executing*. A live query completes in about 0.25 s and an operator task needs 40–60 s to start, with no hook to hold a query; the in-flight orders are APP-012's and APP-014's local campaigns (G10). "Expired" entitlement is not live either (codes run 14 days, the fixture entitlements to 2036); it is APP-073/APP-012 local evidence.

## 7. Database outcomes and audit on RDS (O, ~15:20–16:20)

- [ ] 7.1 [MUTATION · T6] steps `verify-northstar`, `verify-canary` (overrides `admission-verify-*`) → `verified: true`; every outcome paired with its own `command.committed` event; the canary data chain has exactly the two trial events beyond the seed (item 10, item 6 database-backed evidence).
- [ ] 7.2 Insert-only (item 10): the APP-078 qualification receipt's direct-write refusal under all four roles is the pre-trial proof. Post-trial (optional, T6): rerun APP-078 step 12 with a new composition (`--step qualification-post-trial`) and require `passed: true`; the reviewer maps its checks to "audit rows written during the trial are insert-only for every application role" or records the row open (G14).
- [ ] 7.3 Item 1 operator rows: issue (0.6), list (0.6, 2, 6.1), revoke (6.1) on RDS through the real operator image, with their log lines. "An ordinary application/member role and a forged operator identity are denied": mapped to the APP-078 qualification check results (role bindings and refusals) and to `operator.py`'s managed-task refusals (`refused_in_managed_task` for raw `issue`); anything unmapped is recorded open.
- [ ] 7.4 Item 5 inspection: [read] all service log events since 0.8 into `$PRIV/trial/logs-all.json`; `$TC scan --root $PRIV/trial --self-test --output $PRIV/trial/scan.json` → `self_test.passed true`, `findings []` (no access code, cookie, `tinv_` token, JWT, callback code or `request_uri` in logs, operator output or the reports). `admission list` shows the invitations' `state` only. FormaStack attaches its record of the two invited signups (PAR with `invite_token` server-side only, invitation consumed, no implicit administrator role) without identifiers. Victor's step 2 and 8 observations complete the browser side.

## 8. Rollback and restore (O, ~16:20–17:20; Victor optional at the end)

- [ ] 8.1 [MUTATION · T4] `aws ecs update-service --cluster $CLUSTER --service $SERVICE --task-definition $PREVIOUS_TD > $PRIV/trial/rollback.json`; wait stable; the running task is `PREVIOUS_TD` on the same image digest.
- [ ] 8.2 During rollback: `$TC matrix … --check matrix --label rollback --output $PRIV/trial/matrix-rollback.json` → all protected rows the uniform 404; `POST /signin/$NS_HANDLE` (same-origin) → the neutral failure (no provider handoff); `/invite/validate` with any code → the neutral page; logs show no `invite.validate` handoff and no `command.submit`. No ingress change: [read] `aws ec2 describe-security-groups --group-ids sg-0b24119fe39e35f82` → 443 from `0.0.0.0/0` only (no `/32` trial was run).
- [ ] 8.3 [MUTATION · T4] `aws ecs update-service --cluster $CLUSTER --service $SERVICE --task-definition $CANDIDATE_TD`; wait stable; RUNNING, image digest = `SERVICE_DIGEST` recorded as the beta's; `$TC matrix … --label restored` passes as in 1.2. [read] `aws cloudformation detect-stack-drift` then `describe-stack-resource-drifts` on the service → in sync again.
- [ ] 8.4 Victor (optional): sign in to Northstar (`/signin/se45…`) → workspace; sign in to Canary (`/signin/z33g…`) → **No access** (the revoked entitlement is not restored).
- [ ] 8.5 [read] RDS resource id still `db-KW4LRMTPI5YXHZXF4LELQZKTAU`; `aws ecs list-tasks --desired-status RUNNING` → only the service task; every operator task of this trial STOPPED with exit 0; no temporary resource left.

## 9. Ledger, evidence, hand-off (O, ~17:20–18:00)

- [ ] 9.1 Ledger: add the trial's task-seconds and the two redeploys to `$W78/budget-ledger.json` `basis` (November, always-on plan, cap $170 GenUI / $208 project); [read] `app/runtime/.venv/bin/python infra/scripts/readonly_snapshot.py --private-dir $PRIV/snapshot --summary $PRIV/trial/snapshot-after.json` (Cost Explorer month-to-date). Attach FormaStack's fAuth cost receipt (FORMA-367 allowance $33–38). Reconciled against the approved figure, not ADR-003's $100.
- [ ] 9.2 `rm -P $PRIV/trial-cookies.json` (the values are already dead: restart and revocation). Codes: NS stays the beta's; SPARE revoked; CANARY consumed and its entitlement revoked.
- [ ] 9.3 Copy the sanitized files of `$PRIV/trial/` to `evidence/APP-076/trial/` with a SHA-256 list; `$TC scan --root evidence/APP-076 --self-test` → no findings. Write `evidence/APP-076/README.md` binding: candidate commit, CI run ids, `SERVICE_DIGEST`, `CANDIDATE_TD`/`PREVIOUS_TD` revisions, fAuth issuer/client names and image digest (`sha256:d1365238…`), fixture hashes (`contracts/fixtures/impl-2026-10-28.1/*.json`, source revision `9161c1e4…`), each step's result and every failure with its disposition (item 11).
- [ ] 9.4 Tier A reviewer (item 11) binds the receipts; then `evidence/phases/PH01/GATE.md` is filled (ADR-010 §4.4).

## Acceptance items → steps

| Item | Steps | Not live / limitation |
| --- | --- | --- |
| 1 migrations, operator issue/list/revoke, role and forged-operator denial | 0.5, 0.6, 6.1, 7.3 | role denials via APP-078 qualification mapping |
| 2 public-API denials, admitted member succeeds, revocation | 1.2, 3.2, 6.2, 6.4 | in-flight revocation and expired class local only (G10); dispatch counters absent live (G6) |
| 3 public HTTPS, invite-only registration in Chrome, callback, code admission, logout, mailbox, receipts, keyboard/error states | 1.1, V1–V9, V13, 8.4 | Safari/Firefox: APP-079 |
| 4 ADR-006 routes, resend mapping, cooldown, 5/24 h, forged forwarded IP, activation by fresh sign-in | V3–V6, 1.3, logs part 1 | 5/24 h only if V4 is done |
| 5 PAR/mail/storage/log inspection, exposed-token detection | V2, V8, 7.4 | backend PAR attested by FormaStack's records |
| 6 integrated path, retry, stale conflict, restart/reopen, database-backed | V7, V10–V13, 5.1, 7.1 | Spend Review in `…0002` (D1) |
| 8 ingress, rollback redeploy with protected 404, restore, digest, cleanup, ledger | 8.1–8.5, 9.1 | needs a deny-all `PREVIOUS_TD` (G11) |
| 9 private-route refusal matrix, forged token, replayed code, cross-site POST, canary | 1.2, 3.2, 6.2, 6.4 | G7/G8 fixed at `dc78707` (local binary: every row passes); a real consumed code is never replayed by the script (forged codes only; fAuth replay 400 is FormaStack's FORMA-366/367 receipt) |
| 10 audit insert-only, chains verify on RDS | 7.1, 7.2 | G14 mapping |
| 11 reviewer binding | 9.3, 9.4 | — |

## Gaps that stop or weaken the trial (found 2026-10-30 at `89c6ec1`)

**Dispositions (orchestrator, 2026-10-30).**
- **G3, decision D1 reversed:** Spend Review runs in Northstar (`…0001`), as ADR-010 act 6 and demo §4 place it, through a new labeled operator seed command (Tier A, `app/runtime/admission_operator.py`). Victor holds one membership. The canary organization stays a pure outsider for the replay. §4 of this runbook is rewritten for Northstar when that command lands; until then the `…0002` steps are superseded, not runnable.
- **G4 and G12:** fixed in `evidence/APP-078/prep/RUNBOOK.md` (new steps 11.4 bound work `…00f2` through the documented override, creator `…0066`; 11.5 the sign-in page check moved out of 13.3) and `APPROVALS.md` A9.
- **G6:** assigned to the APP-012 review-fix pass (one structured log line per dispatch and release decision).
- **G1, G2:** in the APP-074 / B1 live-identity pass; re-checked when it reports.
- **G5, G11:** the operator-and-refusal gap pass after APP-012 and APP-074 merge (principal id in `list`; a deny-all rollback revision on the migrated schema).
- **G7, G8 fixed** (`dc78707`, 2026-10-30): `http::UniformPath` maps every path rejection to the uniform 404 on every product route with path parameters; `http::uniform` composes the product router so that a method mismatch is byte-identical to an unknown path (no `Allow`). The local-binary matrix passes every row ([local-binary-matrix.json](local-binary-matrix.json)); `test_trial_checks.py`'s local-binary test now requires zero failures.
- **G9 fixed** (`895f530`, 2026-10-30): `GET /workspace` lists the member's openable works; evidence in [`evidence/APP-009/workspace-listing/`](../../APP-009/workspace-listing/README.md).
- **G13 closed:** FormaStack confirmed 2026-10-30 that both return entries are registered since 2026-10-29 (northstar-platform and canary-platform, each to its `/signin/{handle}` URL on `beta.getforma.dev`, exact callback `/auth/callback`).
- **G15 closed:** FormaStack confirmed `victor@getforma.dev` has no account in either beta tenant; the fresh invited signup is the path. Use delivery mode `server_only`; the 60-second resend cooldown starts at signup. FormaStack has no fAuth operation planned during the Nov 1 window or this trial.
- **G10, G14:** reviewer dispositions at the gate.


| # | Gap | Effect | Owner (file) |
| --- | --- | --- | --- |
| G1 | No hosted sign-in wiring: the stack supplies no auth configuration (`GENUI_AUTH_CONFIG_FILE`), no identity registry (`GENUI_IDENTITY_REGISTRY_FILE`) and no provider capability directory (`GENUI_PROVIDER_CAPABILITY_DIR`); the seed binds both organizations to the fixture issuer `app008-fixture-only`, and no operator command binds the real fAuth issuer/tenant to an existing organization (`provision` creates a new one). APP-078 README B1. | **Stops** §2 onward: every sign-in fails neutrally, every protected route is 404. | `infra/src/stack.ts`, `app/runtime/runtime.py` (delivery through the runtime helper on the read-only filesystem), `app/runtime/admission_operator.py` (binding subcommand) |
| G2 | Managed issuance cannot give a code its provider invitation: `issue --code-digest` refuses every provider-invitation flag, `issue --provider-email` and `reconcile` refuse in a managed task, and `/invite/validate` refuses a code with no usable invitation capability (`app/src/auth/invite.rs`: "without a usable invitation" → neutral page). The RDS is reachable only from tasks, so a local `issue --provider-email` cannot write the row either. | **Stops** V1/V9: a code issued by APP-078 15.3 cannot start a signup. | `app/runtime/admission_operator.py` + `app/runtime/provider_management.py` (a managed invitation channel), `infra/src/stack.ts` (capability delivery; custody per APP-084 and the ADR-003 amendment) |
| G3 | Spend Review on RDS exists only in `…0002` (seed); the Northstar-organization Spend Review of the APP-013/014 campaigns needs definition admission, publication and work creation, which have no operator command or route. | Prepared workaround D1 (Spend Review in `…0002`, second code); ADR-010 act 6 / demo §4 place it in Northstar's Finance space. | decision: orchestrator; channel if wanted: `app/runtime/admission_operator.py` (Tier A) |
| G4 | APP-078's runbook has no `create-bound-work` step (APP-012 D1 production order; override `admission-create-bound-work` exists), and APP-078 A9's grants name the placeholder-bound work `…0069`. | **Stops** V7: without the bound work the Northstar views have no uploaded source (`source_unavailable`). | `evidence/APP-078/prep/RUNBOOK.md` §11 (add 11.4) and `APPROVALS.md` A9 |
| G5 | No operator output names the principal of a redeemed code (`admission list` shows codes, status and counts only). | 6.4 cannot target Victor's Canary principal; the revoked class and "no revival" in 8.4 are not run. | `app/runtime/admission_operator.py` (`list` with redemption principals, or a members subcommand) |
| G6 | The deployed service records no query dispatch: `app/src/query/worker.rs` and `query/routes.rs` emit no event on admission or launch; dispatch counters exist only in the local counting executors. | Item 2's "invoke no query engine … retain dispatch counters" has no live evidence beyond statuses. | `app/src/query/worker.rs` (a sanitized `query.dispatch` event) |
| G7 (**fixed** `dc78707`) | An undecodable path segment (`%FF`) on `POST /v1/work/{app}/{work}/commands/{command}` and `GET /work/{app}/{work}/records/{collection}/{record}` answers axum's `400 text/plain` without the security headers, not the uniform 404; both handlers extract `Path<…>` without a rejection handler, unlike `query/routes.rs` and `auth/routes.rs`. Found by `trial_checks.py` on the local binary (12 rows). | Item 9 fails on these rows. | `app/src/commands/routes.rs` (`submit`, `record_page`: `Result<Path<…>, PathRejection>` → `http::not_found()`) |
| G8 (**fixed** `dc78707`) | A method mismatch on an existing route answers the uniform 404 bytes plus `Allow: POST` or `Allow: GET,HEAD` (axum adds it under `method_not_allowed_fallback`), so a wrong method on a protected route differs from an unknown path by one header (30 rows locally). | Item 9 ("same … headers") fails on these rows; route-shape disclosure, not resource existence. | `app/src/main.rs` / `app/src/http/mod.rs` (strip `Allow` from the fallback answer) |
| G9 (**fixed** `895f530`) | `GET /workspace` lists no app or saved work (APP-012 review OQ5; owner being assigned). | Victor types the URLs (table above). ADR-010 act 1 "home page listing the admitted app and saved work" is not shown. | `app/src/auth/routes.rs` `workspace`, `app/src/storage/queries.rs` `Data::openable_works`, `app/admin/src/workspace/app.tsx` |
| G10 | Revocation while a query is queued/executing cannot be timed live (no hook; task start ≫ query time). | Item 2's in-flight half stays local (APP-012/014); needs reviewer disposition. | APP-076 item 2 / gate record |
| G11 | The rollback target must be a deny-all revision on the migrated schema. The pre-window revision `GenuiSyntheticFoundationTaskE2459512:2` (schema 1) is incompatible after migration. If G1 lands as a later revision, APP-078's window revision (same image, no provider routing or registry: deny-all) is `PREVIOUS_TD`; if G1 is wired into the window's own revision, no deny-all revision exists. | **Stops** §8 unless `PREVIOUS_TD` exists. | `infra/src/stack.ts` (with G1) and APP-078's window record (`PREVIOUS_TD`) |
| G12 | APP-078 runbook 13.3 lists `/signin/aaaaaaaaaaaaaaaaaaaaaaaaaa` among the uniform 404 paths; ADR-006 and `auth/routes.rs` (`signin_handle_page`) render the neutral sign-in page (200) for any well-formed handle. | APP-078 13.3 would stop on a correct answer. | `evidence/APP-078/prep/RUNBOOK.md` 13.3 |
| G13 | fAuth `return_entry_url` per organization not yet registered (FORMA-367 receipt). | Not a stop: after confirming the mail, fAuth shows its no-link fallback; V6 returns through the pending page or the typed sign-in URL. | FormaStack lane |
| G14 | No post-trial insert-only proof except a qualification re-run, and its checks must be mapped to item 10's wording. | Item 10 may stay partly open. | APP-078 qualification entry / reviewer |
| G15 | Unknown whether `victor@getforma.dev` already has an account in either beta tenant (FormaStack provisioned Northstar three users, Canary one). | A pre-existing account could refuse the invited signup at V2/V9. | FormaStack confirmation (0.4) |
