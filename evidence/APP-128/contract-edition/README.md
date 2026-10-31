# Contract edition impl-2026-10-31.4 (APP-128)

This branch is `pivot/contract-edition`, cut from `origin/pivot/pitch-mvp` at be9c04a. It opens edition `impl-2026-10-31.4` under [ADR-002](../../../docs/architecture/ADR-002-CONTRACT-EDITION.md). [contracts/README.md](../../../contracts/README.md), section "Edition impl-2026-10-31.4", has the change table. The edition records synthetic contract qualification only. It changes no runtime behavior.

## Authority

- **Approval of the edition.** Victor, 2026-10-31: "Approve new edition". He was answering: "Ten pitch commits (APP-094 efff3ed … APP-120 3c15417) changed contracts/schema.json, schema.py, validate.py, test_contracts.py and the README, and added make_library.py plus 21 library fixtures, without a new contract edition; ADR-002 makes a new edition a contract decision. Approve a new edition so the lock reflects what the pitch actually ships (Contract qualification goes green)?"
- **Choice of option A.** Victor, 2026-10-31: "Ok option A note the trade off and document gaps to revisit if needed". He made this choice after the incompatibility below was reported with options A (rules for new content only), B (supersede beta's revision) and C (accept the verdict change).

## What was found

The lock was last written at 46ab8e9, edition `impl-2026-10-29.3`. The commits that touched `contracts/` since then:

| Commit | Ticket | What it changed |
| --- | --- | --- |
| efff3ed | APP-094 | `make_library.py`, 13 library apps, catalog, CSVs, README library section |
| 5fe8691, f353109, 4825e7f | ADR-011 scenarios | Expense claims app and catalog |
| d312ee2 | APP-101 | Catalog command slugs and aliases |
| 4c0f505 (and the merge 9ab4176) | APP-107 | `$defs.guard`, optional `when` and `initial` on `command_declaration`, their rules, 5 workflow apps |
| 270aed7 | APP-109 | Notes app |
| becf103 | APP-113 | Expense claims declares `initial`; `validate.py` adds "duplicate transition" and "create writing a governed enum field requires initial" |
| e5d55f7, 3c15417 | APP-119, APP-120 | Catalog sidecar only (hints, addresses, display titles) |
| 3679264 | APP-125 | Removed the committed `contracts/.venv` symlink |

**Classification.**
- No new kinds, schema versions, domains or problem codes.
- The APP-107 members widen an existing shape compatibly: they are optional, at definition schema versions 1 and 2.
- The APP-107 rules apply only to content that uses the members.
- The library is operator seed data.
- The two becf103 definition rules are **incompatible with stored data.** [stored-verdicts.txt](stored-verdicts.txt) shows beta's Expense claims bundle under three validators:
  - Edition impl-2026-10-29.3 (46ab8e9): valid.
  - be9c04a: refused, "create writing a governed enum field requires initial".
  - This edition: valid by default, refused with `new_content=True`.

  The bundle is the one seeded at 0c3261b (APP-105 log). Its bytes equal that commit's library file and its catalog `fixture_sha256` `446e5485…d059`. The other 19 apps seeded at 0c3261b validate under every rule.

## Mechanism (option A)

- `validate(entity, *, new_content=False)` and `validate_bundle(bundle, *, new_content=False)` take a keyword argument. The default is the stored-document verdict, identical to edition `impl-2026-10-29.3`'s for every document. `new_content=True` is the authoring verdict and adds the two APP-113 rules in `_definition`.
- **How a caller selects it:** by passing `new_content=True` when judging content being made now.
  - `make_library.py` does this for every library bundle, so `make_library.py --check` enforces it on the library. `test_contracts.py` does the same for the library and for the APP-113 refusal cases.
  - Every other caller keeps the default: `tenant_seed.py`, `admission_operator.py`, `test_library.py` and the qualification tools.
  - The composer's definitions go through Rust `admission::admit_definition`, which enforces neither rule (gap 1).
- **Stored-data vector:** `contracts/fixtures/stored/expense-claims-d70710d3.json`.
  - Python `EditionFourTests.test_stored_definitions_keep_their_verdict` checks that it is valid by default, refused with `new_content=True`, and that the current library's Expense claims is a different revision of the same app.
  - Rust `admission::tests::stored_definitions_that_predate_the_authoring_rules_still_admit` checks that Rust admission admits it with the same digest.

## The trade-off

- Stored definitions that predate the APP-113 rules can still exist and pass validation. Beta's Expense claims revision is one: its filing `submit` writes the governed `status` with no `initial`.
- Their protection is at command time, not admission. Storage's `Declaration::decided` / `fence_starts` (`app/src/storage/commands.rs`) refuses a create that starts a field at another command's transition target unless the field declares an `initial`.
- The Python validator and Rust admission differ in where the rules apply. Python applies them to new content when asked. Rust admission applies neither; APP-113 kept it that way so re-admission does not refuse stored definitions.

## Gaps to revisit

These are also tracked in [APP-128](../../../Tickets/0-backlog/APP-128.md).

1. Rust admission does not enforce either authoring rule for new content, including composer-generated definitions. Decide whether to add an authoring mode there.
2. Beta's stored Expense claims revision could be superseded at a later window (option B), with Victor's word. `seed-library` currently refuses on beta ("existing seed differs").
3. Composer-generated definitions on beta are unverified against the authoring rules. A read-only check on beta needs Victor's word.
4. The catalog's `edition` label (and `make_library.py` `EDITION`, `tenant_seed.py` `LIBRARY_EDITION`) still says `impl-2026-10-28.1`, although six library bundles use members that edition refuses.
5. Contract qualification CI does not run the library: `make_library.py --check` needs DuckDB, which is not in `contracts/requirements.lock`. It is a candidate for `just check` or CI.
6. The lock binds the library CSVs only through the digests inside their bundles.
7. The amendment and authoring rules have no `negative-vectors.json` entries. Rust parity rests on unit tests, not shared vectors, and the JavaScript verifier does not judge guards.

## Retained identities

The lock's `retained_editions` adds `impl-2026-10-29.3`, with lock sha `0e3024b1054b4e065e75e1308aa583f2ab6aec28ec95d536cc379061fc989a50` (the lock file at 46ab8e9). Its fixtures are copied from 46ab8e9 byte for byte into `contracts/fixtures/impl-2026-10-29.3/`:

| File | sha256 |
| --- | --- |
| canonical-vectors.json | eb9a4501813c8c90d8fbb756179a64074dd2df0a31883f39e995601afa9354f9 |
| command-recovery.json | b2539b15b32c7086576bd42aed824d823e5fa4c8ed2f40ed57637cdf79d14ecf |
| negative-vectors.json | 315a9d32e91dbeaf21c0ad3ec6f8e882315e46ef23b7db4c3c47f4b161ae1cbc |
| northstar.json | d9acfd8f399d20ea6dbcabc72dca1437c802a1565ca7119c6dd53587b14e1480 |
| spend-review.json | 186162da87532f368ed3c74faafc46c04780cc7331c095dc92f729bead0f0851 |

Editions `impl-2026-10-29.2` (lock `c1c1c80d…2923`) and `impl-2026-10-28.1` (lock `9bf0bc69…9a45`) are retained unchanged.

## Order of work and commands

1. Retained copies first: `git show 46ab8e9:contracts/fixtures/<f>.json > contracts/fixtures/impl-2026-10-29.3/<f>.json`, each `cmp`-equal to the current file.
2. Then `RETAINED`, `EDITION` and the schema `$id`.
3. Then `contracts/.venv/bin/python contracts/schema.py` and `contracts/.venv/bin/python contracts/make_fixtures.py`. Neither rewrote a fixture.
4. Then `app/operator/.venv/bin/python contracts/make_library.py --check`, which reported 40 files and 20 apps.
5. Then `contracts/.venv/bin/python contracts/lock.py write`, last of all, after the README.

[green.txt](green.txt) records each final check with its exact command and exit status:

| Check | Result |
| --- | --- |
| Contract qualification, fresh `python3 -m venv` (3.11.14) with `pip install --require-hashes --only-binary=:all: -r contracts/requirements.lock` | install OK |
| `python -m unittest discover -s contracts -p 'test_*.py' -v` | 32 tests OK |
| `node contracts/verify_vectors.mjs` (Node 22.21.0) | 6 edition-one and 13 edition-two vectors match |
| `python contracts/lock.py check` | 55 artifacts, 13 artifacts of 3 retained editions, both frozen sources |
| `make_library.py --check` (operator venv) | 40 files, 20 apps |
| `make_fixtures.py` then `git status --porcelain contracts/fixtures` | no change |
| `cargo +1.93.1 test --locked --lib admission::` | 17 passed (includes the stored-definition test) |
| `cargo +1.93.1 test --locked --lib storage::queries` | 5 passed |
| `cargo +1.93.1 test --locked --lib managed_qualification` | 20 passed, 3 ignored |
| `just check` (fmt, clippy, test, msrv, catalog, admin, infra, py-scripts, py-runtime, py-qualification, py-app-scripts, py-infra, schema-pins, contracts, board, bash-n) | all 16 ok; `cargo test --lib` 329 passed, 31 ignored; runtime 179 OK |

The worktree's `contracts/.venv` is uv-made (`scripts/dev/setup.sh`) and has no pip. The workflow's own install step was therefore run in a fresh venv outside the checkout, as recorded in green.txt.

## Collateral edits the edition required

Each was found by a failing check:

- `app/src/admission/tests.rs`: the edition id, 19 embedded fixtures (the retained 29.3 files and the stored vector), `library/` excluded from the locked specimen set (Rust admits the library in `storage::queries`), and the retention and stored-definition tests.
- `app/src/storage/managed_qualification.rs`: the six new embedded files in `SOURCES` (172 to 178), and the `seed_validator_sha256` pin for the changed `contracts/validate.py`.
- `app/qualification/Dockerfile.dockerignore`: the six new embedded files are admitted to the qualification image context.

Not run: the Docker campaigns and the schema qualifier (`just ci`). Nothing touches AWS or beta.
