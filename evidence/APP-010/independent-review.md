# APP-010 independent review: typed definitions, publication and saved work (plan Task 6, migration 0005)

| Item | Value |
| --- | --- |
| Reviewer | Independent reviewer (Claude), not the implementer. Read-only apart from this file; nothing committed. |
| Date | 2026-10-29 |
| Revision reviewed | `phase-one-spikes` at `3f918fb`. The APP-010 commits are `d9940d1` and `0a62122`, merged in `6d877e6`. The fixture provenance fix `faf4b6c` is included. No APP-010 source, migration, catalog or runtime file changed between `6d877e6` and `3f918fb`. |
| Controlling text | `Tickets/2-in-progress/APP-010.md` acceptance. ADR-007 D7.3, D7.6 (A1), D7.8, D7.9 and D7.12. ADR-004. ADR-002. `docs/planning/PHASE-1-EXECUTION-PLAN.md` Task 6. Design §5.3 write path and C09. |
| Author evidence | `evidence/APP-010/README.md` and its JSON reports |
| AWS | Not touched. |

SHA-256 of the reviewed files:

```
e92c593ed255e745db6be6c9d22aa55cdd4cd0f2ea7dc3130075a3aad7135af6  app/src/storage/definitions.rs
0d3ecadf7b68e238bc367bbcdd345e30f94f00534a8a428f6c687ebdd5b497a2  app/src/storage/definition_qualification.rs
eeba7ab4917fe9a16672e3f696b07fb10d50fa0add3aa5f9df119ca409a22174  app/src/storage/authorization.rs
eb1230fce5cfea313bfbb7745b29ef25f5ee574d053ec352e5acdba5d607eae7  app/src/storage/pool.rs
541e4e0b26037bd295e190c90197c3ec5f1691102ed14318d443d4268588c893  app/migrations/0005_definition_work_grants.sql
02e1c79b1a7be6e5a42d04fe3f5ddaee4463a3a57b5fca0f9d36ffb22421e6ff  app/migrations/catalog-v5.json
b855e4bba6b94f858b3ea377c4a2b6d50c5c6c311e8292a7fedd8780bf9bab09  app/runtime/qualify_tenant_schema.py   (equals schema-campaign.json)
c9073aee151f25006219acb15d0e7ac35c01a76751ee3392c02f8f26aa50d4ad  app/runtime/tenant_seed.py             (equals schema-campaign.json)
```

## Verdict

**Changes requested.** I found no blockers.

The grant layer is exact, and so are the RESTRICTIVE chain bindings. Re-admission inside storage holds, and so does the edition-2 refusal. The audit chains verify. The 40001 retry is real, and so is `UnknownOutcome` on a transport loss at COMMIT. I provoked the untested COMMIT paths myself in a scratch copy, and all three behaved as D7.6 requires.

Two majors remain:

- **M1** is a code defect. When publications share a second, `PublishResult::Stale` can name a publication that is not the newest one.
- **M2** needs a decision from the coordinator, not a code change in this slice. Phase 1 has no product path that grants permissions on an app or saved work that a user has just created. As things stand, the creator of a saved work cannot read it back.

M1 should be fixed before APP-010 is marked Done. M2 does not block APP-010's acceptance, but it does block APP-013's S05 run and the beta's real save-and-use flow.

## What I ran

All commands ran at `3f918fb` with `CARGO_BUILD_JOBS=2`. `PYC` is the README's wrapper: the runtime venv with the contracts venv's site-packages on the path.

| Command | Result |
| --- | --- |
| in `app/`: `cargo +1.93.1 test --locked --lib` (`admin/dist` was already built) | exit 0. 74 passed, 7 ignored. The 3 `definitions::tests` and 15 `admission::tests` pass. The README's count of 70 predates later commits. |
| in `app/`: `cargo +1.93.1 fmt --check` | exit 0 |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`. Every pin is 5, the catalog bound is 524,288 everywhere, and the `catalog-v2`–`v5` CHECKs match. |
| `PYC -m unittest discover app/runtime` | 104 tests OK. The README recorded 99 before later commits. |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | OK |
| `PYC -m unittest discover -s fixtures -p 'test_*.py'` | OK. The provenance drift (open question 4) is gone after `faf4b6c`. |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PYC GENUI_DEFINITION_REPORT=<scratch> cargo +1.93.1 test --locked --lib storage::definition_qualification::native_definition_campaign -- --ignored --exact --nocapture` | 1 passed, `passed: true`, 13 checks. `docker ps -a` afterwards showed only `gatewasm-db-test-1`. |
| Catalog delta: `catalog-v4.json` vs `catalog-v5.json`, section by section | Only three changes. `column_acl` gains 132 tuples and removes none. Both version CHECKs move to `BETWEEN 1 AND 5`. Six policies are added. `table_acl`, `function_acl`, relations and triggers are unchanged. |
| Manifest and file hashes | The 0005 and `catalog-v5.json` hashes equal `manifest.json`. Both Rust files and both migration artifacts are in the root `.dockerignore` allowlist. Both Rust files and `catalog-v5.json` are in the qualification-image allowlist, and both Rust files are in `managed_qualification.rs` `SOURCES`. |
| **Review probe (scratch copy only, not in the repository).** I copied the repository to the scratchpad, added a `#[cfg(test)]` `commit` checkpoint just before `COMMIT` in `Lease::commit_serializable`, and added one `#[ignore]` test, `review_commit_probe`. It builds S05 with `spend_review_s05` and runs three cases. | All three cases passed. The container was removed. Details are under "Claims I failed to refute". |

The probe's three cases were:

- **(A)** While the publication is paused before COMMIT, a superuser SERIALIZABLE transaction reads `genui.publications`, inserts policy version 2, and commits first. This dooms the publication through SSI.
- **(B)** The same thing happens on both attempts.
- **(C)** The publication's backend is terminated with `pg_terminate_backend` while it is paused before COMMIT.

I did not re-run the native, admission or schema Docker campaigns, the reset trial or the MSRV check. The README's recorded results stand for those. The schema campaign's recorded source hashes equal the current files.

## Findings

### Blocker

None.

### Major

**M1. `Stale` names an arbitrary publication when publications share a whole second.**

- **Where:** `app/src/storage/definitions.rs:1020`, in `current_revision`: `ORDER BY published_at DESC,publication_id DESC LIMIT 1`.
- **Why it happens:** `published_at` has whole-second precision (contract and `fresh`, `definitions.rs:929`). Within a second, the tiebreak is a random UUIDv4.
- **Why it matters:** `Stale` only arises when this publication lost twice to concurrent publishers. The winners were therefore committed within the last few hundred milliseconds, so a same-second tie is the expected case, not an edge case.
- **Observed in the probe:** in its first run, case B returned `Stale` naming the older S05 publication (`e60b…`). It should have named the publication committed moments later in case A (`e4c2…`). A second run happened to cross a second boundary and named the right one.
- **Why the campaign misses it:** check 9 hides the problem by sleeping 1.1 s before the newest publication (`definition_qualification.rs:826`, comment "Whole-second published_at").
- **Contract at stake:** D7.6 and D7.9 say the 412 names the current revision.
- **Fix:** give publications of an app a monotonic order that the control role can read. Two ways:
  - Add a per-app publication sequence column (or a `published_seq` taken from the control chain's `chain_sequence`, which the publisher already holds under `FOR UPDATE`) in 0005 while 0005 is still unapplied to any managed database, and order by it.
  - Or, if the coordinator prefers no schema change, record in ADR-007 that `Stale` names "a publication of the newest second". Then drop the sleep from check 9 and assert set membership instead.

**M2. Decision needed on open question 2: nobody can grant authority over a newly created app or saved work.**

- **The rule:** D7.3 lets only `admit_redemption` and the operator channel write permission rows.
- **The effect:** `create_work` (`definitions.rs:639`) saves a work that its own creator cannot read. `read_work` requires `record_read` on the work, and APP-013 commands will require `record_write`. The S05 builder writes those rows with fixture-only SQL after `create_work` (`definition_qualification.rs`, `spend_review_s05`, the `work_permissions` insert). The app shell and its `edit_definition`/`run` grants are also fixture SQL.
- **What this blocks:** this is not an APP-010 acceptance failure, because the ticket asks for "a separate work item with compatible source bindings", and that exists. But no product user can save work and then use it. APP-013's S05 and the beta flow depend on that.
- **Options for the coordinator:**
  - (a) An ADR-007 amendment. `create_work` would also insert the creator's own `record_read`/`record_write`/`share_work` rows for the new work. The grant would be column-limited and bound by a RESTRICTIVE policy (`principal_id = current principal`, work exists with no prior permissions), and `work.saved` would carry it.
  - (b) A closed APP-077 operator subcommand per new app and work. This is not usable for self-service.
- **App creation:** for the reason below, app creation in Phase 1 can only be an operator subcommand. A workspace-inserted app would carry no grants, so its owner could not even draft.

### Minor

**m1. The 40001-at-COMMIT and transport-at-COMMIT paths are untested in the repository, and the README describes them inaccurately.**

- The README says a 40001 at COMMIT "goes through the same classification" (`README.md:85`). In fact it is a separate function, `Lease::commit_serializable` (`pool.rs:442`).
- My probe confirmed that the behavior is correct:
  - (A) one retry, then `Published`, with `SERIALIZATION_FAILURES` +1 and publications +1.
  - (B) two failures, then `Stale`, with no publication left.
  - (C) `Err(UnknownOutcome)`, no retry, failure counter unchanged, no publication.
- **Fix:** add a `#[cfg(test)] self.checkpoint("commit")` before `COMMIT` in `commit_serializable`, and the doom recipe above, to `native_definition_campaign`:
  - Superuser `BEGIN ISOLATION LEVEL SERIALIZABLE; SELECT count(*) FROM genui.publications …; INSERT INTO genui.publication_policies … version n …; COMMIT`.
  - For the transport case, `pg_terminate_backend` on the paused `genui_control` backend.
  - A trigger-based injection would not work, because the pool's catalog verification refuses any added `genui` trigger.

**m2. Open question 1: unused INSERT grants on `apps` (workspace) and `publication_policies` (control).** My judgment is to drop both from 0005 now and note the move in ADR-007.

- D7.3's governing sentence is "added by the migration of the ticket whose transaction and audit implementation uses them, never earlier".
- 0005 already reads the D7.3 table as a cumulative end state, because it correctly omits APP-009's rows. So the literal table does not require these two grants in 0005.
- No Phase 1 path can use either grant:
  - App creation must also write permission rows (M2), which D7.3 reserves for the operator.
  - Policy changes are `admin_policy` governance with no path in Phase 1.
- The control INSERT on `publication_policies` matters more than the apps grant. The newest policy version selects the publishing authority (`definitions.rs:911`), so this is a write on the governance table that `publish` trusts.
- **Fix:** remove `0005:22` and `0005:39`, regenerate `catalog-v5.json`, and update `DEFINITION_WORK_GRANTS`, the `genui_control inserts policy rows` probe and the pins. This is cheap only while 0005 is unapplied to any managed database.

**m3. The publisher chooses the policy.**

- `publish` takes the policy id from the caller and honors the newest version of any policy row in the organization (`definitions.rs:911`).
- C09 says the server applies the organization's policy and "not permission for authors to change policy". Design §5.3 requires that "the policy version is current".
- This is not exploitable in Phase 1, because each organization has exactly one `owner_publish` row. Once an `independent_review` policy coexists with an `owner_publish` row, however, an owner could bypass review by naming the latter.
- **Fix:** before a second policy is seeded, resolve the applicable policy server-side (per organization or space) and refuse any other id. Record this as a Phase 1 limit in the README.

**m4. The `independent_review` branch is unexercised** (`definitions.rs:916`, and the owner ≠ approver check after it). No campaign seeds an `independent_review` row or grants `review_publish`. Add one positive and one negative case (owner refused, non-owner reviewer accepted) when convenient. Design G2 names Northstar's review policy.

**m5. The retry budget is not bounded by the D7.14 handler deadline.**

- In the worst case, two SERIALIZABLE attempts plus the `current_revision` read take 3 × 3 s, against a 5 s handler deadline.
- D7.6 says the retry happens "within the D7.14 handler deadline", but `publish` takes no deadline.
- **Fix:** record this for APP-009/012's HTTP mapping, or pass the remaining budget so that attempt 2 is skipped (and `Stale` returned) when it cannot finish.

**m6. The `publication.published` audit event does not identify the publication.**

- Its outcome digest is the definition digest (decision 6), and the event row has no publication column.
- The campaign's five publications of one revision therefore produce five events that differ only in event id and time, and none of them binds the policy, the approver or the envelope.
- **Fix:** propose a `forma:publication:v1` envelope digest domain for the next contract edition (an ADR-002 change). Until then, list this as a limit.

**m7. Plan Task 6 deviation.** The plan asks that Spend Review's "app, publication and saved work" be created in Northstar "through this admitted path". The app shell, its grants and the expenses source are fixture SQL. This is acceptable, since no admitted path for app shells exists, and the README records it. The ticket log should name the deviation when the ticket moves.

**m8. Stale evidence text.**

- The README's test counts (70 lib, 99 runtime) and its pre-existing fixture-drift section predate `faf4b6c`. `reset-trial-blocked-summary.json` is now historical. Add a dated note.
- The plan's global constraint "must stay under 262,144 bytes (v2 is 123,415)" predates the D7.12 cap move. The measure that counts is v5's body of 218,213 bytes against the 419,430 guard (52% of 524,288).
- ADR-007's estimate of 200–220 KB at v7 is already reached at v5. At about 11.6 KB per migration, v7 projects to about 241 KB, which is still well inside the guard.

### Open questions: judgments

| # | Question | Judgment |
| --- | --- | --- |
| 1 | Unused INSERT on `apps` / `publication_policies` | Drop both now and record the move in ADR-007 (m2). "Never earlier" controls, and 0005 already treats the D7.3 table as cumulative. |
| 2 | No product path for app creation or for grants on new apps and work | Coordinator decision needed (M2). In Phase 1, app creation can only be an operator subcommand. The saved-work grant needs either a narrow ADR-007 amendment or an operator subcommand. |
| 3 | `contracts/README.md` note moved to evidence | Accepted. `contracts/README.md` is locked in `contracts/implementation-lock.json:24`, and ADR-002 makes any change to it a new edition. Carry the note into the next edition. |
| 4 | Fixture provenance drift | Resolved by `faf4b6c`; fixture unit tests now pass. I did not re-run the reset trial. Mark the blocked summary as historical (m8). |

## Acceptance items

| Item | Status | Evidence |
| --- | --- | --- |
| Persist a minimal immutable canonical definition digest naming the supported schema, renderer and operator versions, and create a separate work item with compatible source bindings | **Met** | Campaign checks 3–5, re-run. The digest is `sha256:8c815d7e…146d9`, and the publication carries `renderer_version`/`operator_version` 1 and `schema_version` 1 (0002 CHECKs). Immutability holds because no UPDATE or DELETE grant exists on any of these tables: the catalog delta adds INSERT and audit-head columns only, and role-separation check 8 confirms it. The work binds Northstar's expenses revision, whose contract digest equals the published input's (code check and FK). |
| Unknown or executable fields, over-complex definitions, undeclared operators and client-supplied grants fail whole-output admission with a coded reason | **Met** | Campaign check 2 covers six coded refusals plus refusals of a forged, a misnamed and an edition-2 `AdmittedDefinition`, and the row footprint is unchanged. `admission::tests::rejects_over_complex_definitions` (25 views, 9 operations, 256 KiB + 1) and `unregistered_renderer_or_operator_version_refuses` pass. |
| Reload the admitted definition and saved work after restart; the reusable publication contains input contracts and does not embed the author's private evidence binding | **Met, with a limit** | Check 5: the envelope carries no source, revision, work or collection identity. Check 11 reloads byte-identically after every pool closes and new `Stores` are built. Limit: the tmpfs database itself is not restarted (README). |
| Plan Task 6: 0005 grants per D7.3, `catalog-v5`, pins, allowlists, `begin_serializable`, one 40001 retry, `Stale`, the named storage tests, the Rust/Python cross-check | **Met, except M1, m1 and m2** | Grant exactness is verified against the catalog. `begin_serializable` refuses non-control pools (`authorization.rs`). The cross-check is check 6. The `contracts/README.md` note is moved (question 3). |
| Plan Task 6 Step 5: independent review | This file | Move APP-010 to In review after M1 is dispositioned. |

## Claims I failed to refute

- **The grant set is exact and column-limited.** v4 → v5 adds exactly 132 column-ACL tuples. That is 58 for workspace business inserts, 34 for control, 28 for `audit_events` INSERT across both roles, and 12 for `audit_heads` SELECT/UPDATE. There is no table-level grant, no UPDATE or DELETE on any business table, no `saved_work.version`, and no permission table. Nothing goes beyond D7.3's APP-010 rows. The two unused grants are covered by m2.
- **RESTRICTIVE policies bind chain and kind per role.** This covers INSERT on `audit_events`, UPDATE on the own head, and SELECT on the own head, which also covers `FOR UPDATE`. The catalog shows `a`/`w`/`r` policies exactly as 0005 states them. Campaign check 8 confirms that control's UPDATE of the workspace head affects 0 rows. The schema campaign proves the kind and chain refusals.
- **Publish retry semantics.**
  - A 40001 at a statement or at COMMIT is retried exactly once, in a new transaction on a lease that `recycle` re-verifies. The probe's case A covers the COMMIT half.
  - A second 40001 returns `Stale` and leaves nothing written (probe case B, campaign check 9).
  - A transport loss at COMMIT is `UnknownOutcome` and is never retried (probe case C).
  - Other COMMIT errors, and a timeout at COMMIT, are `UnknownOutcome` (`pool.rs:442`). This is conservative and acceptable.
- **The campaign's 40001 races really race, with these qualifications.** The racer pauses at the `publish` checkpoint, after its SERIALIZABLE snapshot and its publication inserts. The winner then commits and advances the control head. The racer's `SELECT … FOR UPDATE` on that head then fails with 40001 ("concurrent update"). `SERIALIZATION_FAILURES` increments only for `Attempt::Serialization`, which is set only when SQLSTATE 40001 is recorded (`authorization.rs`, `note`) or when `commit_serializable` returns `Ok(false)`. So the counts of 1 and 3 prove SQLSTATE-specific failures, not some other error. Two qualifications follow from this:
  - The only conflict source in practice is the organization-wide control head. Two publications of different apps in one organization also race, so `Stale` can be returned for an app whose own premise never moved, as D7.8 accepts.
  - A 40001 at COMMIT is reachable only through SSI pivots involving another SERIALIZABLE transaction, which is what the probe constructs.
- **Re-admission inside storage.** Every storage entry point re-admits a caller's `AdmittedDefinition` from its content, under the trusted organization and path app. It requires equality of all six fields, including `view_count`, before any transaction (`readmit`). The stored revision is re-admitted again at publish and read time, and its stored contracts must equal the derived ones. A forged, patched, misnamed or mis-versioned value is `Denied` with nothing written.
- **Edition-2 refusal.** Re-admission always uses edition 1, so an edition-2 claim fails on `schema_version`. A unit test and campaign check 2 both cover this.
- **Audit events per operation, and chain verification.** One `definition.drafted` per draft, one `publication.published` per publication, and one `work.saved` plus one `work.bound` per binding. The hash matches D7.8 and the operator's vector (unit test). The operator `verify` recomputes both chains from genesis (campaign check 12: 4 workspace and 5 control events).
- **`create_work` authority.** `source_read` is re-read after the fence for each source. A live revision is required, and its contract digest must equal the published input's. Every published input is bound exactly once. Records bindings require equal `fields`. Cross-organization sources are refused (check 7).
- **Catalog budget.** The v5 body of 218,213 bytes is under the 419,430 guard and the 524,288 cap. v1–v4 regenerate byte-identically (`generation.json`).
- **Pins.** All five pins agree at 5 (`check-schema-pins.py`). The native campaign's version literals moved with them.

## Cleanup

Every container the definition campaign and the probe started was removed. `docker ps -a` showed only `gatewasm-db-test-1` before and after. The probe's scratch copy stays under the session scratchpad (`…/scratchpad/app010-review/repo`) together with its logs `probe.log` and `probe2.log`. Its build artifacts went to `app/target`, which is build cache and untracked. The repository working tree is unchanged apart from this file.
