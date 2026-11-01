# Composed app implementation — November 1

Victor explicitly prioritized this implementation and AWS delivery after the initial planning-only pass. The current branch is `codex/composed-app-implementation`, based on trunk `0fd3968`. This record separates implemented behavior, observed qualification and release status.

## Implemented behavior

- APP-155: one semantic document with an app header, introduction, ordered sections and responsive components across the member page, united canvas and authorized shared view. Existing definitions retain their default layout.
- APP-156: generic named actions target an admitted component, either navigating to it or opening it in a dialog. The action label and target are independent of app domain. Modal-only forms retain their original nodes, drafts and command identity. Guests receive only read-authorized targets.
- APP-157: persisted, bounded appearance/accent/density/typography tokens. The owner’s **Design app** editor changes sections, component order/placement, actions and theme through admission, immutable publication and a fenced adoption of the exact saved work. Records and sibling works are preserved.
- APP-158/159: admitted record lists, count/money summaries, monthly trends, rankings and honestly labeled preset budgets. Declared restaurant/date/enum filters operate on the whole bounded snapshot before list pagination or ranking. Exact money arithmetic, receipts, current authority and no-partial-total refusals are shared across member and guest routes.
- APP-160: an empty Dining tracker template uses those general components. The reproducible browser scenario creates it through normal product controls and submits real audited records; no seeded metrics, private source or special tenant branch supplies its numbers.

The extension is contract edition `impl-2026-11-01.2` and additive schema 19. Existing published definitions remain valid. Schema 19 adds only the narrow update privileges needed to adopt a presentation-only publication; admission forbids changing data contracts through that endpoint. Rollback requires a schema-compatible runtime or the reviewed database recovery path, not blindly restarting a catalog-18 executable.

## Qualification and delivery

Source implementation is present; final browser integration and release are in progress. The [contract and adoption evidence](../../evidence/APP-155/composed-contract-2026-11-01/README.md) records real PostgreSQL concurrency, stale/policy/deadline refusal, saved-record preservation and schema qualification. Final integrated evidence, exact candidate and actual deployment will be added after they pass.

Current review found and repaired a competing-save adoption race, unsafe fresh-key behavior after an ambiguous command failure, filter-scope retention on query failure, nullable ranking-group collisions and native transport binding mismatches. These are actual implementation findings, not planning tasks.

## Explicit boundaries

Actions currently support same-app navigation and component dialogs. Arbitrary script execution, arbitrary URLs, workflow automation, receipt-photo verification and CSV export are not implied. Budget values are admitted presets, not a fake editable budget record. Bounded record projections refuse workloads above 2,000 rows or 512 KiB rather than calculating misleading totals from a prefix.

APP-161 sleep/wake remains future work as originally requested. The real hosted recipient’s product email verification, full signup/share acceptance and three-minute recording remain distinct APP-139/140 obligations. Existing owner data and invitations must be preserved during this release. Older phase/board gates are not closed merely because these capabilities now have code.
