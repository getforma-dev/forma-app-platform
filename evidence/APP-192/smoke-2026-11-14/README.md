# APP-192 share dialog browser smoke, 2026-11-14 (W2b)

This is a one-off smoke, not a CI test. The W2b builder ran one Playwright script against the local dev stand of the `forma-w2b` worktree, then ran it once more after a CSS fix the first run found.

Everyone and everything on the screenshots is synthetic. The people are the dev stand's pitch personas: Diego Ramos, Sarah Chen and Priya Natarajan. The app is "Equipment requests", made from a first-party template. No secret, key, token or real person appears here. This is neither hosted acceptance nor Victor's browser pass.

## What ran

Branch `trial/w2b-share-dialog` at `e27aff78`, with the CSS fix recorded below.

1. `just dev -d` started the pitch profile in this worktree on port base 8900: Diego on 8901, Sarah on 8902, Priya on 8903.
2. The script ran: `PLAYWRIGHT_MODULE=<a separately installed playwright 1.66 index.mjs> OWNER_ORIGIN=http://127.0.0.1:8901 OTHER_ORIGIN=http://127.0.0.1:8903 node w2b-smoke.mjs <this folder>`. The script is [w2b-smoke.mjs](w2b-smoke.mjs). It drives headless Chrome with a fresh profile, not a personal browser profile.
3. Diego made a work of his own with `POST /v1/compose` and the `equipment-requests` template. There is no model call on that path. The new work is listed in his "My apps" but is not yet placed on his canvas, so the script opens "Share this app" from the work page `/w/{app}/{work}`. That page uses the same button and the same dialog module as the canvas.
4. `just dev stop` stopped the stand. Its data volume, with the two smoke works, was kept.

The script made two failed starts before the first full run:

- It first looked for the Share button on the canvas, where the new work is not placed.
- It then named the selects by label. A select inside its label has the chosen option in its accessible name, so the exact match found nothing. The script now names the selects by their form names.

## What it found and what was fixed

The first full run passed every check the script made then. Its dialog screenshot, [dialog-before-fix.png](dialog-before-fix.png), showed "Stop sharing with everyone" although nothing was shared with everyone. The cause was the class rule `.share-danger { display: inline-block }`, which outranked the `hidden` attribute.

The fix is one rule in `app/admin/src/styles/sharing.css`: `.share-team [hidden] { display: none; }`. Two guards now cover it:

- `test/team-sharing.test.ts` asserts the rule. B10 in [the break-and-restore record](../break-and-restore-2026-11-14.md) shows that test failing without it.
- The script now asserts that the stop control is hidden before sharing with everyone and after stopping.

The run after the fix is what [smoke.json](smoke.json) and the other screenshots record. It made five checks and saw zero page errors.

## What the screenshots show

- [dialog.png](dialog.png): "People in your organization" with a Person picker (colleagues by name) and Access: "Can view", "Can add and edit their entries", "Can review all entries". Below that, "Everyone in the organization: Not shared with everyone" with no stop control, "Shared with: Not shared with anyone in your organization yet", and then the existing "Someone outside your organization" private-link section.
- [shared-with-a-person.png](shared-with-a-person.png): Sarah Chen shared with as "Can review all entries". The list shows her by name with that access.
- [shared-with-everyone.png](shared-with-everyone.png): "Shared with everyone: Can view. This includes people who join later." with "Change access for everyone" and "Stop sharing with everyone". Priya has no share of her own. Her `/w/{app}/{work}` answered 404 before this share and 200 during it.
- [stopped-sharing-with-everyone.png](stopped-sharing-with-everyone.png): after the stop, "Not shared with everyone" and no stop control. Sarah keeps "Can review all entries". Priya's work page is the 404 again.

## Limits

- This is one run after the fix, on one machine, in headless Chrome only. No Safari, no phone, no hosted fAuth.
- Diego is the work's creator, so all three roles are offered. The script did not reach a sharer without reviewer standing. The `share-routes` campaign covers that case: roles viewer and contributor only, and 422 `reviewer_not_allowed`.
- The "Remove access" control for one person is not shown. The server answers `can_revoke_members: false` until W4's owner-side revoke lands.
- The script did not open the canvas share entry point (`[data-manage-share]` on a placed app, or the united page's Share). Those open the same `openSharing` dialog.
