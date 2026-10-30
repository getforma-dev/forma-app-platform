# APP-078 preparation (Oct 30, MVP plan task 10)

**Preparation, not acceptance.** No AWS mutation was made: no create, update, delete, put, run-task, start, stop or change-set execution, no ECR push, no Secrets Manager write, no DNS change. Read-only AWS calls used the default credentials (IAM user in account `877573132739`). No acceptance box of APP-078 is checked, and neither `Tickets/` nor `docs/` was edited.

| File | What it is |
| --- | --- |
| [snapshot.json](snapshot.json) | Redacted read-only observation, 2026-10-30T02:24:33Z, with the request id of each of its 25 calls. Raw responses (with the database endpoint) are private under `.cache/operations/beta/app-078-prep/snapshot/`. |
| [RUNBOOK.md](RUNBOOK.md) | The ordered Nov 1 checklist: commands, expected outputs, stop conditions, rollback. |
| [APPROVALS.md](APPROVALS.md) | A1–A10, what Victor approves at window time. |
| [ledger-basis.md](ledger-basis.md) | The always-on ledger basis written to a new private file, the diff against the untouched ledger, and the dry run. |

## Read-only snapshot summary (2026-10-30T02:24Z)

- **Stack** `GenuiSyntheticFoundation` `UPDATE_COMPLETE` (last update 2026-10-29T00:43:53Z), 65 resources, no change sets, stack policy equal to `infra/stack-policy.json`. It still runs the APP-007 template: no `QualificationConfigSecretArn`, `QualificationImageDigest` or `OperatorImageDigest` parameter, `MonthlyBudgetUsd 100`, `ImageArchitecture ARM64`, `IngressMode public`, service image `genui-synthetic@sha256:770bcb00…eb22` (APP-007 `8f8c34b`, schema 1).
- **Service** `ACTIVE`, desired/running/pending **0/0/0**, task definition `GenuiSyntheticFoundationTaskE2459512:2`, deployment configuration minimum 100 % / maximum 200 % (the deployed value; see change 3 below). No RUNNING task and no recently stopped task in the cluster. The target group has no registered target.
- **Stop schedule** `ENABLED`, `at(2026-10-29T04:39:50)` UTC (in the past; already delivered), target `DesiredCount 0` for this exact service, 900 s / 3 retries.
- **RDS** `genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg`, resource id `db-KW4LRMTPI5YXHZXF4LELQZKTAU` (equal to the qualification pins), `available`, PostgreSQL 16.13, `db.t4g.small`, 20 GiB gp3, encrypted, not public, single AZ, IAM auth, deletion protection on, automatic minor upgrade off, 7-day backups, two automated snapshots (2026-10-28T23:00Z, 2026-10-29T05:21Z), no manual snapshot. The private endpoint equals the host pinned in `qualification_supervisor.py`.
- **ECR** `genui-synthetic` (immutable tags, scan on push): four image indexes (`app007-8139067-arm64`, `app007-8f8c34b-arm64`, `app006-18c33f5-arm64`, `app006-key-8fca35c-arm64`) with their manifests. `fauth-beta` (FormaStack) holds one image. No `genui-qualification` or `genui-operator` repository exists.
- **Edge** ALB `GenuiS-LoadB-DaSWApcUZWXa` active, HTTPS 443 only, `ELBSecurityPolicy-TLS13-1-2-2021-06`, certificate `832cd214-…` for `beta.getforma.dev` `ISSUED` until 2027-05-13. Public DNS (Porkbun, read by `dig`): `beta.getforma.dev` CNAME → `genuis-loadb-daswapcuzwxa-161166425.us-east-1.elb.amazonaws.com`, equal to the stack's `LoadBalancerDnsName`. No DNS change is needed while the load balancer is not replaced.
- **Cost** No AWS Budget exists. Cost Explorer October month-to-date (estimated, whole account incl. FormaStack): $26.37, largest RDS $14.09, VPC $3.43, Secrets Manager $2.88, EC2 $2.80, App Runner $2.34. No `genui-qualification-` secret exists.
- Additional read-only calls outside the snapshot tool: `cloudformation describe-stacks --stack-name CDKToolkit` (bootstrap version 30; assets bucket `cdk-hnb659fds-assets-877573132739-us-east-1`), `s3api get-bucket-versioning` on it (Enabled), `sts get-caller-identity`, and `dig`.

## Changes made to make the runbook executable

1. **`infra/scripts/preflight.py`** (`96dd5f1`): a closed `always-on-beta` plan (schema 2 ledger, GenUI cap $170, fAuth $33–38, project $208, backstop up to 32 days, `MonthlyBudgetUsd 170`); `qualification_image_digest` and `operator_image_digest` inputs (empty or same-account digests, never the service image, distinct from each other, bound to the window, emitted as `QualificationImageDigest`/`OperatorImageDigest` and compared on activation; an older stack without them is accepted only as disabled). The bounded plan and every earlier window hash are unchanged. 8 new tests (30 in all).
2. **`infra/src/stack.ts`** (`96dd5f1`): `MonthlyBudgetUsd` allows `100` and `170`.
3. **`infra/src/stack.ts`** (`96dd5f1`): service deployments are stop-then-start (`minHealthyPercent 0`, `maxHealthyPercent 100`), as ADR-007 D7.18 requires; the deployed 100/200 would run two tasks with separate in-process limits and session keys during a deployment or a rollback redeploy. This is a conformance fix the orchestrator may split out.
4. **Operator managed channel** (`ff45610`, Tier A surface): `admission issue --code-digest` (code minted locally, only its digest in the task, override, CloudTrail and log; reused digest refuses on the `code_index` key; no provider invitation) and `register-source-revision --record-json` (the non-secret record inline). `operator.py` admits exactly these forms in a managed task; raw `issue`, `--record` and `reconcile` still refuse. Both documented in `overrides.json`.
5. **`verify-catalog --version N`** (`ef9c704`): the read-only baseline proof (version 1) that the ticket requires before the snapshot and the first migration. The database test now reproduces the deployed version 1 state and migrates from it.
6. **`infra/scripts/window_requests.py`** (`bf47174`): composes the RunTask, CreateChangeSet and qualification requests and mints codes locally; reviews a described change set. **`infra/scripts/readonly_snapshot.py`**: the allowlisted read-only observation used above (and in runbook steps 1.2/14.3).

## Checks run (all exit 0)

| Command | Result |
| --- | --- |
| `cd infra && npm run typecheck` | clean |
| `cd infra && npm test` | 26 of 26 (template tests, preflight 30, runtime negatives, window requests 8, snapshot guard 2) |
| `python3 -m unittest discover -s test -p preflight_test.py` (in `infra/`) | 30 passed |
| `python3 -m unittest discover -s test -p window_requests_test.py` / `readonly_snapshot_test.py` | 8 / 2 passed |
| `$OPVENV/bin/python -m unittest discover -s app/operator -p 'test_*.py'` (fresh venv from `app/operator/requirements.lock`, `--require-hashes --only-binary=:all:`; `GENUI_OPERATOR_TEST_PARSER` = the main checkout's `parser/target`; Docker PostgreSQL 16.13) | 38 passed |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime` | 132 passed, 1 skipped (contract encoder without the contracts venv on the path; pre-existing) |
| `npx cdk synth --no-lookups --quiet` | template 106,155 bytes (above the 51,200-byte inline limit, hence TemplateURL); only warning W2509 on `QualificationConfigSecretArn` (an ARN, not a secret) |

Not run: the Rust suites and Docker image campaigns (`check_image.py`, `local_harness.py`, `qualify_image.py`). The changed Python files are packaged into the operator image, so its `check_image.py` must be rerun on the window's build (runbook 2.4); the qualification and service image contexts do not include them.

## Gaps against the acceptance rows

- **Grant/RLS matrix beyond the six qualification checks.** The qualification entry's six checks cover fixed-role readiness, TLS role identity with scoped two-organization reads, RLS and direct-write refusal under all four roles, revocation/epoch/missing-grant and registry/DB mismatches, the action matrix and same-backend reuse. Ticket item 3's wrong user/token, expired material, wrong hostname/CA and plaintext refusals, and the "schema 1 image/source pairing is incompatible" row, have no managed case for schema N yet (APP-007's `managed_sql_trial.py`/`managed_runtime_negatives.py` assert schema 1 and cannot be relabeled). The Tier A reviewer maps each row to a check or records it open; a partial run keeps APP-078 open.
- **S3 policy denials** (unconditional PUT denied, wrong-key PUT denied) have no operator override; only the positive conditional create and HEAD are observed (11.1–11.2).
- **x86_64 images** are not built: the window uses ARM64 for all three (one `ImageArchitecture` parameter governs every task definition).

## Blockers

- **B1 — hosted sign-in wiring (blocked APP-076 and any invite redemption, not APP-078's rows): built 2026-10-30, needs the window steps and a review.** The service reads `GENUI_AUTH_CONFIG_FILE` (provider routing) and `GENUI_IDENTITY_REGISTRY_FILE` (the closed `ten_` → organization registry); without them it starts deny-all. The build: (a) one Secrets Manager secret per environment, `AuthConfiguration` (`infra/src/stack.ts`; own retained rotating key; initial `{}` = not configured; output `AuthConfigurationSecretArn`), injected into the service container only as `GENUI_AUTH_CONFIGURATION_JSON`, which `runtime.py serve()` validates whole and materializes as the two 0600 files in a fresh 0700 directory under `/run/genui`, removing the variable from the child; preflight requires exactly that injection beside the capability secret; (b) `admission_operator.py bind-provider-tenant --organization <uuid> --issuer <token iss> --provider-tenant <ten_…>` writes the `provider_tenant_bindings` row through the operator channel (idempotent; identifiers only, so it is a documented managed override, `admission-bind-provider-tenant`; **no audit event**: ADR-007 D7.8's operator kinds are closed and name no binding kind, as for the seed's own bindings, so a `provider_tenant.bound` kind is an ADR-007 amendment if wanted); (c) `infra/scripts/window_requests.py auth-configuration` composes the secret value from FormaStack's private identifiers file (field names `issuer_origin`, `jwt_iss`, `jwt_aud`, `access_jwks`, `callback`, `tenants[].slug/tenant_id/client_id/genui_org_uuid`; nothing else is read) into a private 0600 file and prints only organization ids, slugs and the digest; (d) the session and invite campaigns now take their bindings from `bind-provider-tenant` and their routing and registry from a composed secret value put into the loopback Secrets Manager stand-in and materialized by the supervisor's own writer. Runbook steps 0.5, 5.9, 10.4 and 15.1 carry the window steps; approval A11. Note that the service compares the token's `iss` claim (the private file's `jwt_iss`) with the registry and binding issuer, not the origin URL; the composer writes that value into both. **What redemption still needs beyond B1:** a redeemable code carries a provider invitation (ADR-006 invitation-first; `POST /invite/validate` refuses a code with no capability), and `issue --provider-email` runs only where the fAuth management key is, never in a task, while the RDS database is reachable only from tasks. The managed `issue --code-digest` form therefore issues codes that no one can redeem. Either a reviewed network path for the operator host to RDS, or a managed form that receives the management key as an ECS secret (with the invitee address kept out of the override), is a separate decision before any invite is redeemed (see RUNBOOK 15.4).
- **B2 — candidate source.** PR 4 (APP-074, 012, 013, 014) has not landed; 0007 moves the catalog to v7 and every qualification pin. Images, the composer (it reads the pins from the checkout) and the synthesized template must all come from the same final commit.
- **B3 — review.** `ff45610` (managed issuance channel, approval A10) and the preflight caps need the APP-078 Tier A review before the window.
- **B4 — APP-075 m10.** Whether CloudFormation accepts the disabled-task intrinsics is shown only by change-set creation (runbook 5.4); a failure stops the window and goes to APP-086.
- **B5 — approvals A1–A10** are pending.

## Commits (worktree branch, not pushed)

`96dd5f1` preflight always-on plan, task-image digests, stop-then-start · `ff45610` operator managed channel · `bf47174` composer and snapshot tool · `ef9c704` baseline catalog proof · plus the commit adding this directory.
