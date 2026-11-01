# Private live app sharing

Source behavior is defined in [ADR-014](../architecture/ADR-014-LIVE-APP-SHARING.md) and tracked by [APP-154](../../Tickets/0-backlog/APP-154.md). This runbook does not record a hosted deployment. Actual hosted identity signup, email verification and a complete guest journey remain APP-139/140 prerequisites before recording them as real provider behavior.

## Service behavior

Share an owned saved app from its Share control, enter the intended recipient's email and copy its link. Access defaults to seven days, view only, with **Allow adding to canvas** unchecked. The link itself confers no access. The recipient signs in, verifies the exact mailbox and explicitly accepts. Acceptance creates no organization membership, principal, entitlement or copied source grants. They can open the live HTML app; when permitted they can explicitly add a connected reference to `/shared` and their existing canvas. Shared-account sign-out preserves the home workspace session.

The scope is every rendered view and record of the saved published app. UI filters are not security boundaries. Share a dedicated metrics app when that is the intended audience. Record changes remain live; changing publication, definition or source bindings pauses the original grant and requires a new share. Owners may turn placement off while keeping view access, or revoke the grant entirely. Restoring placement permission requires a fresh explicit Add action after removal. Other independently granted access is unaffected.

## New-recipient identity broker

The ordinary service never receives the provider management key. `app/operator/shared_invite_broker.py` is packaged in the isolated operator image and must run as a separate, single-writer operator service. Its process entry is `python3 -I -u /operator/app/operator/shared_invite_broker.py`; the image's normal migration entry point is unchanged. Configure one broker for its exact provider tenant. Register the exact application `/auth/shared/callback` URI on each provider client used for shared sign-in. A multi-tenant broker routing expansion requires its own configuration/contract qualification; this initial broker is tenant pinned. Run behind a private HTTPS proxy with no public/browser access. The broker binds only `127.0.0.1` and accepts only its bearer credential on `POST /v1/shared-invitations`. Use the existing provider management configuration and tenant binding, including `GENUI_PROVIDER_MANAGEMENT_URL`, `GENUI_PROVIDER_MANAGEMENT_SECRET_KEY` and `GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID`. These are operator-only injections, not application settings.

The provider loader also requires `GENUI_PROVIDER_CAPABILITY_SECRET_ID`; it validates that existing ARN but the broker does not use its admission capability map. Broker state uses a **different**, dedicated Secrets Manager secret, initialized to `{"version":1,"entries":{},"write_reservations":[]}`. Limit the broker's IAM read/write scope to that dedicated state secret and its tenant management credential. Do not give the normal service access to either. A second broker writer is unsupported because the provider create operation is non-idempotent and the store is not a distributed compare-and-swap service.

`GENUI_SHARED_BROKER_CONFIG_FILE` names an absolute, same-UID private file (0600, parent 0700, no symlink or hardlink), with exactly:

```json
{
  "tenant_slug": "northstar",
  "provider_tenant_id": "the registered provider tenant id",
  "client_token": "a 43-character base64url credential",
  "state_secret_arn": "the dedicated Secrets Manager ARN",
  "port": 8087
}
```

The normal application receives another same-UID private file, selected by `GENUI_SHARED_INVITE_BROKER_FILE`, containing exactly `origin` (the private HTTPS origin, no path) and `token` (the matching broker credential). Its authenticated client follows no redirects, retries or proxy settings. Do not put either private file, the credential, provider tokens or state-secret contents in source, reports or command arguments. This source change supplies the process and client; task definitions, private proxy, secret injection and rollout must be provisioned and qualified in the deployment workflow.

The broker creates only role `user` with provider delivery mode `server_only`. It validates the configured provider tenant before create. Signup uses the capability directly in the provider's pushed authorization request, then returns to the exact app. The provider handles registration and mailbox verification. An unverified callback requests verification, revokes temporary tokens and returns to a neutral page asking the recipient to verify and sign in again.

## Failure and recovery

The broker persists a pending record **before** the non-idempotent provider POST, and persists its confirmed capability before replying. Retried requests and separate shares to the same mailbox reuse that capability while valid. Confirmed expired entries are pruned; ambiguous pending entries remain closed. A timeout or lost durable-write response must not trigger a blind provider create, which could revoke another pending invitation.

For an ambiguous pending operation, stop signup issuance for that recipient and reconcile through the existing operator/provider invitation inspection procedure. Identify the exact tenant and pending provider invitation, confirm or revoke its remote outcome, and retain a sanitized reconciliation receipt before correcting only its broker-state entry. Never delete the whole state map or retry create to infer the earlier result. Broker-state custody does not join or admit anyone to a workspace. The initial service is bounded to 60 KiB of durable state, 1,000 entry identifiers, 16 HTTP handlers, five new invitations/minute and 100/hour. A stricter durable budget permits at most 40 reserved secret writes per rolling 24 hours: a new provider create reserves two before its pending write, an alias reserves one, and exact replay writes nothing. This deliberately small MVP ceiling avoids accumulating uncollectable recent secret versions across restarts; capacity refusal occurs before a new provider POST. These bounds follow [AWS PutSecretValue limits and version guidance](https://docs.aws.amazon.com/secretsmanager/latest/apireference/API_PutSecretValue.html); refusal is explicit and existing view access remains independent of broker availability.

Schema migration 0017 is additive. Use the existing migration/catalog qualification process; do not run it from the normal service. Rollback of a committed schema is not claimed here. Disable new signup by removing broker configuration, and revoke affected share grants through owner controls when access should end. Never revert to copied member/source permissions as a fallback.

## Qualification

`just check` includes native/UI checks, independent shared-receipt schema vectors and isolated broker fault tests. `just campaign live-sharing` exercises real PostgreSQL grants, guest reads, confined query receipts and release races. `just campaign shared-identity` exercises production OAuth routes with a synthetic issuer/broker. Those tests prove application behavior against the provider contract, not live provider delivery, managed secret injection, HTTPS proxy setup or hosted rollout. Retained results and review dispositions belong in `evidence/APP-154`.
