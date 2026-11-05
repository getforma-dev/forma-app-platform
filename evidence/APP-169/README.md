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
