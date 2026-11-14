# APP-192 share dialog browser smoke, 2026-11-14 (W2b)

This is a one-off smoke, not a CI test. It is one Playwright script against the local dev stand of the `forma-w2b` worktree, by the W2b builder.

It ran three times:

1. The first full run found a CSS defect; it was fixed.
2. A second run followed the fix (run 1 below).
3. After the review's fix round and W4's revoke API, it ran once more, so the screenshots show the current wording and the working "Remove access" (run 2 below).

The screenshots are run 2's, except `dialog-before-fix.png`.

Everyone and everything on the screenshots is synthetic. The people are the dev stand's pitch personas: Diego Ramos, Sarah Chen and Priya Natarajan. The app is "Equipment requests", made from a first-party template. No secret, key, token or real person appears here. This is neither hosted acceptance nor Victor's browser pass.

## Run 2 (what the screenshots show)

Branch `trial/w2b-share-dialog` after W4's fix round (`126cd74c`) was merged and the personal revoke was wired, at `e90fc96a`.

1. `just dev reset -d` reset this worktree's own pitch profile. W4 edited the unreleased 0024 in place, so the old local database had to be rebuilt; its data was the earlier smoke's alone. It runs on port base 8900: Diego on 8901, Sarah on 8902, Priya on 8903.
2. The script ran: `PLAYWRIGHT_MODULE=<a separately installed playwright 1.66 index.mjs> OWNER_ORIGIN=http://127.0.0.1:8901 OTHER_ORIGIN=http://127.0.0.1:8903 REVIEWER_ORIGIN=http://127.0.0.1:8902 node w2b-smoke.mjs <this folder>`. The script is [w2b-smoke.mjs](w2b-smoke.mjs). It drives headless Chrome with a fresh profile.
3. [smoke.json](smoke.json) records six checks and zero page errors, on the first try.
4. `just dev stop` stopped the stand.

What the screenshots show:

- [dialog.png](dialog.png): "People in your organization" with Access "Can view", "Can add and edit entries" and "Can review all entries". Below that, "Everyone in the organization: Not shared with everyone", with no stop control.
- [reviewer-confirmation.png](reviewer-confirmation.png): choosing "Can review all entries" for Sarah Chen asks first: "Sarah Chen will see all entries, including private ones, and can take review actions." The buttons are "Yes, share" and "Cancel".
- `shared-with-a-person.png`: after "Yes, share", Sarah Chen is listed with "Can review all entries" and a "Remove access" control. Diego is the app's owner. Sarah's `/w/{app}/{work}` answered 404 before the share and 200 after it.
- `shared-with-everyone.png`: "Shared with everyone: Can view." with "Change access for everyone" and "Stop sharing with everyone". Priya's work page answered 404 before and 200 during.
- `stopped-sharing-with-everyone.png`: after the stop, the share with everyone is gone and so is the stop control. Priya gets the 404 again, and Sarah keeps her access.
- [removed-access.png](removed-access.png): "Remove access" for Sarah Chen. The dialog says "Sarah Chen no longer has access." and "Not shared with anyone in your organization yet." Her work page is the 404 again.

## Run 1 and the CSS fix

Run 1 was on branch `trial/w2b-share-dialog` at `e27aff78`, with the CSS fix recorded below. At that time Access read "Can add and edit their entries"; it now reads "Can add and edit entries".

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

### What run 1's screenshots showed (since replaced by run 2's)

- `dialog.png`: "People in your organization" with a Person picker (colleagues by name) and Access: "Can view", "Can add and edit their entries", "Can review all entries". Below that, "Everyone in the organization: Not shared with everyone" with no stop control, "Shared with: Not shared with anyone in your organization yet", and then the existing "Someone outside your organization" private-link section.
- `shared-with-a-person.png`: Sarah Chen shared with as "Can review all entries". The list shows her by name with that access.
- `shared-with-everyone.png`: "Shared with everyone: Can view. This includes people who join later." with "Change access for everyone" and "Stop sharing with everyone". Priya has no share of her own. Her `/w/{app}/{work}` answered 404 before this share and 200 during it.
- `stopped-sharing-with-everyone.png`: after the stop, "Not shared with everyone" and no stop control. Sarah keeps "Can review all entries". Priya's work page is the 404 again.

## Limits

- This is one run after the fix, on one machine, in headless Chrome only. No Safari, no phone, no hosted fAuth.
- Diego is the work's creator, so all three roles are offered. The script did not reach a sharer without reviewer standing. The `share-routes` campaign covers that case: roles viewer and contributor only, and 422 `reviewer_not_allowed`.
- Run 1 did not show "Remove access" (W4's revoke had not landed). Run 2 does.
- The script did not open the canvas share entry point (`[data-manage-share]` on a placed app, or the united page's Share). Those open the same `openSharing` dialog.
