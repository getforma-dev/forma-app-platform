# APP-213: a replacing entry keeps a reviewer's work; generated answer keys are private (builder B8, 2026-11-14)

Branch `release/c-b8-replace-guard`, built on `3bd4319b` (`release/c-participation`, the integration head of B0–B7). The decisions are [ADR-020](../../docs/architecture/ADR-020-PARTICIPATION-PRIMITIVES.md) §17.43 (fix 1) and §17.44 (fix 2), taken by the coordinator in lieu of Victor from the Release C authorization review's MAJOR-1 and its residual. The implementation and the ADR items are commit `34645237`; the ticket and this record follow it.

## What changed, in one paragraph

**1.** A `one_per_creator` create that replaces the creator's existing entry composed fresh, so every field the create did not write went back to its `initial` value or null: a reviewer's `reviewer_only` score, or a status a reviewer moved by transition, was undone by the creator's next submission, through every commit path. Now `commands::commit` and `Data::dry_run_command`, after the key match and the `Unchanged` check, compare every published field the create neither writes (`writable_fields`) nor stamps (`set_now`) between the matched entry's stored values and the composed state (`commands::untouched_fields_differ`, the typed equality `Unchanged` uses, field by field). If any differs the command is refused on the existing `Error::Denied` path and nothing is written; the fresh form's record id is never created. Before a reviewer acts both sides hold the initial or null, so a resubmission replaces as before; a create that writes every field (the quiz) and a `set_now` stamp (attendance) never lock; the reviewer's own update on the entry is unchanged. **2.** The planner refuses generated content whose `record_summary`/`record_ranking` `matches` `key_input` binds a record schema without `visibility: "own"` (`plan::require_private_keys`, code `shared_matches_key`, after `require_bindable` and through its binding rule, `bound_schema`); the hint is static and the prompt's NEVER line names the shape. Stored content admits as before.

## The interface

| Piece | File | What it does |
| --- | --- | --- |
| The rule | `app/src/storage/commands.rs` | `untouched_fields_differ(spec, declaration, entry, composed) -> Result<bool>`: decodes the matched entry's canonical bytes (`composed_state`; bytes that do not decode are `Unavailable`), filters the published fields to those in neither `writable` nor `set_now`, and answers whether any differs from the composed value (null when absent on either side). |
| The commit | `app/src/storage/commands.rs` (`commit`) | After `keyed_entry`, the `Unchanged` check and the creator check: `require(!untouched_fields_differ(..)?)?`, so a locked entry is `Denied` before any row is written. |
| The dry run | `app/src/storage/actions.rs` (`dry_run_command`) | The same call at the same point, so the action path's confirmation card is `could_not_draft` for the same command the commit would refuse. |
| The planner | `app/src/composer/plan.rs` | `bound_schema(schemas, input)` (the binding rule `require_bindable` and `require_private_keys` share); `require_private_keys(content)` judged in `plan_generated` after `require_bindable`. `plan_adapted` and `admit` are unchanged. |
| The prompt | `app/src/composer/prompt.rs` | `hint("shared_matches_key")`; the NEVER line's last item. |

What the member sees for the refusal is what any value a declared rule refuses shows: the record page's `Notice::Refused` and the canvas's "The command was refused: a value is not one this command accepts. Nothing was changed." (`admin/src/record/app.tsx`, `admin/src/canvas/commands.ts` `COPY.refused`); JSON callers the uniform 404; the action path `could_not_draft`; MCP not-found; Slack its refusal. No new wire code, contract, edition, migration, SQL or client change.

## Tests first

The unit tests and the campaign step were written with the code and run before any proof. All were green on the first run:

- `storage::commands::tests::a_replace_differs_exactly_when_a_field_outside_the_creates_reach_moved`: before a reviewer acts, `Ok(false)` (a resubmission replaces); after the reviewer's update wrote `score`, `Ok(true)`; a create writing every field, `Ok(false)`; a `set_now` stamp re-stamped forty seconds later, `Ok(false)`; bytes that do not decode, `Err(Unavailable)`.
- `composer::tests::a_generated_matches_key_must_be_an_own_list`: the worked participation example plans; with `visibility` dropped from `expected` it still admits as stored content, `require_private_keys` and `plan_generated` refuse it with `shared_matches_key`, the hint differs from the unknown one and names `visibility: own`; `simple_records_example` has no key to judge.
- `composer::prompt::tests::the_prompt_states_the_participation_grammar_and_the_never_rules` (the NEVER item) and `participation_rules_key_static_codes_and_fixes_on_the_editions_messages` (the hint).
- The composer campaign's homework phase (`storage/composer_qualification.rs`, `participation_phase`), the APP-213 check: the dry run of the first resubmission names the entry it replaces (`DryRun::Valid { replaces: Some(first) }`) and the commit replaces it (revision 2); the owner scores it (revision 3); a resubmission after the score is `Err(Denied)` from the dry run and from the commit; the entry is still at revision 3 with answer `final` and score 9; the refused form's record id has no row in `internal_records`; the entry has exactly three rows in `internal_record_revisions`; the owner's rescore commits revision 4.

## Break and restore

Every guard was broken alone by a script (`breaks_b8.py`, kept in the session scratchpad; its results are [break-results-2026-11-14.json](break-results-2026-11-14.json)). For each break the script:

1. required the file's blob hash to equal `HEAD:<file>` and replaced exactly one span of `HEAD` (`34645237`), after asserting the span was unique;
2. ran the command that should catch the break;
3. recorded the exit code, the seconds and the failing assertion;
4. restored the file with `git checkout --`;
5. required the blob hash to equal `HEAD:<file>` again and `git status --porcelain --untracked-files=no -- app` to be empty before the next break.

All four ran at `34645237`; every one was caught and every restore was byte-identical. S2 and S3 are caught over real rows by the campaign, which is the proof that the campaign's APP-213 check depends on each call site and not on the fixture.

| Break | Guard | File: change | Command | Caught (exit, s) | What failed | Restored |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | the rule: a field outside the create's reach that moved locks the entry | `app/src/storage/commands.rs`: the `.any(..)` over the untouched fields → `.any(..) && false` | `cargo test --locked -p forma-app-platform --lib` `a_replace_differs_exactly_when_a_field_outside_the_creates_reach_moved` | yes (101, 15.4) | `a_replace_differs_exactly_when_a_field_outside_the_creates_reach_moved` at `commands.rs:2621`: "once scored, the entry is locked for its creator", left `Ok(false)`, right `Ok(true)`. | yes |
| S2 | the commit refuses a replace that would undo a reviewer's work | `app/src/storage/commands.rs`: the `require(!untouched_fields_differ(..)?)?;` before `target = entry.record;` removed | `just campaign composer` | yes (101, 28.9) | `native_composer_campaign` at `composer_qualification.rs:2295`: "the commit refuses a resubmission of a scored entry" (the commit replaced the scored entry). | yes |
| S3 | the dry run refuses the same replace | `app/src/storage/actions.rs`: the `require(!untouched_fields_differ(..)?)?;` before `replaces = Some(..)` removed | `just campaign composer` | yes (101, 37.8) | `native_composer_campaign` at `composer_qualification.rs:2287`: "the dry run refuses a resubmission of a scored entry" (the dry run answered `Valid` with `replaces`). | yes |
| P1 | the planner refuses a generated matches key list that is not own | `app/src/composer/plan.rs`: `!= Some("own")` → `== Some("shared")` (a spelling admission never admits, so the rule fires on nothing) | `cargo test --locked -p forma-app-platform --lib` `a_generated_matches_key_must_be_an_own_list` | yes (101, 21.6) | `a_generated_matches_key_must_be_an_own_list` at `tests.rs:1332`: `require_private_keys(&shared).unwrap_err()` panicked on `Ok` (the shared key list was planned). | yes |

## Commands and results

Local macOS, toolchains from `just setup` in the worktree `forma-relc-b8` (plus `npm --prefix app/admin run build`, which the lib tests' embedded assets need in a fresh worktree), Docker Desktop up. All at `34645237`, each run once after the break proofs had restored the tree. CI was not observed.

- `cargo test --locked -p forma-app-platform --lib`: 503 passed, 40 ignored (the campaigns and smokes), 0 failed; 17 s. The new tests are `storage::commands::tests::a_replace_differs_exactly_when_a_field_outside_the_creates_reach_moved` and `composer::tests::a_generated_matches_key_must_be_an_own_list`; the two prompt tests named above gained one assertion each.
- `cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings`: clean, 66 s. The same with `--features dev-stand`: clean, 57 s.
- `cargo fmt --check -p forma-app-platform`: clean.
- `just campaign composer`: passed, 25 checks (was 24: the APP-213 check), 21.5 s. Report: [composer-campaign-2026-11-14.json](composer-campaign-2026-11-14.json).
- `just campaign participation`: passed, 20 checks, 82.0 s. `just campaign private-entries`: passed, 60 checks, 27.6 s. Neither changed: the quiz's `responses` and the private-entries apps have no field outside their creates' reach that another command moves, so no replace of theirs is refused.
- `python3 Tickets/_tools/board.py render`, then `check`: "OK: 210 tickets; 9 phases; 15 sprints; 80 requirements; design digests and active links verified."
- The break proofs above (`breaks_b8.py`): four breaks, 15.4 + 28.9 + 37.8 + 21.6 s, all caught, all restored.

## Limits

- **No independent review yet.** The ticket's last acceptance item (the integration into `release/c-participation` and Victor's look on his build day) is open. A reviewer should read `untouched_fields_differ`'s filter (writable or stamped fields are the only ones excused) and the two call sites' position after the `Unchanged` check.
- **The member reads the general refusal.** A locked entry answers the same text as any value a declared rule refuses ("The command was refused: a value is not one this command accepts. Nothing was changed."). A worded refusal ("a reviewer has already acted on your entry") is a new 422 problem code and a contract edition; ADR-020 §17.43 records why it was not opened here and why `requirement_not_met` was not borrowed.
- **Locked means locked until a writer moves it back.** A reviewer who clears a score (sets it to null) unlocks the entry for its creator; one who scores it keeps it locked. A definition author who wants students to resubmit after grading needs a command that writes `score` or an unlock the platform does not yet offer.
- **The planner judges the key's schema only by `visibility`.** The hint also asks for a `reviewer_only` create on the key list; a generated `own` key list with a contributor-open create is admitted (each contributor would file their own private key, which scores nothing of anyone else's). Refusing it would be a further rule.
- **Browser look.** No browser ran the refusal here; it renders through the existing refused notice of the record page and the canvas form. Victor's QA sees it on his build day.
- **CI was not observed** by the builder (no PR; the branch is pushed for the integration).
