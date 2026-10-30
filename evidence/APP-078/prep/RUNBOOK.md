# APP-078 window runbook — Nov 1, ~17:00–20:00 UTC

**Preparation, not evidence.** Nothing below has run. Prepared 2026-10-30 at `e2c0958` plus the APP-078 preparation commits, against the read-only [snapshot](snapshot.json) of 2026-10-30T02:24Z. Every AWS mutation is marked **[MUTATION · Ax]** with the approval in [APPROVALS.md](APPROVALS.md) that covers it; nothing marked so runs without that approval recorded. Read-only calls are marked [read]. Times are UTC.

Stop rules (MVP plan §Stop rules, unchanged): stop at any unexpected readiness difference, any migration that does not apply once-and-again cleanly, or any qualification check that fails. The snapshot is retained and the service stays at desired count 0 until the cause is understood; the beta goes live a day late rather than on an unqualified schema. No mutation without the reserved ledger entry and Victor's slot approval; no fAuth operation overlaps the window. A failed or unknown AWS call keeps its reservation and is reconciled, never retried blindly.

## Fixed names

```sh
export AWS_REGION=us-east-1 AWS_PAGER= AWS_MAX_ATTEMPTS=1
REPO=/Users/victorvillacis/dev/FormaBuiltApps/forma-app-platform        # main checkout at the candidate tip
PRIV=$REPO/.cache/operations/beta/app-078-window                      # gitignored; mkdir -m 700
WINDOW=88d1b19d-db20-4593-993c-de275d4e7318                           # proposed window UUID (A1); keep across retries
STACK=GenuiSyntheticFoundation
REG=877573132739.dkr.ecr.us-east-1.amazonaws.com
CLUSTER=GenuiSyntheticFoundation-ClusterEB0386A7-HMlpmHwGMVSN
SERVICE=GenuiSyntheticFoundation-ServiceD69D759B-pNHHjoez1RWF
DB=genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg             # resource id db-KW4LRMTPI5YXHZXF4LELQZKTAU
LOGS=GenuiSyntheticFoundation-Logs6819BB44-ujo9czITOJoW
ASSETS=cdk-hnb659fds-assets-877573132739-us-east-1
WR="python3 infra/scripts/window_requests.py"                          # from $REPO
```

Every standalone task below follows the same **run pattern**; its expected output is given per step:

```sh
# 1 compose (local)      $WR operator-task --outputs $PRIV/stack-after.json --window-id $WINDOW --step <step> --override <name> [--set=--flag=value ...] --output $PRIV/run-<step>.json
# 2 run   [MUTATION A5]  aws ecs run-task --cli-input-json file://$PRIV/run-<step>.json > $PRIV/run-<step>.receipt.json
# 3 wait  [read]         TASK=$(jq -r '.tasks[0].taskArn' $PRIV/run-<step>.receipt.json); aws ecs wait tasks-stopped --cluster $CLUSTER --tasks $TASK
# 4 check [read]         aws ecs describe-tasks --cluster $CLUSTER --tasks $TASK > $PRIV/run-<step>.stopped.json
#                        expect .tasks[0].lastStatus STOPPED, .containers[0].exitCode 0, stopCode EssentialContainerExited
# 5 log   [read]         aws logs get-log-events --log-group-name $LOGS --log-stream-name genui-operator/operator/${TASK##*/} --start-from-head > $PRIV/run-<step>.log.json
```

`run-task` carries a `clientToken` derived from the window and step, so a lost response retried with the **same file** cannot start a second task. A nonzero exit, a missing log line or a task that does not reach STOPPED is a stop; record the sanitized refusal code (operator refusals print one fixed reason, never a value).

## 0. Before the window (Oct 31 – Nov 1, no AWS mutation)

- [ ] 0.1 Victor's approvals A1–A9 recorded (quote his words in `$PRIV/approvals.md`). FormaStack confirms no fAuth deployment operation 17:00–20:00 and that the `ten_`/`usr_` ids and beta client are unchanged.
- [ ] 0.2 Candidate tip = PR 4's merge (or the contingency candidate branch, MVP plan Task 20). `git -C $REPO status --porcelain` empty; record `CANDIDATE=$(git -C $REPO rev-parse HEAD)`. Its exact-source CI runs are green (record run ids).
- [ ] 0.3 Local checks at the tip, recorded with exit codes: `cd infra && npm ci && npm run typecheck && npm test`; `python3 -m unittest discover -s app/operator -p 'test_*.py'` (operator lock venv, `GENUI_OPERATOR_TEST_PARSER` set); `python -m unittest discover -s app/runtime` (runtime lock venv). Expected: all pass.
- [ ] 0.4 Review the [ten-minute campaign cap](#campaign-cap-and-task-hours) and `extra_task_hours: 3` against the image sizes measured in step 2.

## 1. Observe (read-only)

- [ ] 1.1 [read] `aws sts get-caller-identity` → `Account` `877573132739`. Otherwise stop.
- [ ] 1.2 [read] `app/runtime/.venv/bin/python infra/scripts/readonly_snapshot.py --private-dir $PRIV/snapshot --summary $PRIV/snapshot-before.json`. Expected, as on 2026-10-30: stack `UPDATE_COMPLETE`, no change sets; service `ACTIVE` desired/running/pending **0/0/0**; no RUNNING task in the cluster; RDS `available`, `postgres 16.13`, `db.t4g.small`, encrypted, not public, IAM auth, deletion protection on, auto minor upgrade off, backups 7 days, resource id `db-KW4LRMTPI5YXHZXF4LELQZKTAU`; stack policy equal to `infra/stack-policy.json`; listener 443 with the `beta.getforma.dev` certificate `ISSUED`. Any difference stops the window.
- [ ] 1.3 [read] `dig +short beta.getforma.dev CNAME` → the stack output `LoadBalancerDnsName` (lowercased, trailing dot). This is the existing Porkbun record; no DNS change is planned.

## 2. Build and qualify the three images locally (ARM64)

All three task definitions share the one `ImageArchitecture` parameter, so the three images must be the same architecture. Build **ARM64** natively on the Apple-silicon operator machine (the deployed task definitions are already ARM64; the x86 price is only the ledger's conservative rate). An x86_64 build would have to be all three images.

- [ ] 2.1 `docker build --platform linux/arm64 -f app/Dockerfile -t genui-service:$CANDIDATE .` → built.
- [ ] 2.2 `python3 app/runtime/qualify_image.py --image genui-service:$CANDIDATE --output $PRIV/service-image-qualification.json` → `passed: true`.
- [ ] 2.3 `docker build --platform linux/arm64 -f app/qualification/Dockerfile -t genui-qualification:$CANDIDATE .`; `app/runtime/.venv/bin/python app/qualification/check_image.py --image genui-qualification:$CANDIDATE --base-revision $CANDIDATE --output $PRIV/qualification-image-check.json` → every expectation met, cleanup complete; `app/runtime/.venv/bin/python app/qualification/local_harness.py --image genui-qualification:$CANDIDATE --output $PRIV/qualification-local-positive.json` → 10 of 10.
- [ ] 2.4 `docker build --platform linux/arm64 -f app/operator/Dockerfile -t genui-operator:$CANDIDATE .`; `$OPVENV/bin/python -P app/operator/check_image.py --image genui-operator:$CANDIDATE --base-revision $CANDIDATE --output $PRIV/operator-image-check.json` → every expectation met, cleanup complete.
- [ ] 2.5 Parser identity: the guest digest in the operator image's `parser-manifest.json` equals the service image's (`docker run --rm --entrypoint cat <image> <path>`); record both.
- [ ] 2.6 Record each image's local ID (`docker image inspect --format '{{.Id}}'`), size and `build-identity.json` digest. Expected sizes near 80 MB (service), 498 MB (qualification), 438 MB (operator).

## 3. Registry [MUTATION · A3]

- [ ] 3.1 Repositories. Default (A3): two new private repositories so each execution role pulls only its own artifact: `aws ecr create-repository --repository-name genui-qualification --image-tag-mutability IMMUTABLE --image-scanning-configuration scanOnPush=true --encryption-configuration encryptionType=AES256`, and the same for `genui-operator`. The service image stays in the existing `genui-synthetic`. (Alternative if A3 says so: all three in `genui-synthetic` with distinct tags; digests still keep them distinct.)
- [ ] 3.2 `aws ecr get-login-password | docker login --username AWS --password-stdin $REG` (the token goes to Docker's store only).
- [ ] 3.3 Tag with `app078-${CANDIDATE:0:7}-arm64` and push each: `docker tag genui-service:$CANDIDATE $REG/genui-synthetic:app078-${CANDIDATE:0:7}-arm64 && docker push $REG/genui-synthetic:app078-${CANDIDATE:0:7}-arm64` (and `genui-qualification`, `genui-operator`). Tags are immutable; a repeated push of the same tag fails rather than replacing it.
- [ ] 3.4 [read] `aws ecr describe-images --repository-name <repo> --image-ids imageTag=app078-${CANDIDATE:0:7}-arm64 > $PRIV/ecr-<repo>.json`; record `imageDigest`. `docker image inspect --format '{{json .RepoDigests}}' <local>` must contain `<repo>@<digest>`. A mismatch stops (the pushed artifact is not the one qualified in step 2).
- [ ] 3.5 [read] `aws ecr describe-image-scan-findings --repository-name <repo> --image-id imageDigest=<digest>`; record severity counts. A CRITICAL finding is Victor's call (A3), not silently accepted.

Set `SERVICE_DIGEST=$REG/genui-synthetic@sha256:…`, `QUALIFICATION_DIGEST=$REG/genui-qualification@sha256:…`, `OPERATOR_DIGEST=$REG/genui-operator@sha256:…`.

## 4. Reconcile and reserve (local)

- [ ] 4.1 Adopt the always-on ledger (A2): `install -m 600 $REPO/.cache/operations/beta/app-078-prep/budget-ledger-always-on.proposed.json $PRIV/budget-ledger.json`. The schema 1 `budget-ledger.json` stays untouched as the October record. In `$PRIV/budget-ledger.json` month `2026-11`: set `reconciled_at_utc` to now, replace the `PROPOSED …` sentence of `basis` with the reconciliation actually done (snapshot 1.2, Cost Explorer month-to-date, any November spend so far in `prior_extra_spend_cents`). Keep `variable_reserve_cents` ≥ 1000, `auth_spent_or_reserved_cents` 3800, `unallocated_cents` 0.
- [ ] 4.2 `cp $REPO/.cache/operations/beta/app-078-prep/deployment-input-app078.template.json $PRIV/deployment-input.json` and replace the three `REPLACE_AT_WINDOW_*` values with the digests of step 3. Everything else stays: `operating_plan: always-on-beta`, `ingress_mode: public`, `deployment_expires_at_utc: 2026-11-30T23:00:00Z`, `image_architecture: ARM64`, `service_desired_count: 0`, `extra_task_hours: 3`, empty `qualification_config_secret_arn`.
- [ ] 4.3 `python3 infra/scripts/preflight.py --input $PRIV/deployment-input.json --ledger $PRIV/budget-ledger.json > $PRIV/preflight-receipt.json`. Expected: `status reserved_local_plan_only`, `operating_plan always-on-beta`, `cloudformation_parameters.MonthlyBudgetUsd "170"`, both task-image digests present, `ServiceDesiredCount "0"`; `cost_by_month.2026-11` about `retained_core 5074 + window 8580 + variable 1500 = genui_total 15154` (≤ 17000) and `project_total 18954` (≤ 20800) if reserved at 17:00. A refusal stops; never edit the ledger to pass it.

## 5. Stack update through a reviewed change set

- [ ] 5.1 `cd infra && npm ci && npm run synth -- --quiet`; record `shasum -a 256 cdk.out/GenuiSyntheticFoundation.template.json`.
- [ ] 5.2 `$WR change-set --receipt $PRIV/preflight-receipt.json --template infra/cdk.out/GenuiSyntheticFoundation.template.json --window-id $WINDOW --output $PRIV/change-set-request.json` → prints the upload `key` (`<template sha256>.json`) and `checksum_sha256_base64`. Refuses if the receipt is not this window's or its parameters differ from the template's.
- [ ] 5.3 [MUTATION · A4] `aws s3api put-object --bucket $ASSETS --key <key> --body infra/cdk.out/GenuiSyntheticFoundation.template.json --content-type application/json --checksum-sha256 <b64> --if-none-match '*'` → ETag. A 412 means the same key exists: `aws s3api head-object --bucket $ASSETS --key <key> --checksum-mode ENABLED` must report the same `ChecksumSHA256`.
- [ ] 5.4 [MUTATION · A4] `aws cloudformation create-change-set --cli-input-json file://$PRIV/change-set-request.json > $PRIV/change-set-create.json` → `Id`; [read] `aws cloudformation wait change-set-create-complete --stack-name $STACK --change-set-name genui-app078-$WINDOW`. A `FAILED` change set (for example the APP-075 m10 doubt about the disabled-task intrinsics) stops the window and is recorded for APP-086; it changes nothing.
- [ ] 5.5 [read] `aws cloudformation describe-change-set --stack-name $STACK --change-set-name genui-app078-$WINDOW > $PRIV/change-set-describe.json`; `$WR review-change-set --describe $PRIV/change-set-describe.json --receipt $PRIV/preflight-receipt.json --output $PRIV/change-set-review.json` → `acceptable: true`. It refuses any Remove or True/Conditional replacement of the database, load balancer (the DNS target), listener, cluster, service, VPC, keys, bucket, secrets or log group, and any parameter that differs from the receipt. Expected by hand as well: Add `QualificationTask…`, `QualificationExecutionRole…`, `OperatorTask…`, `OperatorRole…`, `OperatorImagePull`; Modify (replace) `Task…`/`MigrationTask…` (new revisions on the new digest); Modify `Service…` (task definition and deployment configuration 0/100, no replacement); Modify `DeploymentExpiry…` (new `at(2026-11-30T23:00:00)`); tag-only Modify (`ApprovedMonthlyBudgetUsd` 170) on tagged resources including the database, in place. Anything else is reviewed before 5.7.
- [ ] 5.6 [read] `aws cloudformation get-stack-policy --stack-name $STACK` equals `infra/stack-policy.json` (installed in APP-007; re-installing is not needed and is not planned).
- [ ] 5.7 [MUTATION · A5] With 1.2's 0/0/0 still true (re-run `aws ecs describe-services --cluster $CLUSTER --services $SERVICE` [read]): `aws cloudformation execute-change-set --stack-name $STACK --change-set-name genui-app078-$WINDOW --client-request-token app078-$WINDOW`; [read] `aws cloudformation wait stack-update-complete --stack-name $STACK`. Expected `UPDATE_COMPLETE`. `UPDATE_ROLLBACK_*` stops the window (the database cannot be replaced or deleted through it).
- [ ] 5.8 [read] `aws cloudformation describe-stacks --stack-name $STACK > $PRIV/stack-after.json`. Parameters equal the receipt; outputs now include `QualificationTaskDefinition`, `OperatorTaskDefinition`, `OperatorTaskRoleArn`; `TaskDefinition` revision advanced. `aws scheduler get-schedule` (group/name from outputs) → `ENABLED`, `at(2026-11-30T23:00:00)`, UTC, target input `DesiredCount 0`. `aws ecs describe-task-definition` for each → `runtimePlatform.cpuArchitecture ARM64`, images equal the three digests. `aws ecs describe-services` → `deploymentConfiguration` minimum 0 / maximum 100, desired 0. `aws rds describe-db-instances --db-instance-identifier $DB` → same resource id, `available`.

## 6. Read-only readiness comparison before any data mutation

- [ ] 6.1 Run pattern, step `verify-baseline`, override `verify-catalog-baseline` (`verify-catalog --version 1`). Expected log line `{"catalog_version": 1, "command": "verify-catalog", "read_only": true}`. This proves the live catalog equals the reviewed version 1 baseline under the approved creator-grant normalization, origin replication mode and the version row, read-only. `catalog_refused` stops the window: the difference is reviewed, never normalized automatically.
- [ ] 6.2 Run pattern, step `upload-plan`, override `upload-sources-plan`. Expected: one JSON plan whose registration record has `revision_id 9161c1e4-27c0-4cbb-addf-eb7ce504bf45`, `content_digest sha256:6e0f0ed7…4647`, `normalized_digest = object_digest sha256:0c2582d8…a082`, `row_count 6`. This is also the first observation that the Fargate `/run/operator` volume is writable by UID 65532 (APP-077 open question 7); a scratch refusal stops.

## 7. Pre-migration snapshot [MUTATION · A5]

- [ ] 7.1 `aws rds create-db-snapshot --db-instance-identifier $DB --db-snapshot-identifier genui-app078-pre-migration-schema1-20261101 --tags Key=Window,Value=$WINDOW Key=Schema,Value=1 > $PRIV/snapshot-create.json`; [read] `aws rds wait db-snapshot-available --db-snapshot-identifier genui-app078-pre-migration-schema1-20261101`; `aws rds describe-db-snapshots --db-snapshot-identifier …` → `available`, `Encrypted true`, `EngineVersion 16.13`, `DbiResourceId db-KW4LRMTPI5YXHZXF4LELQZKTAU`. Its restore and comparison are APP-080's; it is retained and unproven until then.

## 8. Migrations, once and again

- [ ] 8.1 Step `migrate-1`, override `migrate`. Expected `{"command": "migrate", "database_version": N, "migrations": [...]}` with N the candidate's `app/migrations/manifest.json` version (6 at `e2c0958`; 7 once APP-013's 0007 lands) and one entry per migration with its manifest SHA-256.
- [ ] 8.2 Step `migrate-2`, override `migrate`. Expected: output byte-identical to 8.1 (a repeat is a no-op; the history ledger is proven and unchanged).
- [ ] 8.3 Step `verify-latest`, override `verify-catalog`. Expected `catalog_version N`, `read_only true`.

## 9. Seed, once and again

- [ ] 9.1 Step `seed-1`, override `seed`. Expected `catalog_version N`, `organizations ["…0001", "…0002"]`, the bundle and app list of `tenant_seed.make_bundles()`.
- [ ] 9.2 Step `seed-2`, override `seed`. Expected identical output (exact-match idempotent). `seed_refused` stops.

## 10. Admission foundation

- [ ] 10.1 Steps `backfill-northstar`, `backfill-canary` (overrides `admission-backfill-*`). Expected `{"organization_id": …, "created": […], "existing": […], "deferred": […]}` (genesis audit and sequence heads; idempotent).
- [ ] 10.2 Steps `handle-northstar`, `handle-canary`, override `admission-issue-handle` with `--set=--organization=<org> --set=--handle=<handle>` from [evidence/APP-009/handles.json](../../APP-009/handles.json): Northstar `se45m37vabqrfvbh3gbwn22qkm`, Canary `z33gibxjspwxcnopbsdxqtijey`. Expected `{"handle": <the same handle>, "live_handles": 1, "organization_id": <org>}`. A second run refuses `handle_exists`: run each once.
- [ ] 10.3 Steps `verify-northstar`, `verify-canary` (overrides `admission-verify-*`). Expected `verified: true`.

No `provision` runs: both fixture organizations come from the seed. `provision` is for a new organization only.

## 11. Fixture source objects and their registration

- [ ] 11.1 Step `upload-1`, override `upload-sources`. Expected: exactly one registration record equal to 6.2's (the real objects at `o/…0001/s/…006e/r/9161c1e4-…/original` and `…/part.parquet`). Save it: `jq` the log line into `$PRIV/registration-northstar.json`.
- [ ] 11.2 Step `upload-2`, override `upload-sources`. Expected: the same record, both objects reported unchanged. This is the managed observation that `If-None-Match: *` is enforced, that the identity policy's `s3:if-none-match` condition admits the conditional PUT and that `kms:Decrypt` through S3 admits the checksum-mode HEAD (APP-077 open questions 1 and 8). `object_write_refused` stops for review.
- [ ] 11.3 Step `register-1`: `$WR operator-task … --step register-1 --override admission-register-source-revision-inline --record $PRIV/registration-northstar.json`. Expected `changed: true`, `row_count 6`. Step `register-2`, same record: `changed: false`.

- [ ] 11.4 Step `bound-work-1` (APP-012 D1 production order; gap G4 of the APP-076 runbook): `$WR operator-task … --step bound-work-1 --override admission-create-bound-work` (the documented override in `app/operator/overrides.json` already carries the exact production arguments: organization `…0001`, app `…0065`, from-work `…0069`, new work `…00f2`, creator `…0066`, input `events`, source `…006e`, revision `9161c1e4-…`; no `--set` is needed). The creator is the seeded Northstar member `…0066` (an active member holding `run` on the app and `source_read` on `…006e` in the seed, as the APP-012 campaign uses it; confirm with `list` in 10.x before running), because Victor is not a member until he redeems his code on November 2. Expected `changed: true` and a new saved work at version 1 bound to the registered revision; step `bound-work-2`, the same arguments: `changed: false`. The seeded work `…0069` keeps its placeholder binding and stays `source_unavailable`; APP-076 opens `…00f2`. A `creator_lacks_authority` refusal stops for review.
- [ ] 11.5 Signed-out sign-in page check (moved out of 13.3's uniform-refusal list): `GET /signin/aaaaaaaaaaaaaaaaaaaaaaaaaa` (an unissued handle) answers the ADR-006 sign-in page (200), byte-identical to the page for an issued handle except for nothing that discloses whether the handle exists, with no `Set-Cookie` and no provider redirect until the form is posted. Run it in 13.3's session and save it beside the refusal set.

## 12. Qualification task (six checks, role bindings, refusals)

- [ ] 12.1 Compose and run back to back (the manifest's 600 s span starts at composition): `$WR qualification-task --outputs $PRIV/stack-after.json --window-id $WINDOW --step qualification --rds-resource-id db-KW4LRMTPI5YXHZXF4LELQZKTAU --output $PRIV/run-qualification.json && aws ecs run-task --cli-input-json file://$PRIV/run-qualification.json > $PRIV/run-qualification.receipt.json`. The composer refuses if the live resource id, CA pin, host or registry bytes differ from the supervisor's pins.
- [ ] 12.2 Wait, describe, and read stream `genui-qualification/qualification/<task id>`. Expected: one `APP008_MANAGED_RESULT` line with `passed: true`, this `run_id`, six passed checks and `cleanup_confirmed: true`, then one `APP075_SUPERVISOR` summary; exit 0. Anything else stops. This is also the first observation of the Fargate `/run/genui` task volume (0700, UID 65532) and of the Fargate environment names against the supervisor's allowlist (APP-075 m9, APP-086).

The ticket's full grant/RLS isolation rows beyond these six checks (wrong user/token, expired material, wrong hostname/CA, plaintext, schema 1 image incompatibility) are listed in [README.md](README.md) §Gaps: each needs its own reviewed managed case before it can be checked.

## 13. Production HTTPS surface from the exact image [MUTATION · A7]

- [ ] 13.1 In `$PRIV/deployment-input.json` set `service_desired_count` to `1`; `python3 infra/scripts/preflight.py --input $PRIV/deployment-input.json --ledger $PRIV/budget-ledger.json --activate > $PRIV/activation-1.json`. Expected `reused_reservation true` and `activation.status activation_requested_not_healthy`. It checks the account, the completed stack and its parameters, the enabled UTC stop at `2026-11-30T23:00:00`, the service identity and task revision before requesting count one.
- [ ] 13.2 [read] `aws ecs wait services-stable --cluster $CLUSTER --services $SERVICE`; `describe-services` → running 1; `aws elbv2 describe-target-health` → one `healthy` target. A circuit-breaker rollback or unhealthy target stops (set count 0, step 14.1).
- [ ] 13.3 From the operator machine over the public internet, with certificate and hostname verification (plain `curl`, never `-k` or the raw ALB name), save headers and body per path in `$PRIV/https/`:
  - `https://beta.getforma.dev/` → 200 landing; `/health/live` → 200; `/health/ready` → 200.
  - Uniform refusal: `/does-not-exist`, `/w/00000000-0000-4000-8000-000000000065`, `/api/query`, `/api/commands`, `/admin`, `/auth/qualification/start` → the same status, byte-identical body and the same security headers (compare SHA-256s); no `Set-Cookie`, no provider redirect.
  - [read] `aws ec2 describe-security-groups --group-ids sg-0b24119fe39e35f82` → 443 from `0.0.0.0/0` only (never limited to one operator address).

## 14. Stop, verify cleanup, reconcile

- [ ] 14.1 [MUTATION · A7] `aws ecs update-service --cluster $CLUSTER --service $SERVICE --desired-count 0`; [read] `aws ecs wait services-stable`; `describe-services` → 0/0/0; `aws ecs list-tasks --cluster $CLUSTER --desired-status RUNNING` → empty; `describe-tasks` for every task ARN of this window → STOPPED with its exit code. Record created→stopped seconds per task.
- [ ] 14.2 [read] Temporary task-definition cleanup: `aws ecs list-task-definitions --status ACTIVE` shows only the stack's families (no hand-registered family was used in this window). The pre-migration snapshot is listed `available` under its name. RDS resource id unchanged.
- [ ] 14.3 Ledger reconciliation: add the window's actual standalone task-seconds and any observed charges to `$PRIV/budget-ledger.json` `basis` (reservation retained, never released); [read] Cost Explorer month-to-date via `readonly_snapshot.py --summary $PRIV/snapshot-after.json`.
- [ ] 14.4 Copy the sanitized receipts (no endpoint, operator address, raw environment or code) to `evidence/APP-078/window/` with their hashes. The Tier A review binds them (APP-078 stays open on any partial row or cleanup uncertainty).

## 15. Beta up (plan Task C3; APP-076's precondition, not APP-078's acceptance)

- [ ] 15.1 [MUTATION · A8] `preflight.py --activate` again with the same input (count one) → `activation_requested_not_healthy`; wait stable; healthy; repeat 13.3's landing and health checks.
- [ ] 15.2 DNS: `beta.getforma.dev` already targets this load balancer (1.3) and the change-set review refused any load-balancer replacement, so no DNS change is made. If 5.8 ever showed a different `LoadBalancerDnsName`, stop: a Porkbun change needs its own approval.
- [ ] 15.3 Invite codes [MUTATION · A9]. For each approved recipient label: `$WR mint-code --organization 00000000-0000-4000-8000-000000000001 --label <label> --out-dir $PRIV/codes` (directory `mkdir -m 700`) → prints only the digest; then step `issue-<label>`, override `admission-issue-preminted` with `--set=--code-digest=<digest> --set=--issued-by=app078-operator` and the approved grants (`--set=--grant=<first>` then `--add=--grant=<each further>`), and `--set=--expires-days`/`--set=--max-redemptions` if A9 differs from the defaults (14 days, 1 redemption). Expected log `{"code_digest": <digest>, "code_id": …, "expires_at": …}`; never a `code` field. Then step `list-northstar` (`admission-list-northstar`) → the codes `active` with zero redemptions. Deliver each raw code from `$PRIV/codes/access-code-<label>.json` once, privately, by the channel Victor names (A9); never in a task override, log, ticket, chat transcript or evidence.
- [ ] 15.4 Redemption is **not** possible yet: see README §Blockers B1 (the stack gives the service no provider configuration or identity registry, and no operator command binds the real fAuth tenant to a seeded organization).

## Rollback

| Where it fails | What to do |
| --- | --- |
| Before 5.7 | Nothing changed in the stack; delete nothing. An unexecuted change set may stay for review. |
| 5.7 rolls back | CloudFormation restores the previous template; the database cannot be replaced or deleted through the stack policy. Service stays 0. |
| 6 (baseline differs) | No data changed. Service stays 0; review the sanitized difference. |
| 8.1 fails | `apply_migrations` is one transaction: prove the source stayed at version 1 with step `verify-baseline` again. Forward fix in a later window. |
| After 8.1 committed | No schema rollback. The pre-migration snapshot is retained but unproven (APP-080); the only rollback for a committed migration during the beta is a reviewed forward fix (ADR-008). Switching the image back to the schema 1 digest is **not** a rollback. |
| Service unhealthy after 13.1/15.1 | `aws ecs update-service --desired-count 0` (stop-then-start configuration means at most one task runs). The circuit breaker returns to the last completed deployment on its own. |
| Bad image after go-live | Redeploy the **previous reviewed task-definition revision of the new schema** (record it as `PREVIOUS_TD` after each deployment): `aws ecs update-service --cluster $CLUSTER --service $SERVICE --task-definition $PREVIOUS_TD` [MUTATION]. The pre-window revision `GenuiSyntheticFoundationTaskE2459512:2` (schema 1 image `770bcb00…`) is incompatible with the migrated database and is never a rollback target after 8.1. APP-076 exercises the rollback redeploy with protected routes 404 and the restore. |

## Campaign cap and task-hours

The qualification manifest spans at most 600 s from composition; the supervisor refuses to start with under 60 s left and kills the child at expiry. Locally the entry ran in about 2 s (APP-075 local positive). APP-007's 78 MB standalone tasks took 40–50 s from creation to stop; the 498 MB qualification image adds the pull, so expect 60–150 s of the 600 s before the child starts. The cap is therefore adequate if 12.1 composes and runs back to back; a second attempt needs a new composition (new run id). The window runs about 20 standalone tasks of 1 vCPU/2 GiB (priced at the 2 vCPU/8 GiB rate) of about 1–2 minutes each plus two service starts: under one hour, within `extra_task_hours: 3`.
