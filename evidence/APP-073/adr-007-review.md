# ADR-007 independent review

Date: 2026-10-29. Reviewer: independent reviewer (Claude subagent) who did not author ADR-007. Stance: try to refute each of D7.1–D7.17 against the controlling records and the code; only findings with a path:line citation are listed.

- **Reviewed file:** `docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md`, SHA-256 `4e719606629dbb8715c60af6ee0b6adb9f37213bd1f91e8696adf1d71881615a`. These bytes equal the blob at `phase-one-spikes` HEAD `cff5d6f` ("Draft ADR-007 write authority…").
- **Method:** read-only review of the repository. No cargo, docker, aws, git mutation or commit. To check PostgreSQL behavior, I ran a throwaway local PostgreSQL 16.13 cluster (Homebrew) in a session scratch directory outside the repository, on loopback only, and stopped it afterwards. Appendix A gives the statements and the observed results. These are reviewer reproductions. They are not harness, campaign or CI evidence.
- **Not inspected:** `docs/architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md`, which appeared untracked while this review was in progress.

## Inspected files (SHA-256, first 16 hex)

| File | SHA-256 |
| --- | --- |
| docs/architecture/ADR-002-CONTRACT-EDITION.md | 5bba501cbd3e9b60 |
| docs/architecture/ADR-003-PUBLIC-BETA-ACCESS.md | e76e83a1c0c00cb1 |
| docs/architecture/ADR-004-TENANT-TRANSACTIONS.md | d14eae1dc43fdf8b |
| docs/architecture/ADR-005-PHASE-1-SEQUENCING.md | 0da93711f928fd5f |
| docs/architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md | b0ec0c0dcca10ecb |
| docs/architecture/DECISIONS.md | 46a0a2f51a48ab5f |
| design/forma-generative-app-platform-v2/design.md (§§4 SR10, 6.1–6.4, 8, 9, 10.5–10.6) | c339b8f6d1dbe522 |
| design/forma-northstar-demo/SCENARIOS.md (S05) | 0adc2d6a83df03f5 |
| app/migrations/0001_role_foundation.sql | f98f67b7b5164836 |
| app/migrations/0002_tenant_foundation.sql | 7b163e780756f03a |
| app/migrations/catalog-query.json, catalog-v2.json, manifest.json | e17ceb5e62dc6f03, 51418a12cf45809b, 13976faa01ee527a |
| app/src/storage/mod.rs, authorization.rs, pool.rs, catalog.rs | 5c2c43fc192571fb, c53609ef588351a3, 4a8eae301412ed8c, 8c0462983afb7e7d |
| app/src/readiness.rs | aff294091508d86e |
| app/runtime/runtime.py, tenant_seed.py, qualify_tenant_schema.py | dfef99b5f60fe225, 66dca95fcb22df5f, 84ac10d54a201de4 |
| contracts/schema.json (`command_request`, `command_outcome`, `query_receipt`, `value`), fixtures/northstar.json, fixtures/spend-review.json | 2ccc496d5ef1ec50, 45639ea369193d60, 4a4581f5a7fcfec8 |
| evidence/APP-008/schema-plan.md, managed-plan.md | c5ef304ef491a59a, c9e2821f29e0d0cb |
| docs/planning/PHASE-1-EXECUTION-PLAN.md, QUALITY-GATES.md | edec4b285986f80c, d4010dc26f93d78b |
| spikes/APP-005/README.md, query.py; evidence/APP-005/README.md, dependency.json | 0468f5f60defabf5, d7d724b9e34c3bdb, 12ec7ab3815ed1fe, (version 2.0.0 at :3) |
| infra/src/stack.ts | 8d3058943c360627 |

## Summary

There are 2 blockers, 10 major findings and 13 minor findings.

- **Blocker B1:** the D7.1 grant set cannot perform the row locks that D7.1 itself requires. PostgreSQL refuses them, and I reproduced the refusal.
- **Blocker B2:** the pending-admission and reservation write path is missing, but `admit_redemption` and Tasks 5, 7 and 8 depend on it.

Most major findings are places where ADR-007 changes a controlling rule without naming the change, or where a decision's text cannot express what the decision promises.

---

## Blockers

### B1 — D7.1's grant set cannot take D7.1's own row locks

**Decision:** D7.1 (ADR-007:18, :19, :21).

**Evidence:**
- `admit_redemption` "locks `organizations FOR SHARE`; locks the pending admission and its `access_codes` row `FOR UPDATE`" (ADR-007:18). `advance_member_epoch` "Locks `organizations FOR SHARE` then the membership `FOR UPDATE`" (ADR-007:19).
- The grants for `genui_admit` include UPDATE only on `access_codes(redeemed_count,status)` and `memberships(membership_epoch)` (ADR-007:21). There is no UPDATE column on `genui.organizations` and none on the pending-admission relation. The Consequences paragraph says the campaign asserts "`genui_admit` holds exactly its grants" (ADR-007:98).
- In PostgreSQL, a row-locking SELECT requires UPDATE privilege on at least one column (evidence/APP-008/schema-plan.md:80). The repository relies on this rule: `genui_lock` receives `UPDATE(status)` on organizations for exactly this reason (app/migrations/0002_tenant_foundation.sql:236-237).
- I reproduced the refusal on PostgreSQL 16.13. I used 0002's policy text and a NOBYPASSRLS definer holding grants shaped like D7.1:
  - `organizations … FOR SHARE` failed with `ERROR: permission denied for table organizations`.
  - `FOR UPDATE` on a reservation table where the definer holds INSERT only failed with `ERROR: permission denied for table code_reservations`.
  - Adding `UPDATE(status)` on organizations made the first lock succeed (Appendix A: T1a, T1b, T1f).

**Why it blocks:** as written, every redemption and every sign-out epoch advance fails. The fix is an authority decision: some definer must hold a column on `organizations` that also permits suspension. ADR-004:30/:32 constrain that choice, and ADR-007 says it leaves it unchanged ("`genui_lock` is unchanged", ADR-007:21; the ownership sentence at ADR-007:38).

**Fix (choose one and record it in D7.1):**
- **(a)** Grant `genui_admit` `UPDATE(status) ON genui.organizations` as a lock-enabling column only, mirroring 0002:237. Add to D7.1: "no `genui_admit` function body updates `genui.organizations`; the schema campaign asserts it". Amend ADR-007:38 to say `organizations.status` is held by `genui_lock` and `genui_admit` as lock-enabling columns only.
- **(b)** Add a `genui_lock` function `genui_private.lock_organization_share(p_org uuid) RETURNS boolean`. It checks the scope as 0002:247 does, takes `FOR SHARE`, and grants EXECUTE to `genui_admit` only. Then strike "`genui_lock` is unchanged".

In both cases, list every lock-enabling column the definer holds, including one on the pending-admission relation (B2).

### B2 — No write path for the pre-signup reservation and the post-signup pending admission

**Decisions:** D7.1, D7.3, D7.17.

**What the controlling records require:**
- The callback "records a bounded **pending admission** (a `code_reservations` row bound to the code and to the verified `(issuer, tenant_id, sub)`, 24 hours, counted against the code's remaining redemptions…)" (ADR-006:33).
- `/invite/validate` creates a 10-minute signup handoff (ADR-006:32).
- An expired, revoked or exhausted pending admission is "denied and recorded for operator reconciliation" (ADR-006:43).
- `admit_redemption` takes `p_pending_admission` and locks it (ADR-007:18). D7.17 fixes the 10-minute and 24-hour lifetimes, with the 24-hour admission "counted against remaining redemptions" (ADR-007:94).

**What ADR-007 leaves undecided:**
- No decision names the relation, its writer, its grants, its lock order, or how it is consumed and expired.
- The INSERT list for `genui_admit` (ADR-007:21) excludes it, and the D7.3 table (ADR-007:33-36) gives no role a write on it.
- The Consequences paragraph says "no role beyond the table above holds any write" (ADR-007:98).

**Consumers that depend on it:**
- The plan already calls the missing path: `Workspace::reserve_code(…)` and `record_pending_admission(…)` (docs/planning/PHASE-1-EXECUTION-PLAN.md:158, :189, :205).
- It also defines `genui.code_reservations(organization_id, reservation_id, code_id, context_digest, expires_at, released_at)` (plan:142), which lacks ADR-006:33's identity binding.

**Why a definer is needed, and an arithmetic trap:**
- To count a pending admission against remaining redemptions safely, the writer must lock `access_codes FOR UPDATE`. The workspace role cannot do that (no UPDATE privilege), so the write has to go through a definer.
- "Remaining count" is not defined. If the pending admission being redeemed counts against itself, a single-use code returns `exhausted` to the legitimate redeemer: 1 − 0 − 1 = 0.

**Why it blocks:** ADR-007:7 exists so that no ticket "invent[s] its own write path". The core admission flow (APP-073, APP-009 and APP-074) has none, and D7.1 depends on it.

**Fix:** add the following to D7.1, with EXECUTE granted to `genui_workspace`, and add the relation to D7.3 and to the Consequences paragraph.

- **`genui_private.reserve_code(p_org uuid, p_code_digest text, p_context_digest text) RETURNS TABLE(reservation_id uuid, expires_at timestamptz, outcome text)`**
  - Requires the transaction scope to equal `p_org`.
  - Locks `organizations FOR SHARE`, then `access_codes FOR UPDATE`.
  - Refuses expired, revoked and exhausted codes with the same neutral outcome.
  - Inserts a 10-minute pre-signup row bound to `p_context_digest`.
  - The ADR must state whether this row counts against remaining redemptions (ADR-003:49) and the cap on live reservations per code.
- **`genui_private.record_pending_admission(p_org uuid, p_reservation uuid, p_issuer text, p_provider_tenant text, p_provider_subject text) RETURNS TABLE(pending_admission_id uuid, expires_at timestamptz, outcome text)`**
  - Locks `organizations FOR SHARE`, then `access_codes FOR UPDATE`, then the reservation `FOR UPDATE`.
  - Requires a live reservation, and `max_redemptions − redeemed_count − live pending admissions ≥ 1`.
  - Inserts a 24-hour row bound to `(issuer, tenant, subject)` and consumes the reservation.
- **Changes to `admit_redemption`:**
  - Use the same lock order: organizations, then `access_codes`, then the pending admission. ADR-007:18 currently locks the pending row before the code.
  - Exclude `p_pending_admission` from the remaining count.
  - Mark the pending admission consumed in the same transaction.
  - On expiry, record the state `denied_for_reconciliation` (ADR-006:43).
- **Grants:** `genui_admit` gets INSERT and `UPDATE(state, consumed_at)` on the relation; this is both the lock-enabling column and the lifecycle column. No application role gets a write.
- **Relation:** name it (ADR-006 calls it `code_reservations`) with these columns: kind (presignup or pending), code_id, context_digest, issuer, provider_tenant_id, provider_subject, state, expires_at, consumed_at.

---

## Major

### M1 — D7.6 changes ADR-004's isolation and no-retry rules while saying they are unchanged

**What ADR-007 says:**
- "No automatic mutation retry" is listed among the unchanged ADR-004 bounds (ADR-007:3).
- D7.6 nevertheless adds "publication writes with one retry, then 412/409" (ADR-007:50).

**What the controlling records say:**
- ADR-004 fixes `BEGIN READ COMMITTED` (ADR-004:20), forbids automatic business retry (ADR-004:24) and says "No mutation is automatically retried" (ADR-004:42). The plan repeats this (plan:23).
- The design does require the retry: "the publisher retries once, then 412" (design.md:289). So ADR-007 must amend ADR-004 explicitly. The design also says 412, not "412/409".

**Implementation interaction:**
- `Lease::finish` maps any COMMIT failure to `Error::UnknownOutcome` (app/src/storage/pool.rs:426-431).
- `Transaction::query` maps every SQL error to `Unavailable` (app/src/storage/authorization.rs:138-143).
- Serializable conflicts are often raised at COMMIT. So, as built, a serialization failure cannot be told apart from an ambiguous commit, and ADR-004 forbids retrying an ambiguous commit.

**Fix:** replace D7.6 with the following text, and list the amendment in ADR-007:3.

> "D7.6 amends ADR-004 §Request and operation states (READ COMMITTED) and §Pool and driver lifecycle (no automatic retry) for one case only. A control-role publication transaction that fails with SQLSTATE 40001, at any statement including COMMIT, has definitely rolled back. It is retried once in a new transaction on a freshly verified lease, within the D7.14 handler deadline. A second 40001 returns 412 naming the current revision (design §6.2). A commit transport failure remains `UnknownOutcome` and is never retried. The storage module classifies 40001 at COMMIT separately from transport ambiguity."

### M2 — `advance_member_epoch`: no context check, unreachable revocation reasons, and an unnamed move of revocation authority to workspace

**Evidence:**
- **No context check.** ADR-007:19 accepts any `p_principal` and states no transaction-context check. Compare 0002:247-249 and ADR-004:32, which "checks supplied org/principal/epoch against transaction context". RLS blocks cross-organization effects (Appendix A, T1g), but any workspace transaction can still bump another member of the same organization and write that member's audit event.
- **Revocation belongs to the operator and control.** `entitlement.revoked` and `membership.revoked` are operator actions (ADR-003:51). The design places membership changes and their audit with control:
  - "the publisher takes his membership row `FOR UPDATE`" (design.md:263);
  - the publisher "appends the governance audit: … membership" (design.md:273);
  - "membership, revocation … (`control`)" (design.md:672).
  
  The schema plan anticipated the authority mutation under control (evidence/APP-008/schema-plan.md:65).
- **Wrong caller and no effect.** Under ADR-007's grants, EXECUTE goes only to workspace (ADR-007:19), which is not the revoking authority; ADR-007 grants the operator channel no EXECUTE on these functions. `genui_admit` has no `UPDATE(status)` on memberships and no privilege on entitlements (ADR-007:21). So these two reasons can be invoked only by the session tier, and they would record a revocation that this function cannot actually perform.

**Fix:**
- Restrict `p_reason` to `('session.sign_out','session.sign_out_all')`.
- Before locking, require `genui.organization_id = p_org`, `genui.principal_id = p_principal`, and `genui.membership_epoch` equal to the current epoch, as 0002:247-249 does.
- Specify entitlement and membership revocation as an operator-channel transaction, or a control-only definer, in this order: organizations `FOR SHARE`, member `FOR UPDATE`, status change with epoch + 1, then the audit row.
- Add one sentence that names the deviation from design §6.1/§10.6 and schema-plan:65 for the workspace-invoked admission path. ADR-003:25 and ADR-006:33 justify the admission callback; nothing justifies moving revocation.

### M3 — D7.8: the chain has no defined tail, and the advisory lock does not serialize SERIALIZABLE appenders

**(a) No tail.** ADR-007:58 has no chain position and no head. `occurred_at` (transaction-start `now()`) can tie and can be out of append order, so `previous_hash` has no defined source. The design takes the advisory lock "on its head" (design.md:305).

**(b) The lock does not refresh a SERIALIZABLE snapshot.** Control publications run SERIALIZABLE (ADR-007:50; design.md:289) and append governance audit (design.md:271, :273). A SERIALIZABLE snapshot is taken before the advisory lock is acquired. I reproduced both consequences on PostgreSQL 16.13 (Appendix A, T2):
- With two SERIALIZABLE appenders, the second fails with `40001`.
- With one READ COMMITTED and one SERIALIZABLE appender on the same chain, both commit and the chain **forks**: two rows share `previous_hash = h0`.

D7.1's admission and revocation events come from READ COMMITTED callers, and design.md:672 puts those events on the control chain. That is exactly the mixed case.

**(c) Roles are not bound to kinds.** `writer_role` is a value CHECK only. Workspace, control and data all receive INSERT (ADR-007:33-35), so any of them can insert any `writer_role` and any kind. The design requires "insert-only by role and event kind" (design.md:305) and "insert-only roles, each its kinds" (design.md:672). In addition:
- the kind list is only described as "closed list" (ADR-007:58), not enumerated;
- operator actions (ADR-003:51) have no `writer_role`;
- the chain that `genui_admit` writes is not stated.

**(d) The hash input is undefined.** "Canonical event bytes" and the genesis `previous_hash` are not specified. ADR-002:19 requires domain-separated identities.

**Fix:**
- Add `chain_sequence genui.safe_counter NOT NULL`, `UNIQUE (organization_id, writer_role, chain_sequence)` and `UNIQUE (organization_id, writer_role, previous_hash)`.
- Append through a head row `genui.audit_heads(organization_id, writer_role, chain_sequence, head_hash)` taken `FOR UPDATE`. In my reproduction, READ COMMITTED appenders serialize correctly and a SERIALIZABLE appender gets `40001` instead of forking (Appendix A, T3).
- Alternatively, keep the advisory lock, require every appender on a chain to use READ COMMITTED, and add the unique `previous_hash` guard (Appendix A, T3 variant 5).
- Add RESTRICTIVE INSERT policies per role that bind `writer_role` and kind to `current_user`, including `genui_admit`'s chain.
- Enumerate the kinds per role in the ADR.
- Define `hash = sha256("forma:audit-event:v1" ‖ 0x00 ‖ previous_hash ‖ forma-json-v1 event)` and a fixed genesis value.
- State which chain operator events use.
- Head rows are created together with the organization (M7).

### M4 — D7.9's typed results cannot express D7.9's HTTP mapping, and the S05 status is ambiguous

**Conflict versus Stale is undefined (ADR-007:62).**
- The design says: "A conflict, expected version behind or a key reused with another payload, is 409 … A stale patch, approval or adoption is 412" (design.md:520).
- S05 says: "A distinct stale request expecting version 0 conflicts against version 1" (design/forma-northstar-demo/SCENARIOS.md:76). The plan expects "stale ik-3 → 409" (plan:25).
- If `Stale` is read as "expected record version behind", the S05 acceptance case returns 412.

**"Same key in flight → 409 `Retry-After: 2`" has no source type.** `CommandResult` has four variants and none of them means "in flight" (ADR-007:62). `Error` has five variants (app/src/storage/mod.rs:15-21).

**Several lock waits are unmapped or mapped against the design.**
- Waits past `lock_timeout=1000` (pool.rs:500) on `organization_sequences` (D7.7) and on the record row are not mapped.
- Mapping fence-wait timeouts to 503 contradicts design.md:520, which reserves 503 for a provider outage or a cold query whose store or key is unreachable. It also contradicts the plan's own review focus #4 (plan:37), yet Task 11 asserts the 503 (plan:256).

**Other gaps:**
- D7.6's 412 has no typed result: `publish(...) -> Result<Publication>` (plan:174).
- Today every SQL error is erased to `Unavailable` (authorization.rs:138-143), and every COMMIT error to `UnknownOutcome` (pool.rs:426-431). D7.9 therefore needs internal SQLSTATE classification, which the ADR does not state.

**Fix:**
- `Conflict(RecoveryView)` means an `expected_record_version` mismatch, or the same key with a different `payload_digest`. It maps to 409.
- `Stale(CurrentRevision)` means the saved work's publication or version, or the publication premise, has moved. It maps to 412 naming the current revision.
- Add `CommandResult::InFlight`, mapped to 409 with `Retry-After: 2`. It is produced by 55P03 at the idempotency claim (M5). Map 55P03 on `organization_sequences` and on the record row to `InFlight` too.
- Keep 503 for transport and connection failure and for `UnknownOutcome`.
- Add `pub enum PublishResult { Published(Publication), Stale(CurrentRevision) }`.
- State that storage classifies 23505, 40001, 55P03 and 40P01 internally, while `storage::Error` keeps its five variants.

### M5 — `command_outcomes` is insert-only, but D7.9 and the design need an early unique claim row

**What the grants and contract allow:**
- The data role has INSERT only on `command_outcomes` (ADR-007:35).
- The contract fixes `state` to `"committed"` (contracts/schema.json:2018) and requires `commit_sequence` and `audit_event_id` (:2067-2068).

**What the design and ADR-007 rely on:**
- The design inserts the idempotency outcome under its unique constraint *before* the fences, "so a duplicate serialises on that row" (design.md:305, :524).
- ADR-007:62 relies on a "unique-row wait past `lock_timeout`".
- Task 10 lists both "outcome lookup … or insert" before the fences and "insert … outcome" after the record (plan:240).

**Why neither placement works:**
- A row inserted before the fences cannot yet carry `commit_sequence` or `audit_event_id`, and cannot be completed later without UPDATE.
- A row inserted at the end never produces a unique-row wait: the second request waits on `organization_sequences` instead.
- I reproduced that `lock_timeout` 1000 fires on an in-flight unique insert (Appendix A, T4). The designed 409 path therefore exists only if a row is inserted early.

**Fix:**
- Add an insert-only claim relation:
  ```sql
  genui.command_claims(
    organization_id, principal_id, app_id, work_id, command_id,
    idempotency_key, payload_digest, claimed_at,
    PRIMARY KEY (organization_id, principal_id, work_id, command_id, idempotency_key))
  ```
- Insert into it with `INSERT … ON CONFLICT DO NOTHING` immediately after the precheck, with no savepoint (ADR-004:23).
- If no row was inserted, read the committed outcome (`Replayed`) or detect a payload mismatch (`Conflict`).
- Insert `command_outcomes` at the end, with a foreign key to the claim.
- Grant the data role INSERT on both relations, and state that a rolled-back command releases its claim.

### M6 — D7.10 departs from the qualified APP-005 executor and from design §6.3 without recording the deviation

**What APP-005 qualified:**
- An operating-system boundary: "no mounted repository/home/credentials … the query worker has no network" (spikes/APP-005/README.md:15). The worker ran non-root, with no capabilities, a read-only filesystem, no network, a 3 GiB memory cap and a 2 GiB private tmpfs (:31). The evidence states "the container is the operating-system boundary" (evidence/APP-005/README.md:11, :24).
- A supervised broker that allows "two running and twenty queued operations; excess work returns a 429-class refusal", with a five-second backstop and scratch removal (spikes/APP-005/README.md:33; evidence/APP-005/README.md:14).
- A DuckDB policy of `"memory_limit": "1GiB"` and `"max_temp_directory_size": "2GiB"`, an exact source allowlist followed by `enable_external_access=false`, `lock_configuration=true`, and a per-query `temp_directory` (spikes/APP-005/query.py:27-30, :160, :173-174).

**What D7.10 specifies instead (ADR-007:66):**
- A fresh `python3 /app/runtime.py query` process for each request.
- `memory_limit='1GB'`, where the design says 1 GiB (design.md:303).
- No admission bound. The design says 8 per node, 2 per organization, and 429 beyond 20 (design.md:303).
- No temp or spill directory, and no cleanup after the 1.8 s kill.
- No allowlist or configuration lock; a "working directory" confines nothing.
- The helper itself fetches from S3 "through the tenant role".

**Credential exposure in the deployed task:**
- The helper would follow the existing probe pattern: the Rust binary spawns `runtime.py` with the parent environment plus `GENUI_RUNTIME_CONFIG_FILE` (app/src/readiness.rs:71-76).
- That file holds IAM tokens for all four database roles and both tenants' S3 credentials (app/runtime/runtime.py:247-252, :254-259, :265).
- It lives in the same writable `/run/genui` volume proposed for the cache (infra/src/stack.ts:138, :148).
- So the engine process can reach every credential and could open database connections outside the bounded pools (ADR-004, ADR-007:3).

**Capacity and latency:**
- There is one 8 GiB task (infra/src/stack.ts:132), with no limit on how many 1 GiB engines run at once.
- A fresh interpreter per request is the model whose warm six-view burst took 1.811 s in APP-005's CI (evidence/APP-005/README.md:36), which led to the broker.

**Deviations not named:**
- "One binary holds every component (D11)" (design.md:238).
- APP-D05, "One process initially" (docs/architecture/DECISIONS.md:23).
- `runtime.py`'s own scope: "never an HTTP/business API" (app/runtime/runtime.py:1).
- PH01's gate includes "parser/query confinement" (docs/planning/QUALITY-GATES.md:11).

**Fix:** add a "Deviation" paragraph that names design §6 D11, §6.3 and APP-D05, and states that APP-005's container boundary is not reproduced inside the Fargate task. Then require the following.

1. Copy APP-005's POLICY verbatim: 1GiB, 2GiB spill, the exact source allowlist followed by `enable_external_access=false`, `lock_configuration=true`, extensions off, and a read-only database.
2. Separate fetch from execution:
   - a credentialed step verifies `normalized_digest` and atomically renames the file into a read-only cache;
   - the engine child runs with `env_clear()` and a minimal environment: no `GENUI_RUNTIME_CONFIG_FILE`, no `AWS_*` variables, no inherited file descriptors;
   - each query gets a private scratch directory outside the cache, removed on exit or kill.
3. Admission: 2 running queries per organization, a per-task bound sized to the 8 GiB task, and 429 with `Retry-After` beyond the queue bound.
4. Interrupt at 1.8 s, then kill and reap with the qualified 5 s backstop.
5. APP-012 re-measures the per-request interpreter cost against design §5.2 before acceptance.

### M7 — `organization_sequences` rows (and any chain heads) have no creator, and the seeds already use `commit_sequence` 1

**Evidence:**
- The table is defined (ADR-007:54), but the data role holds only UPDATE on it (ADR-007:35).
- Both seeded organizations already hold record revisions at `commit_sequence` 1 (contracts/fixtures/northstar.json:563; contracts/fixtures/spend-review.json:443), written by app/runtime/tenant_seed.py:117-118.
- Migrations run as `genui_owner` under FORCE RLS and see no tenant rows without per-organization scope (tenant_seed.py:52, :69; 0002:218-219). A backfill such as `INSERT … SELECT FROM genui.organizations` in migration 0006 would therefore insert nothing.
- Starting the sequence at 1 would reuse sequence 1 in organization …0002. Receipts pin this snapshot sequence (SCENARIOS.md:80).

**Fix:** add to D7.7: "The operator provisioning transaction creates the organization's `organization_sequences` row, and its audit head rows, in the same transaction as the organization. For existing organizations, the operator package (APP-077) inserts the row per organization under scope, with `next_commit_sequence = 1 + max(internal_record_revisions.commit_sequence)` (1 if there is none). A command that finds no row returns `Error::Configuration` and writes nothing."

### M8 — The runtime catalog proof cannot see `genui_admit`

**Evidence:**
- Six sections of `app/migrations/catalog-query.json:6` filter on six fixed role names: `roles`, `memberships`, `effective_boundaries`, `role_database_settings`, `role_reachability` and `default_acl`.
- So, at every pool admission (catalog.rs:9; pool.rs:302-308), the following go unverified:
  - `genui_admit`'s LOGIN, BYPASSRLS and INHERIT flags;
  - its database privileges;
  - the `genui_migration → genui_admit` membership edge.
- The normalization that keeps the RDS and local catalogs equal applies only to "one of the six exact fixed roles" (ADR-004:60; evidence/APP-008/managed-plan.md:18; app/runtime/qualify_tenant_schema.py:208).
- There is no safe partial fix. Adding `genui_admit` to the lists without the normalization makes the RDS and local catalogs differ; leaving it out keeps the definer's attributes unchecked.
- ADR-007:98 adds only a test-campaign assertion.

**Fix:** state in D7.1/D7.12 that migration 0003:
- adds `genui_admit` to every fixed-role list in `catalog-query.json` and in `qualify_tenant_schema.py:208`;
- extends ADR-004 §Migration and recovery's normalization to `genui_admit`, as an explicit amendment;
- re-pins `catalog-query.json` in the eight pins (the v1/v2 snapshots stay byte-identical because the role does not exist there);
- records `GRANT genui_admit TO genui_migration WITH INHERIT FALSE, SET TRUE` and the temporary `CREATE ON SCHEMA genui_private`, with its revocation, as 0002:12 and :278-283 did for `genui_lock`.

### M9 — No pre-scope lookup for the organization sign-in handle

**Evidence:**
- The handle is "stored only as a digest on the organization row" (ADR-006:21), and `organizations` is under FORCE RLS keyed on scope (0002:21-24, :216-219).
- D7.5 says flows know their organization "from the sign-in handle" (ADR-007:46).
- D7.2 solves the pre-scope lookup only for access codes (ADR-007:25).
- The identity registry is closed to `{issuer, provider_tenant_id, organization_id}` (ADR-004:50).
- Task 7's `begin_flow` takes `organization` as an input, with no source for it (plan:190).

**Fix:** add a sibling of D7.2:
- `genui_private.handle_index(handle_digest genui.digest PRIMARY KEY, organization_id uuid NOT NULL REFERENCES genui.organizations, retired_at timestamptz)`. It has no RLS and is owned by `genui_owner`. The operator writes it in the provisioning or rotation transaction, and both handles stay live during rotation (ADR-006:21).
- `genui_private.resolve_handle(p_handle_digest text) RETURNS uuid`, owned by `genui_admit`, with EXECUTE granted to `genui_workspace`.
- The tenant slug and client ID are read after scope is set; an unknown handle gets the same neutral page.

Alternatively, amend ADR-004:50's registry contract explicitly.

### M10 — The plan's restatement of D7.1/D7.3 differs from the ADR that Tasks 4–10 consume

**Evidence:**
- **`admit_redemption` signature:**
  - plan:44: `(p_org, p_code_digest, p_issuer, p_provider_tenant, p_provider_subject, p_payload_digest, p_reservation)`;
  - ADR-007:18: `(p_org, p_pending_admission, p_issuer, p_provider_tenant, p_provider_subject, p_payload_digest)`.
- **Locking claim:** plan:44 says "Every function locks `organizations FOR SHARE` then the target `memberships` row `FOR UPDATE` … and writes exactly one audit event". This contradicts ADR-007:17-18: `resolve_code` locks nothing, and `admit_redemption` has no existing member row to lock.
- **Grants:** plan:46 grants `UPDATE(last_seen_at, revoked_at)` on `prelogin_flows`, a table that has no such columns (plan:189), so the GRANT would fail. ADR-007:33 grants that UPDATE only on `sessions` and `pre_access_sessions`.
- **Scope:** plan:162 adds "a sentinel principal", while ADR-007:21 installs organization scope only.
- **Which text controls:** plan:112/:114 say later tasks use "the reviewed names" and that the ADR holds D7.1–D7.17 "verbatim". Yet plan:142 builds "the three functions (D7.1)" without saying which text controls.

**Fix:**
- Replace plan:44-60 with "ADR-007 controls; this list is informative", or regenerate it verbatim from the corrected ADR.
- Align Task 4 with ADR-007:18.
- Remove the sentinel principal, or add it to D7.1 with its exact value and a guarantee that `lock_member_authority` (0002:247-249) can never accept it.

---

## Minor

**m1 — `code_index` is an unnamed exception.**
- ADR-007:3 says "FORCE RLS on every tenant relation" is unchanged, and D7.5 says "Every new relation is organization-leading" (ADR-007:44-46).
- Yet `code_index` (ADR-007:25) is a granted, tenant-bearing relation with no RLS and a digest-leading key, contrary to ADR-004:13.
- **Fix:** name it in ADR-007:3 and in D7.5 as the single exception, together with its compensating control: it is read only by `genui_admit` through `resolve_code`, and the campaign asserts this.

**m2 — D7.3 has no read grants.**
- The table (ADR-007:31-36) and the Consequences paragraph (ADR-007:98) cover writes only, while ADR-004:11 ("Initial application grants are scoped reads") and schema-plan.md:59 require minimum, named SELECT grants. Reads the ADR needs to assign:
  - `entitlements` by workspace, control and data (plan:158 adds an entitlement join to the shared precheck);
  - `access_codes` by workspace;
  - the session tables by workspace;
  - `command_outcomes` and `organization_sequences` by data (replay; the receipt's `record_snapshot_sequence`, contracts/schema.json:1759).
- **Fix:** add a SELECT column to the D7.3 table and assert it in the campaign.

**m3 — The lock order is incomplete.**
- **(a)** D7.7 does not say where `organization_sequences` sits relative to the record row. Task 10 puts it before `internal_records FOR UPDATE` (plan:240); state that in D7.7.
- **(b)** Sign-out deletes session rows and then takes the member row `FOR UPDATE` (ADR-006:36). A request that takes the member fence and then updates its own `last_seen_at` (granted at ADR-007:33) closes a cycle between the session row and the member row. Rule: session-row writes always precede the member fence, or sign-out advances the epoch before deleting sessions.
- **(c)** For code rows, use one order everywhere: organizations `FOR SHARE`, then `access_codes FOR UPDATE`, then the reservation or pending admission (B2). ADR-007:18 currently has the opposite order.

**m4 — D7.12's catalog budget has no estimate and will probably trip during the plan.**
- The cap applies to the database's `jsonb::text` body (`octet_length(body)<=262144` in catalog-query.json:6; catalog.rs:61), not to the file. That body renders v2 at about 125,966 bytes, against 123,415 for the file.
- My estimate, attributing the rows of catalog-v2 to relations:
  - about 4.5 KB per relation (range 1.9–7.0 KB), and 1.35–1.5 KB per small definer function;
  - the roughly twelve relations named for migrations 0003–0006 (ADR-007; plan:142, :189, :239), three larger definer bodies, and about 90 column-ACL rows for `genui_admit` add about 75–95 KB;
  - that puts v6 at about 200–220 KB, against the 209,715 guard.
- Row limits are not at risk: the largest v2 section has 263 rows against a limit of 1,024.
- **Fix:** decide the cap move once, in Task 3: catalog-query.json, `BOUND` in catalog.rs, runtime.py:326/:345/:350, and the SQL literal. Define the 80% guard on the `jsonb::text` body length, not the file size.

**m5 — Version CHECKs and pins.**
- catalog-v2.json:305 pins `CHECK (version >= 1 AND version <= 2)`.
- plan:130 tells `initialize_migration_history` to write `BETWEEN 1 AND <N>`. That breaks the v2 step of a v1 → vN upgrade proof.
- ADR-007:74's rule, that each migration ALTERs both CHECKs, is the consistent one. State that `initialize_migration_history` keeps `BETWEEN 1 AND 2` (runtime.py:380, :383).
- A renumbered migration also forces regeneration of every later cumulative snapshot.
- `check-schema-pins.py` covers five pins, but plan:22 lists eight.

**m6 — D7.17 leaves the admission arithmetic open.**
- Define "remaining" as described in B2, and state whether the 10-minute reservation counts.
- Cap live reservations per code.
- Add CHECKs `max_redemptions BETWEEN 1 AND 10` and an expiry of at most 30 days.
- A resend at hour h issues a 24-hour link (ADR-006:67) that outlives a pending admission created at hour 0 (ADR-006:33). Either refuse resend after the admission expires, or show the remaining time on `/auth/pending`.

**m7 — D7.15 conflicts with the design's error bodies.**
- "Only a stable `code`" (ADR-007:86) conflicts with:
  - 409 carrying the recovery view (ADR-007:62);
  - 412 "naming the current revision" and 403 "naming the rule" (design.md:520).
- The recovery view has no APP-002 `$def` in contracts/schema.json, and ADR-002:47 requires a new edition for new wire shapes.
- **Fix:** say "a stable code plus only the class-specific authorized fields design §9 names", and name the recovery-view contract owner and its edition change.

**m8 — D7.14's 64 KiB JSON limit refuses contract-valid commands.**
- A command may carry 32 values (contracts/schema.json:1956), each up to 2,000 characters (:15).
- That already exceeds 65,536 bytes for ASCII text before JSON overhead, and reaches about 192 KB for multibyte text. The contract decoder's own cap is 1 MiB (contracts/README.md:39).
- The design's record-command errors include no 413 (design.md:516).
- **Fix:** size the command route limit from the maximum canonical command (for example 256 KiB), or bound the record schemas so the maximum command fits. Map an oversize body to 400.

**m9 — The single-task assumption does not hold during deployments.**
- D7.5 and D7.11 (ADR-007:46, :70) assume one task.
- The service runs with `minHealthyPercent 100` and `maxHealthyPercent 200` (infra/src/stack.ts:168), so every deployment briefly runs two tasks without stickiness.
- During that overlap the rate limits double. A session served by the other task cannot decrypt its refresh token, so resend fails and provider logout is skipped.
- **Fix:** deploy stop-then-start (`min 0` / `max 100`) in Phase 1, or record the overlap as a limitation.

**m10 — The grant template's shape is undefined.**
- ADR-007:18/:21 copy "the code's grant template" without requiring explicit resource IDs.
- schema-plan.md:26 forbids an "organization-wide wildcard app ID", and ADR-003:29 keeps reviewer and administrator authority out of invitations.
- **Fix:** template rows are explicit (resource_id, permission) pairs, FK-checked at issuance. `admit_redemption` copies them verbatim, and templates refuse `admin_policy` and `review_publish`.

**m11 — An existing principal at redemption is not a typed outcome.**
- 0002:35 makes `(organization, issuer, tenant, subject)` unique, but ADR-007:18 always inserts a principal and an epoch-1 membership.
- So a revoked identity that is invited again raises `unique_violation`: an untyped error instead of a D7.1 outcome.
- Resetting the member to epoch 1 would violate "never resets it" (schema-plan.md:24).
- **Fix:** add the outcome `already_member`, with no slot consumed. Re-admission of a revoked principal is an operator action that advances the existing epoch.

**m12 — `hashtext` key space (only if the advisory lock is kept).**
- The lock key is a 32-bit hash in the shared single-bigint advisory key space. Collisions only over-serialize unrelated chains, but any role may take any key, which couples tenants' latency.
- **Fix:** use `pg_advisory_xact_lock(<fixed class id>, hashtext(organization_id::text || ':' || writer_role))` with a class ID distinct from the runner's `(1819409017, 2)` at runtime.py:400, or remove the lock with M3's head row.

**m13 — Other plan items conflict with ADR-007 or ADR-006.**
- Task 9's oracle test includes the 114,000 bytes-per-minute ratio and the Spend Review money values (plan:226). ADR-007:66 and plan:220 keep both refused in PH01.
- Task 7's `prelogin_flows` has a `nonce` column (plan:189), but ADR-006:12 says "no nonce".
- Task 6 seeds `owner_publish` rows inside schema migration 0004 (plan:178). Operator seeding is separate (schema-plan.md:72), and a migration needs per-organization scope to write tenant rows.

---

## Answers to the ten requested checks

| # | Check | Result |
| --- | --- | --- |
| 1 | Is every ADR-004 sentence that ADR-007 calls unchanged actually unchanged, and is every bent rule named? | No. The no-retry and READ COMMITTED rules are bent (M1), the FORCE RLS / tenant-leading rule is bent for `code_index` (m1), and the six-role catalog normalization must change (M8). None of these is named. "`genui_lock` is unchanged" holds only if B1 takes option (a). "No direct application grant-table mutation" and "definer privileges never become direct application UPDATE grants" hold (C8). |
| 2 | Can the D7.1 functions be implemented under FORCE RLS with a NOBYPASSRLS definer? | RLS works inside the definer with the caller's GUC, and `WITH CHECK` passes or fails as intended (C3). The privileges do not work (B1). `advance_member_epoch` lacks a context check (M2). `resolve_code` leaks nothing beyond the digest's organization (C4). |
| 3 | Does the D7.3 grant table match design §6.1/§6.3 and the schema-plan matrix? | The core write assignments match (C13). Membership and revocation moved to workspace without naming the deviation (M2). Audit kinds are not bound to roles (M3c). Read grants are missing (m2). |
| 4 | Does D7.7's sequence lock create a deadlock with the fence order? | No deadlock with ADR-004's writers (C5). Row creation is missing (M7), and the remaining lock-order gaps are in m3. |
| 5 | Is the D7.8 audit chain sound under its advisory lock? | Unsound under SERIALIZABLE or mixed isolation; no tail is defined; roles are not bound to kinds (M3). `hashtext` collisions are benign (C7, m12). Per-organization, per-role chains match the design (C6). |
| 6 | Does D7.9's HTTP mapping match design §9 and ADR-002? | The types cannot express the mapping (M4); the claim row is missing (M5); the problem-body rule conflicts (m7). |
| 7 | Is D7.10's subprocess worker within design §6.3 confinement, and is the APP-D05 deviation recorded? | The deviation is not recorded, and confinement is weaker than what APP-005 qualified (M6). |
| 8 | Is the catalog budget sound, and is embedding only the current catalog consistent with the Python upgrade proof? | The budget is likely to trip (m4). The plan's CHECK rule conflicts with the per-step proof (m5). `genui_admit` is invisible to the catalog (M8). Embedding only the current catalog is consistent with the Python proof (C14). |
| 9 | Do the D7.17 defaults satisfy ADR-003 and ADR-006? | The values are bounded and consistent (C12); the arithmetic is open (m6, B2). |
| 10 | Do the names and signatures in Tasks 4–10 match ADR-007? | They do not match (M10, m13), and `reserve_code` / `record_pending_admission` are unauthorized (B2). |

## Claims I tried to refute and could not

- **C1 — ADR-004's numeric bounds are unchanged.**
  - pool.rs:25-27: 5 s acquisition, 3 s transaction, 2 s cleanup.
  - pool.rs:148-152: 8 connections total and 2 per role.
  - pool.rs:500: `statement_timeout 2000`, `lock_timeout 1000`.
  - pool.rs:618: `max_size 2`.
  - No decision alters these. D7.6's retry is a separate issue (M1).
- **C2 — `storage::Error` has exactly five variants** (app/src/storage/mod.rs:15-21).
- **C3 — RLS is evaluated correctly inside a NOBYPASSRLS SECURITY DEFINER.**
  - The definer evaluates `tenant_scope` with the caller's transaction-local GUC.
  - An insert under a matching scope succeeds. A mismatched or missing scope violates `WITH CHECK`. Rows of another organization are invisible.
  - Evidence: Appendix A T1c–T1e and T1g; the policy at 0002:219 has no `TO` clause.
  - The definer therefore needs no separate tenant evaluation. `admit_redemption`'s explicit equality check (ADR-007:18) remains good defense in depth.
- **C4 — `resolve_code` discloses only the organization of a digest the caller already holds.**
  - Application roles cannot read `code_index`: USAGE on the schema (0002:224) grants no SELECT (ADR-007:25).
  - The function returns one uuid or NULL for an exact digest (ADR-007:17).
  - Expired and revoked codes are judged after scope is set.
  - With 128-bit codes (ADR-003:47), probing digests is infeasible.
- **C5 — D7.7 introduces no deadlock with ADR-004's writers.**
  - Commands lock in this order: organization `FOR SHARE`, member `FOR SHARE`, work `FOR SHARE` (0002:250-253, :269-270), then `organization_sequences FOR UPDATE`, then the record.
  - Suspension takes only the organization `FOR UPDATE`. Membership changes take the organization `FOR SHARE`, then the member `FOR UPDATE` (ADR-004:34).
  - The sequence is a separate row, so no SHARE-to-UPDATE upgrade happens on `organizations`.
  - Because the row lock is held until commit, sequence order equals commit order.
  - Per-organization serialization is declared (ADR-007:54) and is within the design's load (design.md:499).
- **C6 — The three per-organization, per-role chains match the design** (design.md:305).
  - PH01's SR10 test is "no role updates or deletes; chain verified" (design.md:170; QUALITY-GATES.md:11).
  - Object Lock anchoring belongs to APP-053 before PH04 (QUALITY-GATES.md:26; Tickets/0-backlog/APP-053.md:38).
- **C7 — `hashtext` collisions affect throughput, not chain correctness** (once M3 defines the tail). The runner's lock uses the two-int advisory form (runtime.py:400), whose lock tags never collide with the bigint form.
- **C8 — ADR-004:32, :34 and :36 hold.**
  - No application role gets permission-table writes, or UPDATE on `organizations.status`, `memberships.membership_epoch` or `saved_work.version` (ADR-007:33-38).
  - `genui_admit` is NOLOGIN with no members (ADR-007:13).
  - The nonmember entry needs neither a member permit nor a bypass flag (ADR-007:21).
- **C9 — D7.11, D7.13 and D7.16 are consistent with their sources.**
  - D7.11 restates ADR-006:50/:54.
  - D7.13 matches ADR-006 §Retirement (:69) and plan:38.
  - D7.16 matches design.md:524 and ADR-002:33.
- **C10 — D7.14's rightmost-`X-Forwarded-For` rule is sound here.** Only the ALB reaches the task (infra/src/stack.ts:161). The ALB drops invalid headers (:162) and appends the peer address by default.
- **C11 — D7.4 is consistent with ADR-004:36 and design.md:259.** No application adoption writer exists before PH03.
- **C12 — D7.17's values are finite and match their sources:** ADR-006:32 (10-minute handoff), ADR-006:33 (24-hour pending admission), ADR-006:35 (60 s cooldown, 5 per 24 h), and ADR-003:47 (one redemption by default).
- **C13 — The core D7.3 role assignments match the design.**
  - Sessions, drafts and saved work go to workspace (design.md:259).
  - Publications and policy go to control (design.md:271).
  - Records, outcomes and receipts go to data (design.md:301-305).
  - The worker gets none.
- **C14 — Embedding only `catalog-vN` in Rust is consistent with the Python per-step proof.**
  - Both use the same `catalog-query.json` (catalog.rs:8; runtime.py:339).
  - The runtime holds every snapshot (runtime.py:334-355; Task 3 generalizes this).
  - The binary refuses any other version (pool.rs:494).
- **C15 — D7.10's engine version and minimal scope are within APP-005's qualification.**
  - DuckDB 2.0.0 is the qualified version (evidence/APP-005/dependency.json:3; spikes/APP-005/README.md:3).
  - The Northstar `total`, including the 900,000-byte destination-filtered total, and `timeline` are within the qualified subset (spikes/APP-005/README.md:19, :39).
  - CIDR and ratios are not qualified (:21).

## Appendix A — reviewer reproductions (local PostgreSQL 16.13, throwaway cluster outside the repository)

**Setup.**
- Roles: `x_owner`, `x_admit` and `x_workspace`, all NOBYPASSRLS; `x_admit` is NOLOGIN NOINHERIT.
- Schemas `genui` and `genui_private`, owned by `x_owner`.
- Tables `organizations`, `memberships` and `code_reservations`, with ENABLE and FORCE RLS and 0002:219's policy text.
- Definer grants shaped like D7.1:
  - `SELECT(organization_id,status)` on organizations;
  - SELECT, INSERT and `UPDATE(membership_epoch)` on memberships;
  - SELECT and INSERT on `code_reservations`.
- SECURITY DEFINER functions owned by `x_admit`, with `SET search_path=pg_catalog,genui,pg_temp` and EXECUTE granted to `x_workspace` only.

**T1 — privileges and RLS inside the definer**

| Case | Action | Result |
| --- | --- | --- |
| T1a | `organizations … FOR SHARE` | `ERROR: permission denied for table organizations` |
| T1b | `code_reservations … FOR UPDATE` | `ERROR: permission denied for table code_reservations` |
| T1c | Insert a membership under a matching scope | Succeeds; epoch 1 |
| T1d | Scope = organization 2, insert into organization 1 | `ERROR: new row violates row-level security policy for table "memberships"` |
| T1e | No scope set | Same error as T1d |
| T1f | After `GRANT UPDATE(status) ON organizations`, take `FOR SHARE` | Returns `active` |
| T1g | Scope = organization 2, take `FOR SHARE` on organization 1 | No row |

**T2 — the D7.8 advisory-lock chain**

Session A runs SERIALIZABLE: it takes its snapshot, sleeps 1.5 s, takes `pg_advisory_xact_lock(hashtext(org||'control'))`, reads the tail and appends. Session B starts 0.5 s later, takes the same lock, appends and commits first.

- **Variant 1 (B SERIALIZABLE):** A fails with `ERROR: could not serialize access due to read/write dependencies among transactions`.
- **Variant 2 (B READ COMMITTED):** both commit. The chain holds `B|1|h0` and `A-serializable|1|h0`: a fork on `h0`.

**T3 — the proposed fixes**

- **Variant 3** (head row `FOR UPDATE`; A SERIALIZABLE, B READ COMMITTED): A fails with `ERROR: could not serialize access due to concurrent update`. The chain stays linear.
- **Variant 4** (head row; both READ COMMITTED): A appends `seq 2` with `prev` equal to B's hash.
- **Variant 5** (advisory lock plus `UNIQUE(org, prev)`; A SERIALIZABLE, B READ COMMITTED): A fails with `ERROR: duplicate key value violates unique constraint`. No fork.

**T4 — same-key wait**

A second same-key `INSERT … ON CONFLICT DO NOTHING`, issued while the first is still in flight, with `lock_timeout=1000`: `ERROR: canceling statement due to lock timeout` (while inserting the index tuple).

changes requested
