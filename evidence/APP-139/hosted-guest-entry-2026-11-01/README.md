# Hosted guest-entry interruption — November 1

The genuine guest registration attempt on deployed candidate **`711d87d985cd0484bd25d2f863f6428d370881ea`** timed out on the shared-app entry page between **20:57:52.171 and 20:58:23.283 UTC**. Registration and the recipient journey did not complete in that attempt. This packet preserves the failure and its read-only reconciliation, followed by the separately observed repair continuation below.

The preceding [read-only mail observation](mail-readiness.json) at **20:57:00.757911 UTC** found the intended recipient's SES verification **Success**, with sending enabled and production access still false. This clears the sandbox-recipient prerequisite; it does not establish delivery of Forma's verification email or verification of a product account. No provider switch or resend was made.

## What the original browser log establishes

[interruption.json](interruption.json) selects metadata from the original private `guest-register-result.json`. Its field named `requests` actually recorded **response events**: one shared document and four asset responses, all GET/200. It recorded a `TimeoutError` and zero JavaScript `pageerror` events. It did not record issued requests, failed requests or security-console messages.

Therefore the GET-only log **cannot prove that no POST occurred**. Zero page-error events also cannot establish that CSP permitted navigation. The original live browser's CSP console was **not captured**. The later read-only profile inspection recorded both entry buttons as present/actionable and a shared-prelogin flow cookie, with no workspace-session or shared-account cookie. That artifact contains no capture timestamp, so none is invented here.

## What reconciliation establishes

At **21:02:46.988534 UTC**, the recorded read-only provider/broker inventory found:

- **One matching guest invitation**, pending, created at **20:57:53.161345 UTC** during the attempted registration interval.
- **One matching broker entry**, issued, with a capability present and its provider invitation identity matching the pending invitation. Only these booleans are published; the capability and identities remain private.
- **Zero matching guest provider accounts**. The provider's one existing user is included only as an aggregate, not as proof of guest signup.
- The shared OAuth client matched deployed configuration, was distinct from the member client, and had the exact expected redirect. The source returned no non-null client status/enabled value; no additional claim is inferred from those fields.

This inventory performed no remote mutation, database query or PAR/authorization-code replay. Two discovery reads returned 404 before the documented OAuth-client inventory endpoint was used; that correction is retained rather than presented as a completely clean first probe. The invitation and broker state mean that blindly repeating the failed browser step would be unsafe. Preserve the existing invitation and reconcile its state before continuing.

## Supported cause and later observation

The shared native renderer on the deployed source omitted the exact configured provider origin from `form-action`. Its local native form starts authentication with a POST and a 303 handoff to the external provider. The existing workspace invite/sign-in renderer already accounts for browsers applying form-action to the redirect chain. The shared-page client controller does not remove the anonymous native authentication form.

A **subsequent** anonymous GET at **21:21:17.699331–21:21:17.849405 UTC** independently returned HTTP 200, `Cache-Control: no-store`, `form-action 'self'`, and both native Sign in/Create account buttons. [subsequent-header-inspection.json](subsequent-header-inspection.json) retains these selected observations and hashes of the private raw HTML/headers. This request sent no cookies, followed no redirects, submitted no POST and performed no provider navigation. It is a later inspection of the still-deployed candidate, not a reconstruction of the original browser console.

The [isolated repair evidence](../../APP-154/shared-auth-entry-2026-11-01/README.md) and its [five-case browser report](../../APP-154/shared-auth-entry-2026-11-01/browser-report.json) support the CSP diagnosis: actual native PageResponse HTML/headers and real loopback HTTPS navigation reproduce the self-only failure; the exact configured provider succeeds after the repair, while a foreign redirect remains blocked. Those endpoints are synthetic. The test proves policy/navigation behavior, not real provider registration, mail delivery or a repaired hosted deployment. A separate stale-workspace-cookie entry defect was also repaired locally; the observed guest profile had no workspace cookie, so that defect is not attributed as this interruption's cause.

## Hosted repair continuation

[continuation.json](continuation.json) records four later observations on runtime candidate **`1d884707223765c782fc16ab00ec1c89f771ec24`**. The original app and record retain creation candidate **`711d87d985cd0484bd25d2f863f6428d370881ea`**. [Deployment evidence](../../APP-140/shared-auth-fix-1d88470/README.md) records the separate delivery proof; these browser observations do not replace it.

- **21:54:23.533–21:54:26.146 UTC:** a fresh real owner sign-in completed and returned to the workspace. The browser recorded 18 issued requests and 18 response events, with zero failed requests, security-console errors or page errors.
- **21:54:33.722–21:54:36.613 UTC:** the fresh owner session reopened the original work. Its exact record listing, values, version, commit metadata and publication pins matched the retained pre-deployment listing. The one original named share remained pending with its target and Add-to-canvas permission intact. The original work-state file remained byte-identical; no application write request was allowed by this check.
- **21:55:13.808154–21:55:13.940240 UTC:** a separate anonymous GET returned HTTP 200 and `no-store`, with both native authentication buttons and `form-action` containing self plus the exact configured provider origin, without a wildcard. It sent no cookies or POST and followed no redirects. System curl verified TLS. A preceding Python probe failed against its local CA store before receiving an application HTTP response; that tooling failure is retained rather than counted as a product failure or a successful probe.
- **21:55:19.968–21:55:22.797 UTC:** the fresh native guest signup flow reached the provider, whose registration POST returned **201**. The shared callback returned **303**, and the browser returned to the exact shared app in **verification-pending** state. The browser recorded 22 issued requests and 22 response events, with zero failed requests, security-console errors or page errors. This establishes registration progress, not verified guest access or app acceptance.

The response-event totals include assets. They are separate from issued-request and failure counters and do not establish invitation or account counts. The original interrupted attempt and its missing instrumentation remain unchanged. The new attempt used fresh native entry; no old PAR/authorization-code replay is claimed.

The separate read-only provider/broker reconciliation at **21:57:42.513337 UTC**, also selected in [continuation.json](continuation.json), found **one guest account**, created at **21:55:22.319826 UTC**, with **email verified false**. The provider had two users in total, one verified. It found one guest provider invitation, now **accepted**, with its original creation time, expiry and role unchanged; the total provider invitation count remained two. That provider invitation status concerns registration, **not explicit acceptance of the private app share**. The latest app-share read above still showed pending before registration.

Two issued broker cache entries currently reference the **one** guest provider invitation. The initial reconciliation expected exactly one cache entry; that predicate was too strict. The retained interpretation explains the additional idempotency alias using the existing [broker copy behavior](../../../app/operator/shared_invite_broker.py), and preserves both receipts. Current exact ID equality is established privately; historical continuity is supported by the matching original timestamps, role and broker metadata. The earlier sanitized receipt did not retain the original invitation ID for direct comparison, so that stronger comparison is not claimed. No duplicate provider invitation was observed. This reconciliation establishes neither verification-email delivery nor database membership/grant absence.

## Sanitization and remaining acceptance

The JSON files were built by **positive selection** of named fields. They retain source basenames, original byte lengths and SHA-256 hashes so an authorized reviewer can reconcile the private originals without publishing their contents. The original browser log, reconciliation receipt and read-only profile artifact were not modified. The fresh raw header/body artifacts were written privately with mode 0600. No private screenshot is copied into this packet.

Recipient/owner addresses, share/work/organization/account/invitation IDs, exact private paths, cookies, tokens, capabilities, verification links, secret references, raw management bodies, CSP nonces and request identifiers are omitted. Counts and match booleans are observations, not substitute identities or grants. The sanitized result preserves the failed outcome and explicitly records missing instrumentation.

The continuation also uses positive selection: timestamps, statuses, path classes, counts, booleans and source hashes. The raw header/body, authentication reports, attempt journals, input and original work-state provenance remain private. No email address, authentication query or locator is copied into the continuation.

At this packet's cutoff, guest registration has returned to verification pending. Product-email verification, fresh verified guest sign-in, explicit app acceptance, connected placement, live updates, revocation/reconnect and the narrated recording are **not passed here**. Browser access denials cannot establish the absence of database membership or resource-grant rows; no such hosted database proof is claimed. Future observed continuation evidence may extend this directory, preserving each earlier outcome.
