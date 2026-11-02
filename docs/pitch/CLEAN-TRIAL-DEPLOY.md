# Clean hosted trial deployment (APP-140)

This is the clean profile of [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md), prepared on November 1. It reuses the existing beta stack and replaces the served sandbox with one fresh organization. It retains the old tenants' data and keys, but their auth routes, demo mode and persona overrides are absent from the new service configuration. Do not run the sandbox seed, library upload or library-grant steps for this profile.

Local source qualification is not a hosted acceptance result. Complete the deployment receipt, real registration/sign-in and owner-to-external-recipient journey before closing APP-140. Keep the owner and recipient mailboxes, codes, provider keys and tokens in the private operator directory only.

## Current status — November 2, 03:29 UTC

The [workspace-entry release](../../evidence/APP-162/aws-workspace-entry-2026-11-02/README.md) activated exact candidate `7c3ab12` at **03:29:22 UTC**, retaining schema **19** and the same trial identity/storage. Application TD **16** and broker TD **4** are healthy at `1/1/0`; all five TLS probes and readiness passed. Window `5b239cd0` supersedes `f4449af8`; both enabled shutdown schedules now target **November 30 at 14:00 UTC**, with a canonical forecast of **$169.84 GenUI / $207.84 combined** under the unchanged caps and all prior history/allowances preserved. The only presentation configuration addition is the reviewed `forma-trial` alias pointing to the existing trial handle. No migration, snapshot, operator task, provider/secret/account change, invitation replay or reset was performed.

Use [Sign in](https://beta.getforma.dev/signin) with workspace `forma-trial`, or its [direct link](https://beta.getforma.dev/signin/forma-trial). Existing opaque links remain supported. Verified-email discovery, authenticated multi-workspace switching and actual invitation-email delivery are APP-163–165; Northstar is not silently re-enabled by this form. The genuine recipient's final hosted acceptance and timed recording remain separate.

## Previous composed-app release — November 2, 00:42 UTC

The [composed-app release](../../evidence/APP-140/composed-app-release-2026-11-02/README.md) activated exact candidate `9ee4a01` at **00:40:47 UTC**, with schema **19**, application TD **15** and broker TD **3** healthy at `1/1/0`. Window `f4449af8` supersedes `a58e69cd`; that window’s enabled shutdown schedules targeted **November 30 at 16:00 UTC**. Its reviewed reservation projected **$169.83 GenUI / $207.83 combined**, preserving prior history and allowances under the same caps. The available pre-migration snapshot, migration and repeated catalog checks, all five public TLS probes and readiness check passed. No tenant, invitation, secret, seed or library setup was replayed.

Fresh genuine owner sign-in passed. The original record and share inventories were unchanged, and the live composed-app journey passed empty Dining tracker creation, persisted layout/theme/action editing, one audited $12.50 meal with reloaded dashboard/list, and phone/canvas dialogs. Guest product-email verification, verified acceptance and the complete recorded journey remain separate and open.

## Previous authentication repair — November 1, 21:57 UTC

The [authentication repair](../../evidence/APP-140/shared-auth-fix-1d88470/README.md) activated exact candidate `1d88470` at **21:53:16 UTC**, with application TD **14** and broker TD **2** healthy at `1/1/0`. Window `a58e69cd` superseded `1608eea9`; its shutdown schedules targeted **November 30 at 18:00 UTC**. Its reviewed reservation projected **$169.82 GenUI / $207.82 combined**, preserving prior history and allowances under the same caps. The same schema-18 manifest required no migration, provisioning, operator task, provider invitation or configuration-secret change during this delivery. The composed-app release above supersedes this window.

The [actual product continuation](../../evidence/APP-139/hosted-guest-entry-2026-11-01/README.md) passed fresh owner sign-in and exact retained data/share checks. SES recipient verification became Success; genuine guest registration then returned to the shared app with product email verification pending. Read-only provider reconciliation at 21:57:42 UTC confirms one unverified guest account and one guest invitation referenced by two broker idempotency aliases. Provider invitation acceptance is not acceptance of the private app share. Human product-email verification, fresh guest sign-in, explicit app acceptance, connected/live/revoke checks and the recording remain open.

The sections below preserve the original clean-world preparation and first execution. **Do not replay tenant creation, migration, provisioning or owner-invitation steps against the existing trial.** Any further deployment requires fresh observations and its own qualified, reviewed window.

## Initial execution record — November 1, 20:26 UTC

Window `1608eea9` completed clean activation at **20:13:41 UTC**: tested candidate `711d87d`, schema **18**, application TD **13**, broker TD **1**, both services healthy. That window's reservation was superseded and originally targeted **November 30 at 20:00 UTC**, with projected totals **$169.81 GenUI / $207.81 combined** under the unchanged caps. These are that original execution window's reservation and expiry; the later repair status above supersedes them. [Actual activation evidence](../../evidence/APP-140/hosted-activation-2026-11-01/README.md) retains the verified baseline catalog 13, snapshot, migration to 18, healthy tasks, images and corrected public verifier.

The [genuine owner journey](../../evidence/APP-139/hosted-owner-2026-11-01/README.md) subsequently passed admission, provider verification/sign-in, empty-workspace app creation, a committed record, a pending private share and contextual Ask. At that snapshot, new-recipient signup/acceptance and the final recording remained open. The following baseline, provider setup and deployment sequence document the preparation and completed execution; **do not replay them against this completed window**. A later deployment needs its own current observations, qualified candidate and reviewed window. The original timed observations and amounts remain below for audit.

## Observed baseline and cost

The read-only AWS snapshot at `2026-11-01T18:08:53Z` confirmed account `877573132739`, region `us-east-1`, `GenuiSyntheticFoundation` in `UPDATE_COMPLETE`, no pending change set and one healthy service at TD:12 (one desired/running, zero pending). Its ARM64 service digest was `sha256:b093b942d8840e2c1b580708b11ed95e8ceb4c4306f1710d3737d530d6b1e4c3`. The expiry was `2026-11-30T23:00:00Z`. This observation did not inspect the database catalog; schema 13 remains historical inference until the managed catalog check proves it.

The active private state is in the `forma-pitch-mvp-devflow` checkout's `.cache/ops/beta`, not the absent directory under the Git common checkout. Select that existing directory with `GENUI_OPS_PRIV`; never initialize another live ledger. The observed active window was `1540d71f-1707-4b44-a427-72d855553c55`, with its input under `windows/_pre-adr-012/deployment-input-alias.json`. Re-observe before execution.

The existing ledger was reconciled at `2026-11-01T18:38:19Z` under its persistent lock. Only November's basis and reconciliation timestamp changed; October, all reservations and every monetary amount were preserved. The basis uses the live resource observation, the October 31 all-account estimated charge of `$5.1327227476`, and the November 1 Cost Explorer response (estimated zero with no service groups, explicitly treated as billing lag). It preserves the `$15` variable reserve, `$38` fAuth reserve and three additional task-hours. The variable reserve already includes snapshot storage, ECR, secrets, LCU, logs, S3/KMS and Cost Explorer requests. Actual November charges must be reconciled when available; a zero estimate does not erase reserved costs.

At 18:35Z, replacing the existing window with the clean service and broker through the same expiry projected `$169.80` GenUI and `$207.80` combined, below the unchanged `$170`/`$208` caps by only `$0.20`. This was a calculation, not a new reservation. The new profile adds `$3.60` retained monthly resources (third tenant key, broker key, three broker secrets, fresh provider management secret) and a conservative `$0.0173428/hour` broker rate, including IPv4. The preflight counts those explicitly and releases only the superseded future compute. No additional unreserved task or resource fits implicitly. Rates use the existing conservative x86 convention even for ARM64: [Fargate](https://aws.amazon.com/fargate/pricing/), [VPC IPv4](https://aws.amazon.com/vpc/pricing/), [Secrets Manager](https://aws.amazon.com/secrets-manager/pricing/) and [KMS](https://aws.amazon.com/kms/pricing/).

## Provider and private inputs

Existing provider management reads confirmed active invite-only tenants, but their platform clients allow only `/auth/callback`. fAuth binds one exact redirect URI to each public client. The clean tenant needs two distinct clients; changing an existing member client to the shared callback breaks workspace sign-in.

The existing platform-admin secret can list tenants through `GET /platform/tenants`; the desired slug `forma-trial-20261101` was absent. The upstream platform API supports creating a tenant and managing its clients without a browser. The reviewed private provider packet names:

| Item | Value |
| --- | --- |
| Provider origin | `https://auth-staging.getforma.dev` |
| Fresh tenant slug | `forma-trial-20261101` |
| Tenant policy | email and password, invite-only, maximum 10 users |
| Member client | `forma-trial-20261101-platform` → `https://beta.getforma.dev/auth/callback` |
| Shared client | `forma-trial-20261101-shared` → `https://beta.getforma.dev/auth/shared/callback` |
| New management secret | `fauth-beta/management/forma-trial-20261101`, AWS-managed Secrets Manager encryption |
| Existing platform key | `fauth-beta/platform-admin`, read only into the provisioning process |
| Pre-created users | none; no `POST /manage/users` |

Use the reviewed owning-provider provisioner or the same documented calls: first page through `GET /platform/tenants`; then one `POST /platform/tenants`, persist the returned key immediately, set invite-only email configuration with `PATCH /manage/config`, and register the two exact clients with `POST /manage/oauth-clients`. Read them back. A timeout after tenant creation is an unknown outcome: look up the exact slug and reconcile its secret; do not blindly create again. Do not alter the old tenants. Provider tenant creation itself creates no app-platform membership or app.

The actual provider SMTP configuration uses SES in `us-east-1`. SES sending is enabled but production access is off, so the external recipient must first confirm its SES identity. One specifically authorized verification request was accepted at `2026-11-01T18:27:55Z`. Do not resend while pending. This only permits SES delivery; the recipient must still complete fAuth's own account/email verification. Real owner and guest account actions remain interactive, never supplied through seeded identities.

After provider creation, write these private files under an owned 0700 directory, with files 0600:

1. `clean-trial.json`: exactly `organization_id` (fresh UUID), `tenant_slug` and the returned `provider_tenant_id`. It contains identifiers only. The organization UUID is drawn once and retained across retries.
2. `identifiers.json`: the verified issuer metadata (`issuer_origin`, `jwt_iss`, `jwt_aud`, `access_jwks`), both exact `callback` and `shared_callback`, and one `tenants` entry containing `slug`, `tenant_id`, `client_id`, `shared_client_id`, `genui_org_uuid`. Preserve the actual issuer/audience contract; do not infer it from the hostname.
3. The private recipients file for the real owner, the pre-minted code and later the managed invitation input. The external guest must never appear in this membership invitation file; guest signup uses only the separate broker/account flow.

## Candidate and stack

1. Integrate and review the final source, run `just check` and the affected native campaigns, and check the exact candidate's CI. The candidate includes additive migration 0018; use its manifest, not a hard-coded prior schema. Build all three qualified images from that clean committed checkout with `just images <full-candidate-sha>`. This runs `docker build --platform linux/arm64` for service, qualification and operator, then their three image qualifiers and parser-manifest agreement check. Keep `.cache/images/<sha7>/build.json`; no dirty-build waiver is a deployment qualification.
2. Select the existing `GENUI_OPS_PRIV`. Create the window with `just deploy new --candidate <sha> --supersedes <observed-head> --input-from <existing-live-input> --clean-trial-file <private-file>`. The wrapper copies the identifiers and excludes sandbox persona files. A clean redeploy must pass its same reviewed clean identifiers again.
3. Run `just deploy observe <window>`, then the normal qualified-image push. Update the window's `base-input.json` with the **new tenant's** complete management secret ARN and the verified provider management origin. Keep expiry/budget limits and reserved task allowance unless a reviewed reconciliation changes them. Run `just deploy preflight <window>` at count zero; it records the real reservation and refuses stale or over-budget input.
4. Run `just deploy synth`, `diff`, `change-set`, `review` for that window. Review the additions: third tenant prefix/role/key; isolated broker task, service, roles, secrets and stop schedule; exact `/v1/shared-invitations` listener route; application broker-client secret injection; demo mode off. Replacement/removal of retained database, keys, secrets, old tenant resources or load balancer is refused. The normal app never receives a provider management key.
5. Before executing the reviewed change set, explicitly stop every existing application/broker service and observe desired/running/pending counts all zero. If the initial deployment introduces a service, prove its prior absence. Later redeploys must stop **both** services, even when both appear in the change set: a template desired-count parameter already at zero does not prove that running services will stop. Never replace a stop schedule while its task runs. Then execute at count zero and recheck stopped state before any required migration. The clean wrapper also enforces the migration boundary with a read-only lifecycle check before `migrate`.

The existing script records the user's current scoped authorization with each mutation. Current authorization to finish this trial supplies that scope; old per-action wording does not require asking the same question again. Unknown outcomes, a changed resource boundary or a failed budget guard require reconciliation before proceeding. No deployment was performed merely by preparing this document.

## Catalog, empty organization and real owner

Run the candidate operator's `verify-catalog-baseline` override with `--set=--version=<observed-version>` to prove the earlier catalog, take the pre-migration RDS snapshot, then run `just deploy migrate <window>` twice. Both catalog checks must match the candidate manifest and the second migration must be unchanged. The migration task does not seed application data.

Provision the fresh organization through the existing operator channel:

```sh
ops/beta/run-step.sh --window "$WINDOW_ID" --approved "$AUTHORIZED_WORDS" \
  provision-clean admission-provision \
  --set=--organization="$TRIAL_ORG" \
  --set=--display-name="Forma trial workspace" \
  --set=--issuer="$VERIFIED_ISSUER" \
  --set=--provider-tenant="$PROVIDER_TENANT"
```

Provision now atomically creates the organization, binding, audit heads, record sequence, sign-in handle, **one empty space and one version-1 owner-publish policy**. These defaults are required by first compose. It creates zero principals, members, entitlements, apps, works, content or resource grants. Provisioning an existing organization refuses without changing its policy or data.

Compose the auth secret with `window_requests.py auth-configuration --clean-trial <file> --identifiers <file> --app-origin https://beta.getforma.dev --output <private-file>` and write it to the stack's `AuthConfigurationSecretArn`. The composer requires exactly the fresh organization and separate shared client; older tenants remain stored but unroutable from this clean service.

Mint the owner's code using `window_requests.py mint-code --clean-trial <file> --organization <uuid> --label owner --out-dir <private-dir>`. Compose `invitation-input` with the same clean-trial/identifiers files, the private owner recipients file and the codes directory; write that input to `OperatorInvitationInputSecretArn` immediately before issuance. Invoke `admission-issue-authoring-invitation` through `run-step.sh`, setting `--organization` and the **digest**, never the code or email. This documented override uses `--kind member_invite --authoring-only --provider-input-from-secret`.

`--authoring-only` is explicit and mutually exclusive with `--grant`. Ordinary issuance still requires resource grants. This code admits a normal entitled member with zero access to existing resources; it does not confer admin or organization-wide authority. First explicit compose then creates that member's shell and its three scoped grants under APP-145. Clear the one-shot invitation input secret after the recorded outcome. Preserve the normal membership capability map, reconcile uncertain provider creates, and deliver the private admission code only through the authorized owner flow. No guest receives this code.

## Broker and activation

Use `window_requests.py shared-broker-configuration --clean-trial <file> --outputs "$W/stack-after.json" --app-origin https://beta.getforma.dev --out-dir <private-dir>`. It verifies the stack's three clean identifiers, draws one token and writes `shared-broker-client.json` and `shared-broker-server.json` together, privately and without overwriting an existing pair. Write them respectively to `SharedBrokerClientConfigurationSecretArn` and `SharedBrokerConfigurationSecretArn` through file input, never argv values. Do not reset `SharedBrokerStateSecretArn` after use.

The broker is a separate 0.25-vCPU/0.5-GiB task with a dedicated role, no app role, database credential or object-store access, and write authority only to its bounded state secret. It receives only its new tenant management key and broker configuration; the app gets only the narrow broker client token. Both injectors remove raw JSON before launching their child and materialize owned private files. The public ALB routes exactly `/v1/shared-invitations`; the request requires the secret bearer token and rejects browser Origin. This is publicly reachable authenticated routing, not a claim of private network isolation. Bounds and ambiguous-create recovery are in [LIVE-SHARING.md](../operations/LIVE-SHARING.md).

`just deploy activate <window>` verifies both future stop schedules and exact task boundaries before requesting either service at one. It waits for application health and a completed healthy broker rollout before recording release/tag success. A requested activation is not readiness. Keep the reservation after failure or unknown outcome; inspect the exact service/task rather than retrying a provider create. At expiry, verify both services stop; retained data, load balancer and keys continue to cost money.

## Real rehearsal and evidence

Capture the qualified candidate/image/schema/window identities and actual counts, then use the real owner to register/sign in. Before first creation, there must be no app, work, publication, record or per-resource grant in the new organization. Create an app through the normal intent or template path, save real records, open its app page and share it to the named external recipient. The recipient signs up with the real provider, verifies its mailbox, explicitly accepts the live share, and may separately consent to canvas placement only when permitted. Confirm no membership, organization permission or source permission appeared for that external account. Change a record as the owner and observe its live shared result; revoke and observe uniform access denial.

Synthetic input data is permitted; model outputs, ingestion, storage, auth, queries and sharing outcomes must be real. The isolated Northstar local sandbox remains useful for development, but its seeded personas or local shared-session helper do not establish hosted acceptance. Do not restart or reset the owner's ports 4001–4010 as part of deployment preparation. Record hosted/browser proof separately from local campaign proof, including any email or user-interaction step still pending.
