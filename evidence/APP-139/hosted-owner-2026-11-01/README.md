# Genuine clean hosted owner journey — November 1

The real owner completed invitation redemption, provider email verification, fresh sign-in, creation from an empty workspace, one committed record, one private named share and contextual Ask on deployed candidate **`711d87d985cd0484bd25d2f863f6428d370881ea`**. These are actual product actions against the [clean beta deployment](../../APP-140/hosted-activation-2026-11-01/README.md), using the owner's persisted browser profile and provider session. No synthetic identity listener, injected application session, seeded app or mocked response was used. The recipient has **not registered or accepted**; the full shared-user journey and three-minute recording remain open.

## Observed progression

[stages.json](stages.json) retains positive-selected action outcomes and timestamps, omitting private authentication paths. Each successful product creation, record and share stage observed one POST and no page errors.

| UTC | Actual action | Evidence |
| --- | --- | --- |
| 20:15:17–20:15:59 | Redeemed the intended owner admission, verified the provider email and completed a fresh member sign-in. | Stage receipts; [provider aggregate](provider-aggregate.json) later observed one user, one email-verified user. |
| 20:16:08–20:16:21 | Opened an empty workspace/canvas; selected Equipment requests and edited its description to retain its base fields and add a due date. Actual model customization returned an admitted app with zero records and placed it on the canvas. | [Empty canvas](empty-canvas.png), [customization](customize-template.png), [placed app](created-on-canvas.png), [empty document](empty-admitted-document.png). |
| 20:16:32–20:16:36 | Submitted a fictional request through the native admitted form. The authorized listing contained exactly one committed record. | [Form result](first-record-saved.png), [command receipt](record-receipt.json). |
| 20:22:23–20:22:25 | Created one private named view-only share with separate permission to add a connected canvas reference. | [Fresh persisted-share proof](persisted-share-proof.json), [redacted settled dialog](named-private-share-public.png). The grant is pending, not accepted. |
| 20:23:56–20:24:01 | Used the app's Ask button and asked for the saved request's item and due date. One real model request returned the exact item/date and one full server record receipt. | [Ask result](ask-result.json), [answer/receipt](ask-answer.json), [expanded receipt screenshot](ask-answer.png), [unchanged before/after comparison](ask-record-comparison.json). |

The fictional input was a **Portable display**, due **2026-11-02**, for a Monday customer workshop. The actual command committed revision **1**, commit sequence **1**, at **20:16:34 UTC**. This is synthetic content entered through real admission, persistence and audited command paths. It is not an imported dataset, a query aggregate, or proof of billing. The empty-canvas screenshot catches an initial reconnect indicator; the subsequent customization frame shows Live. The first frame alone is not used as live-connection proof.

The [clean provisioning receipt](../../APP-140/hosted-activation-2026-11-01/deployment.json), empty authoring-only admission qualification and native APP-145 first-compose checks establish the no-precreated-app setup. This browser progression supplies the previously missing real-provider first-member observation. It does not replace the separate native read-only/cancel/refusal checks.

## Real model and receipt evidence

[Creation correlation](create-model-correlation.json) records the actual `claude-sonnet-4-6` event during the customization interval: 4,448 input tokens, 626 output tokens, 7,543 ms. [Ask correlation](ask-model-correlation.json) records `claude-haiku-4-5-20251001`, 527 input tokens, 54 output tokens, 1,028 ms, followed by `talk.answered` with source `model`, one work, one record and one reference. Both observations came from the exact healthy deployed task, with no model endpoint override. Correlation uses that task and the browser time interval; the application does not log a shared browser request identifier in `composer.model`.

The Ask HTTP response was 200 in 1,520 ms. Its rendered answer and expanded receipt match the exact saved record; the full authorized before/after listing hashes are equal. The scoped question added no other app or collection, and Ask changed no record. Source-query/static-result Ask remains unsupported; this proof covers an admitted records app only.

## Share harness corrections retained

[share-harness-history.json](share-harness-history.json) retains two unsuccessful automation attempts before the successful share stage. The first clicked before the asynchronous document controller was ready. The second used an exact label selector that did not match the expiry select's accessible name. Read-only inspection then used the observed `select[name=expires_in_days]` and waited for the document's ready status.

Both attempts stopped **before the mutation journal and before any POST**, with zero non-GET requests. Separate read-only observations still found zero shares. They were harness precondition/selector failures, not product runtime defects or repeated grants. Only the later successful action wrote the one pending share; a fresh subsequent GET confirmed its exact intended recipient, bounded expiry and separate canvas permission.

## Remaining acceptance and retention

At **20:26:05 UTC**, [mail status](mail-status.json) still showed the recipient's SES identity verification **Pending**, SES sending enabled but production access false, and **no guest account created**. SES recipient verification, AWS production-access approval and provider account email verification are distinct. The configured mail provider is unchanged; no resend or provider switch was performed. Real guest signup/verification, explicit exact-app acceptance, connected placement, live owner update, revoke/reconnect and no-membership observations still require their hosted continuation. Their existing synthetic local proofs do not close this gate.

Seven retained screenshots were visually inspected. [screenshot-redaction.json](screenshot-redaction.json) discloses that only the private recipient label was temporarily replaced in the browser DOM before the public share screenshot; no server data or grant changed. Earlier unredacted or unsuccessfully masked captures stay private. The generic `colleague@example.com` input placeholder is product copy, not the recipient. The other images contain the owner's display name and fictional sample content, with no email, token or private sign-in handle. Raw authentication footage, profiles, cookies, URLs, invitation codes and operator bodies remain private. [sources.json](sources.json) retains original hashes for reconciliation.

These staged checks are not a continuous narrated take and do not establish a maximum-180-second recording. The pinned local twin, complete new-recipient journey, any additional panel accounts and final recording remain independent APP-139/140 obligations. Ticket Backlog status and predecessor/phase gates are preserved.
