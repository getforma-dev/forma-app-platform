# APP-197: the recheck's fixes (2026-11-14)

By Claude (Opus 5.5), on `trial/app-197-catalog-cadence`, from `48e342e6`. This answers the recheck appended to [review-tier-a-2026-11-14.md](review-tier-a-2026-11-14.md): one MAJOR (R-M1), one MINOR (R-m1), and three NITs: two corrections to the A5 text and a forward note.

## R-M1: the baseline check runs first as well as last

**The defect.** The previous round moved `verify_baseline` from first to last on reuse. That let a reused session run its `verify_identity` statement before its `search_path` was known to be clean. The identity statement compared names with an unqualified `=`, so a planted `=` in a shadowing schema ran.

**The fix** (`app/src/storage/pool.rs`):

- **Order.** `recycle` runs `verify_baseline` first, before anything else on the session, and again last. Between them run identity, the version relation's identity, the schema version and, once the window has passed, the catalog comparison.
- **Qualified operators.** The two identity statements and the baseline statement are now consts: `IDENTITY`, `MEMBERSHIP` and `BASELINE_CHECK`. Every operator in them is written `OPERATOR(pg_catalog.…)`: `=`, `<>`, `>` and `||`. `rolname NOT IN (current_user,'rds_iam')` became two `OPERATOR(pg_catalog.<>)` comparisons, because `NOT IN` expands to a search-path-resolved `<>`. The baseline statement contains no operator.
- **A static test.** `statement_tests::unverified_session_statements_resolve_no_operator_through_the_search_path` removes string literals, casts and `OPERATOR(pg_catalog.…)` from the three statements. It then asserts that no operator character is left, and no keyword that expands to an operator (`IN`, `LIKE`, `BETWEEN`, `ANY`, `DISTINCT`, `CASE` and others). `ALL` is allowed only in `UNION ALL`.
- `verify_identity` is `pub(super)`, so the campaign can call it directly.

**A finding while testing.** A new backend never sees a planted database default. `connect` fixes `search_path=pg_catalog, pg_temp` in its startup options, and those win over database and role defaults. So once the baseline runs first, no path in the shadow check runs the identity statement on a shadowed session. The planted operator alone therefore cannot catch an unqualified operator. Break B's first attempt showed it: the campaign passed, and only the static test failed. The shadow check now calls `verify_identity` directly on the shadowed session, so each defence is asserted on its own.

**The APP-008 shadow check** (`app/src/storage/qualification.rs`) keeps S1's planted operator permanently:

- The trap adds `reviewer_probe.name_eq(name,name)`, a definer function that calls `mark()`, and `CREATE OPERATOR reviewer_probe.= (name, name)` over it, next to the existing `reviewer_probe.pg_roles` view.
- **A control for the operator alone.** As `genui_worker` with the shadowing path, a query on the qualified `pg_catalog.pg_roles` with an unqualified `=` marks. So the operator is resolved from the probe schema.
- **The first defence.** `recycle` is called directly on the shadowed worker session inside a `trace::scope` probe, so that the pool's replacement backend does not overwrite the recorded refusal. The probe must read step `None` and cause `baseline_search_path`: the baseline refused the session before the identity step began.
- **The second defence.** `verify_identity` is run on the same shadowed session. It must refuse with `identity_search_path` and leave the marker at 0.
- The existing end state is unchanged: the replacement backend is refused by its catalog comparison (the probe schema is drift), every role refuses after the window, and the marker is 0. That comment in the check was wrong (it credited the database default) and is corrected.

## Tests first

The planted operator was added to the shadow check on `48e342e6`, with `pool.rs` unchanged (`e116bbba…ceb6bf`). `just campaign postgres` failed at the check's final marker assertion (`qualification.rs:1230`): left `"37"`, right `"0"`. The reused shadowed session's identity statement had evaluated the planted `=` once per `pg_roles` row.

## Break proofs

The final tree: `pool.rs` `7e62055c19ec46097f8c02d35ea1f88614a3e50e6ad4f05a59cbec8847af552b`, `qualification.rs` `037f4c6ccc6bbb134e589939114d6ead59485c50b0bbd912f22c8a4fb527dac6`. Each break was applied to `pool.rs` alone. Then `just campaign postgres` and `cargo test --lib statement_tests` ran, and the file was restored from a saved copy. `shasum -a 256 -c` printed OK for both files after every restore.

| Break | `just campaign postgres` | Static test | What catches it |
| --- | --- | --- | --- |
| **A.** Drop the first `verify_baseline` in `recycle` | **fails**, `qualification.rs:1251`, "the baseline refuses the shadowed reuse first": left `(Some("verify_identity"), Some("identity_search_path"), Some(Unavailable))`, right `(None, Some("baseline_search_path"), Some(Unavailable))` | passes (no statement changed) | S1's probe: the identity statement ran on the shadowed session |
| **B.** Unqualify the planted operator: `WHERE rolname=current_user` at the end of `IDENTITY` | **fails**, `qualification.rs:1267`, "the identity statement used a planted object": left `"24"`, right `"0"` | **fails**: "an unqualified operator in SELECT session_user…" | both: S1's direct identity check and the static test |
| **C.** Unqualify an operator S1 does not plant: `a.attnum>0` in `IDENTITY` | passes | **fails** | the static test only |

- **A.** The marker alone would not fail here. The green run's direct identity check shows that the qualified statement, run on a shadowed session, leaves the marker at 0. That is why the probe asserts the order itself.
- **B, first attempt.** Before the direct identity check was added, the same break passed the campaign, and only the static test failed (see "A finding while testing").
- **C.** S1 plants only an `=` on names, so the static test is what covers the other operators.

## The A5 text

`docs/architecture/ADR-004-TENANT-TRANSACTIONS.md`, amendment A5:

- **First and last.** The reuse order now begins with `verify_baseline` and ends with it again. A new bullet, "Defence in depth for an unverified search path", says which statements can run before a session's search path is verified, that they are fully qualified, that a new connection's search path is fixed by its startup options, and which tests hold this.
- **The transaction in flight.** "At most 3 s" was wrong. It now says at most 3 s, or 60 s for an import (`import_deadline`).
- **Role attributes.** Only other roles' attributes move to the 30 s cadence. Those are the roles no pool connects as (`genui_owner`, `genui_lock`, `genui_admit`), and the other pooled roles, which their own pools check on every reuse. A pooled role's own attributes are part of `verify_identity` and stay checked on every reuse. The catalog document's `roles` section lists all seven roles, which matches this.
- **Forward note.** Before any PostgreSQL major upgrade past 16, the version relation's identity check must also require the heap access method (`pg_class.relam`) and no generated columns (`pg_attribute.attgenerated`). PostgreSQL 18 adds virtual generated columns, which are computed when read. Recorded in A5; not implemented, since the fixture and the target are PostgreSQL 16.
- The header says A5 was revised again after the recheck.

## R-m1: the cap-24 run's refusal

[review-fixes-2026-11-14.md](review-fixes-2026-11-14.md) now records it under a marked correction, from [the cap-24 report](refresh-herd-gate-snapshot24-experiment-2026-11-14.json):

- gate A had one data-role admission refusal (`admission_saturated` `{"data": 1}`);
- herd 2 had one first-attempt 503 (`reads_with_a_503` 0, 0, 1), which its retry recovered (`terminal_failures` 0).

The "no refusals" claim is narrowed to the cap-4 runs with a saved report (local 1, local 2 and CI), which do show none. The recheck's other R-m1 items are open: put data-role refusals in the gate's output and in APP-188's capacity statement, and decide whether read refusals at 15 pages are acceptable headroom. They belong to the integration with APP-200 and to APP-188.

## The 26-page measurement recipe

[release-measurement-recipe.md](release-measurement-recipe.md), prepared in the previous round and left uncommitted then, is committed with this round. It is unchanged.

## Commands and results

On the final tree above (host load 4–11 during the runs):

| Command | Result |
| --- | --- |
| `just check` | 21/21 steps ok (437 s). Library 416 passed, 35 ignored: one more than before, the static test. Admin 311/311. `board` OK: 199 tickets |
| `just campaign postgres` | ok (52.52 s), 37 checks. One earlier run of the same tree failed with a readiness `Deadline` at `qualification.rs:980`, the row-security policy subcase's `readiness()`, which this round does not touch. That is the known readiness flake under host load (the recheck saw it twice at `qualification.rs:885`). The rerun passed |
| `just campaign managed` | ok (12.04 s) |
| `just campaign sharing` | ok (15.00 s) |
| `just campaign live-sharing` | ok (22.48 s) |
| `cargo test --lib statement_tests` (in `app/`) | ok, 1 test |
| `python3 Tickets/_tools/board.py render`, then `check` | OK: 199 tickets; no generated view changed |

## Limitations

- The static test reads the statements' text. It knows PostgreSQL's operator characters and the keywords that expand to operators, not the full grammar. A new construct needs a new entry.
- S1 plants one operator, an `=` on names. The static test covers the rest.
- The recheck's three NITs are all addressed: two are the A5 text corrections, and the third is the forward note. R-m1's follow-ups (above) are open.

## The final check's F-m1: names other than operators

The [final check](review-tier-a-2026-11-14.md) at `3718cf23` found no BLOCKER or MAJOR. Its one MINOR, F-m1, was that the static test guarded only operators. An unqualified cast type (`c.relkind::text`, probe U1) or function call (`array_to_string(`, probe U2) passed the test, and the reviewer showed that both ran planted code on PostgreSQL 16.15 under a poisoned `search_path`. Fixed from `2a93ba42`.

**The static test** (`app/src/storage/pool.rs`, `statement_tests`):

- `search_path_names(statement)` returns every name in a statement that would resolve through the search path. After string literals and `OPERATOR(pg_catalog.…)` are removed, it tokenizes the statement and finds:
  - an operator character, or a keyword that expands to an operator (as before);
  - a `::` type or `CAST(… AS` type that is not `pg_catalog.`-qualified, or a `pg_catalog.reg…` type, which resolves the name it is given;
  - a name before `(` that is not `pg_catalog.`-qualified, apart from `BEFORE_PARENTHESIS`;
  - a `FROM` or `JOIN` relation (after `LATERAL` or `ONLY`) that is neither `pg_catalog.`-qualified nor a subquery, or a comma join after one;
  - a quoted identifier, or `COLLATE`.
- `BEFORE_PARENTHESIS` allows four words, each justified in the code. All four are reserved, so none can name a function unless quoted, and quoted identifiers are refused:
  - `ARRAY`, the array constructor;
  - `AND`, boolean grouping;
  - `LATERAL`, a subquery in `FROM`;
  - `CAST`, whose target type is checked separately.
- `unverified_session_statements_resolve_nothing_through_the_search_path` (renamed from `…_resolve_no_operator_…`) asserts that `IDENTITY`, `MEMBERSHIP` and `BASELINE_CHECK` give no findings.
- `the_search_path_check_refuses_each_unqualified_form` checks the checker itself.
  - Fifteen counterexamples must each be refused for their named reason, including U1, U2, `CAST(… AS text)`, a `regclass` cast, `lower(`, `FROM pg_roles`, `FROM ONLY genui_meta…`, `JOIN pg_auth_members`, two comma joins, `=`, `IS NOT DISTINCT FROM`, `NOT IN`, a quoted function name and `COLLATE`.
  - Two fully qualified statements must pass. One has a literal containing `''`, `(`, `FROM`; the other has `CAST(… AS pg_catalog.text)` and `UNION ALL`.

**The shadow trap** (`app/src/storage/qualification.rs`) also plants:

- a composite `reviewer_probe.text` type, with a definer cast from `pg_catalog.name` that calls `mark()`;
- `reviewer_probe.array_to_string(anyarray, pg_catalog.text)`, which calls `mark()`.

Each planted object has its own control, run as `genui_worker` under the shadowing path: `rolname=current_user`, `session_user::text` and `array_to_string(ARRAY[1],',')`. Each control must mark. The direct identity check now asserts the marker before the refusal cause, so a planted object that ran is reported as such.

**A5** now states exactly the rule the test enforces, its four allowed words, and what it does not cover: a name that a function resolves from a text argument, such as `to_regclass`, which the three statements do not use. The code comment above the consts says the same. The shadow check's plants are listed.

### Break proofs

The final tree: `pool.rs` `250547c02251036919aee11ead1a593bbc79fa99055133a6a524ebd0c5999abb`, `qualification.rs` `4413183c6e7f302b057e064a66e121740e48eb4ab4720ca1808da10f91f8202e`.

- Each break was applied to `IDENTITY` alone. Then `cargo test --lib statement_tests` and `just campaign postgres` ran, and `pool.rs` was restored from a saved copy. `shasum -a 256 -c` printed OK for both files after each restore.
- The campaign breaks ran on `pool.rs` `860ce084…04aa`. That copy differs from the final one only in the test helper's `CAST` scan, which was rewritten as an iterator for clippy (`needless_range_loop`); the rewrite does not change what the scan finds. Both static-test breaks were redone on the final `pool.rs`, with the same results.

| Break | `statement_tests` | `just campaign postgres` |
| --- | --- | --- |
| **Cast:** `session_user::pg_catalog.text` → `session_user::text` | **fails**: `["an unqualified type: text"]` | **fails** at `qualification.rs:1279`, "the identity statement used a planted object": left `"1"`, right `"0"`. The planted cast ran |
| **Function:** `pg_catalog.array_to_string(` → `array_to_string(` | **fails**: `["an unqualified function: array_to_string("]` | **fails** at `qualification.rs:1279`: left `"1"`, right `"0"`. The planted function ran |

### Commands and results (F-m1)

| Command | Result |
| --- | --- |
| `cargo test --lib statement_tests` (in `app/`) | ok, 2 passed |
| `just check` | 21/21 steps ok (385 s). Library 417 passed, 35 ignored: one more than before, the counterexample test. Admin 311/311. `board` OK. An earlier run failed `clippy` and `clippy-dev` on `needless_range_loop` in the new helper, which was fixed |
| `just campaign postgres` | ok (52.41 s) on the final tree, 37 checks. Earlier, on `860ce084…`: ok (53.41 s) after one readiness `Deadline` at `qualification.rs:910`. That is the credential-generation readiness step, which this round does not touch, and it failed while another worktree's native campaign ran at the same time |
| `python3 Tickets/_tools/board.py render`, then `check` | OK |

### Limitations (F-m1)

- The check reads the statements' syntax with a small tokenizer, not PostgreSQL's parser. It fails closed on a form it does not know, such as `EXTRACT(x FROM y)`, where it would refuse `FROM y` as a relation. A form it does not model at all would need a new rule.
- It does not see names that functions resolve from text arguments at run time (`to_regclass('x')`, `has_table_privilege('x', …)`). The three statements use none.
