# Schema1 callback activation and bounded identity rehearsal

Prepared 2026-10-29. **Proposed operating sequence; independent operational review and a coordinated start are still required.** This document prepares existing supported interfaces. It does not activate resources, create a secret, select a start time, clear APP-006, or qualify public signup/application sessions. Root owns GenUI cloud/DNS/secret operations; FormaStack owns provider provisioning and its cleanup. Keep one operator per side and no concurrent schema or secret writer.

## Evidence and exact artifact selection

| Use | Selected source/artifact | Meaning |
| --- | --- | --- |
| GenUI HTTP callback | Source `18c33f538a19d06cb77f37d091c71b1a8d5e3c39`; `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-synthetic@sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08`; ARM64 child `sha256:08d350fd186f72886823c9b33be21b1df1a70d4f96b1a35e63d639fb7c775ac9` | Reviewed schema1 callback and private secret transport; [eight source checks passed](../secret-wiring/ci.json). Use this full ECR index reference as `image_digest`; verify the observed task child separately. |
| Independent key tool | Source `8fca35c35b80828d4effb17e59f9bbd80ae237ea`; index `sha256:5ee5ad165bca1e37bc2efe13f2fb727912c008cbe721b18f452a98482404cedc` | [One-shot canonical verifier](../../hosted-key-qualification/registry-binding.json), not an HTTP service. FormaStack uses the separately reviewed private invocation bridge. Its cache/rotation qualification must remain one process; never restart it to manufacture a pass. |
| Later tenant foundation | Source `677d7bdd7cf41e11a49e85491f50f82b886070e6` | APP-008/011 candidate. [CI never started because of account billing](../../../APP-008/ci/677d7bd-billing-block.json). Do not deploy its schema2 runtime or migration as part of this schema1 rehearsal. Its local results do not inherit the callback image's CI. |
| Retained service fallback | Foundation source `8f8c34ba0b5e47a55990b273fc6d555d41369791`, index `sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22` | Previously qualified schema1 foundation. A rollback may stop at count zero; restarting a fallback requires its own valid reserved input and matching live checks. |

The [managed SQL observation](sql-preflight/managed-observation.json) records the actual read-only task at03:58:08–03:59:01 UTC, exit0, PostgreSQL16.13, verified TLS, canonical version1 catalog before metadata, and cleanup. The temporary definition is inactive; original migration definition was unchanged; service desired/running/pending were0/0/0 and no cluster consumers remained. This was within the old window ending **2026-10-29T04:39:50Z**, with378.872 known standalone seconds of7200 reserved. It is historical point-in-time proof, not a future activation window or permission to mutate schema.

The current deployment implementation files `infra/src/stack.ts`, `src/main.ts`, `scripts/preflight.py`, `package.json`, `package-lock.json`, `cdk.json` and `stack-policy.json` were compared byte-for-byte with accepted source `8fca35c`; all match. In particular stack SHA256 is `8d3058943c36062724a6dfd21c1c78d57219dfad6359b7d3326ec663f95c3cfa` and preflight SHA256 is `47037e5a307195efa1e95e26fc4d3925fbc8b7657d9bbfe02836444d8c5d34cd`. This is a scoped infrastructure source comparison, not a new CI pass for677d. The callback source and tests also still match18c33f5 exactly.

## Before either team starts the clock

1. Independently review this sequence and the fixed two-segment browser matrix below. Confirm Chrome, **native Safari** and Firefox capability against FormaStack's actual acceptance plan. WebKit-engine evidence is additional; it does not substitute for native Safari. Provider IDs, passwords, invitations and the configuration secret do not need to exist yet.
2. Prepare the unchanged deployment code, callback image receipts, closed configuration field list, existing private ledger location, current issued beta certificate, stack identity and cleanup checklist. Inspect registry/source bindings when root performs live preparation. Preserve the exact `beta.getforma.dev` origin and `https://beta.getforma.dev/auth/callback`; no alternate callback or wildcard.
3. Agree a single recorded `T0` and absolute `Tend`, with `Tend <= T0 + six hours`. The interval includes provider creation, DNS/TLS, private handoff, stack update, browser/key tests and cleanup. Select an explicit earlier `Tstop` that leaves cleanup time; set both the callback's Unix `expires_at` and the GenUI deployment expiry to that same cutoff. Do not calculate a new six-hour interval at a segment restart. Preflight itself accepts up to48 hours; the six-hour restriction is an additional operator obligation here.
4. Preserve the $5 auth allowance **once inside the $100 total**. It is already a ledger category, not another$5 per stage. Reconcile the real ledger and prior task/reservation evidence; the [earlier forecast](budget-forecast.json) is not a reservation. A new GenUI UUID/input reservation is required for the callback image, selected future expiry and actual configuration-secret ARN. Never overwrite the old window's immutable inputs or reset spending to zero.
5. Reobserve service desired/running/pending0/0/0, all previously seen task ARNs STOPPED and no active/pending consumers. Keep the protected RDS identity, version1 schema, database defaults and grants quiescent. No APP-008 migration, seed, restore or mutation may overlap. Repeat the reviewed read-only SQL preflight only if state changed, evidence is no longer sufficient or the operator cannot establish quiescence; do not run the migration task as a substitute.

The first coordinated signal means **ready to enter bounded provisioning**, not that a callback already has provider IDs or that browser integration has passed. AfterT0, provider provisioning supplies actual identities; GenUI then completes the configured deployment. This avoids requiring identifiers before the resources that create them exist. Neither an unfinished browser capability check nor an unreviewed operating gap is resolved by starting early.

## Supported deployment command and input contract

From the repository root, the existing commands are:

```sh
python3 infra/scripts/preflight.py --input /private/OPERATOR_SELECTED/callback-deployment.json --ledger /private/OPERATOR_SELECTED/budget-ledger.json
python3 infra/scripts/preflight.py --input /private/OPERATOR_SELECTED/callback-deployment.json --ledger /private/OPERATOR_SELECTED/budget-ledger.json --activate
```

These are command shapes, not actual private paths. The first command **writes a cumulative reservation**; it is not a read-only dry run. `--activate` makes live AWS checks and can request count one. There is no `--force`, `--skip-checks`, `--renew`, `--dry-run` or source-deploy option. Use the same selected input/ledger throughout a window. Only `ingress_mode` and `service_desired_count` may change without invalidating its immutable binding.

| Exact input key | Value for this rehearsal |
| --- | --- |
| `window_id` | New canonical UUID for this new immutable callback window; retain it across retries/unknown outcomes. |
| `account`, `region`, `hostname` | `877573132739`, `us-east-1`, `beta.getforma.dev`. |
| `operator` | Actual named responsible operator,1–100 characters. |
| `ingress_mode` | `public`; public HTTPS is the selected mode. |
| `operator_ipv4_cidr` | Observed real public IPv4 `/32`; still required by the closed contract even in public mode. |
| `deployment_expires_at_utc` | Absolute `Tstop`, whole-second UTC `YYYY-MM-DDTHH:MM:SSZ`;10 minutes–48 hours from each preflight invocation, and within the stricter agreed six-hour interval. |
| `tls_certificate_arn` | Actual issued beta ACM certificate in this account/region; verify hostname and expiry independently. |
| `image_digest`, `image_architecture` | Exact callback ECR index URI from the artifact table; `ARM64`. |
| `service_desired_count` | `0` while preparing/updating; change only this field to `1` for explicit live-checked activation. |
| `extra_task_hours` | Reviewed aggregate standalone/overlap allowance, integer at least2; retain its actual-use accounting. This is not an instruction to run another migration. |
| `qualification_config_secret_arn` | Actual existing full ARN matching the approved `genui-qualification-` prefix. No secret JSON in this file. |

Preflight emits these exact CloudFormation parameters: `Operator`, `MonthlyBudgetUsd`, `IngressMode`, `OperatorIpv4Cidr`, `DeploymentExpiresAtUtc`, `TlsCertificateArn`, `ImageDigest`, `ImageArchitecture`, `ServiceDesiredCount` and `QualificationConfigSecretArn`. **The emitted `ServiceDesiredCount` is always`0`, including activation preparation.** Do not replace it with1 in the template.

The current local synthesis script is `npm run synth -- --quiet` from `infra/` (`cdk synth --no-lookups` under the locked package); there is no npm deployment script. Reproduce the existing typecheck/test/synthesis procedure when preparing the actual template. Do not reuse stale `cdk.out`: the cached template inspected during this preparation lacked the optional qualification-secret parameter. Fresh synthesis and its exact template hash must be reviewed before use. Preserve any additional unchanged template parameters, such as `BootstrapVersion`, through their actual existing values; do not invent or change bootstrap resources.

For a reviewed existing-stack update, the installed local AWS service models support the following request shapes. Root must construct actual JSON from the reviewed template and preflight receipt, inspect it, then use structured arguments (for example `aws cloudformation create-change-set --cli-input-json file:///private/OPERATOR_SELECTED/change-set.json --region us-east-1 --output json`). No such request was created or run by this plan:

| Operation | Exact fields used and required observations |
| --- | --- |
| `cloudformation get-stack-policy` | `StackName`: exact observed stack ARN. Preserve the currently installed RDS replacement/delete protection. |
| `cloudformation create-change-set` | `StackName`, unique `ChangeSetName`, retained `ClientToken`, `ChangeSetType: "UPDATE"`, actual `TemplateBody` or separately reviewed `TemplateURL`, `Parameters` entries with `ParameterKey` and `ParameterValue` (or `UsePreviousValue:true` for unchanged extra parameters), and the exact required `Capabilities` from the reviewed stack/template. Never use both template forms or an unreviewed upload destination. |
| `cloudformation describe-change-set` | `ChangeSetName`/`StackName`; follow pagination. Inspect every changed resource. Refuse database replacement/deletion, new customer access, image/startup drift, unexpected network/data/IAM changes or loss of the stop schedule. |
| `cloudformation execute-change-set` | Exact returned `ChangeSetName` ARN, `StackName`, retained `ClientRequestToken`; no temporary stack-policy override or disabled rollback. |
| `cloudformation describe-stacks` | Exact `StackName`; require `UPDATE_COMPLETE` and inspect actual parameters/outputs, RDS identity and policy, task definition and execution-role secret policy. A returned update request is not completion. |

The source does not provide a turnkey change-set/secret orchestration command. That concrete operator request preparation and independent comparison remain required; this table is not permission to fabricate missing identifiers or bypass an SDK/CLI limit. Inspect the actual serialized template size before choosing its supported transport. Use the established bounded AWS call policy: one attempt,5-second connection limit,10-second read limit and20-second outer process bound; an unknown outcome requires reconciliation with the retained request identity before retry.

## Provisioning and private configuration afterT0

1. FormaStack provisions only its approved synthetic resources, then supplies actual provider source/image/health evidence, Northstar/canary tenant and client IDs, exact redirect registrations, intended synthetic email/subject information and least-privilege server-only invitation capabilities. Keep management credentials and passwords outside GenUI's callback configuration. Root handles only its agreed DNS records and retains their actual record IDs; no unrelated DNS/OVH/GateWasm resource is changed.
2. Build the complete first-segment JSON in an approved private channel. The callback accepts **exactly** the following fields; unknown fields refuse startup:

| Field | Required binding |
| --- | --- |
| `run_id` | New segment identifier,1–64 ASCII letters/digits/hyphens; privately map it to the single campaign/window and process allocation. |
| `app_source_revision` | `18c33f538a19d06cb77f37d091c71b1a8d5e3c39`. |
| `provider_source_revision` | Actual independently accepted provider source pin,40 hexadecimal characters. The selected provider plan names `dfba787295b880816fd95295b507a4527c51e62f`; reconcile against its real deployed receipt before use. |
| `expires_at` | Original absolute `Tstop` as Unix seconds, identical across every segment. No extension on restart. |
| `max_attempts`, `max_flows` | Preallocated remaining segment starts (1–12) and simultaneous flows (1–3, no greater than starts). The finite plan below uses one flow at a time. |
| `synthetic_tenant` | `true`. |
| `auth_origin` | The actually verified `https://auth-staging.getforma.dev`, with no path/query/credentials. |
| `tenant_slug`, `tenant_id`, `client_id` | Actual registered provider values. Tenant ID is `ten_<UUID>`; do not derive it from a GenUI organization or invent it. Client ID must satisfy the reviewed schema and not end in`-hosted`. |
| `issuer`, `audience`, `max_access_lifetime_seconds` | Actual accepted provider contract; the selected plan uses `gatewasm`, `gatewasm-app`,900. Confirm rather than infer these from the URL/client. |
| `expected_email` | One exact intended synthetic provider email, private. |
| `expected_subject` | `null` only for the new invited-user segment; actual provider `usr_<UUID>` for the existing-Maya login segment; verify it in each callback. |
| `expected_email_verified` | Actual expected backend response boolean. The no-SMTP synthetic plan uses`false`; that never qualifies verified-mail activation. |
| `operator_token` | Fresh cryptographically random32-byte capability, canonical unpadded base64url43 characters, separately supplied to the approved operator runner. Rotate per segment. |
| `invite_token` | Actual confidential provider invitation for signup; `null` for every existing-user login segment. Never derive it from a GenUI invite digest. |

3. Create one campaign-owned Secrets Manager secret using the existing default-key path and an allowed `genui-qualification-<bounded-name>` name. The actual `CreateSecret` request uses `Name`, a retained `ClientRequestToken`, and `SecretString` containing the exact private UTF-8 JSON; omit `KmsKeyId` to use the reviewed default-key path. Only the returned actual ARN goes in deployment inputs. Privately bind campaign/window_id, segment run_id, exact ARN/returned VersionId, SHA256 of exact UTF-8 configuration bytes, original expiry and max_attempts/max_flows; append the observed task ARN after startup. Retain the capability confidentially, never in the public receipt; do not print/store the raw value in source, public evidence, command arguments or CloudFormation. No dedicated secure issuance helper is supplied by this repository: root must prepare/review the private request transport without logging it. An unknown creation result must be reconciled by its existing name/token, not repeated with a new name.
4. Reserve the new GenUI window using the count-zero input and cumulative ledger, then prepare/review/execute the count-zero stack change set. Complete the whole update, including its conditional execution-role policy, before activation. The selected secret ARN must be the only `GENUI_AUTH_QUALIFICATION_CONFIG_JSON` secret entry in the single `workspace` container; no plain auth environment variables, `environmentFiles`, startup overrides or simultaneous file-path injection. The app execution role gets only that exact optional secret read; the app task role does not gain the migration password or management authority.
5. Verify the actual task image/ARM64/UID65532, private `/run/genui` volume, TLS/ingress, service revision and future exact stop schedule. At this point provider IDs, actual secret VersionId, concrete input/template and deployment receipts exist. Change the input's `service_desired_count` to1 and invoke the existing `--activate` command. It checks stack/parameter/task/secret identity and the live future schedule before its sole count-one path. Its result is **activation requested, not healthy**.
6. Observe one intended healthy task, no old/pending consumers, ALB readiness and real beta TLS. Verify actual image/task definition against the callback artifact. The runtime privately consumes JSON into an owned0600 file under0700, removes raw JSON from its environment and gives only the path to Rust; it validates the closed schema/expiry before listening. Do not enable ECS Exec or expose a debug/config endpoint to inspect the secret. Managed delivery is supported by the exact task secret binding, successful startup, operator status and retained private version record, not by dumping the file.

## Fixed two-segment browser plan — independent review before launch

Use **two planned process slots**, no spare/retry slot, with four total configured starts: S1=3 and S2=1. Both use `max_flows=1`, identical original expiry and immutable source pins. This implements the provider team's relayed existing-Maya three-browser flow plus a separate fresh invitation/signup. The [retained identity contract](../../upstream-auth-source/GENUI-IDENTITY-CONTRACT-2026-10-28.md) supplies the actual identifier and registration rules; Maya's actual subject/email and the new signup identity are private provisioning outputs, not values inferred from that document.

| Slot | Configuration and fixed observations |
| --- | --- |
| S1: existing Maya login | Actual Northstar tenant/client and seeded Maya email/subject, `invite_token:null`, `max_attempts:3`. Run three serial fresh state/PKCE flows, one each in Chrome, native Safari and Firefox. Verify each callback's actual subject and expected mailbox state; correlate browser with attempt/completion receipt. |
| S2: fresh invited signup | One fresh Northstar invitation and a separate exact synthetic email, `expected_subject:null`, `max_attempts:1`; use Chrome. Verify returned identity binding and record its actual new subject plus cleanup outcome. This invitation may be consumed once; do not reuse it for login or another browser. |

Provider-owned canary, replay and tenant controls remain in FormaStack's separate accepted plan and receipts. No third GenUI configuration is allocated here. A provider rejection alone does not prove GenUI application authorization; those downstream application checks remain open. If an accepted scenario requires additional starts/processes, amend and independently review the finite plan **before** launch; do not silently expand it while running.

The source enforces **256 start/callback requests per process**, including invalid/refused public requests; status reads do not consume that counter. Two deliberate slots allocate at most512 requests under this operator-controlled sequence, four starts (within the existing overall maximum12), and one flow at a time. Retain actual counters and charge unknown outcomes at the full slot allocation. These are finite operator allocations, not a durable global counter.

ECS or a supervisor may replace/restart a process using the same secret version and reset its in-memory counters. The binary/service does not enforce a global process-count cap. Any unexpected replacement or reset aborts browser work: request count zero and reconcile; there is no spare slot or automatic continuation. Do not repurpose S2 as a retry, reset counters for more attempts, or claim512 is enforced across uncontrolled replacements.

### Explicit stopped transition on the same secret ARN

The default sequence changes from existing-Maya login (S1) to separate fresh signup (S2). Invitation presence always selects PAR signup; absence requires an actual expected subject for login. After S2, the consumed invitation cannot be reused. The existing [callback contract](../README.md) has no reload API or durable session state; the following stopped transition is an operator procedure, not an automated version attestation.

1. Save the authorized status receipt, counters, pending-flow state and actual subject; reconcile `cleanup_unknown` with FormaStack. `logout_acknowledged:true` is not proof of refresh revocation. Do not lose unresolved provider outcomes by restarting.
2. Request count zero on the exact observed service, observe desired/running/pending0/0/0 and every previously seen task STOPPED, and confirm no other secret writer. No browser traffic occurs during a transition. Never change AWSCURRENT while an old task may read it or continue a flow.
3. Build the next complete private JSON with its new `run_id` and operator capability, the preallocated starts/flows, original unchanged `expires_at` and source pins, and the actual next tenant/email/subject/mode. Preserve old private configuration/version receipts and bind the new version to campaign/window_id, segment run_id, exact ARN/VersionId, exact UTF-8 configuration SHA256, original expiry and allocated attempts/flows; append its new observed task ARN. A supported `PutSecretValue` request uses `SecretId` equal to the **same exact ARN**, a new retained `ClientRequestToken`, the exact `SecretString`, and `VersionStages:["AWSCURRENT"]`. The full raw document remains private. Record returned VersionId and verify `DescribeSecret.VersionIdsToStages`; do not mutate an existing version, extend expiry or choose a new ARN merely to reset configuration.
4. This same-ARN transition does not change preflight's immutable input. Preflight binds the **ARN only**, not VersionId, AWSCURRENT bytes, expected subject or limits. Exclusive-writer operation, private exact-byte/version records and the unique new run ID/capability provide the operating linkage. They are not runtime attestation. An unknown update result must be reconciled by version token/stage before another write or activation.
5. Keep the same window/input and existing stack/task definition, then invoke `--activate` only after stopped-state and future-schedule checks. New tasks read the current secret; changing a secret does not reconfigure an already running process. This avoids a forced rolling deployment and overlapping old/new configurations. The actual operator must inspect the newly started task and use the new capability to validate supported status fields before starting the next browser flow.

### What status can actually prove

`GET /auth/qualification/status` requires the operator bearer header, no query and no body. Compare `open:true`, new `run_id`, `configured_source_pins.app/provider`, `issuer`, `fauth_tenant_id`, `fauth_tenant_slug`, `client_id`, fresh expected `counts` and `pending_flows:0`. The deployment/image receipts must support the configured pins; `source_pins_require_deployment_receipts:true` is explicit. Keep `workspace_access_granted`, `mailbox_delivery_proved` and `provider_revocation_proved` false as their actual meaning.

Status does **not** expose configured expected subject/email, invite mode, expiry, configured limits, secret ARN/VersionId or a configuration digest. Before browser traffic, the private exact-byte/version record plus new run ID/capability and stopped-old-task observation supports an **operator-bound inference** that the intended configuration loaded. It does not attest those hidden fields. No new runtime field or debug read is authorized here. On the first controlled login, verify `exchanges[].verified_subject` equals the privately bound actual subject, `identity_qualified:true`, the expected mailbox boolean and cleanup acknowledgement; correlate its attempt/completion time with that browser flow. The endpoint has no browser label, so the runner must retain that nonsecret correlation.

The existing callback APIs remain: backend operator `POST /auth/qualification/start` with an empty body and exact bearer header; then same-context browser navigation to the returned303 Location; provider return to the exact callback; neutral result page; authorized status fetch. Use Node-side requests with redirects disabled and the same browser cookie store. Do not place the operator token in global browser headers, URLs, client storage or public receipts. The actual provider password stays in the approved browser/test-private channel. Do not turn this rehearsal into GenUI session/admission implementation.

## Failure, rollback and shutdown

The immediate safe state is **count zero**, reached with the existing AWS operation `ecs update-service` using `cluster`, `service` and `desiredCount:0` (CLI flags `--cluster`, `--service`, `--desired-count 0`). Count-one requests must still use preflight; never use this stop command as an activation shortcut. After a stop request, describe the exact service and task ARNs until the service is0/0/0 and every known task isSTOPPED. Inspect cluster tasks and account for owned one-offs; the service expiry does not stop standalone tasks. Stop only those owned tasks with `ecs stop-task` (`cluster`, exact `task`, nonsecret `reason`) and retain actual exit/stop results.

On secret startup rejection, wrong status/source/tenant/subject, missing readiness, unexplained counter reset, unresolved provider cleanup or any exceeded allocation: stop browser work, save sanitized evidence, request/observe zero, and reconcile. Do not renew the clock, rewrite the same reservation, retry an ambiguous non-idempotent provider invitation or add a process slot. A rollback does not need to restart anything.

If an image/configuration rollback is necessary, first observe all consumers stopped. Select only an independently qualified **schema1-compatible** image/configuration. Changing image, secret ARN or expiry changes preflight's immutable input and needs a newly reconciled/reserved plan if service is to run again; inability to fit the original time/budget means remain stopped. An earlier secret version may contain a consumed invite or obsolete subject and cannot be blindly restored as a working login plan. Do not replay old state/cookies. Do not invoke any database migration/downgrade during this rehearsal.

BeforeTstop, save each slot's status/identity and bounded key-tool evidence, reconcile remote logout/revocation/unknown outcomes with FormaStack, and explicitly stop GenUI service and owned one-offs. The existing future scheduler is a fallback stop request with finite retries, not proof of a timely stop or billing cutoff. Observe stopped resources throughTend, reconcile durations/reservations and preserve the schema1 baseline. FormaStack performs its approved provider teardown; GenUI removes only the auth DNS records it created using their retained IDs. Do not delete the shared GenUI RDS/ALB/keys or unrelated DNS.

Only after consumers are stopped, remove/disable the campaign capability under the reviewed cleanup plan. An AWS `DeleteSecret` request uses exact `SecretId` and an explicitly selected supported recovery window; do not invent a force-delete requirement. The stack may continue referencing an unavailable secret while count zero, which must be recorded as intentionally non-activatable. Clearing/changing the ARN for a later running deployment changes the immutable budget input and requires its own valid plan. Default retained logs/storage/backups/ALB/RDS/keys remain billable; count zero is not teardown. Preserve the private version/audit evidence needed for review without retaining plaintext capabilities unnecessarily.

## Open operational gates

- Independent review and provider confirmation of the fixed two-segment matrix and stopped-version transition; this document does not implement an automated issuer, version attestor, browser runner or aggregate ledger.
- Native Safari capability/manual acceptance procedure and both teams' agreed start/cleanup interval.
- Actual provider identity/configuration and secure secret creation/transition **inside** that interval; fresh immutable GenUI reservation, concrete request/template review, count-zero stack completion and live-checked activation.
- The active runtime's expected subject/version is not exposed before a callback; retain the operator inference and first-login verification described above without mislabeling it as attestation.
- Actual hosted browser/signup evidence and the separately owned provider tenant/canary, PKCE/replay, key and cleanup evidence. The no-SMTP plan cannot pass real email delivery/verification-return or external-beta admission; those remain separate blocked acceptance.
- Later APP-008/011 exact-source CI remains blocked by GitHub account billing, independently of this already-qualified schema1 callback image. No newly passing CI, schema2 acceptance or completed Phase1 is claimed.
