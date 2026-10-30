# Pitch deploy runbook (APP-096)

This runbook takes beta.getforma.dev from the Phase 1 build (`aa220b3`, TaskDefinition:4) to the pitch build on `pivot/pitch-mvp`. It is the APP-078 window runbook (`forma-app-platform/evidence/APP-078/prep/RUNBOOK.md`) cut down to a redeploy of the existing stack, and it reuses that runbook's helpers (`$WR`, the operator-task run pattern, `infra/scripts/preflight.py`).

Every step marked **[MUTATION]** needs Victor's word, recorded verbatim in `$PRIV/approvals.md`. **[read]** steps and local steps need none.

Some steps involve secrets: the Anthropic key and the Slack bot token and signing secret. Victor runs those himself, from the clone's `.env`. The commands below read the values from the file and pipe them, so they never appear in chat, in argv or in a log.

## Decisions before the window

| # | Decision | Recommendation |
| --- | --- | --- |
| P1 | Which organization is the demo organization (`GENUI_DEMO_ORGANIZATIONS`)? The stack names Canary (`…0002`). Victor's two redeemed accounts are in Northstar (`…0001`). | **Northstar.** Both orgs are synthetic fixtures, and the personas are Victor's accounts in Northstar. Without this, everything still works through the real model (about 1 s per parse); only the instant rehearsed sentences, the weather fixture and the receipt fixture are off. One-line change: `DEMO_ORGANIZATION_ID`. |
| P2 | Persona names: which account is Diego and which is Sarah? | Laptop account (victorvillacis99) = Diego Ramos, filer. Phone account (victor.villacis@outlook) = Sarah Chen, approver. |
| P3 | Slack in this deploy? It needs the app created from `slack-app-manifest.json` first. | Yes if the app exists by then; otherwise deploy without it. The Slack routes answer 404 until the secret is filled, and a later secret put plus a forced redeploy adds it. |
| P4 | Brief downtime during migration? The new task needs schema 10; the live database is at schema 7 (the Phase 1 build embeds `catalog-v7`). | Yes: about 10–15 minutes at desired count 0, as in the APP-078 window. |

## 0. Local (no AWS)

- [ ] 0.1 `CANDIDATE=$(git -C forma-pitch-mvp rev-parse HEAD)`; the working tree is clean and the commit is pushed.
- [ ] 0.2 Checks at the tip:
  - `cd app && cargo +1.93.1 fmt --check && cargo +1.93.1 clippy --locked --all-targets -- -D warnings && cargo +1.93.1 test --locked`
  - `cd infra && npm ci && npm run typecheck && npm test`
  - `npm --prefix app/admin run typecheck && npm --prefix app/admin run build`
  - `python3 scripts/check-schema-pins.py`
  - `app/runtime/qualify_tenant_schema.py` (passed 365 checks at c2778f2)
  - `python3 Tickets/_tools/board.py check`
- [ ] 0.3 Build the three ARM64 images and qualify them, exactly as APP-078 §2.1–2.6:
  - images: `genui-service`, `genui-qualification`, `genui-operator`, each tagged `pitch-${CANDIDATE:0:7}`;
  - checks: `qualify_image.py`, `qualification/check_image.py`, `operator/check_image.py`;
  - the parser identity must agree between images.
- [ ] 0.4 The private names file `$PRIV/display-names.json` (mode 0600, never committed) holds `{"members":[{"principal_id":"<Diego's>","display_name":"Diego Ramos"},{"principal_id":"<Sarah's>","display_name":"Sarah Chen"}]}`. The principal ids come from the APP-076 trial record.
- [ ] 0.5 If P1 = Northstar, set `DEMO_ORGANIZATION_ID` in `infra/src/stack.ts` to `…0001`, update its test and commit. That makes a new candidate; rerun 0.2–0.3.

## 1. Observe [read]

- [ ] 1.1 `aws sts get-caller-identity` returns account `877573132739`.
- [ ] 1.2 `readonly_snapshot.py`: stack `UPDATE_COMPLETE`, service 1/1 on TaskDefinition:4, no change sets pending.

## 2. Registry [MUTATION]

- [ ] 2.1 Push to the existing repositories with the tag `pitch-${CANDIDATE:0:7}-arm64`:
  - `genui-synthetic` (service)
  - `genui-qualification`
  - `genui-operator`
- [ ] 2.2 Record each digest.
- [ ] 2.3 Basic scanning skips OCI indexes, so scan findings come from the **arm64 child** digest. A CRITICAL finding is Victor's call.

## 3. Preflight and change set

- [ ] 3.1 Start from `$PRIV/deployment-input-2.json`. Set:
  - the three digests;
  - `service_desired_count: 0`;
  - `supersedes_window_id: 88d1b19d-db20-4593-993c-de275d4e7318`.

  Then run `infra/scripts/preflight.py --input … --ledger $PRIV/budget-ledger.json` to get the receipt. The caps stay $170 GenUI / $208 project, so check the ledger's month-to-date first.
- [ ] 3.2 `cd infra && npm run synth -- --quiet -c displayNamesFile=$PRIV/display-names.json`, then record the template sha256.
- [ ] 3.3 `$WR change-set`, upload the template **[MUTATION]**, then create the change set **[MUTATION]** and review it **[read]**. The change set should contain:
  - new resources: `SlackApp` secret, its KMS key, and output `SlackAppSecretArn`;
  - service task definition environment: `GENUI_DEMO_MODE=1`, `GENUI_DISPLAY_NAMES_JSON` and the Slack secret references;
  - operator role: Get/Put on `SlackApp`;
  - the three image digests.

  Anything else (a replacement, any database change, any DNS change) stops the window.
- [ ] 3.4 Execute **[MUTATION]**, then wait for `UPDATE_COMPLETE` and save `stack-after.json`.

## 4. Secrets: Victor runs these [MUTATION]

```bash
cd /Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-mvp
COMPOSER=$(jq -r '.Stacks[0].Outputs[]|select(.OutputKey=="ComposerModelSecretArn").OutputValue' "$PRIV/stack-after.json")
grep '^ANTHROPIC_API_KEY=' .env | cut -d= -f2- | tr -d '\n' | aws secretsmanager put-secret-value --secret-id "$COMPOSER" --secret-string file:///dev/stdin --query VersionId
```

If P3 is yes, also run:

```bash
SLACK=$(jq -r '.Stacks[0].Outputs[]|select(.OutputKey=="SlackAppSecretArn").OutputValue' "$PRIV/stack-after.json")
python3 -c 'import json;e=dict(l.rstrip("\n").split("=",1) for l in open(".env") if "=" in l and not l.startswith("#"));print(json.dumps({"bot_token":e["SLACK_BOT_TOKEN"],"signing_secret":e["SLACK_SIGNING_SECRET"]}),end="")' | aws secretsmanager put-secret-value --secret-id "$SLACK" --secret-string file:///dev/stdin --query VersionId
```

## 5. Database [MUTATION]

- [ ] 5.1 Take an RDS snapshot: `genui-pitch-pre-migration-schema7-<date>`.
- [ ] 5.2 Operator task `migrate` twice: `database_version 10` both times, and the second output is identical. Then `verify-catalog`: `catalog_version 10`, read-only.
- [ ] 5.3 Run `seed-library --organization <demo org>` twice; the repeat should report `changed: false`. Northstar already holds Spend Review; the library adds its 14 apps.
- [ ] 5.4 Library source bytes:
  1. `python3 app/runtime/library_sources.py build --organization <org> --cache-root $PRIV/library` builds the files locally.
  2. `python3 app/runtime/library_sources.py plan --organization <org>` lists the object keys.
  3. Put each object with `If-None-Match: *` and the bucket's required SSE-KMS headers. Read the bucket policy first **[read]**: the bucket refuses an upload without the expected encryption key. If the operator machine's role cannot write the prefix, run the upload through the operator task, as APP-078 §11 did.
- [ ] 5.5 `grant-library-access --organization <org> --principal <id> --record-write --authoring-app <new uuid>` for Diego and for Sarah, twice each to show it is idempotent.

## 6. Activate and check

- [ ] 6.1 In the deployment input, set `service_desired_count: 1` and run `preflight.py --activate` **[MUTATION]**. Wait for the service to be stable, with one healthy target.
- [ ] 6.2 HTTPS surface from the operator machine, checked with plain `curl` (no `-k`): `/health/live` 200; `/canvas` signed out gives the sign-in page; `/v1/mcp` without a bearer gives the uniform 404; `/v1/slack/commands` unsigned gives 401 (Slack on) or 404 (Slack off).
- [ ] 6.3 Laptop as Diego and phone as Sarah, scenarios 1–4 of `SCENARIOS.md`:
  - type an expense, then confirm;
  - Sarah's canvas shows the claim live; she approves; Diego's canvas flips;
  - "Review expenses over $200" reshapes the expense app;
  - `/weather`, `/calculator`, TI-84 Matrix;
  - a receipt photo becomes a draft.
- [ ] 6.4 Claude Code: in the canvas, open "Connect an agent", then `claude mcp add --transport http forma https://beta.getforma.dev/v1/mcp --header "Authorization: Bearer <token>"` (Victor pastes the token himself). Run scenario 6. For OpenAI: put `OPENAI_API_KEY` in `.env`, then run `app/scripts/mcp_openai_smoke.py`.
- [ ] 6.5 Slack, if on: `/forma link` for each persona, then scenario 5.

## Rollback

The Phase 1 build checks the database against its embedded `catalog-v7` at startup, so it does **not** run on schema 10.
- **Before 5.2 (migrations):** set the service back to TaskDefinition:4 (`aws ecs update-service --task-definition $CUR_TD`) **[MUTATION]**.
- **After 5.2:** either fix forward on the pitch build, or restore the 5.1 snapshot to a new instance, repoint, and then roll back to TaskDefinition:4. That is a long operation; fixing forward is the default.
