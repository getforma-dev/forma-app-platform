# APP-169 — durable classification and legacy placement

Date 2026-11-05. Prepared by Grok.

A member's Card or App choice is a row in `genui.work_artifact_kinds`, keyed by organization, principal, app and work. The row is visible only to that member. A work they cannot open is not listed, and the write is the uniform 404. Absence of a row, and an explicit `app` row, are both shown as an App. The server never counts components. A Card with several renderers stays one saved work.

`POST /v1/library/{app}/{work}/kind` with `{"kind":"card"}` records a Card. `{"kind":"app"}` writes `app`, which is the reversible step back. Make a card, Keep data and Show in Cards call that route. A new browser reads `data-artifact-kind` from the canvas page and does not need the old `forma.canvas.standalone.v1` entry.

## Legacy placement

`app/admin/src/canvas/stacks.ts` is still loaded from `app.tsx` (`startStacks`). Saved piles stay in this browser under `forma.canvas.stacks.v1`. The Library no longer places individual components; Browse and Open are whole-work views. `migrateAppViews` turns an old pile into one Stack or Open view and does not split the work. The admin suite, including `app-view-layout.test.ts`, passed with this source.

Rollback: leave `startStacks(document)` in place. Do not delete `stacks.ts` or the saved-work tables. Clearing a classification is the `app` write above; it does not delete the work, its grants, records or receipts.

## Checks on this source

- `app/runtime/.venv/bin/python scripts/check-schema-pins.py` printed `agree: true` for version 21 across the manifest, runtime, catalog include, managed-qualification include and pool literal. Exit 0.
- `cargo test --locked -p forma-app-platform --lib -- only_card_and_app_are_kinds a_card_kind_stays_one_work_and_an_empty_kind_stays_an_app` — 2 passed, 431 filtered out.
- `cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings` — exit 0. `cargo fmt -p forma-app-platform -- --check` — exit 0.
- `npm test` in `app/admin` — 272 passed, 0 failed.
- `npm run build` in `app/admin` (`tsx build.ts --ssr --strict`) — exit 0. `/canvas` gzip 244.2KB is over the 195.3125KB report threshold and under the 373,760 byte canvas JavaScript ceiling (214,949 bytes). That warning is the existing canvas budget report.
- `app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output evidence/APP-169/schema-campaign.json` — `"passed": true`, PostgreSQL 16.15, container removed. The new checks passed: the policy binds organization and principal; an unknown kind is `23514`; a write as another principal is `42501`; DELETE is `42501`; the writer stores `card` then `app`, and the other principal sees and changes nothing.
- Qualification `ServiceImageTests` for compiled sources and the service context, and operator `PackageTests.test_dockerignore_is_an_exact_allowlist_of_tracked_files` — OK. The new migration, catalog and `artifact_kind.rs` are on the service and qualification allowlists. The operator allowlist names the migration files.

## Browser

Worktree pitch only, port base 7000. Diego `http://127.0.0.1:7001/canvas`, Sarah `http://127.0.0.1:7002/canvas`. Ports 4001–4010 were not restarted. The server was stopped afterward; its data volume was kept.

Expenses is one work with 8 components. Show in Cards posted `{"kind":"card"}` and returned 200. After `localStorage` was cleared and the page was loaded again, that row's `data-artifact-kind` was `card`, it sat in the Cards category, and the control read Show in Apps. Sarah's canvas listed the same work in Finance with an empty kind and no card rows. Show in Apps posted `{"kind":"app"}` and returned 200. The next load put Expenses back in Finance with an empty kind and the same 8 components.

## Review and limits

[independent-review.md](independent-review.md) records no blocking findings. One note: the browser posts the kind and does not wait, and the `app` write is an UPDATE. An `app` write before any row exists changes nothing, which is the ambiguous App. A card insert that commits after that UPDATE can leave `card`. The saved work is not deleted.

Migration `0021_work_artifact_kinds.sql` is version 21 on this branch. Open PR 83 (APP-084) adds a different migration at version 21. One of those branches has to be renumbered before both can merge.

This is not a hosted or RDS run. No cloud change is in this ticket. Merging to `main` prepares a beta packet because `app/` changed, and that needs Victor's approval before any AWS change.

## Review fixes, 2026-11-05

Prepared by Claude on `app169/review-fixes` (PR 84 head `494073b` merged with `main` at `73f18d8`), commits `e82291e` and `188c426`. This section supersedes the race note under "Review and limits": `app` is now an upsert and the browser sends one write per work at a time.

- First load: a library row carries `data-artifact-kind="card"`, `"app"`, or no attribute when nothing is saved. The canvas carries `data-artifact-kinds="true"` once the kinds were read. The board keeps a local Card that has no saved row and uploads it once. Only a saved `app` drops a local Card. Without the page signal nothing local changes and nothing is sent. A failed kinds read renders the canvas without kinds instead of refusing it.
- Writes: a refused or failed write restores the last saved choice and announces it. The route spends the member write window, returns 503 when the session store is unavailable, decodes a closed body, and logs `work.kind_set`.
- Not changed here: the write is still not audited, and the policy is still the shared `tenant_scope` form. Both need the migration change. No backfill of existing ambiguous works and no retirement of `stacks.ts` or `startStacks` is implemented.

Checks on `188c426`:

- `just check admin fmt clippy test http-smoke`: all ok. Admin tests 277 passed, 0 failed. Rust library tests 405 passed, 34 ignored; binary 8 passed; doctests 3 passed.
- `npx tsx --test test/artifact-kind-board.test.ts test/artifact-kind-board-unread.test.ts` in `app/admin` with the previous `board.ts` and `layout.ts` restored: 4 failed. The first failure was `a local Card with no server row is kept`. With the fix all 4 pass.
- `cargo test --locked -p forma-app-platform --lib -- the_kind_route the_kind_body only_card_and_app_are_kinds a_card_kind_stays an_unreadable_kind`: 5 passed.
- `cargo test --locked -p forma-app-platform --bins -- canvas_page_renders_groups_cards_and_receipts_without_javascript`: 1 passed. It checks the `card` row, the row with no attribute, no `data-artifact-kind=""`, an `app` row, and the page signal present or absent.
- `just campaign canvas`: passed, including the `artifact kind (APP-169)` check. `just campaign query`: passed, and its store-outage check now includes `POST /v1/library/{app}/{work}/kind` (503).

Limit: a failing kinds read is covered by unit and SSR tests only. The campaign could not hold the table without also blocking new pooled connections' catalog check, and a privilege change trips the pinned catalog.

## Renumbered to 0022 and stacked on APP-084 (2026-11-05/06, Claude)

Both PR #83 and this PR claimed migration 21. APP-084 keeps 0021; this branch merges its review-fix branch (final head `a033d6b`) and moves the table to `0022_work_artifact_kinds.sql`, with every pin at 22. Two review findings land in the migration:

- **Audit (finding 3):** each change of kind writes one `work.kind_set` workspace-chain event in the same transaction; 0022 adds the kind to `workspace_chain` (0017's 22 kinds kept). The upsert returns a row only on a real change, so the same kind again writes nothing.
- **RLS (minor):** `tenant_scope` binds the organization and a RESTRICTIVE `work_artifact_kinds_principal` binds `genui_workspace` to the scoped principal, the 0014/0015 pattern.

`catalog-v22.json` was regenerated from the SQL in a fresh PostgreSQL ([receipt](catalog-generation-2026-11-05.json)); v1–v21 are byte-identical to the committed files, and v22 adds only the table, its two policies, its grants, the widened `workspace_chain` and the version checks.

Checks on merge `5f918f6` (re-checked after merging `a033d6b`):

- `qualify_tenant_schema.py`: passed, 395 checks, including both 0021 upgrade checks and the artifact-kind policy check ([report](schema-campaign-review-fixes-2026-11-05.json)).
- `just campaign canvas`: passed, including two `work.kind_set` events for app then card and none on a repeat ([report](canvas-campaign-review-fixes-2026-11-05.json)). `just campaign query`: passed ([report](query-campaign-review-fixes-2026-11-05.json)).
- Python runtime 206 tests, `app/qualification` 16, Rust library 405 and binary 8, admin tests, fmt and clippy `-D warnings`: passed. After merging `a033d6b`, the 36 admission-operator tests passed again.

Acceptance (b) and (c) were not implemented by this PR and moved to APP-176.

## Second review fixes (2026-11-06, Claude)

The second independent review of `a033d6b..1bf6dd9` found no blocker. Its findings are fixed on `app169/review-fixes`:

- Creation intent (Make a card, Keep data, a replaced Card work) stays a Card when its save fails. The next load uploads it.
- A Card the server has not confirmed is never evicted from this browser's 64-entry list by a confirmed one. Confirmed Cards are written first, so the bound drops them first. The page restores them on the next load.
- A failed older write no longer stops the member's newer choice, and the message names the newest choice.
- First-load uploads go one at a time, at most 16 per load, and stop at the first failure. They send `{"kind":"card","if_absent":true}`. The route accepts only that extra field, and only as `true`. Storage then inserts only where no choice is saved, and otherwise answers with the saved kind and writes nothing. A choice saved on another device after the page rendered therefore wins over the upload.
- ADR-007 D7.8 gained a dated `work.kind_set` amendment (a suffix only).

Checks:

- Each new board test was run before its fix and failed on that fix's own assertion: creation intent reverted; the oldest local Card was evicted; the member's newer App choice was never sent; uploads went out in parallel without `if_absent`. All pass after the fixes.
- `just check admin fmt clippy test http-smoke board`: all ok. Admin 281/281; Rust library 405 passed, 34 ignored; binary 8; doctests 3.
- `just campaign canvas`: passed. It now also checks that an `if_absent` write over a saved choice answers with that choice and writes no event, that one with nothing saved stores the kind with one event, and that operator `verify` succeeds with these events in the workspace chain.

Browser evidence: `docs/DEVELOPMENT.md` and `just campaigns` name no browser campaign for the Library or kinds. The uncatalogued APP-168 Playwright script `app/scripts/app_view_browser.mjs` toggles the classification but does not clear storage or use a second device, and Playwright is not installed in this checkout. The "Browser" section above ran on 0021 and the old merge code, so it is not evidence for the final head. Acceptance (a) is unticked until a fresh browser run on the final head.

## Browser evidence on the final head (2026-11-12, Claude)

Acceptance (a), "admitted durable kind survives new device/login while unauthorized work remains undiscoverable", on `main` at `5695869` (PRs #83 and #84 merged; schema 22). A fresh pitch instance from a clean worktree of that commit (`just setup`, `just dev -d`; port base 5600; its own Compose project, so nothing shared with any other checkout), driven in Chrome through the real UI.

1. **Before.** Diego (`127.0.0.1:5601/canvas`): `#cv-canvas` carries `data-artifact-kinds="true"`; all 20 Library rows carry no `data-artifact-kind`; Diego's member-scoped store `forma.canvas.standalone.v1:32f5442b…` is empty.
2. **Classify through the UI.** Library → the row "Budget vs actual" → its options menu → **Show in Cards**. The local store then holds that work (`…1401/…1404`).
3. **New device.** `localStorage.clear()` and `sessionStorage.clear()` on Diego's origin, then reload. The server renders the row with `data-artifact-kind="card"` (the only row with a kind), and the board adopts it back into a fresh local store. The choice survived losing every byte of browser state.
4. **Another member.** Sarah (`127.0.0.1:5602/canvas`, same organization, also granted that work): the row is present, carries **no** kind, and her own store `…:c7d20ddf…` is empty. A member's classification is personal.
5. **Undiscoverable.** From Sarah's page, `POST /v1/library/{app}/{work}/kind` for a work that does not exist and for an app that does not exist both answer `404 {"code":"not_found","status":404}`, the same uniform body the canvas campaign proves for a work without a grant; neither wrote a row.
6. **Database** (read-only `psql` in that instance's container): schema version `22`; exactly one `genui.work_artifact_kinds` row (Diego's principal, work `…1404`, `card`); exactly one `work.kind_set` workspace-chain audit event for that work.

Limit: one browser and one organization; the no-grant 404 for a real, ungranted work is proven by the canvas campaign (`canvas_campaign.rs`), not re-driven here.
