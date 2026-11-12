# Private live app sharing

Source behavior is defined in [ADR-014](../architecture/ADR-014-LIVE-APP-SHARING.md) and tracked by [APP-154](../../Tickets/4-done/APP-154.md). This runbook does not record a hosted deployment. Actual hosted identity signup, email verification and a complete guest journey remain APP-139/140 prerequisites before recording them as real provider behavior.

## Service behavior

Share an owned saved app from its Share control, enter the intended recipient's email and copy its link. Access defaults to seven days, view only, with **Allow adding to canvas** unchecked. The link itself confers no access. The recipient signs in, verifies the exact mailbox and explicitly accepts. Acceptance creates no organization membership, principal, entitlement or copied source grants. They can open the live HTML app; when permitted they can explicitly add a connected reference to `/shared` and their existing canvas. Shared-account sign-out preserves the home workspace session.

The scope is every rendered view and record of the saved published app. UI filters are not security boundaries. Share a dedicated metrics app when that is the intended audience. Record changes remain live; changing publication, definition or source bindings pauses the original grant and requires a new share. Owners may turn placement off while keeping view access, or revoke the grant entirely. Restoring placement permission requires a fresh explicit Add action after removal. Other independently granted access is unaffected.

## New-recipient identity broker

The ordinary service never receives the provider management key. `app/operator/shared_invite_broker.py` is packaged in the operator image and runs in a **separate ECS task and service**. The task has its own execution and task roles, no database password or connection role, no S3 role, and no ordinary membership capability-map grant. One exact provider tenant is configured. A second writer is unsupported: rollout is stop-then-start with minimum 0%, maximum 100%, desired count at most one, and a separate expiry schedule.

fAuth registers **one exact redirect per OAuth client**. Register a member public client for `/auth/callback` and a different public client for `/auth/shared/callback`; set `client_id` and `shared_client_id` respectively on the organization's deployed auth configuration. Neither identifier may use the reserved `-hosted` suffix. A legacy organization without `shared_client_id` still supports member login; shared login refuses before PAR. The clean-trial composer requires the separate client and both callback registrations. Registering the second URI under the member's existing client identifier conflicts with the provider contract.

The reviewed hosted topology uses the existing HTTPS ALB, with only the exact `/v1/shared-invitations` path forwarded to broker port 3001. Its security group permits that port only from the ALB; the broker's other outbound traffic is HTTPS. The listener is publicly reachable, so this is **not a private network endpoint**. Authorization is the high-entropy server-only bearer capability; browser Origin requests are refused, request bodies and handlers are bounded, and unauthenticated calls cannot contact the provider. A public caller can still consume bounded transport capacity. This MVP tradeoff avoids another load balancer while keeping management credentials in a separate task identity. No secret appears in ALB rules or ordinary environment values. Local manual operation defaults to loopback; the reviewed task explicitly selects `0.0.0.0` behind that security group.

ECS injects `GENUI_SHARED_BROKER_CONFIG_JSON` into the broker only. Startup removes it from the environment and writes a temporary 0600 file under a 0700 directory in `/run/operator`, deleting it on exit. A manual deployment may instead supply `GENUI_SHARED_BROKER_CONFIG_FILE`; both at once refuse. The closed configuration is:

```json
{
  "tenant_slug": "the-fresh-trial-slug",
  "provider_tenant_id": "ten_the_registered_provider_tenant",
  "client_token": "a 43-character base64url credential",
  "state_secret_arn": "the dedicated Secrets Manager ARN",
  "port": 3001
}
```

The provider key and its tenant binding arrive only through `GENUI_PROVIDER_MANAGEMENT_SECRET_KEY` and `GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID`, beside `GENUI_PROVIDER_MANAGEMENT_URL`. Broker startup uses the existing transport validator with no admission capability store. `GENUI_PROVIDER_CAPABILITY_SECRET_ID` is refused here; it must never point this task at the ordinary membership invitation map. Broker state is a different retained secret, initialized to `{"version":1,"entries":{},"write_reservations":[]}`. Its task role can read and write that exact secret only; the execution role reads its configuration and attached tenant management secret.

The application receives `GENUI_SHARED_INVITE_BROKER_JSON`, containing only `origin` (the exact HTTPS origin, no path) and the matching `token`. `runtime.py serve()` removes the raw injection before any child launch, validates it and hands the Rust service a private temporary `GENUI_SHARED_INVITE_BROKER_FILE`. `{}` disables new signup. Malformed or conflicting injected/file configuration refuses startup before credential refresh. The client follows no redirects, retries or proxy settings. The deny-all rollback task receives no broker configuration.

Use [the clean-trial deployment sequence](../pitch/CLEAN-TRIAL-DEPLOY.md) to compose both private configuration files together, write their matching stack secrets and activate the checked topology. Do not put those files, credentials, provider tokens or state contents in source, reports or command arguments. Source integration and local qualification are implemented; the recorded beta remains the October 31 sandbox until an actual deployment receipt supersedes it.

The broker creates only role `user` with provider delivery mode `server_only`. It validates the configured provider tenant before create. Signup uses the capability directly in the provider's pushed authorization request, then returns to the exact app. The provider handles registration and mailbox verification. An unverified callback requests verification, revokes temporary tokens and returns to a neutral page asking the recipient to verify and sign in again.

## Failure and recovery

The broker persists a pending record **before** the non-idempotent provider POST, and persists its confirmed capability before replying. Retried requests and separate shares to the same mailbox reuse that capability while valid. Confirmed expired entries are pruned; ambiguous pending entries remain closed. A timeout or lost durable-write response must not trigger a blind provider create, which could revoke another pending invitation.

For an ambiguous pending operation, stop signup issuance for that recipient and reconcile through the existing operator/provider invitation inspection procedure. Identify the exact tenant and pending provider invitation, confirm or revoke its remote outcome, and retain a sanitized reconciliation receipt before correcting only its broker-state entry. Never delete the whole state map or retry create to infer the earlier result. Broker-state custody does not join or admit anyone to a workspace. The initial service is bounded to 60 KiB of durable state, 1,000 entry identifiers, 16 HTTP handlers, five new invitations/minute and 100/hour. A stricter durable budget permits at most 40 reserved secret writes per rolling 24 hours: a new provider create reserves two before its pending write, an alias reserves one, and exact replay writes nothing. This deliberately small MVP ceiling avoids accumulating uncollectable recent secret versions across restarts; capacity refusal occurs before a new provider POST. These bounds follow [AWS PutSecretValue limits and version guidance](https://docs.aws.amazon.com/secretsmanager/latest/apireference/API_PutSecretValue.html); refusal is explicit and existing view access remains independent of broker availability.

Schema migration 0017 is additive. Use the existing migration/catalog qualification process; do not run it from the normal service. Rollback of a committed schema is not claimed here. Disable new signup by removing broker configuration, and revoke affected share grants through owner controls when access should end. Never revert to copied member/source permissions as a fallback.

## Qualification

`just check` includes native/UI checks, independent shared-receipt schema vectors and isolated broker fault tests. `just campaign live-sharing` exercises real PostgreSQL grants, guest reads, confined query receipts and release races. `just campaign shared-identity` exercises production OAuth routes with a synthetic issuer/broker. Those tests prove application behavior against the provider contract, not live provider delivery, managed secret injection, HTTPS proxy setup or hosted rollout. Retained results and review dispositions belong in `evidence/APP-154`.
