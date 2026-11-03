# PR40 theme review

This review covers the browser-local canvas palette and app Shape themes, against active trunk `07ea5aac85b69ad90d7bfb2274fb50395705e26f`. It does not change permissions or published app definitions. PR40's original two `.dockerignore` allowlist additions (`canvas/palette.ts` and `styles/themes.css`) remain deployment-contract changes requiring the normal reviewed promotion path.

The review repaired default-renderer markers incorrectly excluding every real app from Shape, restored Revert controls after reload, and made demo palette selection avoid both reading and writing browser preferences. Adopting an admitted presentation clears a browser preview; Shape cannot paint over that published design or report an unapplied change as successful. The dock has 44px touch targets. The narrow header keeps its brand separate from a scrolling navigation row, preserving Workspace, Connected apps and Sign out. Default-mode apps wearing a local dark palette now use that palette's readable busy/error colors; Stone's busy color was darkened to pass 4.5:1 on its sunken surface.

Qualification uses an owned clean stand on loopback 5210, a fresh headless browser context, synthetic local member identity, and real admitted model output for Shape. The Expense review library fixture is installed synthetic data, explicitly labeled as such. A separately created Dining template proves published-design refusal; it is not a hosted customer or signup test.

The retained browser packet records actual checks and their limits. The first attempt's app frame did not return after reload while repository compilation was running. No network capture exists for that first attempt, so its cause is not established. A subsequent run passed all theme behaviors but recorded auxiliary 503 responses; these remain availability observations, not discarded harness failures. Later final qualification and exact source hashes are recorded separately.

Manual screen-reader verification, another browser/device, account switching across providers, and hosted deployment are outside this packet. Native appearance/published-design tests remain separate from browser-local Shape persistence. No provider credentials, cookies, real recipient data or cloud actions were used.

## Observed final browser result

[Browser receipt](browser.json): 5 checks passed on the final asset manifest `9ffdb00e691895807fa74f92c3ffb915953b55f1d5e543bb43d343abe0c9a827`, 2026-11-02 10:42:23–10:42:49 UTC. There were no page errors or 5xx responses in that run. Four lineage lookups returned 404 for apps without folder lineage; no records or permissions were written by Shape. The separate Dining creation used the ordinary template picker and admission route.

- [Applied Harbor](app-harbor-dark.png) and [restored Revert](restored-revert.png): the same palette returns after reload; the second image intentionally captures real loading state with the corrected busy color.
- [320px](mobile-320.png), [390px](mobile-390.png), and [create dialog](mobile-create.png): no header/brand overlap or page overflow, all five dock controls at least 44×44. The navigation scrolls inside its own row; the 390px capture shows its right side after focusing Sign out.
- [Initial timeout](initial-reload-timeout.png) and [prior mobile overlap](before-mobile-header-fix.png) retain the earlier observations. The subsequent pass does not establish the earlier restoration failure's cause or prove that all availability issues are resolved.

[Hashes](hashes.json) bind the final source, screenshots, original private reports and executed harness. `browser.json` omits browser storage and synthetic locators; the retained PNGs contain only local synthetic data. All seven retained images were visually inspected.

To reproduce, start an owned non-demo clean profile, run its supported `seed-library` command, and then use an existing Playwright installation:

```sh
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
THEME_ORIGIN=http://127.0.0.1:5210 \
node evidence/APP-166/pr40-review/browser.mjs .cache/reports/theme-fresh
```

Use a fresh output directory. This sends one real model Shape request and creates one empty Dining template through the normal product controls. It does not submit a record command, invite, share or provider login. The reproduction harness differs from the executed harness only in configurable module/origin paths and the reserved-port guard; it refuses owner ports 4001–4010.

## Repository checks

[Check receipt](checks.json): all 21 `just check` steps passed. Final affected reruns after the last CSS corrections passed strict native admin build with 110 TypeScript tests, 375 Rust library tests, 8 native binary tests, 3 doctests and HTTP smoke. The 34 ignored campaigns remain explicit operator/campaign entry points; this review does not claim they ran. Board render/check and diff whitespace checks passed. An independent agent reviewed the final default/authored ownership, asynchronous Shape guards, Revert restoration, demo isolation and mobile CSS without a remaining blocker.
