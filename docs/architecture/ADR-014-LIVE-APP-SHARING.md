# ADR-014: Named live app sharing and connected canvases

Status: accepted product decision; implementation and qualification tracked in [APP-154](../../Tickets/4-done/APP-154.md). Date: 2026-11-01. Supersedes the assumption in APP-139 that accepting an app share makes its recipient an organization member.

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

## Addendum, 2026-11-12: a suspended organization pauses connected links

Decided by Claude (Opus 5.5) under Victor's delegation of 2026-11-12 ("Make your best judgement and answer for yourself based on the product and continue with tickets"), for [APP-178](../../Tickets/4-done/APP-178.md). Victor can reverse it. It applies the principle in the "Offline/reconnect/account change" row above to a suspended sponsor organization: a refusal that is not proof of revocation must not silently delete placement.

**The question.** When the sponsor organization is suspended (`genui.organizations.status='suspended'`), what happens to a guest's connected link? Before this decision, the link was removed if the guest happened to list their canvas or open a shared page during the suspension, and restoring the organization did not bring it back. What the guest saw after a restore depended on whether they had looked. Meanwhile, management showed the grant as "paused".

**The decision.** A suspension pauses connected links. It does not remove them.
- **While suspended:** the link is hidden, and every read, query, release and resolution refuses. That fail-closed behaviour is unchanged.
- **Kept:** the reference keeps its row, with no unlink and no `app.unlinked` event.
- **On restore:** it shows again with no new Add, provided the grant is otherwise unchanged.

**What still removes a link.** A refusal for any other reason: revocation, a changed grant version or placement permission, expiry, the sponsor losing access, or a changed app definition. Each of those removes it, audited, as before.

**Why.**
- A suspension is an administrative state the organization can leave again, not a revocation. The owner's share listing already shows such a grant as paused rather than revoked.
- Each connected reference keeps its own share identity (the "Multiple grants" row).
- Box 1 of APP-178 says restoring the organization restores access.
- The guest's outcome no longer depends on timing.

**Cost.** A paused link still counts toward the guest's placement cap while it is hidden. While it is hidden, the guest cannot remove it from the canvas, because it is not shown. It can still be removed by its key. If it is revoked or expires during the suspension, it is removed automatically, audited.

**Implementation.** `linked_apps` asks `link_paused` before unlinking. The question is whether the grant is live in every respect apart from the organization's state, which may be active or suspended. It uses the same `live_clause`, with the organization predicate as a parameter. Accepting either state means a restore that commits between the listing's access check and this check keeps the link instead of removing it. Tested in the live-sharing campaign's `fail_closed` step.

## Amendment 2026-11-15: a sponsor's sign-out no longer pauses their guest shares (APP-215)

Decided by the coordinator in lieu of Victor from the 2026-11-15 sharing review (finding M3, availability), for [APP-215](../../Tickets/0-backlog/APP-215.md). Victor can reverse it.

**The footgun.** A live share pinned the sponsor's membership epoch at creation (`sponsor_epoch`) and every guest resolution, read, query and release required the sponsor's current `membership_epoch` to equal it. Sign-out and sign-out-everywhere advance the epoch by design (ADR-006 D7.9; the 2026-10-29 amendment there). So a student who shared an app with a friend for seven days and signed out that evening killed the link for good: the friend's page masked its data, the management list said "paused", and the epoch never goes back. The "Owner leaves or loses access" row above never meant an ordinary sign-out.

**The decision.** `sponsor_clause_for` (`storage/shared_access.rs`) no longer compares the sponsor's membership epoch with the share's pinned one. The share still fails closed on everything that is proof of lost authority, each already in the clause: the sponsor's membership must be `active` and their entitlement live (both end on removal through `admin_remove_member` or the operator's `revoke-membership`), they must still hold `record_read` on the work, and they must still hold share authority (unrevoked creator of the work, or `share_work`). Revocation, expiry, a changed publication, definition digest, work version or policy version, and the suspended-organization pause (the 2026-11-12 amendment) are unchanged. The pinned `sponsor_epoch` stays in the share row and in the guest's resolved access (`check_guest`), where it binds the access to the share row; it is no longer a session-validity input, which is what the 2026-10-29 ADR-006 amendment says the epoch is not.

**Proof.** The live-sharing campaign signs the sponsor out everywhere through the real `sign_out_all` (epoch 1 → 2) and shows the guest still reads, queries and releases while management reports the share active; then removes the sponsor through the real `admin_remove_member` and shows every guest read, release, query and resolution is `Denied`.

**What this does not do.** It does not let a removed member's organization-wide share (`work_shares`, the sharing review's M2) end on its own; that needs a migration and is not in this release. The class-time interim is the operator SQL in the trial runbook §10.
