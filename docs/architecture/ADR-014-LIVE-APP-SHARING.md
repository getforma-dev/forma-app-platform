# ADR-014: Named live app sharing and connected canvases

Status: accepted product decision; implementation and qualification tracked in [APP-154](../../Tickets/0-backlog/APP-154.md). Date: 2026-11-01. Supersedes the assumption in APP-139 that accepting an app share makes its recipient an organization member.

An app share is private, live and view-only by default. It gives a named verified person access to one saved app, without inviting them into Northstar. Inviting someone to the platform or an organization is a separate action. A shared URL identifies the invitation; possessing or forwarding it grants no authority. There is no anonymous public-link mode in this decision. Static snapshots belong to explicit export, not ordinary sharing.

The owner can separately enable **Allow adding to canvas**. Acceptance never places the app automatically. Placement stores a connected reference to the original share and saved work, not a copy of records, an adapted app or a new publication. Revoking placement removes that reference from visible canvases while view access may continue. Revoking the share removes both. A recipient may remove a reference without revoking access.

The owner shares all rendered views and records of the exact saved work, including subsequent record updates. The dialog states that scope. A metrics-only audience should receive an app containing those metrics; a client filter is not a data security policy. No raw file/source API, commands, edit-definition, organization discovery, export or resharing permission is implied. An owner cannot delegate authority they no longer hold. The initial publication, admitted definition digest and work/source bindings are pinned. Changing those pauses access until the owner creates a renewed share, preventing accidental disclosure of newly added views or data.

| Scenario | Decision |
| --- | --- |
| Northstar colleague | Named app access; existing independent membership/grants are unchanged. |
| Existing Forma user in another organization | Verify their own identity, open the shared app, retain their home workspace session. No Northstar membership. |
| New recipient | Identity-only signup/sign-in returns to the exact shared app. They may use an account-owned shared-app canvas if placement is permitted. |
| Forwarded link or wrong signed-in account | No content or recipient details; offer a deliberate identity change through sign-in. |
| View only | Open the live HTML app; no automatic canvas placement or record commands. |
| Placement enabled | Explicit Add to canvas creates a reference; updates continue from the original app. |
| Expiry or owner revocation | Default seven days, owner-selectable one to thirty days. Current authorization is checked again before release. Cached visible content is cleared on refusal. |
| Owner leaves or loses access | Share fails closed; no persistent copied source grants. Another authorized owner must create a new share. |
| App definition/source bindings change | Pause the pinned share; require a new grant. Record updates within the approved app remain live. |
| Multiple grants | Revoking one share does not remove unrelated direct or shared authority. Each connected reference retains its own share identity. |
| Offline/reconnect/account change | Reauthorize before displaying fresh data or restoring a reference. A refresh failure is not proof of revocation and must not silently delete placement. |
| Lost create/accept/update response | Idempotent creation/acceptance and versioned policy changes recover without duplicate grants or widened permissions. |

Account identity and share sessions are separate from workspace sessions. A guest read carries its actual account, share ID and grant version, never the owner's member identity. The query engine remains confined and runs only admitted published views with pinned source dependencies. Guest receipts explicitly attribute the account and delegation. Every document/query release rechecks the live account session, exact grant, sponsor authority, source availability and pins. HTTP output retains native PageResponse/CSP, no-store and noindex controls.

The additive `shared-query-receipt-v1.schema.json` contract uses kind `shared_query_receipt`, `account_id`, `share_id` and `share_version`, with digest domain `forma:shared-query-receipt:v1`. It cannot substitute an owner principal or membership epoch. The existing query receipt schema, historical fixtures and Python domain registry remain unchanged. Implementation edition `impl-2026-11-01.1` records the additive admission/domain change; the standalone Python schema/vector test and Rust admission test qualify it independently.

Visible client data has a five-second authority lease, measured from the start of the successful authority request. Polling runs every two seconds with a two-second timeout; source queries cannot extend that lease. Offline, hidden, page-exit, refusal and timeout states mask data. A later successful authorization and data fetch may restore it. These controls cannot retract a screenshot or data a recipient already read. The server remains authoritative and rechecks before release.

The convention follows restricted named access and explicit roles in [Google Drive](https://support.google.com/drive/answer/7166529?hl=en), and the distinction between resource guests and organizational membership in [Figma](https://help.figma.com/hc/en-us/articles/4420557314967-Members-guests-and-limited-access). Forma's narrower default is view-only; record collaboration still requires its existing explicit member workflow permissions.

Legacy same-organization workflow integrations retain their existing API while the new app Share UI uses this grant model. They are not evidence of external guest access. Live hosted signup, provider delivery and deployment must be qualified separately from local synthetic authentication.

New-recipient signup obtains a single-purpose invitation through the isolated operator broker described in the [sharing runbook](../operations/LIVE-SHARING.md). The normal application receives only its broker credential. Without that configured service, existing-account sign-in remains available and signup is hidden. The broker is a single writer with durable request state; unknown provider outcomes require reconciliation, never blind invitation recreation.
