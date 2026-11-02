# Composed app browser checks

Run these repository test scripts only against an owned disposable loopback stand. Ports 4001–4010 belong to the owner and are excluded by the palette/guest runners. Use installed Playwright and Chrome; these scripts do not download a browser, create provider accounts or use a personal browser profile. Local dev personas are synthetic identities, so a pass does not establish hosted signup acceptance.

The member runner `composed_app_browser.mjs` creates the Dining and unrelated Equipment reference apps through ordinary product controls and writes `fixture.json` under its chosen `--output`. Preserve that fixture and every interrupted mutation journal. Reconcile unknown results before retrying a product mutation. The guest runner `composed_guest_browser.mjs` uses a fresh synthetic named share and verifies actual commands, live views, receipts and revocation; use a fresh output directory for each new live-update qualification.

The palette runner reads the completed member fixture **from the explicit argument**, so it does not depend on a particular task cache. It makes no command or presentation writes: it opens each existing app, uses Unite, opens the generic form, types an unsent draft, then moves that same component through Fan out and Focus. It measures rendered colors and text contrast for the dark Dining and default-light Equipment apps, including platform header buttons/links/origin badges, inputs, labels and a focused record receipt. The record-components POST endpoint is an authoritative **read query**, and is counted separately only for the exact two fixture works. Any other unsafe request is blocked and fails the check. Service workers are disabled in this read-only palette probe; it does not qualify service-worker/offline behavior. The component node, unsent value and idempotency key must remain the same.

From the repository root, after the strict admin build and owned stand are healthy:

```sh
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs node app/scripts/composed_palette_browser.mjs --fixture=/path/to/member-run/fixture.json --owner=http://127.0.0.1:4801 --output=.cache/reports/palette-new
```

`--help` prints the invocation without opening a browser. The palette runner refuses an output directory that already contains a report. Reports include the input fixture hash and before/after asset identity; retain the exact source/build and inspect the screenshots. An asset rebuild during a run invalidates that run. A palette/keyboard pass is not a manual screen-reader pass or a complete cross-platform accessibility claim.
