# APP-084 independent review (Tier A for the migration and the custody story)

Date 2026-11-05. SHA read: `a9a730531dc6644d2fa6ef291a967080886f596a` (`git rev-parse HEAD` on `chore/app-084-residuals`). That commit is `origin/main`. The APP-084 change is an uncommitted worktree on it, not a descendant commit. Reviewer independent of the implementing session. This file is the only write. Nothing committed. AWS was not called. Nothing on `127.0.0.1:4001` was touched.

## Method

Read the worktree diff against `a9a730531dc6644d2fa6ef291a967080886f596a`, then the source those hunks call: `app/migrations/0021_operator_audit_kinds.sql`, `app/runtime/admission_operator.py` (`append`, `verify`, `note_invitation`, `claim_provider_tenants`, `bound_elsewhere`, `bind_provider_tenant`, `provision`, `issue`, `issue_handle`, the provider create/reconcile/revoke writers, `register_source_revision`, `create_bound_work`, `seed_spend_review`), the edited tests, `app/src/storage/query_qualification.rs`, ADR-002 line 33, ADR-003's 2026-10-29 and 2026-10-30 custody amendments, and ADR-007 D7.8 / D7.19. Confirmed `manifest.json` sha256 for `0021_operator_audit_kinds.sql` and `catalog-v21.json` against the files. Read `evidence/APP-084/schema-campaign.json` (`passed: true`, server 16.15, fresh version 21). Did not re-run the unit tests, `just check fmt`, `just check clippy`, or `just campaign query`; those results below are the implementer's report, and the new asserts are in the source.

## Verdict

**M1 is closed.** No blocking finding remains. Acceptance item 5 can be checked.

## Acceptance

| Item | Judgment |
| --- | --- |
| 1 Custody | **Met without this diff calling AWS, and without live rotation.** ADR-003's amendment of 2026-10-30 records the decision and supersedes the 2026-10-29 0600-file default: capabilities go to Secrets Manager under the environment key (APP-074). This worktree does not call AWS and does not issue an invitation under that file default. Live secret rotation and the stack-parameter clear are item 7 / operator-gaps M3. They are not item 1. The item 1 box does not stay open for them. |
| 2 Provider kinds, policy, one event, `verify`, campaigns | **Met in this tree and in the local schema campaign. Not met "in CI".** D7.8 names the three provider kinds. Migration 0021 replaces `operator_chain`'s `WITH CHECK` with the previous kinds plus those three and the four command kinds. `note_invitation` writes one event when the recorded state changes. `verify` recomputes the chain. `evidence/APP-084/schema-campaign.json` is `passed: true`. The provider unit file was reported 84 tests passed together with the admission operator file. The tree is uncommitted, so CI has not run on it. That keeps the CI sentence of the box open. It is not a second code defect. |
| 3 Source revision and bound work | **Met in source.** Each command appends once on insert and returns before `append` on an unchanged rerun. The query campaign counts `source_revision.registered` and `work.seeded` after the rerun and expects `"1"`. Reported `native_query_campaign` passed. Not re-run here. |
| 4 Every other finding fixed or recorded | **M4 is met by the backfill below.** Operator-gaps M3 / live secret cleanup is still recorded as not run, not as an accepted limit, so that row leaves the box open. It is not a defect in this diff. The ADR-002 six-column sentence, the D7.8 kind list, D7.19's code-extension-plus-reissue sentence, and the plan Task 5 seed sentence match the source. |
| 5 This review, no blocking finding | **Can be checked.** M1 is closed and nothing new blocks. |

## M1 — blocking. The private index still misses an active binding that has no handle and no code

`0021_operator_audit_kinds.sql` backfills `genui_private.provider_tenant_index` only from `handle_index` ∪ `code_index`. The comment says an organization that already holds a binding and neither a handle nor a code is claimed later. `bound_elsewhere` reads only the index. `claim_provider_tenants` inserts only the caller's own active bindings.

So the case operator-gaps M4 named is still open. Organization A has an active `(issuer, tenant)` and is not in the index (the migration skipped it; `tenant_seed.seed` still inserts bindings and never claims). Organization B's `bind-provider-tenant` sees no index row, inserts a second active binding, and `claim_provider_tenants` records B as the owner. Both bindings commit. A's later `issue-handle` or `issue` then fails `provider_tenant_bound_elsewhere`, so the later claimant keeps the index and the earlier organization is the one refused. `bind` of a row A already has returns `existing` and does not claim, so a retry on A does not repair it.

`ON CONFLICT DO NOTHING` does not hide a collision that is already in the index: the following `SELECT` refuses a different `organization_id` and `require` aborts the transaction. It also cannot see a binding that was never indexed. Two concurrent first claims of an absent key still serialize on the primary key; the loser rolls back. The hole is the unindexed row, not a swallowed unique violation between two indexed owners.

The regression test plants the index row itself (`test_admission_operator.py`, the `elsewhere-iss` insert into `provider_tenant_index`). A handle plus an active binding and no index row is no longer what the test refuses. `provision` and a creating `bind` do claim in the same transaction, so a new provision is not this hole. The registry remains one binding per routed organization, so this is not a cross-tenant session. It is still a broken uniqueness migration: the table's primary key does not mean one active binding.

Fix: backfill every active `genui.provider_tenant_bindings` row (a definer, or another owner-visible set that is not only handles and codes), and make `bound_elsewhere` / `claim` refuse when any other organization already has that active pair, including one that is not yet indexed. `seed` has to claim too. The test should refuse from the binding alone.

### Fix disposition (2026-11-05, backfill only)

**Closed.** No new blocking finding. Re-read `0021_operator_audit_kinds.sql` after the edit. The copy is no longer `handle_index` ∪ `code_index`. It turns off row security on `genui.provider_tenant_bindings` (`NO FORCE`, then `DISABLE`), inserts every `active` row into `genui_private.provider_tenant_index`, then `ENABLE` and `FORCE` again before `RESET ROLE`. Two organizations already sharing one issuer and tenant fail that primary key, including an organization with neither a handle nor a code. PostgreSQL runs foreign-key checks outside row security, so the insert does not need `genui.organization_id` set. `ALTER TABLE` is transactional and takes `AccessExclusiveLock`, so another session never sees row security off; a failed copy rolls the disable back with the migration. The file's sha256 is `ea740b7f31c534b8f0ab6598543b57b637e3b58b30c7f8637de6f850caf4742e`, which is the manifest pin. The implementer's rerun of the schema campaign passed, including the `catalog-v21.json` match, so `FORCE` is on at the end. Not re-run here.

`tenant_seed.seed` can still insert a binding after 0021 without claiming. That is not this miss: the rows 0021 sees are copied, and `provision` plus a creating `bind` claim in the same transaction. Seed's own tenants are one `ten_{organization}` each. The bind test still plants the index row; it does not reopen the migration hole.

## Checked, not findings

`append` hashes a 13-key body whose `outcome_digest` is the `sha256:` string or JSON null. `verify` rebuilds that same key set from `outcome_digest::text` (domain `genui.digest` is text) and formats `occurred_at` the same way. Key order does not matter (`sort_keys`). `verify` recomputes the chain over stored columns. It does not recompute the outcome from the transition fields, because those fields are not columns. That is D7.8's rule. The seed test and `test_reconcile_propagates_expiry_and_exhaustion_and_ends_the_capability` call `verify` after events that carry an outcome digest; the reported run passed.

A provider retry that leaves the state unchanged writes nothing: `note_invitation` returns when `state == state_before`, and create / reconcile / revoke select only rows still in a prior state. A later change (`pending` → `unknown` → `failed` → `confirmed`) is a new transition and a new event. `ended_remotely` hardcodes `state_before` as `pending` only for a revoke row `record_revocations` just inserted as `pending`.

The new table's ACL matches `handle_index` and `code_index`: owner privileges, no RLS, no application grant. `catalog-v21.json` contains the new kinds and the index. Checksums in `manifest.json` match the files.

Non-blocking residue: the comment above the provider state machine (`admission_operator.py`, the paragraph ending "D7.8's operator kinds are closed and name none") is stale. The code writes the events. `seed-library` still returns `audit_event: null`; the ticket does not name that command.
