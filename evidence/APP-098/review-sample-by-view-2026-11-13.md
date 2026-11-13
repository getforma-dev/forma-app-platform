# Independent review: APP-098 DS-84, sample data judged by the inputs views read

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-13. I did not write this change.
- **Reviewed revision:** `c4a945f` on `app098/sample-by-view-inputs`, in the `forma-work` worktree (`git show HEAD`, 7 files, +98/−17).
- **Protocol:** I read the code and the catalog fixtures, ran unit tests, and ran a read-only Python walk over the catalog. I used no browser, no AWS, no dev stand and no campaigns. I edited nothing except this file.
- **Verdict: approve.** I found no blocker and no major. The walk cannot under-label an admitted definition. All 20 shipped catalog apps stay flagged. The three readers share one rule. The ticket text is accurate. Two minors below are optional.

## Findings

- **m1 (minor, precision; over-label only).** `query_inputs` (`storage/queries.rs:951`) also collects the `input_id` of a `reference` filter literal (`admission/validate.rs:1089-1093`). That id is the target of a reference value (`validate.rs:2503`), and the target can be any declared input, a source included (`validate.rs:1899-1906`). It is not an input the query reads.
  - So a records-input query that filters on a reference into a sample source would still be flagged. That errs toward over-labelling, the safer side, and it matches `editable_definition`'s `rewrite` (`data_copy.rs:88-113`).
  - Admission and the engine both treat `query.inputs` as the complete read set: `validate.rs:2337-2339` refuses a field from an input outside `inputs`, and `queries.rs:421-427` runs exactly `inputs[0]`. So `view["query"]["inputs"]` alone would be exact.
  - No shipped app is affected: for all 20, the walk and an `inputs`-only read give the same set.
- **m2 (minor, test reach).** The new test (`data_copy.rs:388`) exercises the pure functions only, on a fixture that is not admitted (no `renderer_version`, `operator_version` or `null_policy`).
  - No campaign or storage test publishes a data copy whose source binding is a catalog source and then checks that the flag is off on the three readers.
  - The campaigns cover the other direction: library works stay flagged.
  - Sound as it stands, because the copy is re-admitted (`boxes/routes.rs:1445`, `:1528`) and `rewrite` redirects every source input. Worth a storage-level case if a sample-bound data copy becomes a reachable demo path.

## Answers to the four questions

1. **Does the walk find every way a view reads an input? Yes.** A view reads data through `query` or `records`.
   - `query.inputs` must name declared inputs (`validate.rs:2215-2219`). Every `FieldRef` must be one of them (`validate.rs:2337-2339`). That covers project, filter, aggregate, sort, time_bucket and both sides of `lookup` (`validate.rs:1474-1480`, `:2286-2296`).
   - A `records` view, a `record_filters` entry and a records binding all require a records input (`validate.rs:1972-1982`, `:2094`). A records binding has `source_id IS NULL` by CHECK (`migrations/0002_tenant_foundation.sql:149-152`), so it can never be catalog-bound.
   - `links[*].input_id` names a cross-view input. It is not a separate read, and both of its views carry queries anyway.
   - I found no key the walk misses that could hide a sample read, so there is no under-label.
2. **Are the shipped catalog views still flagged? Yes, all 20.** I ran a script that applies the same walk to `definition.content` in each fixture listed in `contracts/fixtures/library/catalog.json`, intersected with `work.bindings` whose `source_id` is a catalog source. For every app the bound set equals the query set (one source input each), so the intersection is never empty.
3. **The call sites: correct.**
   - `work_outline` (`queries.rs:1171-1180`), `work_reads` (`storage/actions.rs:322`, `:343-346`) and `guest_work_reads` (`storage/shared_reads.rs:61-65`) each look up the same `(app, work)` key that they passed to `sample_works`.
   - Each passes `query_inputs` from the outline of that work's pinned, re-admitted content (`outline_of` → `outline`, `queries.rs:621-629`, `:1120`).
   - A grep finds no other `sample_works` caller and no other writer of `sample_data` outside tests. Consumers (`query/canvas.rs:1493`, `query/page.rs:776`, `sharing/live.rs:744`, `mcp/tools.rs:643`) read only the bool. `WorkOutline` derives no `Serialize`, so `query_inputs` does not reach any response.
   - **SQL** (`queries.rs:1756`):
     - `input_id` is `genui.local_id NOT NULL`, so the `::text` cast is safe.
     - `source_id = ANY(...)` is NULL, never true, for records rows, so they drop out.
     - The primary key `(organization_id, app_id, work_id, input_id)` makes `DISTINCT` redundant but harmless.
     - The table-level `GRANT SELECT` to `genui_workspace, genui_data` (`0002_tenant_foundation.sql:232-233`) covers the new column for both member and guest transactions. RLS is unchanged.
4. **The ticket text is accurate.**
   - `editable_definition` refuses unless every input is `kind: "source"` and there are no record schemas and no commands (`data_copy.rs:45-52`). That is stricter than "refuses a records input", and the ticket's statement is true.
   - Every one of the 20 catalog apps has a records input (`decisions`, `claims`, `requests`, `invoices` or `notes`), and none passes that check. So "a catalog app itself cannot be data-copied today" holds.
   - The log line's "414 passed" matches: the lib has 448 tests, of which 34 are ignored.
   - The test is red under the old rule: the old rule is `bound.is_some()`, so the "copy reads its own records" assertion would fail.

## Commands run (in `/Users/victorvillacis/dev/FormaBuiltApps/forma-work`)

- `cargo test --locked -p forma-app-platform --lib storage::data_copy`: 2 passed, 0 failed (includes `a_data_copy_no_longer_reads_its_source_input`).
- `cargo test --locked -p forma-app-platform --lib query::`: 46 passed, 0 failed, 1 ignored.
- `cargo test --locked -p forma-app-platform --lib`: 414 passed, 0 failed, 34 ignored (to check the log line's count).
- `cargo test --locked -p forma-app-platform --lib -- --list` and `-- --list --ignored`: 448 tests, 34 ignored.
- A Python catalog walk (scratch script, not committed) over the 20 fixtures: 0 apps unflagged; the walk equals `query.inputs` for all; every app has a records input; none passes `editable_definition`'s precondition.
- Not run: campaigns, `just check`, clippy, binary (`main.rs`) tests. The commit updates the `main.rs` outline literal (`main.rs:1077`), but I did not compile the binary tests.
