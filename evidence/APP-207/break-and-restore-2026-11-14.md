# APP-207: the composer generates the participation primitives (builder B5, 2026-11-14)

Branch `release/c-b5-composer`, built on `51810561` (`release/c-participation`, which integrates B0–B4 and release B's `main`). The decision is [ADR-020](../../docs/architecture/ADR-020-PARTICIPATION-PRIMITIVES.md) §10 (C7); its §17 items 28–32 are this builder's clarifications. Implementation commit `9c8fef16`.

## What changed, in one paragraph

A member can now type a participation app in plain words and the composer writes it with Release C's primitives. The generation prompt names edition `impl-2026-11-14.2`. It states the members `one_per_creator`, `requires` (with `via` and the server-time literal `now`), `set_now`, `countdown` and `record_ranking` by creator with `matches`. It also maps the member's words to those members ("once per question and can change it", "until it closes", "closes after 30 seconds", "a leaderboard on my screen", "only I grade"), lists ADR-020's "never" rules, and carries one general worked example: items, responses and expected, with no classroom words. Every generated definition still goes through admission whole, and model output stays data. The planner now admits a reference field when it points at a records list of the same app, and refuses one that points anywhere else. It refuses `multi_enum` until APP-208 lands. Each of the edition's 28 reference rules gets its own static fix on retry. One real defect was found and fixed on the way: two lists with the same fields (§10's `responses` and `expected`) were bound to the same collection, so the leaderboard would have scored every answer against itself.

## What plain words now produce

| Sentence | What the composer writes (stub model, admitted and run) |
| --- | --- |
| "a quiz where each student answers once per question and can change it until the question closes after 30 seconds, with a leaderboard on my screen" | A shared `items` list. The reviewer-only `post` (create) and `reopen` (update) stamp `opened_at` with `set_now`, and `close` is a reviewer-only update; none of them has a declared transition. An `own` `responses` list is keyed `one_per_creator: [item]` by a reference to `items`. Its `respond` create requires `{via: item, state eq open}` and `{via: item, opened_at gte now−30 s}`. An `own` `expected` list has a reviewer-only create. The views are a countdown (`opened_at` + 30, closed when state is closed), the items list, a response count, and a `record_ranking` with `group_by: "creator"` and `matches` against `expected`. The ids are general. |
| "attendance check-in once per session" | A shared `sessions` list, opened (`set_now: opened_at`) and closed by reviewer-only commands. An `own` `checkins` list is keyed `one_per_creator: [session]`. `check_in` requires `{via: session, state eq open}`, with no timer, and stamps `checked_at`. The views are a check-in count and check-ins per person (a ranking by creator, `count`). |
| "homework submissions, one per assignment, reviewer-only grade" | A shared `tasks` list with a reviewer-only post. An `own` `submissions` list is keyed `one_per_creator: [task]`, and its `submit` create has no `requires`, because the sentence has no closing words. `review` is a reviewer-only update that writes only `score`, never the key. |

## The interface

- `prompt::EDITION`, `prompt::participation_example()`, `prompt::PARTICIPATION_RULES` (28 `{message, code, fix}`), `prompt::participation_rule(detail)`, `prompt::build_generation_system(multi_enum)`.
- `plan::MULTI_ENUM_GENERATED` (false), `plan::require_usable_fields(content, multi_enum)`, `plan::require_bindable(content)`.
- New refusal codes: `participation_rule: <edition message>` (one per rule), `unusable_field`, `ambiguous_records_input`. `reference_field` is narrowed.
- `storage::composer_bindings`: a records input binds the schema with its own id when several share its fields (§17.28).

## Tests first

- The binding defect was red before the fix: `prompt::tests::the_participation_example_admits_binds_and_is_the_contract_specimen` failed with `left: [("items", "items"), ("responses", "responses"), ("expected", "responses")]`. The new storage unit test `records_inputs_of_the_same_fields_bind_the_schema_of_their_own_id` then went green with the fix.
- The other new tests were written against the planner and prompt in the same change, so their first red is the breaks below. Each one was broken alone and caught.
- New unit tests (`composer::prompt::tests`, `composer::tests`):
  - the example admits, binds and equals the specimen;
  - the prompt states the grammar, the plain-words mapping and the never rules, and the examples say nothing about permissions or classrooms;
  - `multi_enum` appears only behind the switch;
  - the 28 rules key unique static codes and fixes;
  - the three sentences compose through admission;
  - every rule broken once in a fixture gets its own code and fix, and the four schema-step refusals keep `encoding`;
  - a retry carries the code and fix and no document value;
  - `multi_enum` and ambiguous lists are refused;
  - the smoke's feature judgment holds for the example and fails for a plain list.
- The existing reference test now admits a reference to a records input and refuses one to a source input.

## What the composer campaign proves (stub model, real rows)

`just campaign composer` passes its 24 checks, three of them new. The new phase drives the real `Anthropic` client against a loopback Messages stub that answers each router call with `none` and each generation call with the fixture output.

1. All three sentences become generated definitions. Each is admitted, owner-published and saved on the member's authoring shell. Each generation request carries the participation prompt, with the sentence as JSON data. Each records input binds its own collection (`expected:expected,items:items,responses:responses`), and `responses` and `expected` are own collections.
2. The quiz runs end to end:
   - A shared-in contributor cannot post (`Denied`).
   - The owner's post stamps `opened_at` with the transaction's instant.
   - The contributor's answer `a` is filed. Answer `b` replaces it: the same entry at version 2, and the second form's id is never created. Answer `b` again is `Refused(Unchanged)`.
   - On the owner's page the ranking by creator shows one group with score 1 against the expected `b`, the count is 1, and the countdown is open. On the contributor's page the ranking shows only them, at 0.
   - The owner closes the item. The countdown reads closed, the next answer is `Refused(RequirementNotMet)`, and the ranking is unchanged.
3. Attendance and homework:
   - A second check-in replaces the first. A check-in after the owner closes the session is refused. The count per person is 1.
   - A resubmission replaces the submission. The contributor's score is `Denied` (reviewer-only), and the owner's commits at version 3.

## Break and restore

Each guard was broken alone by a script (`breaks_b5.py`, kept in the session scratchpad) against `9c8fef16`. For each break, the script:

1. replaced exactly one span of `HEAD`, after asserting the span was unique;
2. ran the command that should catch the break;
3. recorded what failed;
4. restored the file with `git checkout --`;
5. required the file's blob hash to equal `HEAD:<file>` and `git status --porcelain --untracked-files=no` to be empty before the next break.

S1–P2 ran in a first pass. Its P3 restore met a transient `git checkout` failure (exit 128). The file was restored by hand and checked byte-identical, and P3–E3 ran again in a second pass with a git retry. Every restore was byte-identical. The raw results are in [break-results-2026-11-14.json](break-results-2026-11-14.json).

| Break | Guard | Change | Command | Caught (exit, s) | What failed | Restored |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | a records input binds the schema of its own id | `app/src/storage/composer.rs`: the same-id preference in `bindings` → `.find(equal)?` (the first schema of the fields) | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 136) | `the_participation_example_admits_binds_and_is_the_contract_specimen`: `("expected", "responses")`, expected `("expected", "expected")`; also `plain_words_compose_participation_apps_through_admission` and the storage unit test | yes |
| S2 | the same, over real rows (the composed quiz's work bindings) | the same break | `just campaign composer` | yes (101, 49) | `composer` campaign: the quiz's work bindings `expected:responses,…`, expected `expected:expected,…` (the APP-207 phase's first check) | yes |
| P1 | a generated reference must target a records input of the content | `app/src/composer/plan.rs`: `require_usable_fields` accepts any reference target | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 55) | `which_x_is_an_enum_or_text_…`: a reference to a source input was admitted | yes |
| P2 | a reference to a records input is no longer refused (ADR-020 C7) | `plan.rs`: `require_usable_fields` refuses every reference (the APP-193 rule) | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 34) | `the_participation_example_…`, `plain_words_…`, the retry test, `multi_enum_waits_…`, `which_x_…`: `reference_field` where the example admits | yes |
| P3 | multi_enum is refused in generated content until APP-208 | `plan.rs`: the `multi_enum` arm fires only when generated content may carry it | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 44) | `multi_enum_waits_for_app_208_…`: the multi-select fixture was admitted | yes |
| P4 | ambiguous records inputs are refused | `plan.rs`: the `require_bindable` call removed from `plan_generated` | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 29) | `multi_enum_waits_…`: the renamed lists (`response`, `expected_choice`) were admitted | yes |
| P5 | an edition rule's refusal carries its own static code | `plan.rs`: `From<AdmissionError>` never maps a message to its rule | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 29) | `each_participation_rule_is_refused_…` and the retry test: `encoding`, expected the rule's code | yes |
| H1 | the hint is the rule's fix | `app/src/composer/prompt.rs`: `hint` never looks a rule's code up | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 25) | `participation_rules_key_…` and `each_participation_rule_…`: the generic hint, expected the rule's fix | yes |
| H2 | only the whole message after the path matches a rule | `prompt.rs`: `participation_rule` matches a message anywhere in the report (`contains`) | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 23) | `participation_rules_key_…` and `each_participation_rule_…`: a longer report matched another rule (`now requires a timestamp field`) | yes |
| G1 | the prompt states the never rule for set_now | `prompt.rs`: "a set_now field in writable_fields; " removed from the NEVER line | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 23) | `the_prompt_states_the_participation_grammar_and_the_never_rules`: the NEVER rule missing | yes |
| G2 | the prompt states requires in the command grammar | `prompt.rs`: `requires?: [guards]` removed from the commands grammar | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 20) | `the_prompt_states_…`: `requires?: [guards]` missing | yes |
| G3 | the prompt omits multi_enum until APP-208 | `prompt.rs`: `generation_system()` built with `multi_enum` on | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 18) | `multi_enum_is_stated_only_…` and `the_prompt_states_…`: the prompt said multi_enum | yes |
| E1 | the example's post is reviewer-only | `prompt.rs`: the example's `post` without `reviewer_only` | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 19) | `the_participation_example_…` (commands differ from the specimen), `plain_words_…`, `the_smokes_participation_features_…` | yes |
| E2 | the example omits a zero offset (the countdown's 30 s) | `prompt.rs`: the example's countdown `offset_seconds: 30` → `0` | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 19) | five tests: the example refused `encoding` (an explicit zero offset) | yes |
| E3 | the example carries no classroom word | `prompt.rs`: the example's note says "a question" | `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | yes (101, 25) | `the_prompt_states_…` and `the_participation_example_…`: a classroom word in an example | yes |

The edition's own refusals are judged by admission (B0, APP-202). They are not broken here. What this ticket proves is that each one reaches the model as its own static fix (P5, H1, H2).

## The real-model smoke (acceptance 4)

The command was `GENUI_ENV_FILE=<the main checkout's .env> app/scripts/composer_smoke.sh`. That wrapper exports the one key into the test process only, and the key is never printed. It runs `composer::tests::real_model_smoke` with `claude-haiku-4-5-20251001` for both steps. The acceptance sentence runs first. Both runs, with their admitted definitions, are in [smoke-composer-2026-11-14.json](smoke-composer-2026-11-14.json).

| Run | Prompt | Acceptance sentence | Features (own list keyed by a reference; `requires` with `via` and `now`; `set_now` on a reviewer-only create and update; countdown; ranking by creator with `matches`) | General ids | Other steps |
| --- | --- | --- | --- | --- | --- |
| 1 | `9c8fef16` | admitted on the first attempt (`refused=[]`), 9 views, 14.2 s | all present | **no**: `questions`, `question`, `answers`, `correct` | both routings `high`; traffic generated after one `encoding` retry; reading list with no transitions |
| 2 | plus the general-ids line ("never build an id from the member's domain words") | admitted on the first attempt, 9 views, 13.8 s | all present | **no**: `questions`, `question` (it now uses `prompt` and `expected`) | as run 1 |

The miss was fixed in the prompt and the smoke re-run once, as acceptance 4 allows. The functional shape held both times. The ids still carry the member's word, so the smoke reports `missed ["general ids"]` and exits 101. Nothing in planning enforces general ids, because an id is data and grants nothing. It stays a prompt matter, recorded under the limits.


## Checks

| Command | Tree | Result |
| --- | --- | --- |
| `cargo test --locked -p forma-app-platform --lib -- composer:: storage::composer::tests` | `9c8fef16` | 102 passed, 2 ignored (the two real-model smokes). One earlier run failed `composer::matcher::tests::every_example_picks_its_app_and_no_negative_does` at its p95-latency assertion under load. That test is untouched here; it passed alone, passed at the base, and passed in the next two full runs. |
| `cargo clippy --locked -p forma-app-platform --all-targets [--features dev-stand] -- -D warnings`, `cargo fmt --all --check` | `9c8fef16` | clean |
| `just campaign composer` | the tree committed as `9c8fef16`, before the general-ids prompt line | passed, 24 checks, the three APP-207 checks among them ([report](composer-campaign-2026-11-14.json)) |
| `just check` | `2abdbd58` | 19 of 21 steps ok: fmt, admin, clippy, clippy-dev, test (488 lib tests: the composer, prompt and admission suites and every database-free test), http-smoke, msrv, catalog, infra, py-scripts, shared-broker, py-qualification, py-app-scripts, py-ops, py-infra, schema-pins, contracts, board, bash-n. **py-promotion** failed as expected: `test_committed_release_is_bound_to_this_tree`, the release-B record's binding on this branch. **py-runtime** failed only because Docker Desktop went down during the run: `test_admission_operator.DatabaseTests` and `test_provider_invitations.ProviderDatabaseTests` setUpClass, "Cannot connect to the Docker daemon". |
| `python3 Tickets/_tools/board.py render`, `… check` | `2abdbd58` | OK: 208 tickets |
| `just campaign composer`, `just campaign participation`, `just campaign actions` | `2abdbd58` | **not run**: Docker Desktop was down, and the coordinator stopped the wait. CI's tenant-foundation workflow runs them. |

## Limits

- **Campaigns at the head.** The `participation` and `actions` campaigns were not run by this builder, and `composer` was not re-run after the prompt's general-ids line. That line changes only the prompt text the stub ignores; the campaign's request check ("PLAIN WORDS TO MEMBERS") still holds. All three are left to CI's tenant-foundation workflow. `py-runtime` needs a re-run with Docker up.
- **The real model's ids.** Haiku named the quiz's lists from the member's words in both smoke runs (`questions`, `question`). The functional shape was right, and admission and storage treat ids as data. A stronger prompt, or a planner rule, is a follow-up if Victor wants general ids enforced.
- **`multi_enum`.** It is not in generated content until APP-208 sets `plan::MULTI_ENUM_GENERATED`. The grammar line and the planner switch are in place and tested both ways.
- **Out-of-row changes.** `app/src/storage/composer.rs` (§17.28), which needed one `find` and a unit test. `composer/mod.rs` was not touched (§17.29).
- **Not covered here.** The UI (B3) for reviewer-only forms on a contributor's page, and how the canvas lays out a composed participation app. A form whose command the member may not run renders without writable fields. Victor's browser QA covers these.
- CI was not observed.
