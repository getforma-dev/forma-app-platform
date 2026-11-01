# Composed guest app: actual local browser qualification

This packet records the APP-156/157 interaction and appearance seam and the guest portion of APP-160 on strict native build `d708e8042295` (manifest SHA256 in [source identities](source-hashes.json)). It is an uncommitted local implementation candidate, not a hosted release or full APP-160 completion. The identities are the existing **synthetic local owner and account-only guest** on owned ports 4801/4804; no provider signup or real recipient is represented.

The browser used a disposable headless Chrome profile and ordinary product paths: the Dining template picker created an empty adapted app, the exact-version presentation endpoint admitted/published its generic actions and dark/green/mono theme, the owner created a named view-only share with placement permission, and the guest explicitly accepted and added its connected reference. The template has a preset USD 100 budget, clearly labeled as such. A real owner form submitted **12.50**, persisting the ordinary money value and audited record. No row injection, canned query result or privileged fixture operation supplied the app data.

## Observed results

The [passing report](report.json) records the final run at 23:22:40–23:23:06 UTC on 2026-11-01. Asset hashes remained unchanged. Both guest dialogs were already open before the owner submitted; the shared document updated 3322 ms and the connected app 4329 ms after the command response. Visible record values/version matched the owner projection and the actual snapshot/result receipt digests. The command HTTP response took 267 ms. These are observed latencies, not a service-level guarantee.

Both surfaces preserve the same declared section order and permitted generic component actions. View-only projection has no member command form/action, Ask/editor or work links. Forward/reverse Tab cycles stay within both actual dialogs. Native dialog semantics provide background inertness; Escape works and only one component instance is moved. The focused presentation suite passes 7/7, including draft/key/node preservation, target removal, refresh/revocation cleanup and boundary focus wrapping. This is keyboard/browser/source evidence; **no VoiceOver or human screen-reader pass is claimed**.

Actual computed root, card, text and receipt colors match the published dark palette on member, shared and connected live surfaces. Separate JavaScript-disabled member and shared pages retain native record views/receipts and the same colors. The no-JS connected canvas deliberately returns authorized inventory plus an exact Open app link; it contains no record components/actions until its live authority/data refresh. This is the existing progressive-enhancement boundary, not a claimed full no-JS connected dashboard.

Offline cleared both protected roots and their open dialogs in 130 ms. Reconnect recorded 3 successful authority responses before restoring the views and did not reopen old dialogs. Actual owner revocation, with both record dialogs open, cleared both in 750 ms after its HTTP response. Shared JSON/authority and the member work/workspace routes refused, and the revoked reference left connected inventory. Browser denials do not prove exhaustive database membership/grant absence. Delayed-authority, hidden/resume, expiry, wrong-account and broader forged-body cases require their separate authority/backend evidence; this run does not claim to repeat them all.

## Findings and retained trial history

Visual review of an earlier run found surface CSS overriding the published dark card/root colors and leaving white-on-white cards. The fixed finite-token selectors now outrank surface defaults, receipt summaries use the theme accent, and native work/shared/connected roots carry the composed-app class before JavaScript. Root's member browser also found Tab escaping the dialog; a small per-event visible-control wrap fixes both directions without cloning components or importing the canvas runtime. Final desktop, mobile and connected screenshots below were inspected after both fixes.

The first final-build attempt created/published/accepted/linked the same fixture, then stopped on an incorrect **test oracle** that expected the no-JS connected inventory to contain full record data. Its [failed report](initial-native-inventory-oracle.json) is retained. The canonical server deliberately supplies metadata-only inventory. The corrected assertion checks that boundary and the exact Open app fallback. No command had run and no unresolved mutation journal remained, so the final attempt resumed that confirmed fixture and issued exactly one owner record command and one revoke. It did not recreate the app or duplicate the invitation. Earlier cache-only rehearsal failures involving a Close sharing selector and reading a response body after navigation were harness findings, reconciled through ordinary read endpoints before continuation; they are not counted as product failures or as this final run.

- [Shared record dialog](guest-live-dialog.png)
- [Connected record dialog](connected-live-dialog.png)
- [390px informational dialog](guest-mobile-dialog.png)
- [Shared app after revoke](guest-revoked.png)
- [Connected canvas after revoke](connected-revoked.png)

## Reproduce and limits

From an owned, healthy stand and stable strict build, run `node app/scripts/composed_guest_browser.mjs --owner=http://127.0.0.1:4801 --guest=http://127.0.0.1:4804 --output=.cache/reports/composed-guest-new`. The [script](../../../app/scripts/composed_guest_browser.mjs) refuses owner ports 4001–4010, unresolved mutation journals and fixtures that already contain a record command. A fresh output is required for a new live-update qualification. An interrupted mutation must be reconciled before any retry; the runner does not blindly repeat it. Test fixtures, cookies and raw native HTML remain outside this public packet.

The run does not complete member Unite/Focus/Fan out coverage, every filter/partial/failure scenario, a second unrelated-app browser scenario, full screen-reader testing or hosted deployment. APP-156/157/160 retain their predecessor and release gates. The admitted vocabulary remains closed; these results do not imply arbitrary model CSS/code, new guest write rights, AI receipt verification, export or hibernation.
