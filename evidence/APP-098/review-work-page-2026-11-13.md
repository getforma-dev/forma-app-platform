# Independent review: APP-098 DS-84, the work page "Sample data" note

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-13. I did not write this change.
- **Reviewed revision:** `28a7da2` on `app098/work-page-sample-note`, in the `forma-work` worktree (`git show 28a7da2`, 5 files, +68/−3).
- **Protocol:** I read code and ran unit tests and the admin typecheck. I used no browser, no AWS, no dev stand and no campaigns. I made no edits other than this file.
- **Verdict: approve with changes.** The code is correct and its tests pass. Fix M1 in the ticket. m1 and m2 are recommended.

## Findings

- **M1 (major, ticket text).** At `Tickets/2-in-progress/APP-098.md:402`, the new 2026-11-13 log entry sits between the 2026-11-12 review entry (:401, "It requested changes, all fixed:") and that entry's sub-bullets (:403-410: B1 through M3, plus "Checks: `just campaign pitch`, `actions`, `canvas` and `mcp` passed"). Those sub-bullets now read as part of this slice, so the earlier review's fixes and four campaign passes are credited to the wrong change. Move the new entry to after :410.
- **m1 (minor, wording, for Victor's QA).** `app/admin/src/work/app.tsx:177` says "...not your own." Every catalog app has a records input (`decisions`, `claims`, `requests`, `notes` in `contracts/fixtures/library/catalog.json`), so the same page shows records the member filed, and those are the member's own. The shipped catalog views read only the source input, so "The numbers" holds for them. It can fail elsewhere:
  - `sample_works` (`storage/queries.rs:1697`) flags any work with a binding to a catalog source. A composed or adapted work can bind a sample source and still run views over a records input.
  - Data copy keeps the source bindings (`storage/data_copy.rs:322`) but points the views at editable records (`data_copy.rs:40-41`), so the flag stays set after the member edits the copy.
  - The canvas chip only says "Sample data". The new sentence makes a stronger claim than the chip.
  - Suggested wording: "Charts and tables here read Forma's synthetic sample data. Records you file are your own."
- **m2 (minor, ticket text).** The reason for leaving out the guest page (`APP-098.md:216`) is accurate only in its narrow form. `guest_work_reads` (`storage/shared_reads.rs:31-95`) does not read `genui.work_bindings`. But the same guest transaction already reads that table in `guest_run_query`, through `queries::bound_source` (`shared_reads.rs:128`, SQL at `queries.rs:691`) and `record_query::binding` (`shared_reads.rs:123`). So adding `sample_works` there would not give the guest role a new table. Deferring is still reasonable: the guest props, the template and a guest campaign check are new work. Reword the reason to say that.
- **n1 (nit).** The pitch check (`storage/pitch_qualification.rs:874-881`) reads `work_pages.last` without checking `props.work == adapted_work`. It is sound today because this is the campaign's only `/w/` GET and `RecordingPages.last` starts as `None`. It would pass without testing anything if an earlier work-page GET were added. Add the `work` check.

## Answers to the five questions

1. **Every route uses `page::props`.** Yes. All four renders in `query/routes.rs` call `page::props` with the outline from `document()` (:803), which calls `Data::work_reads` (`storage/actions.rs:322`, flag set at :343):
   - :970, the GET, both the HTML and the JSON forms;
   - :1303, the POST with selections and live refresh, both forms;
   - :1333, the 422 refusal;
   - :1366, the run without a cookie.

   The only `WorkProps {` literal is `query/page.rs:741`. `dev/mod.rs` and `auth/tests.rs` only bind the type or implement the trait. `release_document` (:900) compares the whole outline, `sample_data` included, against a second read from the same source. Nothing drops the flag.
2. **Contract.** The prop set is pinned only by `pages.bind::<WorkProps>("work")` at startup (`main.rs:192`, `dev/mod.rs:443`). The SSR test exercises that bind against the real dist (`main.rs:942`).
   - `admin/build.ts` maps routes to assets only. No contract in `contracts/` pins these props.
   - The JSON form uses serde field names, so it gains `sample_data`. `DocumentProps` (`admin/src/work/render.ts:35-38`) is a subset type with no strict parse, so the field is ignored.
   - Live refresh replaces `[data-document-parts]`, `[data-document-records]`, the title text and the presentation (`document.ts:122-131`). The note is outside all of these, so it stays, as the ticket says.
   - Bindings change in place only by `publication_id` (`data_copy.rs:322`, `work_presentation.rs:138`), so a stale note is not a practical risk.
   - The change adds no script, no inline style and no new response path. `.badge` is styled by `styles/app.css:236`, which the route loads.
3. **Correctness of the flag.** The work page and the canvas chip both come from `work_reads` and `sample_works`, and `work_outline` (`queries.rs:1125`) uses the same function, so the page always matches the chip.
   - Over-labelling: a binding to a catalog source id that no view reads, or other content an operator registered under a catalog id. The second is already listed under DS-84 "Remaining".
   - Under-labelling: synthetic fixtures outside the catalog. This is already listed in box 5. This change adds no new gap.
4. **Wording.** See m1. The placement under the lede, before "Open on the canvas", is reasonable.
5. **Ticket claims.** They match the code and the tests, apart from M1 and m2. The line cited, `pitch_qualification.rs:874`, is the new `assert!`. I did not reproduce the "forced off fails" run, because campaigns were out of scope.

## What I ran

| Command | Result |
| --- | --- |
| `cargo test --locked -p forma-app-platform --lib query::` | 46 passed, 0 failed, 1 ignored (`engine_child_campaign`, needs `GENUI_QUERY_PYTHON`), 400 filtered |
| `cargo test --locked -p forma-app-platform --bin forma-app-platform work_page` | 1 passed (`work_page_renders_every_view_state_without_a_fabricated_zero`) |
| `npm run -s typecheck` (in `app/admin`) | clean, exit 0 |
| `python3 Tickets/_tools/board.py check` | OK: 178 tickets; 9 phases; 15 sprints; 80 requirements |
| `strings app/admin/dist/work.f0bcea80.ir \| grep -i sample` | the note's text, `data-sample-data` and `sampleData` are present, so the bin test rendered the committed TSX |

Limitations: no campaign was run (`pitch`, `canvas` or `actions`), and nothing was checked in a browser.
