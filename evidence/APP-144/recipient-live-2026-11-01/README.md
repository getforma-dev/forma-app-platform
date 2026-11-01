# APP-144 integrated local recipient qualification

Five checks passed using the owned local 7601 member, 7603 stranger and 7604 account-only guest stand. Product source matches the prior `0ec5e94` baseline; candidate `711d87d` changes only the storage fixture test and runtime package pins. This is local runtime evidence, not hosted signup or replacement-image qualification. No tracked source was edited.

## Observed behavior

- An actual owner form command committed record `ed4f4977-a2e1-4645-bed0-9c3834efbd66`, version 1, commit 17, with audit event `3c5e0b32-c981-47df-b727-6b5b89ee336c`. See `exact-command-receipt.json`.
- The already-open complete guest app document showed it 2,300 ms after HTTP 200; the already-open connected app showed it 3,305 ms after HTTP 200. The HTTP command itself took 229 ms. Values, revision, committed timestamp and commit sequence matched the member row and the actual command outcome. No reload/navigation. Admitted form label, literal note and layout order matched; guest controls remained read-only.
- Browser network emulation offline masked title and protected content on both surfaces in 2 ms observed. Reconnect restored the same values/receipt metadata in 2,429 ms after fresh successful authority reads. Connected x/y/width stayed fixed. Timing measures observation after the operation, not an exact server propagation guarantee.
- A later already-open session removed access through the owner’s real Share dialog. Both recipient and connected content disappeared 948 ms after DELETE 200. Data and authority routes returned 404; inventory omitted the grant. An additional offline/reconnect cycle did not revive it over six seconds. Revocation was real storage state, not an injected HTTP refusal.
- An unrelated member’s existing work and shared JSON request matched nonexistent-resource 404s. The guest’s member `/w` and `/workspace` routes returned 404. The member document rejected a query string. Share HTML deliberately remains a neutral identity landing; it is not claimed to return 404.
- A real Clock widget remained on the canvas, had no work-page/share links, and its actual widget identifier produced 404 under `/w`.

This records-only app has real record receipt metadata, not a governed query-result receipt. No receipt was invented. Exact governed query receipts remain covered by the 20-library/native APP-144 evidence. Member unsent draft and same-key uncertain retry after a real commit are retained in `evidence/APP-144/completion-2026-11-01/browser.json`; guests have no editable form.

## Reproducibility and evidence limits

`browser.mjs` performed the real command/live/offline/reconnect run. The retained `command-live.json` also records its stop at an over-scoped share-row selector before revocation. `finish.mjs` opened fresh guest pages and performed real revocation and reconnect non-revival; `revoke-live.json` records those passes and a later widget-selector timeout. `widget.mjs` selected the actual widget article and completed the final independent case. `report.json` combines these explicitly identified sessions and all five passing assertions. Preparatory selector issues are disclosed, not presented as product faults; no command was replayed by either continuation. The earlier record-key correction resulted in a separate real preparatory record, visible in screenshots.

All contexts are closed. This guest grant is now revoked; do not blindly rerun a mutating script against it. A new reproduction requires a separately created and accepted test share, then replacing the exact locator fixture. No hosted account was touched. Screenshots were visually inspected; all identities and record content here are synthetic.

Retained after PR 31 merged, with all seven screenshots independently inspected. The original intermediate reports remain unchanged, including their selector failures. `retention.json` records original artifact hashes. Scripts retain their original local Playwright import/output paths and synthetic `example.test` fixture address; reproduce only with a separately prepared disposable share and matching fixture files. No private hosted identity, cookie, provider token or credential is included.
