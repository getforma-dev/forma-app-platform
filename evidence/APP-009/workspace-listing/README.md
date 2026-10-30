# APP-009 follow-up: the workspace listing (APP-076 G9, APP-012 routes OQ5)

Controlling text: [ADR-010](../../../docs/architecture/ADR-010-NOV-2-MVP-CUT.md) §1 act 1 ("the home page listing the admitted app and saved work") and acts 2/3 (the "seeded by the operator" and "seeded definition" labels); [ADR-006](../../../docs/architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) (`GET /workspace` is the protected entry, uniform 404 for every refusal); [ADR-007](../../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) D7.3 grants and the 2026-10-29 creator-authority amendment; [APP-076 RUNBOOK](../../APP-076/prep/RUNBOOK.md) gap G9.

Commit: `895f530` (on the worktree branch, after `dc78707`, which fixed G7/G8).

## What changed

| Part | Where | What |
| --- | --- | --- |
| Storage read | `app/src/storage/queries.rs` `Data::openable_works` | One READ COMMITTED transaction on the data role, scoped to the session's organization and principal (RLS `set_config`), deadline 5 s. The member fence first (`Transaction::fence_member`: active organization and membership at the request's epoch, active entitlement, `lock_member_authority`, entitlement again). Then at most 50 of the organization's saved works, each kept only when `discover` on its app **and** `record_read` on the work hold, judged by `Permit::also` with the same permission clauses the work page (`work_outline`) and the record page (`read_record_detail`) apply: `record_read` includes `run` on the app, live `source_read` on every dependency (no deleted revision), and the unrevoked creator's authority (ADR-007 amendment). A kept work's pinned publication is read under `lock_work_revision` and its stored definition re-admitted for the title, exactly as `work_outline` does. `record_write` decides whether record links appear. Nothing is written. |
| Fence | `app/src/storage/authorization.rs` `fence_member` | The member half of `precheck` + `fence` with no resource permission of its own; the permit names the organization with `Discover`, which `Permit::publication` and `Permit::record` refuse, so it authorizes nothing by itself. `precheck` and `fence` now share `current_member` and `lock_member` (no behavior change). |
| Route | `app/src/auth/routes.rs` `workspace`, `listing`, `workspace_item` | After `resolve_session` returns a live request: the listing. A denial, an empty result or a store failure (logged as `workspace.listing_unavailable` with its code only) is the empty listing, so the entry and its sign-out stay reachable and nothing about a refusal is shown. Every session refusal is still the uniform 404. |
| Page | `app/admin/src/workspace/app.tsx` | Native SSR, no JavaScript. Each work: a plain link to `/w/{app}/{work}` with the definition's title and the label "Seeded definition, retrieved from the library. Sources seeded by the operator." (the work page's wording); with `record_write`, one link per record collection, "New {schema} record", to `/work/{app}/{work}/records/{collection}/{record}` (the APP-013 route in `commands/routes.rs`) with a fresh UUID v4 record id per render (`capability::random_uuid`), which the record page shows at version 0 with its `create` form. Empty: "Nothing to open yet." No organization picker. |

## Results

| Command | Result |
| --- | --- |
| `app/admin`: `npm ci && npm run build && npm run typecheck` | exit 0. `workspace.ir` 648 B → 1,500 B, 0 islands; `/workspace` "no interactive region, no JavaScript". `/` Forma 26,514 of 28,420 B, JavaScript 28,168 of 76,800 B (before: 26,515 / 28,172). |
| `cargo +1.93.1 fmt --check`; `clippy --locked --all-targets -- -D warnings`; `test --locked`; `cargo +1.85.0 check --locked --lib --tests` | all exit 0; `test`: 130 lib + 5 binary + 3 doc tests pass, 13 ignored (the native campaigns). The binary test `auth_pages_render_typed_props_without_javascript` renders the empty listing (the neutral line, no list) and a two-item listing (both links, the record link, the seeded label once per item, a title with `<…>` escaped as text, no script). |
| `auth::tests::native_session_campaign` (`GENUI_SESSION_REPORT`) | `passed: true`, **20 checks** (19 before), container removed: [session-campaign.json](session-campaign.json). New check 5b, through the operator's own `grant-permission`: discover and run alone list nothing; `record_read` on the seeded Northstar work `…0069` lists nothing until `source_read` holds on both its dependencies (`…006e`, `…0070`); then exactly `Northstar investigation` → `/w/…0065/…0069` with no record link; with `record_write` one "New decision record" link to `/work/…0065/…0069/records/…006a/{uuid v4}`, a different id on each render; deleting one dependency's `source_read` row (fixture SQL, no epoch advance, so the same session shows that each request re-reads it) unlists the work, and restoring it lists it again; nothing of the canary organization (Spend Review `…00c9`) is listed; grants advance no epoch. |
| `auth::invite_tests::native_invite_campaign` | `passed: true`, 13 checks; route-family check (item 2/5) unchanged with the tightened `uniform_404` predicate. |
| `storage::command_qualification::native_command_campaign` (21 checks), `storage::query_qualification::native_query_campaign` (`GENUI_QUERY_REPORT`, 20 checks), `storage::race_qualification::native_race_campaign` (26 checks) | all `ok`, containers removed. |
| `app/runtime/smoke_http.py --binary app/target/debug/forma-app-platform` | passes (`protected_uniform_404: true`, 7 native pages, fresh nonces, no-store). |

## Notes

- Spend Review works carry their decision collection, so the "New decision record" link is the way into act 6 once the member holds `record_write`; the APP-076 trial still types its recorded record ids (`…13b1`, spares `…13b2`/`…13b3`), since a link mints a new id per render.
- The listing does not show existing records of a collection (no records index exists in Phase 1); it shows only the record page of a fresh record.
- An app the member may discover but with no openable work is not listed: there is no app page to open.
