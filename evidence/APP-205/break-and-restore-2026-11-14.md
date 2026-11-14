# APP-205: the reference picker, break-and-restore proofs and checks (builder B3, 2026-11-14)

Branch `release/c-b1-rules` (B1 and B3 in one branch, as assigned), built on `release/c-participation` at `35d47c37`. The implementation commit under proof is `0f557305`. ADR-020 §6 ("The reference picker") and the clarifications 13–14 of §17 are what it builds.

## What was built

- `commands::FieldRow` carries `choices` (value = record id, label = the record's first text value, at most 100 characters, else "Record"; one selected) and `nothing_to_choose`; `commands::fill_references` fills every writable `reference` field from the work's listing (read under `own_rows`), resolving the field's `target_input` through the work's records bindings (`RecordDetail::bindings`, a new read in `read_record_detail`), newest commit first, at most 200, the current record (§3) preselected unless the field already holds an offered record.
- The canvas form card, the FMIR document form and the record page render the picker through the existing `has_choices` select (option text = the label) and an empty or unbound target as a disabled select, "Nothing to choose yet", that posts no value. The record page reads the listing only when its form writes a reference, and refuses (503) a listing whose pin differs from the detail's.
- Fields a command stamps with `set_now` are not in its `writable_fields` (admission), so they stay read-only and are never posted.
- The canvas client (`canvas/render.ts`, `work/render.ts`) rebuilds the same picker from the form JSON, and `canvas/commands.ts` skips disabled controls as a native submission does.

## Checks

- `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`: order, labels (null first text, 150-character text cut to 100), the selected record (first, or the held value), only the bound target's collection, unbound and empty targets, the 200 bound, a reader's view and a form writing no reference.
- `commands::tests::a_stamped_field_is_read_only_in_the_form`; `query::canvas_tests::a_form_card_carries_the_reference_picker_and_keeps_stamped_fields_read_only`.
- Binary SSR tests (`cargo test -p forma-app-platform --bin forma-app-platform`): the record page, the FMIR document form and the canvas form each render the picker with the current record selected and the label as the option text (escaped), and the disabled "Nothing to choose yet" select.
- Admin: `test/reference-picker.test.ts` (the client rebuild of the picker and the disabled select); the admin suite passes (329 tests) and typechecks.
- The `participation` campaign (Docker, `evidence/APP-203/`): a contributor's `respond` form offers every readable item newest commit first, the current item preselected and labelled by its prompt; the post form's stamped `opened_at` is read-only; the selection posted through the native form's own parse (`routes::parse_form_for_tests`) commits a typed reference storage accepts; and (judged under the bypass) a picker over an own list offers a contributor only their own sessions and a reviewer every member's, and a member who can read none gets the disabled picker.

## Break and restore

Each rule was broken on its own in a separate detached worktree at `0f557305`, the named tests run, the file restored with `git checkout --` and the tree required clean (byte-identical to `HEAD`) before the next break. Unit rows ran `cargo test --locked -p forma-app-platform --lib -- commands::tests query::canvas::tests`; client rows ran `npx tsx --test test/reference-picker.test.ts` in `app/admin`. The first pass named the canvas tests `query::canvas_tests` (the file name; the module is `query::canvas::tests`), which selects nothing: B1–B7 were still detected by `commands::tests`, while B8 and B9 passed unseen. Both were run again with the corrected filter and are detected; the rows show that run.

| Break | Rule | Break (file: before → after) | What failed (first panic) | Detected | Restored, tree clean |
| --- | --- | --- | --- | --- | --- |
| B1 | the picker lists only the target collection | `mod.rs`: `.filter(\|record\| record.collection == collection)` → `.filter(\|_\| !collection.is_empty())` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion `left == right` failed | yes | yes |
| B2 | at most 200 choices | `mod.rs`: `.take(REFERENCE_CHOICES)` → `.take(usize::MAX)` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion `left == right` failed | yes | yes |
| B3 | a label is at most 100 characters | `mod.rs`: `.map(\|text\| text.chars().take(REFERENCE_LABEL_CHARACTERS).collect())` → `.map(\|text\| text.chars().collect())` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion `left == right` failed | yes | yes |
| B4 | only a writable reference gets a picker | `mod.rs`: `.filter(\|row\| row.writable && row.kind == "reference")` → `.filter(\|row\| row.kind == "reference")` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion failed: shown.fields[2].choices.is_empty() && !shown.fields[2].nothing_to_choose | yes | yes |
| B5 | an empty target is nothing to choose | `mod.rs`: `row.nothing_to_choose = choices.is_empty();` → `row.nothing_to_choose = false;` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion failed: empty.fields[0].choices.is_empty() && empty.fields[0].nothing_to_choose | yes | yes |
| B6 | the held value stays selected | `mod.rs`: `choices.get_mut(held.unwrap_or(0))` → `choices.get_mut(0 * held.unwrap_or(0))` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion `left == right` failed | yes | yes |
| B7 | the target resolves through the work's bindings | `mod.rs`: `.and_then(\|input\| bindings.iter().find(\|(bound, _)\| bound == input))` → `.and_then(\|_\| bindings.iter().find(\|(bound, _)\| bound == "responses"))` | exit 101; `commands::tests::the_reference_picker_lists_the_readable_target_records_newest_first`; assertion `left == right` failed | yes | yes |
| B8 | the form card carries the picker as choices with labels | `canvas.rs`: `label: choice.label.clone(),` → `label: choice.value.clone(),` | exit 101; `query::canvas::tests::a_form_card_carries_the_reference_picker_and_keeps_stamped_fields_read_only`; assertion `left == right` failed | yes | yes |
| B9 | an empty target is a disabled picker on the card | `canvas.rs`: `let no_choices = field.writable && field.nothing_to_choose;` → `let no_choices = field.writable && false;` | exit 101; `query::canvas::tests::a_form_card_carries_the_reference_picker_and_keeps_stamped_fields_read_only`; assertion failed: nothing.no_choices && !nothing.has_choices && !nothing.has_text | yes | yes |
| B10 | the canvas client selects the server-marked choice | `render.ts`: `option.selected = choice.selected ?? choice.value === field.value;` → `option.selected = choice.value === field.value;` | exit 1; `the reference picker selects the marked record and an empty target is disabled`;  | yes | yes |
| B11 | the canvas client renders the disabled picker | `render.ts`: `if (field.no_choices) {` → `if (false && field.no_choices) {` | exit 1; `the reference picker selects the marked record and an empty target is disabled`;  | yes | yes |

Not broken by the script: the record page's pin comparison and the client's skip of disabled controls have no unit seam (the first needs a served page with a moved pin, the second a DOM form); both are small and named here for the reviewer.

## Limits

- The ticket's Playwright smoke on beta's clean profile was not run: this branch is not deployed and the builder has no deploy authority. The canvas campaign over HTTP was not extended; the picker's reads are qualified in the participation campaign and its rendering in the SSR tests.
- The picker inherits the listing's bound (§17.13): a target record outside the work's newest 200 current records is not offered.
- CI was not observed by the builder.
