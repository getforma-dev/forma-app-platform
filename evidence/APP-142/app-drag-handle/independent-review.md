# PR 41 follow-up review on current trunk

An independent review on November 2 found no additional production defect in PR 41's final implementation. Original head `73c2afc63c28ae951bd105042032c47ad8e68048` was integrated with current trunk `07ea5aac85b69ad90d7bfb2274fb50395705e26f` through normal local merge `bf165cc`. The reviewed `unite.ts` and `canvas.css` hashes remain identical to the original retained review. This follow-up adds evidence and updates the owning ticket; it does not change the production implementation.

The review followed item creation, adaptive rail placement, pointer capture and cancellation, keyboard dispatch, nested interactive-control exclusion, resize clipping, frame removal and listener cleanup, and member-scoped local layout persistence. The Move chrome is outside the scrolling page. It moves the mounted frame and its existing children through the established canvas path. Primary interactive controls inside the page retain their native click/focus; Hand, Space-pan and non-primary gestures keep their existing paths. The exact-work rename guard updates the page, canvas item and Move accessible names after the normal save/refresh completes. No API, permission, dependency, schema or layout-format change was found.

[Fresh isolated Chrome observations](independent-browser.json) passed five checks on build `b1d6b75109852cff`:

- The hit-testable rail moved the complete four-part app from `(64,64)` to `(129,99)`.
- Right and Shift+Down moved it by the app's documented 1px/10px increments; reload restored `(130,109)`.
- Editing a nested form did not move the app. Dragging preserved the same form node, unsent value and idempotency key.
- An ordinary Library name edit updated the app and Move accessible names after the editor finished saving.
- Real browser touch input at 390px moved the same complete app by `(24,28)` using its reachable edge rail.

The [desktop](independent-desktop.png) and [phone](independent-phone.png) captures were visually inspected. The phone capture is a panned canvas, with a transient Reconnecting indicator; it establishes rail reachability, not complete narrow-layout or live-stream acceptance. No command was submitted. The original packet retains resize and fan-out/reunite coverage; no new hosted, authentication or screen-reader acceptance is claimed.

Three intermediate test failures were harness assumptions, preserved in the private cache: Shift+Arrow was initially expected to be4px rather than the app's10px; reopening a work into a fresh canvas was expected to Unite automatically; and a rename label was inspected before the editor completed its command-list refresh. The corrected pass reused the original synthetic work. Production source and assertions about authority were unchanged.

[`just check`](independent-checks.json) passed all21 steps, including the new promotion tests, with exit0 and no skips. Board generation/check and `git diff --check` were rerun after these evidence and ticket edits. CI, merge and deployment of this integrated branch remain the release owner's separate gates.
