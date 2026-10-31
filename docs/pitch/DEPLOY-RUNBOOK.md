# Pitch deploy runbook (APP-096, APP-105, APP-113)

This runbook redeploys the existing `GenuiSyntheticFoundation` stack on beta.getforma.dev with a new pitch build from `pivot/pitch-mvp`. It is the APP-078 window runbook (`evidence/APP-078/prep/RUNBOOK.md`) cut down to a redeploy, and it reuses that runbook's names and helpers (`$PRIV`, `$WR`, `$STACK`, `$CLUSTER`, `$SERVICE`, `$ASSETS`, the operator-task run pattern, `infra/scripts/preflight.py`).

It follows the sequence the five redeploys of 2026-10-30 actually ran (APP-096 and APP-105 logs):

1. preflight with `service_desired_count: 0`;
2. change set;
3. execute (the service goes to 0 here);
4. snapshot;
5. migrate;
6. seed and grants;
7. `preflight.py --activate`.

Every step marked **[MUTATION]** needs Victor's word, recorded verbatim in `$PRIV/approvals.md`. **[read]** steps and local steps need none.

**Recipes (ADR-012 D12.6).** Each checklist item names the `just` recipe that performs it; the scripts are in `ops/beta/` and `just deploy help` lists the steps. Every [MUTATION] step prints its exact commands (stack identifiers by variable name) and refuses unless it is given `--approved "<Victor's verbatim words>"`, which it appends to `$PRIV/approvals.md` before any AWS call; `--dry-run` prints and records nothing. The runbook stays the authority: a recipe performs an item, it does not replace reading it.

**The private directory.** `$PRIV` is `<checkout>/.cache/ops/beta/` (gitignored; directories 0700, files 0600), no longer the forma-app-platform clone's `.cache/operations/beta/app-078-window`:

| Path | Holds |
| --- | --- |
| `env.sh` | the stack's identifiers, named in `ops/beta/env.example.sh`; `REPO`, `PRIV` and the operator venv are derived from the checkout |
| `approvals.md`, `budget-ledger.json` | Victor's words per action; the preflight ledger |
| `display-names.json`, `signin-aliases.json` | the private names files (0.5) |
| `windows/<window-id>/` | one deploy: `release.json` (candidate, tag, supersedes, digests, template sha256, times), `base-input.json`, `deployment-input.json`, `receipt.json`, `stack-before.json`, `template-before.json`, `template.json`, `cdk-diff.txt`, `change-set-*.json`, `stack-after.json`, `run-<label>-<n>.*`, `activation.json` |
| `windows/_pre-adr-012/` | the APP-078 window's flat directory as of 2026-10-31, copied once as history (`ops/beta/import-legacy-priv.sh`) |

The operator venv is `app/operator/.venv` (`just setup`), not `opvenv`.

Some steps involve secrets: the Anthropic key, the Slack bot token and signing secret, and a member's agent token. Victor runs those steps himself. Values are read from the clone's `.env` by `app/scripts/read_env.py` and piped, so they never appear in chat, in argv, in shell history or in a log.

## State at the last deploy (2026-10-30)

| Item | Value |
| --- | --- |
| Live build | `422d332` (folders), TaskDefinition:8, 1/1 healthy |
| Live window | `34e746fa-…`, the head of the chain `88d1b19d` → `10e6ecc5` → `3b63b1f9` → `5742ecc3` → `34e746fa` |
| Database | schema 11 (`database_version 11`, `verify-catalog 11`) |
| Library | 20 apps in Northstar, grants for both personas |
| Demo organization (P1) | Northstar, `…0001`; `DEMO_ORGANIZATION_ID` in `infra/src/stack.ts` is `TENANT_IDS[0]` |
| Personas (P2) | Laptop account (victorvillacis99) is Diego Ramos, the filer; phone account (victor.villacis@outlook) is Sarah Chen, the approver |
| Slack (P3) | Secret placeholder only, so every Slack route answers 404; the app is not created yet |
| Snapshots | `genui-pitch-pre-migration-schema7-20261030`, `genui-pitch-pre-migration-schema10-20261030` |

Every window recorded so far requested activation, so none of them can be abandoned and the ledger needs no repair (see 3.1).

## Downtime

The service template pins `ServiceDesiredCount` to `0` (the only allowed value). Every change set that modifies the `Service` resource therefore sets the desired count to 0 when it executes. A new image always does this, because the task definition is replaced. ECS then stops the running task straight away: the service is configured stop-then-start (minimum 0 %, maximum 100 %), so one task at most ever runs. This is what happened on 2026-10-30: after each execute the service sat on the new revision at count 0.

So no separate scale-to-zero is needed before the change set. Keep the site up while the change set is created and reviewed (steps 3.1–3.5). The outage starts at **3.6 (execute)** and lasts until **6.1** reports a healthy target. That covers:

- the stack update;
- the snapshot and migrations, when the schema changes;
- the library steps;
- the task start: up to 60 s of container start period, a 90 s grace period, and two ALB checks 30 s apart.

Plan for about 10–15 minutes with a migration (decision P4), and less without one. Every signed-in member is signed out by the restart.

If the change set does **not** modify `Service`, the old task keeps running at count 1. This happens when only the operator or qualification image changed, or only the expiry. In that case, scale it to zero explicitly **[MUTATION]** before 3.6. The stop schedule must never be replaced while compute runs (`infra/README.md`, "Updating an existing stack"):

```bash
aws ecs update-service --cluster $CLUSTER --service $SERVICE --desired-count 0
aws ecs wait services-stable --cluster $CLUSTER --services $SERVICE
```

## 0. Local (no AWS)

- [ ] 0.1 `CANDIDATE=$(git -C forma-pitch-mvp rev-parse HEAD)`. The working tree is clean and the commit is pushed. (`just images` and `just deploy new` refuse another commit; `new` warns when it is not pushed.)
- [ ] 0.2 Checks at the tip: **`just check`** runs all of these (and the MSRV floor, the contracts and `bash -n`); `qualify_tenant_schema.py` is in `just ci`. The toolchain comes from `rust-toolchain.toml`, so no `+1.93.1`:
  - `cargo fmt --check -p forma-app-platform && cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings && cargo test --locked -p forma-app-platform` (from the repository root: the root Cargo workspace, ADR-013 D13.3)
  - `cd infra && npm ci && npm run typecheck && npm test`
  - `cd infra/test && python3 -m unittest preflight_test window_requests_test`
  - `python3 -m unittest discover -s app/scripts -p 'test_*.py'`
  - `npm --prefix app/admin run typecheck && npm --prefix app/admin run build`
  - `python3 scripts/check-schema-pins.py`
  - `app/runtime/qualify_tenant_schema.py` (369 checks at schema 11, `cefa484`)
  - `python3 Tickets/_tools/board.py check`
- [ ] 0.3 Build and qualify the three ARM64 images exactly as APP-078 §2.1–2.6 → **`just images <candidate>`** (reports in `.cache/images/<sha7>/`):
  - images `genui-service`, `genui-qualification` and `genui-operator`, each tagged `pitch-${CANDIDATE:0:7}`;
  - checks `qualify_image.py`, `qualification/check_image.py` and `operator/check_image.py`;
  - the parser identity must agree between the images.
- [ ] 0.4 Make a new private window directory and keep every file of this deploy in it: `W=$PRIV/windows/<new window id>; mkdir -m 700 -p $W`. It is the rollback record. → **`just deploy new --candidate <sha> --supersedes <live window id> [--input-from <its deployment input>]`** draws the window id (3.1), makes the directory and `release.json`, and copies the superseded window's input as `base-input.json` (for the first window after ADR-012, the live window's input is under `windows/_pre-adr-012/`).
- [ ] 0.5 The private names file `$PRIV/display-names.json` (mode 0600, never committed) holds `{"members":[{"principal_id":"<Diego's>","display_name":"Diego Ramos"},{"principal_id":"<Sarah's>","display_name":"Sarah Chen"}]}`. The principal ids come from the APP-076 trial record. The synth refuses the file in these cases:
  - a duplicate `principal_id`;
  - a blank name, or a name over 80 characters;
  - a name with a control, format (bidi, zero-width) or line-separator character.
- [ ] 0.6 Schema: compare the candidate's `app/migrations/manifest.json` `database_version` with the live one (11 at the last deploy). If they are equal, skip 5.1–5.2.

## 1. Observe [read]

Steps 1.1–1.3: **`just deploy observe <W>`** [read].

- [ ] 1.1 `aws sts get-caller-identity` returns account `877573132739`.
- [ ] 1.2 `readonly_snapshot.py` shows the stack `UPDATE_COMPLETE`, the service 1/1 on the live revision (TaskDefinition:8 at the last deploy) and no change sets pending.
- [ ] 1.3 Save the rollback record of the live state:
  - `aws cloudformation describe-stacks --stack-name $STACK > $W/stack-before.json`
  - `aws cloudformation get-template --stack-name $STACK --template-stage Original --query TemplateBody > $W/template-before.json`

## 2. Registry [MUTATION]

Steps 2.1–2.2: **`just deploy push <W> --approved "…"`** (digests into `release.json`).

- [ ] 2.1 Push to the existing repositories with the tag `pitch-${CANDIDATE:0:7}-arm64`: `genui-synthetic` (the service), `genui-qualification` and `genui-operator`.
- [ ] 2.2 Record each digest.
- [ ] 2.3 Basic scanning skips OCI indexes, so read the scan findings from the **arm64 child** digest. A CRITICAL finding is Victor's call.

## 3. Preflight and change set

- [ ] 3.1 Copy the live window's input to `$W/deployment-input.json` and change these fields:

  | Field | New value |
  | --- | --- |
  | `window_id` | a fresh UUID (`python3 -c 'import uuid;print(uuid.uuid4())'`). It is the directory name from 0.4. Keep it across retries of this deploy. |
  | `supersedes_window_id` | the live window: the head of the chain, `34e746fa-…` at the last deploy. Never the earlier ones. |
  | `image_digest`, `qualification_image_digest`, `operator_image_digest` | the three digests from 2.2 |
  | `service_desired_count` | `0` |
  | `deployment_expires_at_utc` | the always-on backstop: whole-second UTC with `Z`, between 10 minutes and 32 days from now. Keep the live window's value while it is still more than a day away; otherwise renew it. |

  All other fields keep the live window's values. Before running the preflight, check the ledger's month-to-date. The caps stay $170 GenUI and $208 project.

  Then run `python3 infra/scripts/preflight.py --input $W/deployment-input.json --ledger $PRIV/budget-ledger.json > $W/receipt.json`. → **`just deploy preflight <W>`** [local] writes `deployment-input.json` from `base-input.json` with the window id, supersedes, the three digests and count 0, and prints the expiry it kept, then runs this; **`just deploy abandon <W>`** is the `--abandon` below.

  The receipt should show:
  - `superseded_window_id` = the live window;
  - `ServiceDesiredCount "0"`;
  - `DeploymentExpiresAtUtc` equal to the input.

  The supersession is recorded in the ledger now, at reservation time. If this deploy is given up before 6.1 (activation), run `preflight.py --ledger $PRIV/budget-ledger.json --abandon <new window id>` (local only). It undoes the supersession:
  - the live window counts its whole reservation again and its input works again;
  - the abandoned window keeps only the compute it already used.

  A window that has requested activation cannot be abandoned: `--activate` records the time in the ledger before it calls AWS.
- [ ] 3.2 → **`just deploy synth <W>`** [local]. `cd infra && npm run synth -- --quiet -c displayNamesFile=$PRIV/display-names.json -c signinAliasesFile=$PRIV/signin-aliases.json`. Then:
  - copy `cdk.out/GenuiSyntheticFoundation.template.json` to `$W/template.json`;
  - record its sha256 in `$W/template.sha256`.
- [ ] 3.3 → **`just deploy diff <W>`**. Diff against the live template [local]: `cd infra && npx cdk diff GenuiSyntheticFoundation --no-lookups -c displayNamesFile=$PRIV/display-names.json -c signinAliasesFile=$PRIV/signin-aliases.json --template $W/template-before.json > $W/cdk-diff.txt`.

  `--template` compares with the file saved at 1.3 (it implies `--method=template`), so the diff makes no AWS call and creates no change set. It shows the content of every change, including task definition environment, policies and the ALB attributes. CloudFormation's change set only lists resources, and it cannot see parameter values.
- [ ] 3.4 Create the change set: 1–3 → **`just deploy change-set <W> --approved "…"`**; 4 → **`just deploy review <W>`** [read].
  1. Compose the request: `$WR change-set --receipt $W/receipt.json --template $W/template.json --window-id <id> --output $W/change-set-request.json`.
  2. Upload the template **[MUTATION]**.
  3. Create the change set **[MUTATION]**.
  4. Describe it and run `$WR review-change-set --describe $W/change-set-describe.json --receipt $W/receipt.json` **[read]**. The review refuses any Remove or replacement of a protected type and any parameter that differs from the receipt.
- [ ] 3.5 Check the change set by hand against `cdk-diff.txt`. **Every redeploy:**

  | Change | Resources |
  | --- | --- |
  | Replace (a new revision; task definitions are immutable) | `Task…`, `RollbackTask…`, `MigrationTask…` (service digest); `OperatorTask…` (if the operator digest changed); `QualificationTask…` (if the qualification digest changed) |
  | Modify, `Replacement: False` | `Service…` (new task definition; `DesiredCount` from the parameter, 0) |
  | Modify, only if the expiry changed | `DeploymentExpiry` (new `at(...)`) |
  | May appear as Modify | `ExecutionRoleDefaultPolicy…`, `MigrationExecutionRoleDefaultPolicy…`, `QualificationExecutionRoleDefaultPolicy…`, `OperatorImagePull`. They name the repository derived from a digest parameter. On 2026-10-30, same-repository redeploys listed none of them. |

  **Only when the template changed.** `cdk-diff.txt` shows each of these, and each must match a commit you can name:
  - a new secret with its KMS key and output (the first pitch deploy added `ComposerModel` and `SlackApp`, their keys and `ComposerModelSecretArn`/`SlackAppSecretArn`);
  - a load-balancer attribute, Modify without replacement (the first pitch deploy raised the idle timeout to 130 s);
  - policy changes. The first deploy of APP-113 modifies `OperatorRoleDefaultPolicy…`, removing the operator role's grants on `ComposerModel` and `SlackApp`;
  - task environment changes, inside the task definition replacement (for example `GENUI_DISPLAY_NAMES_JSON` from a changed names file, or the demo variables).

  **Stop the window for any of these:**
  - a Remove;
  - any replacement other than a task definition;
  - any Modify of the database or its subnet or parameter group, the VPC, subnets or security groups, the listener, the target group, the cluster, the log group, the bucket or its policy, or a KMS key;
  - any Modify of an existing secret (`ProviderCapabilities`, `AuthConfiguration`, `ComposerModel`, `SlackApp`, `OperatorInvitationInput`, `MigrationSecret`). A changed secret resource can rewrite its live value with the template's placeholder.
  - a DNS change;
  - anything in the change set that `cdk-diff.txt` does not explain.
- [ ] 3.6 Execute **[MUTATION]** → **`just deploy execute <W> --approved "…"`** (refuses without the 3.4.4 review file). **The outage starts here.**
  1. Wait for `UPDATE_COMPLETE`.
  2. Save `aws cloudformation describe-stacks --stack-name $STACK > $W/stack-after.json`, and copy it to `$PRIV/stack-after.json` for the APP-078 operator-task run pattern.
  3. Observe the stop rather than inferring it from the parameter: `aws ecs describe-services --cluster $CLUSTER --services $SERVICE` [read] shows desired 0, running 0, pending 0 and the new `TaskDefinition` output. Wait with `aws ecs wait services-stable` if a task is still draining. A desired count other than 0 is a stop; see "Downtime" above.

## 4. Secrets: Victor runs these [MUTATION] (no recipe, by design)

Skip this section on a redeploy unless a value changes: `ComposerModel` already holds the key. Run from the clone:

```bash
cd /Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-mvp
COMPOSER=$(jq -r '.Stacks[0].Outputs[]|select(.OutputKey=="ComposerModelSecretArn").OutputValue' "$W/stack-after.json")
python3 -I app/scripts/read_env.py --raw ANTHROPIC_API_KEY | wc -c        # [local] a byte count > 0; nothing else is printed
python3 -I app/scripts/read_env.py --raw ANTHROPIC_API_KEY | aws secretsmanager put-secret-value --secret-id "$COMPOSER" --secret-string file:///dev/stdin --query VersionId
```

`read_env.py` handles the `.env` format: it drops a leading `export`, one pair of surrounding quotes, a trailing `\r` and an inline ` # comment`. It refuses three cases:
- a name assigned twice;
- a missing or empty value;
- a control character.

It also refuses to print to a terminal. Its errors name the variable and the line, never the value.

If P3 is yes (the Slack app exists), also run:

```bash
SLACK=$(jq -r '.Stacks[0].Outputs[]|select(.OutputKey=="SlackAppSecretArn").OutputValue' "$W/stack-after.json")
python3 -I app/scripts/read_env.py --object bot_token=SLACK_BOT_TOKEN signing_secret=SLACK_SIGNING_SECRET | wc -c   # [local] > 0
python3 -I app/scripts/read_env.py --object bot_token=SLACK_BOT_TOKEN signing_secret=SLACK_SIGNING_SECRET | aws secretsmanager put-secret-value --secret-id "$SLACK" --secret-string file:///dev/stdin --query VersionId
```

A new value reaches the service at its next task start. Here that is the activation in 6.1. On a live service it takes `aws ecs update-service --cluster $CLUSTER --service $SERVICE --force-new-deployment` **[MUTATION]**.


> **Beta note (APP-113, 2026-10-30):** do not rerun `seed-library` on the existing beta organization after the expense fixture gained `initial: submitted` — the exact-match seed refuses the whole run ("existing seed differs"). Beta is protected by the storage rule that refuses a create starting at another command's transition target. Re-seeding needs a fresh organization.

## 5. Database [MUTATION]

Skip 5.1–5.2 if 0.6 found no schema change.

- [ ] 5.1 → **`just deploy snapshot <W> --name genui-pitch-pre-migration-schema<N>-<yyyymmdd> --approved "…"`**. Take an RDS snapshot named `genui-pitch-pre-migration-schema<live version>-<date>` and wait for `available`.
- [ ] 5.2 → **`just deploy migrate <W> --approved "…"`** (migrate, then verify-catalog; run it twice; each run keeps `run-<label>-<n>.*`). Run the operator task `migrate` twice. Both runs report `database_version` equal to the candidate's manifest (12 if `0012` lands), and the second output is identical. Then run `verify-catalog`: the same `catalog_version`, read-only.
- [ ] 5.3 → **`just deploy seed <W> --approved "…" -- --set=--organization=<org>`**. Run `seed-library --organization 00000000-0000-4000-8000-000000000001` (Northstar) twice. The repeat reports `changed: false`. The library holds 20 apps at `422d332`; the first run reports the candidate catalog's count.
- [ ] 5.4 → **`just deploy library <W> [--organization <org>] --approved "…"`** (1–3 with `ops/beta/upload-library.py`, reading the bucket and tenants from the live task definition). Library source bytes:
  1. `python3 app/runtime/library_sources.py build --organization <org> --cache-root $PRIV/library` builds the files locally.
  2. `python3 app/runtime/library_sources.py plan --organization <org>` lists the object keys.
  3. Put each **new** object with `If-None-Match: *` and the bucket's SSE-KMS headers under Northstar's key. A 412 on an existing key is expected; compare its checksum. If the operator machine's role cannot write the prefix, run the upload through the operator task, as APP-078 §11 did.
- [ ] 5.5 → **`just deploy grants <W> --approved "…" -- <the operator flags>`**. Run `grant-library-access --organization <org> --principal <id> --record-write --authoring-app <uuid>` for Diego and for Sarah, twice each, to show it is idempotent (`changed: false` on the repeat).

## 6. Activate and check

- [ ] 6.1 → **`just deploy activate <W> --approved "…"`** (writes `deployment-input-activate.json`). In `$W/deployment-input.json`, set `service_desired_count: 1` and run `preflight.py --input $W/deployment-input.json --ledger $PRIV/budget-ledger.json --activate > $W/activation.json` **[MUTATION]**.

  The window now records its activation request, so it can no longer be abandoned. Activation checks that each of these matches the reservation:
  - the stack parameters;
  - the stop schedule;
  - the live revision's image and its exact five injected secrets;
  - the deny-all rollback revision.

  It refuses the Anthropic key, the Slack token or the Slack signing secret as a plain environment variable. Wait until the service is stable with one healthy target. **The outage ends here.**
- [ ] 6.2 → **`just deploy verify`**. HTTPS surface from the operator machine, checked with plain `curl` (no `-k`):
  - `/health/live` gives 200;
  - `/canvas` signed out gives the sign-in page;
  - `/v1/mcp` without a bearer gives the uniform 404;
  - `/v1/slack/commands` unsigned gives 401 (Slack on) or 404 (Slack off).
- [ ] 6.3 With the laptop as Diego and the phone as Sarah, walk scenarios 1–4 of `SCENARIOS.md`:
  - type an expense, then confirm;
  - Sarah's canvas shows the claim live; she approves; Diego's canvas flips;
  - "Review expenses over $200" reshapes the expense app;
  - `/weather`, `/calculator`, TI-84 Matrix;
  - a receipt photo becomes a draft.
- [ ] 6.4 Claude Code. In the canvas, open "Connect an agent" and copy the token. The token never goes on a command line: Claude Code expands `${FORMA_AGENT_TOKEN}` from the environment in a project `.mcp.json`.

  ```bash
  mkdir -m 700 -p "$PRIV/claude-agent" && cd "$PRIV/claude-agent"
  cat > .mcp.json <<'EOF'
  {"mcpServers": {"forma": {"type": "http", "url": "https://beta.getforma.dev/v1/mcp",
    "headers": {"Authorization": "Bearer ${FORMA_AGENT_TOKEN}"}}}}
  EOF
  read -rs FORMA_AGENT_TOKEN && export FORMA_AGENT_TOKEN   # paste the token: not echoed, not in history
  claude                                                    # approve the project server "forma"; /mcp shows it connected
  ```

  Run scenario 6.

  For OpenAI:
  1. Put `OPENAI_API_KEY` in `.env`.
  2. With the same `FORMA_AGENT_TOKEN` exported, run `python3 app/scripts/mcp_openai_smoke.py --server-url https://beta.getforma.dev/v1/mcp`. It needs https, sends `store: false` and follows no redirect.
- [ ] 6.5 Slack, if on: each persona runs `/forma link`, then walk scenario 5.

## Rollback

Fixing forward is the default: a corrected build is a new window that supersedes the current one, through sections 0–6. The options below explain why `aws ecs update-service --task-definition <an older revision>` is **not** a rollback:
- when a revision is replaced, CloudFormation deregisters the old one: it becomes INACTIVE, and ECS will not deploy it;
- even an ACTIVE older revision would leave the service drifted from the stack, and `preflight --activate` refuses that drift.

Each build checks the database against the catalog it embeds (`storage/catalog.rs`, `catalog-v<N>`) and refuses another schema at startup. So once 5.2 has run, no earlier build starts.

| Where it fails | What to do |
| --- | --- |
| Before 3.6 (execute) | Nothing live changed. Delete the change set if wanted (**[MUTATION]**, harmless). Run `preflight.py --ledger $PRIV/budget-ledger.json --abandon <new window id>`: the live window's input is then live in the ledger again. The service never stopped. |
| 3.6 rolls back (`UPDATE_ROLLBACK_COMPLETE`) | CloudFormation restored the previous template and parameters, and the stack policy forbids replacing or deleting the database. The service may sit at count 0. Run `--abandon <new window id>`, then `preflight.py --activate` with the **live window's own input** at count 1. Its parameters match the restored stack. |
| After 3.6, before 5.2 (no migration ran), never activated | Fix forward, or put the stack back on the previous template **[MUTATION]**. To put it back:<br>1. Run `--abandon <new window id>`.<br>2. Re-run `preflight.py --input <the previous window's input>` at count 0. It reuses that window's reservation and prints its receipt.<br>3. Compose a change set with `$WR change-set` from that receipt and the previous template: `$W/template-before.json` from 1.3, or `$PRIV/windows/<previous>/template.json`. Change `ChangeSetName` and `ClientToken` to fresh values first; they only identify the request.<br>4. Review it as in 3.5: task definitions replaced back and the service modified.<br>5. Execute it, then run `preflight.py --activate` with the previous window's input at count 1.<br>This only works while that window's `deployment_expires_at_utc` is still more than 10 minutes away. Otherwise make a new window with the previous digests and template, superseding the live window. |
| After 6.1 (activated), no migration ran | `--abandon` refuses: the window may have run. Make a rollback **window**: a new `window_id`, the previous build's three digests and the previous `template.json`, with `supersedes_window_id` set to the failing window. Then run 3–6, skipping 5.1–5.2. |
| A bad image or an exposed route on the live build | First, stop exposure without a schema question **[MUTATION]** (**`just deploy rollback-deny <W> --approved "…"`**; `--restore` is the restoring call): `aws ecs update-service --cluster $CLUSTER --service $SERVICE --task-definition <RollbackTaskDefinition output>`. That is the deny-all revision on the same image, part of the current stack and so ACTIVE. Every sign-in fails, and every protected route answers 404. Restore with the same call and the `TaskDefinition` output. Then fix forward. |
| After 5.2 (migration committed) | No earlier build starts on the new schema, so fix forward. The 5.1 snapshot protects the data; it is not a quick rollback. Restoring it means:<br>- restoring a new instance (RDS cannot restore in place);<br>- stopping the service;<br>- swapping instance identifiers so the stack's endpoint name points at the restored data.<br>The restored instance has a new `DbiResourceId`, and the stack's IAM `rds-db:connect` grants name the old one, so the application roles cannot sign in until a reviewed stack update re-points them. Treat it as its own reviewed operation (APP-097), only for data loss, with Victor's word for each step. |

Keep `$PRIV/windows/<id>/` for every window. Its `receipt.json`, `template.json`, `stack-before.json` and `template-before.json` are what a stack-level rollback needs.
