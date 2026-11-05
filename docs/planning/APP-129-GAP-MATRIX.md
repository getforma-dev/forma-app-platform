> **Historical (pitch era, kept as a record).** Current guidance: [DEVELOPMENT.md](../DEVELOPMENT.md).

# APP-129 gap matrix

Read against `af63feba41aa825a972dde354e0a5d2195f7d608` (`origin/pivot/pitch-mvp`). Source only. No application code changed. No test command was run for this note. No new APP id is taken; the board allocates the next id when someone adds a ticket file and runs `Tickets/_tools/board.py`.

The seven gaps are the list under "Gaps to revisit" in [contracts/README.md](../../contracts/README.md) and [APP-129](../../Tickets/0-backlog/APP-129.md).

| # | Status | Why |
| --- | --- | --- |
| 1 | Open, narrowed | `admit_definition` still does not apply the two authoring rules. A model-generated folder app does, through `plan_generated`. The deterministic publisher emits no commands. |
| 2 | Deferred | Victor's option A, 2026-10-31. Option B stays a later window and needs his word again. |
| 3 | Open, blocked on authorization | No read-only beta check of composer-generated definitions is recorded. This note does not query beta. |
| 4 | Open | The seed label is still `impl-2026-10-28.1` in the three places the gap names. |
| 5 | Closed for the workflow | Contract qualification and `just check contracts` run `make_library.py --check` with the APP-005 DuckDB pin in the contracts venv. Merged as `1b10ab1` in `1a285e2`. |
| 6 | Open | `contracts/lock.py` lists no library CSV. |
| 7 | Open | The current negative vectors and `verify_vectors.mjs` do not carry the amendment or the authoring rules. |

## 1. Authoring rules inside Rust admission

**Status.** Open. The missing invariant is narrower than the original sentence.

`admission::admit_definition` (`app/src/admission/validate.rs`) admits a definition whole and does not call the two APP-113 rules. `admission::tests::stored_definitions_that_predate_the_authoring_rules_still_admit` records why the default stays that way: beta's stored Expense claims definition `sha256:d70710d3…d0de` must still admit.

`composer::plan::plan_generated` admits first, then calls `require_authored` (`app/src/composer/plan.rs`). That function is the two rules: one `(field, from, to)` per command, and `initial` on a transition-free create that writes an enum another command governs. `composer::tests` asserts that `admit` accepts a missing `initial` and a duplicate transition, and that `plan_generated` refuses both with `authoring_rule`. Those assertions were read, not executed here.

A model-generated folder app uses that path. `boxes/routes.rs` calls `Composer::generate_with`, which calls `plan_generated` (`composer/mod.rs`), which calls `require_authored`. `publish_rule_built` is a different caller. It admits, calls `require_runnable`, and publishes the trusted `derive::{data,metrics,card}_definition` values. Their shared builder sets `record_schemas` and `commands` to empty arrays (`boxes/derive.rs`). Those current outputs cannot violate the two command-authoring rules. `plan_adapted` and `plan_transformed` also re-admit without `require_authored`. Those two change title, description, or view operations of a library definition; they do not declare new commands.

**Smallest slice.** Leave the model path as it is. A call to `require_authored` inside `publish_rule_built` would be defense in depth for a future caller, not a fix for an unguarded model command. Leave `admit_definition` unchanged so stored re-admission keeps working. Do not implement a generated-command bypass on the strength of this row.

**Modules.** `app/src/boxes/routes.rs`, `app/src/composer/plan.rs`, `app/src/boxes/tests.rs` or the box campaign.

**Acceptance.** No code change is required for the current callers. A model-generated folder app is refused by `require_authored` when it breaks either rule. A deterministic data, metrics, or card definition still publishes with empty `commands` and `record_schemas`. The stored Expense claims fixture still admits. A later guard inside `publish_rule_built` is optional and is not this gap's fix.

**Authorization.** No grant, role, or tenant change. The refusal is a definition rule. Do not put the rules inside default admission: that would start refusing the stored beta definition on re-admission.

**Depends on.** Nothing new. The helper already exists.

## 2. Supersede beta's stored Expense claims revision

**Status.** Deferred by the recorded decision. Victor, 2026-10-31: "Ok option A note the trade off and document gaps to revisit if needed." Option A keeps the stored verdict. Option B (a new revision and publication, then adoption) was the alternative he did not choose.

The refusal is still in the tree. `fixtures/reset_trial.py` expects `existing seed differs; no authority overwritten`. `docs/pitch/DEPLOY-RUNBOOK.md` says not to rerun `seed-library` on the existing beta organization after Expense claims gained `initial`.

**If it is reopened.** The slice is an operator window: publish a new Expense claims revision that declares `initial`, adopt it for new work, and leave the stored revision readable. `seed-library` stays an exact match. That needs Victor's word again. This note does not schedule it.

**Authorization.** A re-seed that overwrites an existing definition would change what beta already published. The current refusal is the protection. Do not weaken it to make option B pass.

## 3. Composer definitions already on beta

**Status.** Open, and blocked without Victor's authorization. The gap asks for a read-only check that definitions composed on beta meet the authoring rules. No such check is recorded in APP-129's log or `evidence/APP-129/`. This note did not connect to beta.

Locally, new sentence compositions and model-generated folder apps now pass through `require_authored` (gap 1). That does not inspect definitions already stored on beta.

**Smallest slice, after authorization.** A read-only query of beta's composed definition revisions, judged with `validate(..., new_content=True)` or the same two rules, writing a count of refusals and no rows. No publish, no seed, no migration.

**Modules.** A one-off operator or SQL read plus `contracts/validate.py`. No service change unless a refusal must be repaired, and a repair is gap 2's kind of decision.

**Authorization.** Read-only access to the beta database is an operator action. It is not granted by this note.

## 4. The library edition label

**Status.** Open. These three still say `impl-2026-10-28.1`:

- `contracts/fixtures/library/catalog.json` field `edition`
- `contracts/make_library.py` `EDITION`
- `app/runtime/tenant_seed.py` `LIBRARY_EDITION`

`tenant_seed.py` refuses a catalog whose `edition` is not `LIBRARY_EDITION`. The contract README still says six bundles (Access requests, Deal desk, Expense claims, Purchase requests, Time off, Vendor invoices) declare `initial`, and Purchase requests also has `when` guards, which the retained `impl-2026-10-28.1` validator would refuse. The bytes are valid as new content under `impl-2026-10-31.4` because `make_library.py` calls `validate_bundle(..., new_content=True)`. The label and the validator edition are different strings.

**Smallest slice.** Decide the label is the seed identity, and say that in the catalog comment and the contract README, leaving the string `impl-2026-10-28.1`. Or change all three strings together in one commit so `make_library.py --check` and `tenant_seed` still agree.

**Modules.** The three files above, plus any pin of `seed_library_sha256` if the catalog bytes change (`app/src/storage` managed qualification).

**Acceptance.** `catalog.edition`, `EDITION`, and `LIBRARY_EDITION` are one string. `make_library.py --check` still passes. A second `seed-library` of an unchanged tree still reports no change.

**Authorization.** `EDITION` is written onto `catalog.json` only. The seeded bundle rows do not carry that label. `library_catalog` refuses a catalog whose `edition` is not `LIBRARY_EDITION` (`library catalog refused`), and `seed-library` surfaces that as `seed_bundle_refused` before any row is compared. `existing seed differs` is the later check, and it fires only when a seeded row's values change. A label-only change of `catalog.edition`, `EDITION`, and `LIBRARY_EDITION` together, with those two still equal, leaves the seed rows identical, so a second `seed-library` reports `changed: false`. Do not overwrite stored definitions to make a label match. The pin that does change when the catalog bytes change is `seed_library_sha256`.

**Depends on.** Gap 2 if the intent of the label change is to re-seed beta. A comment-only decision depends on nothing.

## 5. Library check in contract qualification

**Status.** Closed for the workflow. The reading below is the `af63feb` baseline. Pull request 11 (`1b10ab1`, merged into trunk at `1a285e2`) is the current disposition.

On `af63feb`, `.github/workflows/contracts.yml` ran the contract unittests, `contracts/verify_vectors.mjs`, and `contracts/lock.py check`. `scripts/dev/check.sh` named the same three commands. Neither ran `make_library.py --check`. `contracts/requirements.lock` had no DuckDB. `app/runtime/requirements.lock` pinned `duckdb==2.0.0`.

**Delivered.** Contract qualification and `just check contracts` run `contracts/.venv/bin/python contracts/make_library.py --check`. `just setup` installs `spikes/APP-005/requirements.lock` (`duckdb==2.0.0`, the same hashed block as the runtime lock) into `contracts/.venv` beside the unchanged validator lock. `--check` compares generated bytes in temporary storage, names a stale path under `contracts/fixtures/library/`, and does not rewrite fixtures. The earlier acceptance that the contracts venv stay free of DuckDB is stale: that venv now has the APP-005 pin, and `contracts/requirements.lock` still does not.

**Authorization.** None. The check is local and CI.

## 6. CSV bytes in the lock

**Status.** Open. `contracts/lock.py` has no `.csv` path. The edition table says each library CSV is bound by the `content_digest` inside its bundle. A CSV can change if the bundle's digest is updated in the same commit and the lock is regenerated to the new bundle; the lock does not name the CSV file.

**Smallest slice.** Add each `contracts/fixtures/library/data/*.csv` to the edition's lock map, hashed as the file bytes, and teach `lock.py check` to require them. Keep the bundle `content_digest` check as well.

**Modules.** `contracts/lock.py`, `contracts/make_fixtures.py` if it regenerates the lock, and the lock test.

**Acceptance.** Changing one CSV byte without updating the lock fails `lock.py check`. A regenerated library whose digests and CSV hashes move together passes.

**Authorization.** None.

## 7. Shared negative vectors for guards and authoring rules

**Status.** Open. There is no `contracts/fixtures/impl-2026-10-31.4/negative-vectors.json`. The README says the current `negative-vectors.json` is edition three's bytes. `contracts/verify_vectors.mjs` does not mention guards. Rust coverage that was read, not run, is `admission::tests::declared_guards_and_initial_values_match_validate_py` and the composer tests around `authoring_rule`. Python coverage is `EditionFourTests` and the `new_content` cases, as APP-129 already records.

**Smallest slice.** Add negative documents for a bad guard (unknown field, wrong type, unordered comparison) and for the two authoring rules, in a vector file the Python tests and a Rust test both load. Leave `verify_vectors.mjs` on digest and canonical checks unless it is also taught the guard rules. Do not claim JavaScript judges guards until it does.

**Modules.** `contracts/fixtures/` (the vector file), `contracts/test_contracts.py`, `app/src/admission/tests.rs`. `verify_vectors.mjs` only if the slice includes it.

**Acceptance.** The same refused document is refused by `validate(..., new_content=True)` and by the Rust test that loads it. A document that only fails the authoring rules still passes `admit_definition`, matching gap 1's stored-verdict rule.

**Authorization.** None. Vectors are fixtures.

## What this note does not do

It does not close APP-129, retitle the edition, or treat a source reading as a test run. Gaps 2 and 3 stay with Victor. The other open slices can land separately.
