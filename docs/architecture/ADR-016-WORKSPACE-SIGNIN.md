# ADR-016: Workspace entry and verified workspace discovery

Accepted November 1, 2026 (America/New_York), following Victor's explicit instruction to lock the Slack-like workspace model and implement the workspace-entry path first. This amends the entry presentation in [ADR-006](ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md); its identity, membership, verification, session and refusal boundaries remain binding.

## Decision

The product calls an organization a **workspace**. Sign in opens one useful entry page. A person enters a workspace name or pastes its Forma sign-in link, then submits directly to that workspace's existing hosted authentication. The page always offers an editable input; remembered and direct-link selections also offer **Switch workspace** to return to an empty input. A remembered workspace is only an editable routing convenience, never proof of access or an unavoidable redirect. Existing direct sign-in links remain compatible.

Invitation and shared-app links keep their original server-bound destination and acceptance rules. A membership invitation authorizes only its named workspace admission; a shared-app invitation does not grant workspace membership. The intended email experience lets a recipient invited to multiple workspaces use each workspace's own invitation. After admission they can return through its sign-in link. Current provider management invitations use `server_only` delivery: no invitation email is sent by that operation. Verification email is a separate message. APP-165 owns actual invitation delivery and workspace-aware email content; this entry release does not claim it exists.

Credentials stay at the existing identity provider. Successful entry still requires the expected issuer, tenant and subject, verified identity, active membership and entitlement. Matching email addresses never merge accounts or grant cross-workspace access. No password is tried against multiple tenants.

## Current implementation slice — APP-162

Add a native, keyboard-accessible workspace input to `/signin`, accepting a configured readable alias, opaque handle, or a sign-in link on this application's exact origin. POST it through the existing same-origin, bounded, rate-limited PAR/PKCE flow. Reject arbitrary origins, userinfo, query/fragment-bearing links, extra path segments and malformed/ambiguous form fields. Do not fetch the supplied URL or treat it as a redirect destination. Preserve the original handle POST endpoint for compatibility. Unknown, disabled and unavailable workspaces use the same useful recovery page without a directory or autocomplete.

The current trial retains its organization/provider IDs, accounts, applications, records and invitations. A friendly alias is a configuration mapping to that existing identity, not a recreated trial. Restoring Northstar as a served workspace requires separately reviewed configuration and retained-data checks under APP-117; the entry form alone cannot make an unconfigured tenant available. No provider, database or tenant reset belongs to this change.

## Privacy and later slices

Readable aliases are optional discoverable addresses. A successful redirect can reveal that a guessed alias exists, even if initial GET pages and failures are uniform. We do not claim otherwise. Private workspaces retain unguessable links and verified discovery. Rate limits and neutral errors reduce enumeration; they do not turn a readable address into a secret.

**APP-163: Find my workspaces.** Enter email, receive a uniform response, prove mailbox control with a short-lived one-use code/link, then see only eligible workspace links under discovery policy. A maintained authoritative membership mapping must handle verification, email changes, membership removal, disabled workspaces and expiry. Discovery is not workspace authentication: each destination still applies its existing login/SSO and membership requirements. Keep invitations and shared-app access distinct from memberships. No anonymous email-to-workspace list, global credential probing or implicit account linking.

**APP-164: Authenticated workspace switching.** After deliberate authentication/linking, list only authorized workspaces and allow switching while preserving each workspace's identity, policies and destinations. Revocation, logout, multiple tabs, drafts and guest-only access need explicit behavior. A single remembered routing hint is not a multi-workspace session manager.

**APP-165: Workspace-aware invitation delivery.** Deliver the actual app-owned membership/share invitation through an authorized transport, with its workspace/app identity, purpose-bound acceptance URL and appropriate stable return link. This can ship independently of email discovery. Preserve the provider's server-only admission handoff; an upstream provider invitation must not bypass Forma's own code/share acceptance.

## Delivery and evidence

APP-162 ships first with independent authentication review, native rendering, actual route/campaign tests, mobile/keyboard checks and a qualified AWS release. APP-163/164 remain backlog until their own implementations and evidence exist. No planning record or passing local test establishes hosted delivery. Preserve the trial throughout and record any remaining email/discovery limitation explicitly.
