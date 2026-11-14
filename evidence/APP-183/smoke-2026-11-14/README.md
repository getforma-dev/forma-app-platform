# APP-183 / APP-190 browser smoke, 2026-11-14 (W2)

**One-off smoke, not a CI test.** One run of a Playwright script against the local dev stand of the `forma-w2` worktree, by the W2 builder. Everyone and everything on the screenshots is synthetic: the dev stand's pitch personas (Diego Ramos, Sarah Chen, Priya Natarajan) in the fixture organization "Synthetic organization 0001". No secret, key, token or real person appears here. This is not hosted acceptance and not Victor's browser pass.

## What ran

The branch was `trial/w2-admin-cap` after W1's storage bodies were merged (`9950f140`, with `c9838fa1` for the routes campaign). The commands were:

1. `just dev -d` in the worktree. This is the pitch profile. Port base 8700: Diego on 8701, Sarah on 8702, Priya on 8703.
2. No dev persona holds `admin_policy`, so the builder made Sarah an organization admin with one insert into the worktree's own local dev database (the operator's `grant-admin` is W3's):
   `docker exec <worktree pitch db container> psql -U postgres -d genui -c "BEGIN; SET LOCAL ROLE genui_owner; SELECT set_config('genui.organization_id','00000000-0000-4000-8000-000000000001',true); INSERT INTO genui.organization_permissions VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-00000000006d','admin_policy'); COMMIT;"`
3. `PLAYWRIGHT_MODULE=<a separately installed playwright/index.mjs> node w2-smoke.mjs <output dir>`. The script is [w2-smoke.mjs](w2-smoke.mjs). It uses headless Chrome with a fresh profile and no personal browser profile. Its report is [smoke.json](smoke.json): three checks, zero page errors.
4. After a CSS fix found by step 3, one visual check: `node prompt-fix.mjs <output dir>` ([prompt-fix.mjs](prompt-fix.mjs)). It reported the banner box at x 434, y 84, 498 × 54, with no page errors.
5. `just dev stop`. The stand's data volume was kept.

## What it showed

- [invite.png](invite.png): `/invite` shows the beta notice and one required "I have read the notice and agree" checkbox. That is W1's page.
- [name-prompt.png](name-prompt.png): Priya had no stored name, so the canvas asked "What should others call you?". Saving "Priya N." returned to the canvas without the prompt. **This screenshot shows a layout bug:** the prompt was a grid item of the canvas shell and pushed the canvas out of its row. `3eb52928` fixed it.
- [name-prompt-banner.png](name-prompt-banner.png): after the fix, the prompt is a banner over the top of the canvas, shown here for Diego, who had no name.
- [admin.png](admin.png): Sarah's Admin link opened `/admin`. It lists 6 members, "Priya N." among them, and the feedback link. **This screenshot predates review fix m5 (`62f4e082`):** it still shows a Remove form on Sarah's own row ("Admin · you"). Since m5 the own row has no Remove; `org_admin_routes_campaign` asserts that.

## Limits

- One run, on one machine, in headless Chrome only. No Safari, no phone, no hosted fAuth.
- The admin grant is a local SQL stand-in.
- Two later review fixes are not on these screenshots: the closed name notice (m2) and the Cf name rule (m1). They are covered by `org_admin_routes_campaign` and the unit tests.
