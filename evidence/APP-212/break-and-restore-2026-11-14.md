# APP-212: break-and-restore proofs for the purge's guest-link guards

2026-11-14 UTC, worktree `forma-a212`, branch `ops/app-212-purge-guest-links`, local Docker PostgreSQL 16.15 (the test fixture's pinned image). Each guard in `app/runtime/admission_operator.py` was broken by one exact-string edit, the three purge tests were run, and the file was restored from the original bytes; its SHA-256 was compared before and after each break.

Tests run per break (`cd app/runtime && .venv/bin/python -m unittest -k <name> … test_admission_operator`):

- `DatabaseTests.test_purge_deletes_guest_links_at_schema_24_keeps_their_chains_and_refuses_below_it`
- `DatabaseTests.test_purge_empties_every_content_table_of_a_suspended_memberless_organization_and_keeps_the_chains`
- `PureTests.test_purge_order_is_children_first_and_the_kept_set_is_closed`

Clean run before the breaks: `.venv/bin/python -m unittest -k purge -k gated test_admission_operator` → `Ran 4 tests in 14.422s OK`.

`sha256(admission_operator.py)` before every break and after every restore: `69c6cc58804a326933de9928a0c176a5f639bbf432a18c362b112a911f762b59`.

| Break | Edit | Result | Restored |
| --- | --- | --- | --- |
| B1 the gate below schema 24 disabled | `require(gated or not any(guest_rows.values()), "guest_linked_shares_present")` → `require(True, …)` | `ERROR` in the schema-24 test's `< 24` block: `psycopg.errors.ForeignKeyViolation: update or delete on table "live_work_shares" violates foreign key constraint "linked_apps_organization_id_share_id_fkey" on table "linked_apps"` instead of `Refused("guest_linked_shares_present")`. `Ran 3 tests, FAILED (errors=1)`. Without the gate the key is the only backstop and the operator would report the generic `admission_operator_refused`. | identical |
| B2 the declared gate moved to 25 | `GATED_PURGE_DELETIONS = {IDENTITY_LINKS: 24}` → `{IDENTITY_LINKS: 25}` | `FAIL` ×2: the schema-24 test (`24 not greater than or equal to 25`, the version precondition; the purge at 24 would then refuse `guest_linked_shares_present`) and the pure test pinning the table (`{'genui_identity.linked_apps': 25} != {'genui_identity.linked_apps': 24}`). `FAILED (failures=2)`. | identical |
| B3 `purge_guest_links_remain` inverted | `require(not any(guest_after[table] for table in gated), …)` → `require(any(…), …)` | `FAIL` ×2: both database purges refuse `{"refused": "purge_guest_links_remain"}` (exit 1 where 0 was expected). The guard reads the real post-deletion counts. `FAILED (failures=2)`. | identical |
| B4 `purge_guest_chains_changed` inverted | `guest_after[table] == guest_rows[table]` → `!=` | `FAIL` ×2: both database purges refuse `{"refused": "purge_guest_chains_changed"}`. The guard compares the real before and after `share_events` counts. `FAILED (failures=2)`. | identical |
| B5 the second FORCE restore removed | the `lift_identity_rls(c, forced=True)` after `guest_after = guest_row_counts(c, org)` deleted | `FAIL` ×2: both database purges refuse `{"refused": "purge_rls_not_restored"}`; the re-check catches a FORCE left lifted after the proof read, and the refusal rolls the transaction back. `FAILED (failures=2)`. | identical |

Final `sha256(admission_operator.py)`: `69c6cc58804a326933de9928a0c176a5f639bbf432a18c362b112a911f762b59` (identical to the original). The driver script and its full log stayed in the session scratchpad; this page records what ran.

Not broken here: `purge_audit_changed`, `purge_incomplete`, `purge_rls_state_refused` and the first `purge_rls_not_restored` are unchanged from APP-185/APP-188 and keep their existing coverage.
