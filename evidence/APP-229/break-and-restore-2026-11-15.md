# Break-and-restore evidence: entitlement extension and member erasure (APP-229, APP-230; 2026-11-15)

Branch `feat/app-229-230-extend-erase`, worktree `forma-erase`, migration 0026. Each break edits one
rule of `app/migrations/0026_member_lifecycle.sql` in place with a reversible script (the exact edit,
then its exact reverse; the files are compared by SHA-256 with the state before the break), repins
the manifest to the broken migration and regenerates `catalog-v26.json` from it in the disposable
PostgreSQL 16.13 (`generate_schema_catalog.py`), so the break reaches the tests instead of the
catalog pin (`check-schema-pins.py`: `"agree": true` on the broken pins too), runs the test that
guards the rule, records the failing assertion, restores, regenerates and runs the test again. The
script ran once, unattended, in the session scratchpad; the log lines quoted here are copied from it.

Before every break: `0026_member_lifecycle.sql` `5d8cd998…8abaf`, `catalog-v26.json` `f642958b…24de7`.

## (a) Never lowered (`genui_private.extend_entitlement`)

Break: `SET expires_at=greatest(e.expires_at,p_until) … AND e.expires_at<p_until` becomes
`SET expires_at=p_until` with the `expires_at<p_until` predicate dropped, so an earlier `--until`
overwrites a later date. Broken SQL `6ad89892…5844c`, broken catalog `c296d82f…f4a7b`.

- `LifecycleTests.test_extend_entitlement_moves_dates_later_never_earlier_revives_a_lapse_and_is_audited`
  fails at the second call (`test_admission_operator.py:1955`), the one with the 30-day date after the
  60-day extension:
  `AssertionError: Tuples differ: (1, 0, True, '2026-12-15T22:55:55Z') != (0, 1, False, '2027-01-14T22:55:54Z')`:
  the member's expiry was lowered from Jan 14 to Dec 15 and audited as a change. With the guard,
  the earlier date is `changed: false`, no event is appended and the January date stands.

Restored: the edit reversed (`grep -c "BREAK (a)"` is 0), the catalog regenerated and the manifest
repinned from the restored SQL; both files byte-identical to before (`5d8cd998…8abaf`,
`f642958b…24de7`); the test passes (`exit 0`).

## (b) The anonymization of others' entries (`genui_private.erase_member`)

Break: the membership update `SET display_name=NULL,role='member',membership_epoch=…` loses
`display_name=NULL`, so the entries the member made in another owner's app would stay attributed
to a principal that still carries the name. Broken SQL `3c18a095…7cebcf`, broken catalog
`abec7fe2…982d0`.

- `LifecycleTests.test_erase_member_removes_their_own_world_keeps_their_entries_elsewhere_nameless_and_is_audited_once`
  fails at the erase call itself (`test_admission_operator.py:2143`): the operator's own proof refuses
  before the test can read a row, `AssertionError: 1 != 0 : {"refused": "erase_incomplete"}`:
  `erase_proofs` requires `display_name IS NULL` on the revoked membership and rolls the whole
  transaction back, so nothing of the member's world was deleted either (the second layer holds when
  the first is broken). Had the proof not refused, the test's next assertions pin the name
  (`("revoked", None, "member", epoch + 1)`) and the other member's record still reading with a
  nameless creator (`[(erased, None)]`), and the campaign pins `revoked:<none>:member` and
  `{ERASED}:<none>` on the entry in the other member's app.

Restored: the edit reversed (`grep -c "BREAK (b)"` is 0), the catalog regenerated and the manifest
repinned; both files byte-identical to before; the test passes (`exit 0`).

## (c) The last owner (`genui_private.erase_member`)

Break: `IF m_role='owner' AND o_status='active' THEN … 'last_owner'` becomes `IF false THEN`, so a
revoked sole owner of an active organization is erased (role to member) and the organization is
left without an owner. Broken SQL `4315267a…d9202`, broken catalog `cd91610b…97c16`.

- `LifecycleTests.test_erase_member_refuses_the_last_owner_unless_the_organization_is_being_purged`
  fails at the first erase (`test_admission_operator.py:2200`): the command that must refuse
  `last_owner` succeeds, `AssertionError: 0 != 1`. With the guard, the sole owner refuses until
  `set-role owner` names a successor, and only a suspended organization (the purge path) is exempt.
  The campaign pins the same refusal over real rows (`last_owner` with `owner:usr_…` still on the
  row), and `verify` recomputes every chain afterwards.

Restored: the edit reversed (`grep -c "BREAK (c)"` is 0), the catalog regenerated and the manifest
repinned; both files byte-identical to before; the test passes (`exit 0`).

## After the three cycles

`0026_member_lifecycle.sql` `5d8cd998…8abaf`, `catalog-v26.json` `f642958b…24de7`,
`manifest.json` identical to the committed bytes (`manifest_restored: true`). The script's working
copies and the disposable generator containers were removed; the log stayed in the session
scratchpad and is not committed.

## After the Tier A review of PR #163 (the same day): two more guards

The review fixes rewrote `erase_member` (the walk is one loop over constant relation/predicate
pairs, the guests' sign-in flows go first, the admission's subject is rewritten only inside the
tombstone guard). Before these two breaks: `0026_member_lifecycle.sql` `5e62751e…0c7b`,
`catalog-v26.json` `08a41856…a076`. The same script, the same method; the guarding test is the
erase test, whose fixture now admits the member through a consumed `pending` reservation and
carries an expired guest sign-in flow for their live share.

### (d) A repeat is harmless (`genui_private.erase_member`; review MAJOR-1)

Break: the `UPDATE genui.code_reservations SET provider_subject=v_tomb … provider_subject=v_subject`
is moved back outside `IF v_subject<>v_tomb` (the first draft's shape), so on the second run, when
`v_subject` is already the tombstone, it matches the tombstoned row again. Broken SQL
`eff3a9c1…b078`, broken catalog `e7fccea1…339a`.

- `LifecycleTests.test_erase_member_removes_their_own_world_keeps_their_entries_elsewhere_nameless_and_is_audited_once`
  fails at the repeat (`test_admission_operator.py:2226`):
  `AssertionError: Tuples differ: (True, 0, 'b5e330fb-…', None, 4) != (False, 0, None, None, 3)`:
  the second run reports `changed: true`, appends a second `member.erased` and advances the epoch
  to 4. With the guard, the second run is `changed: false`, writes nothing, and the dump is byte
  for byte the one before it (the test compares both, and the `member.erased` count).

Restored: the edit reversed (`grep -c "BREAK (d)"` is 0), the catalog regenerated and the manifest
repinned; both files byte-identical to before (`5e62751e…0c7b`, `08a41856…a076`); the test passes
(`exit 0`).

### (e) The guests' abandoned sign-in flows (`genui_private.erase_member`; review MAJOR-2)

Break: `genui_identity.shared_auth_flows` is dropped from the walk's relation list (its slot
repeats `linked_apps`, so the predicate list keeps its shape), so the expired flow naming the
member's live share is still there when the share is deleted. Broken SQL `2b7b522d…27de`, broken
catalog `b54e4e19…925e`.

- The same test fails at the erase itself (`test_admission_operator.py:2171`):
  `AssertionError: 1 != 0 : {"refused": "admission_operator_refused"}`: the non-deferrable key
  `shared_auth_flows(owner_organization_id, share_id) → live_work_shares` raises inside the
  definer, the operator maps the error to its fixed generic refusal, and the whole transaction rolls
  back (the member's world is intact, which the test's next assertions would have shown). With the
  flows deleted first, under the `operator_scope` policy the migration adds on that table, the
  share and its links go and the test passes.

Restored: the edit reversed (`grep -c "BREAK (e)"` is 0), the catalog regenerated and the manifest
repinned; both files byte-identical to before; the test passes (`exit 0`).

After the two cycles: `0026_member_lifecycle.sql` `5e62751e…0c7b`, `catalog-v26.json`
`08a41856…a076`, `manifest.json` identical to the committed bytes (`manifest_restored: true`).
