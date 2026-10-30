# Independent review of ADR-009 (Phase 2 data plane)

- **Reviewed file:** `docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md`, SHA-256 `870ff618f685c92f500c59b671dd3f44e9ba9f9de44bd4d190987ef7232b7319` (hash taken before and after the review; unchanged).
- **Date:** 2026-10-29. Reviewer: independent agent, read-only except this file; nothing committed.
- **Method:** each decision D9.1–D9.15 was read against the controlling records: platform design (`design/forma-generative-app-platform-v2/design.md`), demo design (`design/forma-northstar-demo/design.md`), `SHARED-CONTRACTS.md`, ADR-001–ADR-008, `contracts/schema.json` and `contracts/README.md`, `app/migrations/0002_tenant_foundation.sql`, `app/src/storage/*.rs`, `app/Dockerfile`, `infra/src/stack.ts`, `spikes/APP-004`, `spikes/APP-005`, `evidence/APP-004`, `evidence/APP-005`, the 17 ticket files `Tickets/0-backlog/APP-015.md`–`APP-031.md` and the map `evidence/APP-015/ph02-map.md` §5. The reviewer tried to refute each decision on implementability, contradiction with controlling records, dropped acceptance sentences or requirements, numeric coherence and privilege escalation. Planning files that another agent is editing were not used as evidence.
- **Empirical probes (all local and disposable, no cloud resources):**
  - **PG:** a scratch PostgreSQL **16.13** cluster (the RDS version in `stack.ts:69`) with the repository's `0001` and `0002` migrations applied unchanged, plus a minimal `genui.jobs`/`genui_private.job_queue`/`genui_jobs` model written exactly as D9.4 describes. The cluster was stopped after use.
  - **NPROC:** the existing local app image `genui-app008:local-r5` (built from `app/Dockerfile`, user 65532) was run with `--network none` to test `RLIMIT_NPROC` semantics.
  - Probe scripts are in the session scratchpad, not the repository. Their decisive output is quoted below.

## Verdict

**Changes requested.** There are 2 blockers, 14 majors and 11 minors. Two decisions cannot be implemented as written:
- **B4** (D9.8, `REPEATABLE READ READ ONLY`);
- **B2** (D9.4, the cross-organization claim).

D9.5 (creator authority) and D9.13 (beta limits) need rework before any PH02 migration.

## Blockers

### X1 — B4's `REPEATABLE READ READ ONLY` cannot take ADR-004's authority fence, and its "cannot hit 40001" claim is false (D9.8)

**Evidence.**
- ADR-009:37 says a data-role read joining record revisions with the commit sequence "runs `REPEATABLE READ READ ONLY` … read-only transactions cannot hit 40001 retries".
- ADR-004:22 requires every present read to take the authority fence first.
- ADR-004:26 says: "Read-only business operations use a read-write SQL transaction when their fence needs row locks."
- The fence is `lock_member_authority`, which runs `SELECT … FOR SHARE` (`0002:250`, `0002:253`). The data role calls it on every read (`authorization.rs:181`, used by `read_record` at `authorization.rs:303-312`).
- Probe on PG 16.13 with migration 0002, `genui_data` in `BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY` with scope installed:
  ```
  ERROR:  cannot execute SELECT FOR SHARE in a read-only transaction
  CONTEXT: … PL/pgSQL function genui_private.lock_member_authority(uuid,uuid,bigint) line 7
  ```
  The same call in `READ COMMITTED READ WRITE` returns `t | t | 1`.
- Probe in `REPEATABLE READ` (read-write) after the snapshot was taken, while another session updated the membership row: `ERROR: could not serialize access due to concurrent update` (SQLSTATE 40001).
- `advance_member_epoch` updates that row on every sign-out (ADR-007:22). ADR-004:34 updates it on every grant change.

So B4 has two possible readings, and both fail:
- **Read-only:** the fence cannot be taken. This contradicts ADR-004:22 and :26, and B4 does not name that amendment.
- **Read-write:** ordinary sign-outs produce 40001 on reads, and D7.6 allows no retry outside the control role (ADR-007:56).

**Exact fix.** Delete B4. Keep D7.6 unchanged (READ COMMITTED READ WRITE).
1. Take the fences as today.
2. Read the record rows and the snapshot sequence in **one SQL statement**. A single READ COMMITTED statement sees one snapshot, so `record_snapshot_sequence` and the rows are consistent. For example, a CTE that selects `organization_sequences.next_commit_sequence - 1` and the revisions with `commit_sequence <= that value`.
3. State that a record input larger than one statement's result is out of scope. The design ceiling is about 100,000 rows (design:313).

If several statements are unavoidable, filter every statement by `commit_sequence <= S`, where S is read once. Committed revisions are immutable, so the filtered reads are stable.

### X2 — B2's queue, claim and transaction shape cannot be implemented under FORCE RLS and ADR-004 (D9.4)

**Evidence.**
1. **Rows hidden by RLS.** `genui.jobs` is FORCE RLS keyed on `genui.organization_id` (ADR-009:21; the policy shape is `0002:219`). `genui_jobs` is NOBYPASSRLS, and inside a definer RLS uses the caller's scope (ADR-007:13). The worker calls `claim_next_job` precisely because it does not yet know the organization, so the function cannot read `claim_sequence` from `genui.jobs` or mark the job running. Probe output:
   - claim with no scope: `(0 rows)`;
   - the same call with that organization's scope installed: `(1 row)`.

   A definer that sets scope itself is forbidden for definers (ADR-004:32, "no scope-setting or privilege-changing behavior").
2. **Row lock needs a write grant.** `FOR UPDATE SKIP LOCKED` on `genui_private.job_queue` needs an UPDATE privilege. Probe: `ERROR: permission denied for table job_queue` until `UPDATE(available_at)` was granted. ADR-009 grants `genui_jobs` nothing.
3. **No enqueue path.** The ADR does not define how a row gets into `job_queue`. The workspace role creates `genui.jobs` rows (ADR-009:21), but `job_queue` "grant[s] nothing to any application role" by construction.
4. **Cross-organization state hidden.** Lease expiry and "one import running per organization" (D9.2) must be decided across organizations at claim time. Both live only in RLS-hidden `genui.jobs.lease_expires_at`/`state`. A crashed worker's job would never be reclaimed (APP-015.md:38 requires crash/reclaim).
5. **Transaction shape.** "Claim, result and one audit event commit in one transaction" (ADR-009:21; copied from design:319) breaks in three ways:
   - The job runs for up to 120 s (APP-004 README "120 seconds" deadline). ADR-004:42 caps a transaction at 3 s.
   - Lease renewal "every 30 s" compares a `claim_sequence` that nobody else can see until the claim commits.
   - The claim and the result must therefore be separate transactions.
6. **Worker privileges missing.** The worker lacks:
   - USAGE on `genui_private` (`0002:224` omits `genui_worker`);
   - SELECT on `memberships`/permission tables (`0002:226`);
   - EXECUTE on `lock_member_authority` (`0002:276`; ADR-004:13 gives the member fence to workspace/control/data only).

   In addition, `precheck` refuses `Role::Worker` (`authorization.rs:145`), and `Transaction::begin` needs a `TrustedRequest` with issuer, provider tenant and registry hash (`authorization.rs:102-115`), which a job row does not carry.
7. **Catalog normalization.** `genui_jobs` is an eighth fixed role. ADR-007 D7.12 A3 (ADR-007:80) extended the catalog normalization to seven roles only. `catalog-query.json` and `qualify_tenant_schema.py` would reject the new role, and the ADR does not say so.
8. **Cancellation audit chain.** `job.cancelled` is placed on the `worker` chain. A user cancels through a workspace request, and the RESTRICTIVE policy lets each role append only its own chain (ADR-007:64).

**Exact fix.** Replace the B2 paragraph with all of the following:

**Queue table.**
- `genui_private.job_queue(organization_id, job_id, kind, state CHECK IN ('queued','running'), claim_sequence genui.safe_counter, lease_expires_at, available_at, PRIMARY KEY (organization_id, job_id))`, owned by `genui_owner`, no RLS.
- It holds the cross-organization scheduling facts. `genui.jobs` stays the scoped, authoritative record.

**Definer functions.** All are owned by `genui_jobs`, `SECURITY DEFINER`, `search_path = pg_catalog, pg_temp`, with PUBLIC revoked, and none sets scope:
- `enqueue_job(p_org, p_job, p_kind)`. EXECUTE to `genui_workspace`. It requires `current_setting('genui.organization_id') = p_org` and that the scoped `genui.jobs` row exists. It inserts the queue row.
- `claim_next_job(p_kinds)`. EXECUTE to `genui_worker`. It selects the next eligible row from `job_queue` (queued, or running with an expired lease) under `FOR UPDATE SKIP LOCKED`, respects one running import per task and per organization, and serves organizations round-robin by the oldest `last_claimed_at` per organization (an extra column or a small per-organization cursor). It increments `claim_sequence`, sets the lease to `now() + 120 s` and returns `(organization_id, job_id, claim_sequence)`. Commit.
- `renew_job` and `release_job`, which compare `claim_sequence`.

**Grants for `genui_jobs`:** SELECT, INSERT, UPDATE(state, claim_sequence, lease_expires_at, available_at, last_claimed_at) and DELETE on `job_queue`.

**Worker transactions.** The worker then:
1. opens a scoped transaction with that organization and the job's principal;
2. copies `claim_sequence`/state into `genui.jobs`, rechecks authority, writes the `job.claimed` audit row if one is wanted, and commits.

The result transaction then:
1. compares `claim_sequence` in `job_queue` (through `release_job`) and in `genui.jobs`;
2. writes the result, the state and one audit event;
3. deletes the queue row;
4. commits.

Each transaction stays within ADR-004's 3 s.

**Worker grants.** Add a D7.3-style grant row for `genui_worker`:
- USAGE on `genui_private`;
- SELECT on `memberships`, `entitlements`, the permission tables and `genui.jobs`;
- UPDATE(state, claim_sequence, lease_expires_at, failure_code, result_digest, updated_at) on `genui.jobs`;
- INSERT on `sources`, `source_revisions` and `audit_events`;
- UPDATE on its own `audit_heads` chain;
- EXECUTE on `lock_member_authority`.

Name the ADR-004:13 amendment.

**Storage entry and catalog.**
- Add a storage-private `WorkerScope` constructor built from a job row (organization, principal, epoch read under scope). It must never be a `TrustedRequest`.
- Extend A3's normalization to `genui_jobs` in the same migration, exactly as 0003 does for `genui_admit`.

**Cancellation.** The workspace sets `genui.jobs.cancel_requested_at`, and the workspace chain gets the kind `job.cancel_requested`. The worker observes the request and writes `job.cancelled` on its own chain.

## Majors

### M1 — D9.5 creator authority uses permission names that do not exist, is unrevocable, leaves import authority undefined and moves member sharing to the operator without naming it

**Evidence.**
- **Names.** ADR-009:25 grants "`work_read`/`work_write`". The closed work permissions are `record_read`, `record_write`, `share_work` and `export` (`0002:201`; `authorization.rs:31-76`).
- **Scope of the bypass.** The work clause also requires app `run` and `source_read` on every dependency (`authorization.rs:174`). D9.5 does not say whether creator authority bypasses those. If it does, an operator who revokes a creator's app `run` (an epoch-fenced permission change, ADR-004:34) leaves the creator reading that work's records. This is an escalation against the operator's intent.
- **Revocation.** No transaction can remove creator authority short of membership revocation. ADR-004:34's model is that every per-principal permission is an operator-changeable row under the fence.
- **Re-admission.**
  - Today `admit_redemption` returns `already_member` for an existing principal (ADR-007:21), so there is no re-admission path.
  - Any future reinstatement would silently revive every creator authority without a fenced grant event.
- **Cross-organization ids.** These are closed only if `created_by` is a composite foreign key `(organization_id, created_by) → principals` (`0002:34`) and the WITH CHECK enforces `created_by = current_setting('genui.principal_id')::uuid`. The ADR states neither, so a member could attribute a source or work to another member.
- **Import authority.**
  - No `Action` authorizes import or new work (`authorization.rs:31-42`), yet the design gates import on the builder role (design:369 step 1; R3 at design:115 "A builder MUST import").
  - Under D9.5, any active member can import, receive `source_read` and consume organization quota.
- **Sharing.** "Sharing with another member remains a permission-row write through the operator channel" contradicts the design's member-initiated share (design:402 step 6; R11 at design:123). The amendment is not named.

**Exact fix.** Rewrite D9.5 as follows:
1. **Source creator.** `sources.created_by uuid NOT NULL` with FK `(organization_id, created_by) REFERENCES genui.memberships`, plus a RESTRICTIVE `WITH CHECK (created_by = nullif(current_setting('genui.principal_id', true), '')::uuid)` for the inserting role. Creator authority equals `source_read` only.
2. **Work creator.** `saved_work.created_by`, with the same constraints. Creator authority equals `record_read` and `record_write` only (not `share_work` or `export`). It replaces only the `work_permissions` lookup: the app `run` EXISTS clause and the per-dependency `source_read` (or creator) clause still apply.
3. **Import authority.** Add `import_source` to the organization or app permission CHECK. Grant it through `code_grant_templates`; it is required at enqueue, claim and result.
4. **Revocation.** Add an operator subcommand `revoke-creator-authority` that sets `creator_revoked_at` under ADR-004:34's fence and epoch advance. State that membership revocation also ends creator authority.
5. **Sharing.** Name the R11 / design:402 deviation, "member-initiated sharing deferred to PH0x; operator channel only in PH02", as a numbered amendment.

### M2 — The D9.4 epoch recheck either cancels imports on sign-out and grant changes, or checks nothing

**Evidence.**
- ADR-009:21 says the worker installs "the enqueuing principal" and rechecks "membership, epoch and entitlement at claim and again before the result commits".
- The job stores `membership_epoch_at_enqueue`.
- The epoch advances on every sign-out (ADR-007:22, `session.sign_out`) and on every operator permission change (ADR-004:34). If the worker compares against the enqueue epoch, signing out or being granted a new share kills the member's queued imports.
- If the worker installs the current epoch read from the database, the epoch comparison in `precheck` (`authorization.rs:151`) is vacuous.
- APP-015.md:41 only needs "an expired or revoked trial cannot use a queued job".

**Exact fix.** State that at claim and at result the worker checks:
- organization active;
- membership `status = 'active'`;
- entitlement active and unexpired;
- the job's own permission (`import_source`, per M1).

All of these are read under the member fence with the current epoch. The epoch is **not** compared to the enqueue value, so sign-out and unrelated grants do not cancel jobs. Record `membership_epoch_at_enqueue` for audit only.

### M3 — B3 (one query per organization, queue 5) breaks N3 and the demo's six-view take, and silently amends ADR-007 D7.10(3)

**Evidence.**
- **Arithmetic.**
  - B3 allows 2 running per task and 1 per organization, with a queue of 5 per organization (ADR-009:33). A six-view refresh therefore runs as six serial waves.
  - APP-005 measured 213–244 ms of fresh-process wall time per query (`evidence/APP-005/README.md:7`). Six serial queries come to roughly 1.3–1.5 s before RDS authorization.
  - That misses N3's warm p50 of 500 ms (design:141).
  - It also misses the demo fit check, which assumes "2 running and 4 waiting, three waves and 0.3 s" (demo:182, demo:528). Linux CI already saw a 1.811 s burst with two slots (`evidence/APP-005/README.md:39`).
- **Queue depth.** One running plus five queued is exactly six. A second filter change while the first epoch's queries are still queued returns 429. That is S02's own sequence: epoch 12, then 13 (design:337).
- **Unnamed amendment.** ADR-007 D7.10(3) says "at most 2 running queries per organization and 4 per task, a queue of 20" (ADR-007:72). B3 names only "design §6.3".

**Exact fix.** Rewrite B3 so that:
1. A task runs 2 queries; an organization may use both slots while no other organization is waiting (work-conserving).
2. Dequeue is round-robin across organizations, so a waiting organization gets the next free slot. This meets APP-024.md:38 "while another organization continues making progress".
3. A new filter epoch cancels that work's queued older-epoch queries before admission.
4. The queue is 12 per organization and 20 per task.
5. B3 explicitly amends ADR-007 D7.10(3).
6. The N3 expectation is recorded: per-request child at about 230 ms × 3 waves ≈ 0.7 s means APP-012's broker fallback (D7.10(5)) is required for p50.

### M4 — D9.7's `RLIMIT_NPROC 16` is per user ID and will starve the engine child; `RLIMIT_AS 1.5 GiB` is unmeasured

**Evidence.**
- Every process in the task runs as UID 65532 (`app/Dockerfile:33`; `stack.ts:143`). D7.10 runs the engine child "of the same UID" (ADR-007:72).
- On Linux, `RLIMIT_NPROC` counts all threads of the real UID.
- Probe in the app image (user 65532): a parent with 4 threads lets the limited child start 2 threads. A parent with 20 threads gives `child thread start failed after 0 : can't start new thread`.
- The real binary has tokio workers and blocking-pool threads, a parser-host and the Python supervisor, so DuckDB's `threads=2` would fail intermittently with load.
- `RLIMIT_AS` was never measured. APP-005 used a 3 GiB cgroup limit, not an address-space limit (`evidence/APP-005/README.md:24`). Python, DuckDB and allocator virtual reservations against a 1 GiB `memory_limit` are unqualified.

**Exact fix.**
- Drop `RLIMIT_NPROC`, or run each engine child under a dedicated UID. The image would need a second unprivileged user, and spawning as it needs `CAP_SETUID`, which the task does not have. The practical choice is therefore to drop it and record the limitation next to the network-namespace limitation.
- Keep `RLIMIT_AS` only after APP-012 records `VmPeak` for the heaviest admitted query. Set the limit from that measurement plus margin, with a regression test that the S02 and 1,000,000-row queries pass under it.

### M5 — The normalization job's input, memory and slot are unspecified (SR5 and demo §11.2)

**Evidence.**
- **Input.** D9.3 names the writer, `python3 /app/runtime.py normalize` with DuckDB (ADR-009:17), but not its input. If it reads the original CSV/XLSX with DuckDB's readers, untrusted files are parsed outside the WebAssembly sandbox. That violates SR5 (design:165, "Imports MUST parse in a WebAssembly sandbox") and C05.
- **Memory.** No `memory_limit` or thread count is set, and DuckDB defaults to 80 % of RAM.
- **Slot.** D9.2 limits "one import running", but `normalize` is a separate job kind (ADR-009:21). It could run beside a 2 GiB parse and two query children.
- **Budget.** The demo's 8 GiB budget (demo:528: binary 1 GiB, two DuckDB at 1 GiB, parser 2 GiB, cache 2 GiB, headroom 1 GiB) is not re-derived for the PH02 set:
  - two query children at 1.5 GiB (address space);
  - parser 2 GiB guest plus host overhead;
  - decoding up to 128 MiB of parser JSON in the binary (`guest/src/lib.rs:74`);
  - the unbounded normalize child.

  An out-of-memory kill takes down the single beta task.

**Exact fix.** State the following:
- Normalization consumes only the parser-host's typed, validated output. It re-runs `parser-host` on the staged original with the confirmed sheet/mapping and pipes the typed cells to `normalize` on stdin. DuckDB never opens the original.
- The normalize child gets `memory_limit='1GiB'`, `threads=1`, the D7.10 confinement (`env_clear`, scratch directory, `enable_external_access=false`) and the 120 s deadline.
- `import` and `normalize` share the single job slot.
- Add a memory budget table that sums to at most 7 GiB, leaving 1 GiB of headroom.

### M6 — D9.3 redefines `normalized_digest` for rows that already exist, and `object_digest` has no column (unnamed D7.10 amendment)

**Evidence.**
- **PH01 meaning.** D7.10 fetches bytes and "verifies `source_revisions.normalized_digest`" (ADR-007:72), so the operator-seeded PH01 Northstar revisions hold the digest of the Parquet bytes.
- **PH02 meaning.** D9.3 redefines `normalized_digest` as a row-stream digest and moves fetch verification to a new `object_digest` (ADR-009:17).
- **Nowhere to put it.** `source_revisions` has only `content_digest`, `normalized_digest` and `contract_digest` (`0002:121-129`), and the contract has no `object_digest` (`contracts/schema.json:1672-1744`).
- **Values are frozen.** `normalized_digest` is referenced by the foreign keys of `work_bindings`, `work_source_dependencies` and `record_source_dependencies` (`0002:147`, `0002:160`, `0002:187`) and by receipts, so existing values cannot be rewritten.
- **Effect.** After PH02, `verify-receipts` would recompute the new meaning over the PH01 rows and "quarantine" them (ADR-009:37).

**Exact fix.** Choose one of the following and name it as an amendment of D7.10:
- **Option (a), recommended:** keep `normalized_digest` = SHA-256 of the Parquet object bytes, which is what PH01 verifies. Add `rows_digest genui.digest NULL` with the `forma:normalized-rows:v1` meaning, required for PH02 revisions and NULL for PH01 ones.
- **Option (b):** add `digest_scheme smallint NOT NULL DEFAULT 1` and treat scheme 1 rows by D7.10's rule.

In either case, add the column and the contract field in D9.6's edition.

### M7 — Staging deletes in the versioned bucket do not remove bytes; no lifecycle rule exists (D9.3, SR11)

**Evidence.**
- The bucket is versioned (`stack.ts:83`; ADR-002:41 "versioned S3").
- A `DeleteObject` without a version id only adds a delete marker. The noncurrent version, the uploaded original, stays indefinitely: the stack has no lifecycle rule and the tenant role has no `s3:DeleteObjectVersion` (`stack.ts:103`).
- ADR-009:17 claims a failed job "leaves at most a staging object that the next upload or the operator sweep removes". It does not, and SR11 requires shredding (design:171).
- No ticket owns "the operator sweep".
- Separately, `CopyObject` to the final key must send `x-amz-server-side-encryption: aws:kms` and the tenant key id explicitly, or the bucket's deny statements refuse it (`stack.ts:106-107`). The ADR does not say so.

**Exact fix.** Add to D9.3:
- A CDK lifecycle rule on prefix filter `o/` with tag `stage=staging`, or a per-tenant `o/{tenant}/staging/` rule: `NoncurrentVersionExpiration` 1 day, `Expiration` 2 days for current staging objects, and `AbortIncompleteMultipartUpload` 1 day.
- State that `CopyObject` sets the SSE-KMS headers with the tenant key.
- Name APP-016 as the owner of the sweep and lifecycle rule.
- Replace "removes" with "expires within 2 days".

### M8 — The upload route still runs under D7.14's 5 s handler deadline (D9.9)

**Evidence.**
- D9.9 lifts only the D7.14 body limit (ADR-009:41). D7.14 also sets "Handler deadline 5 s" (ADR-007:88).
- A 10 MiB CSV needs at least 16.8 Mbit/s of uplink to finish in 5 s, so ordinary connections fail.
- The order of upload PUT and job enqueue is unspecified. A job enqueued before the PUT returns can be claimed against a missing object.

**Exact fix.** Give `POST /v1/sources/uploads` its own deadline: 120 s absolute, with a minimum-throughput floor such as 64 KiB/s over any 10 s window, so slow-drip bodies are cut off. Then specify the order:
1. PUT returns;
2. verify the length and SHA-256;
3. one short transaction inserts the `genui.jobs` row and calls `enqueue_job` (X2) with `upload_id` and `content_digest` in the payload.

Hold no database transaction during the transfer.

### M9 — D9.13's limits contradict the design's own load and lock out a beta user permanently

**Evidence.**
- **Query cap.** 1,000 queries/hour per entitlement (ADR-009:57) is below:
  - platform A2: one interaction per 10 s over six views is 2,160/hour (design:80);
  - demo A4: 0.8 actions/s over four sessions is 0.2/s per session × 6 = 4,320/hour (demo:77, demo:182).

  A rehearsal take with one active actor reaches the cap in about 14 minutes.
- **Storage cap.** "200 MiB of stored originals" combined with D9.3's "never deleted by the application" means 20 maximum-size imports lock the entitlement out forever.
- **Where the values live.** "Recorded on the entitlement kind" has no relation behind it: an entitlement is a 30-day row created by `admit_redemption` (ADR-007:21, D7.17). D7.19's operator subcommands are a closed list (ADR-007:108).

**Exact fix.**
- Set queries to 6,000/hour, which covers demo A4 with margin, or meter refreshes instead (600 refreshes/hour).
- Count stored originals over non-deleted revisions only, and add a member-visible source deletion (tombstone, D9.12) that frees quota.
- Define `genui.entitlement_limits(organization_id, entitlement_id, imports_per_day, stored_original_bytes, queries_per_hour)` with FORCE RLS, written by a new D7.19 operator subcommand `set-limits` on the `operator` chain, and readable by the workspace and data roles.
- State that counters are durable for daily and storage limits (count `genui.jobs` and `source_revisions`) and in-process for hourly ones (D7.5).

### M10 — Moving `query_receipts` to APP-023 contradicts ADR-007 and ADR-008's Phase 1 journey (D9.8)

**Evidence.**
- ADR-007's Consequences assign `query_receipts` to APP-013's migration 0006 (ADR-007:112), and D7.3 grants `genui_data` INSERT on it (ADR-007:40).
- ADR-008 keeps "the admitted query → receipt → command → reopen journey" as a beta-critical Phase 1 item (ADR-008:9).
- D9.8 says "APP-023's migration creates `genui.query_receipts`" (ADR-009:37) and names no amendment. If APP-013 already created it, APP-023's `CREATE` fails. If not, the PH01 journey has nowhere to store a cited receipt.

**Exact fix.**
- Keep creation in APP-013's 0006 using D9.8's column set (`organization_id`, `receipt_id`, `digest` unique per organization, `canonical_receipt`, `cited_by_outcome`, `created_at`).
- APP-023 adds only additive columns (for example `mapping_version`, `quarantined_at`).
- Record this in D9.8 and in the Consequences paragraph.

### M11 — APP-024's cache acceptance is silently dropped, and the source cache key must include the organization (D9.7)

**Evidence.**
- APP-024.md:40 says "Cached results include tenant/effective scope/policy/input/query/record state and reauthorize at release; revoked and foreign cached receipt lookups disclose nothing". APP-024.md:42 says "cached delivery consume current trial entitlement".
- D9.7 turns the result cache off (ADR-009:33) without saying how these sentences are satisfied or amended.
- D9.7 also keys the source cache "by revision object digest" only. D7.10 placed the cache under `/run/genui/cache/o/<organization>/` (ADR-007:72). APP-016.md:39 requires that "identical content does not permit foreign-tenant reads": two organizations importing identical bytes share a digest.

**Exact fix.**
- Key the source cache by `(organization_id, object_digest)` inside D7.10's per-organization directory.
- Record a ticket amendment for APP-024: the cache sentences are proved on the source cache and on the stored receipt lookup by digest. Specifically:
  - re-authorization before releasing a cached source to an engine;
  - foreign or revoked receipt-by-digest lookup returns the uniform 404;
  - no engine slot is reserved for inactive or pre-access sessions.

  The result-cache wording is deferred, with the reason (C06 optional, SHARED-CONTRACTS:57).

### M12 — The demo A6 profile does not reach 100,000 CSV records for the Northstar shape; the A5 reduction is unnamed (D9.2)

**Evidence.**
- The demo profile's 320,000 aggregate-cell cap applies to CSV as well as XLSX (`spikes/APP-004/guest/src/lib.rs:71`, `:534`; CSV cells pass through `push_cell`).
- Northstar events have 5 columns (`fixtures/generate.py:17`; `fixtures/phase-one/v1/sources/northstar-events.csv` header). The effective bound is therefore 64,000 records, not demo A6's 100,000 (demo:79).
- D9.2 names only "platform A4's 1,000,000 records … not built" (ADR-009:13).
- Platform A5, "one import … per organization, three … globally" (design:83), becomes one per task without being named. A5's own "If wrong: fair scheduling or separate workers, never silent loss" applies.

**Exact fix.** In D9.2, do two things:
- Either raise the demo CSV cell cap to 100,000 × 32 = 3,200,000, still bounded by the 128 MiB output cap, with fresh APP-004 qualification; or name the reduction "CSV records ≤ min(100,000, ⌊320,000 / columns⌋)".
- Name **A5's reduction** (one import globally on the beta task, the queue visible, no silent loss) as a numbered amendment.

### M13 — The amendment ledger is incomplete: "B5" is promised and never defined, and several amendments are unnamed

**Evidence.** ADR-009:3 says "amendments are named **B1–B5** below". `B5` occurs only in that line; the body defines B1–B4. Amendments of controlling records that carry no label:

1. ADR-007 D7.8: the `worker` chain, the `writer_role` CHECK and a sixth `audit_heads` row per organization, which D7.7 provisions as five (ADR-007:60, :64; ADR-009:21 says only "D7.8 amended").
2. D7.8's closed kinds, extended by `view.shared`, `alias.created` and `alias.retired` (ADR-009:45), each "an amendment of this ADR" per ADR-007:64.
3. ADR-007 D7.10(3) admission (M3).
4. D7.10's fetch verification (M6).
5. ADR-007 D7.3's literal grant table: the worker, `genui_jobs`, and the new workspace and data grants. The campaign asserts equality with that table (ADR-007:44).
6. D7.12 A3's seven-role normalization (X2 item 7).
7. ADR-004:13, the member fence for the worker.
8. ADR-004:26, by B4 (X1).
9. ADR-007:112, the `query_receipts` owner (M10).
10. Design A5 (M12).
11. Design R11 and design:402, member sharing (M1).
12. Design N3/A14 28,100 B and ADR-001:18–20, the canvas budget and native-group adoption, "available but deferred" (D9.11).
13. Design §6.4 "workers … read no revision rows" / §6.3 "only credential that reads Parquet" (design:299, 317): the normalize job writes Parquet and inserts `source_revisions`.

**Exact fix.** Replace the header sentence with an **Amendments** table: id, controlling record and line, old text, new text, and the decision that carries it. Number every item above, either as B5…Bn or by folding them into existing labels, and remove "B5" if it is not used. Mirror the table in the Consequences paragraph.

### M14 — Arbitrary uploads on the live beta contradict "synthetic data" without a control (D9.13 / D9.15)

**Evidence.**
- ADR-008:7: invited viewers "use [the beta] on synthetic data". Design A12, "Synthetic only … If wrong: data classes, threat model and policy are settled before any real row" (design:90).
- ADR-008:8: APP-079/080 "block any real organization".
- D9.15 runs PH02 on the live beta with invited users (ADR-009:65), and D9.9 accepts any CSV or XLSX they upload. No notice, attestation, retention or purge is decided.
- With M7, uploaded bytes persist indefinitely.

**Exact fix.** Add to D9.15:
- a visible pre-upload notice and per-upload attestation that the file is synthetic, recorded in the job payload;
- a 90-day expiry lifecycle on `o/` objects (platform A6, design:84);
- an operator `purge-organization-objects` subcommand.

Alternatively, restrict beta imports to the fixture files until the data-class policy exists. State which, as a named amendment of ADR-008:7's premise.

## Minors

- **m1 (D9.1)** The startup check compares the WASM file against a manifest produced by the same build in the same image, so it adds nothing beyond the image digest. The APP-004 obligation "a fixed built-artifact allowlist" (`spikes/APP-004/README.md`, Reproducibility) means the qualified bytes. *Fix:*
  - CI extracts `parser_guest.wasm` from the built image;
  - it runs the APP-004 hostile suite and `fixtures/qualify.py` against those exact bytes;
  - it records the digest in the release evidence;
  - a release refuses an image whose parser digest lacks a passing record.

  Also add `--remap-path-prefix` for `CARGO_HOME` and the checkout path, so the digest is cross-host stable (the cause in `evidence/APP-004/reproducibility-initial-failure.json`). Spawn `parser-host` with `env_clear()`, as D7.10 does for the engine.
- **m2 (D9.10)** "For the session's own principal only" on `personal_view_state` is enforced only in the application: `tenant_scope` checks only the organization (`0002:219`). *Fix:* a RESTRICTIVE policy `USING/WITH CHECK (principal_id = current principal)`.
  - Aliases are organization-unique and workspace-owned with no authority stated, so any member can claim an alias for the organization. R16 says "Administrators MUST manage … aliases" (design:128). *Fix:* require `admin_policy` for alias create and retire.
  - Name the chain for the three new audit kinds.
- **m3 (D9.12)** `source_deleted` is a new problem code, absent from D9.6's edition list and from design §9. Deletion must also evict the `(organization, digest)` source-cache entry (APP-016.md:40, "invalidate derived cache references"). *Fix:* add both.
- **m4 (D9.8)** "A mismatch quarantines the revision" has no column, writer or read-path effect. *Fix:* `source_revisions.quarantined_at`, written by the operator command on the operator chain; queries refuse with a named code.
- **m5 (D9.6 / D9.15)** "The matching 0002 CHECK migration" for per-operator and per-renderer identities is underspecified. Publications hold scalar `operator_version`/`renderer_version` (`0002:85`), so per-operator identities need a child relation or a canonical-envelope rule. D9.15's "additive changes only" must define whether broadening a CHECK (`audit_events.writer_role`, `publications`) counts as additive. *Fix:* state both.
- **m6 (D9.11)** It closes ADR-001:18 and adopts ADR-001:20's "available but deferred" native group without APP-003/FORMA-232 measurement. The 482 B gained, from 162 B of headroom to 28,100 B plus 320 B, is not shown to fit APP-027's geometry. *Fix:* label it as an amendment and require APP-027 to record per-feature byte deltas before closing.
- **m7 (D9.15)** Gaps in the trial-organization pool:
  - Each trial organization also needs an fAuth tenant and a registry binding. The registry holds at most 32 bindings, and replacing it requires a restart (ADR-004:50-52).
  - The cost delta (8 keys at $1 ≈ $8/month, demo:537's unit price; keys are `RETAIN`, `stack.ts:80`) belongs in ADR-008:17's pending hosting figure. Each tenant adds two bucket-policy statements (`stack.ts:106-107`).

  *Fix:* pre-register the 8 bindings, add the delta to the ADR-008 figure and state the deletion schedule for keys.
- **m8 (D9.4)** "Fair round-robin" has no state behind it (see X2). Job retention, the stuck-job view (`forma admin jobs --stuck`, design:786/795) and dead-letter handling are still unowned (map §5.1 #17). *Fix:* name an owner and a retention period (platform A6, 90 days).
- **m9 (D9.3)** Showing a beyond-int64 value as "unknown" leaves open what an aggregate over that column returns. APP-021.md:39 requires rejecting "unsafe large-int transport". *Fix:* a sum, count or ratio over a column with any unknown value is refused with a named code, never computed over the rest.
- **m10 (D9.15)** Running per-ticket acceptance on "local S3-compatible storage" conflicts with ADR-002:41 (local doubles "cannot satisfy … isolation"). APP-016.md:38-39's tenant-isolation sentences and D9.3's IAM delete/copy need the managed bucket. *Fix:* name those sentences as managed-only. The S3 and KMS calls are cents and need no running task.
- **m11 (D9.2)** "Per task and per organization" is redundant while one import runs per task. A queue of 20 at up to 120 s each is a 40-minute worst wait. *Fix:* show the expected wait in the queued state, so "never silent loss" (A5) is visible.

## Claims I tried and failed to refute

- **D9.1 separate executable.** The app pins `rust-version = "1.85"` and exact serde, serde_json and sha2 versions (`app/Cargo.toml`). The APP-004 workspace pins different exact versions and toolchain 1.93.1 (`spikes/APP-004/README.md`). A separate lockfile is the only conflict-free option. The image builder is already `rust:1.93.1` (`app/Dockerfile:10`), so building `parser-host` in the image is feasible. One process per job gives each job its own Engine and epoch timer, removing the host's per-engine serialization (APP-004 README, "serializes jobs on an engine").
- **D9.3 IAM scope.** Copy and delete stay inside `o/${tenant}/*`. `s3:GetObject`/`PutObject` on that prefix (`stack.ts:103`) and `grantEncryptDecrypt` (`stack.ts:105`, which covers `kms:Decrypt` and `GenerateDataKey*`) suffice for a same-tenant `CopyObject`, given the explicit headers in M7. A cross-tenant copy is impossible because the role is prefix-scoped. Adding `s3:DeleteObject` on `o/${tenant}/staging/*` only does not widen final-key mutability.
- **Object before row** (D9.3, D9.9) matches ADR-002:37 and demo:342.
- **D9.5 cross-organization escalation.** Comparisons occur within one organization's rows under `tenant_scope` (`0002:219`). Principals are keyed `(organization_id, principal_id)` (`0002:34`). Given the composite foreign key in M1's fix, no foreign principal id can confer authority. A revoked member is stopped by `precheck`'s status and epoch test (`authorization.rs:146-152`) and by the fence.
- **D9.6** follows ADR-002:21/47 (new edition, prior vectors retained, explicit semantic change) and keeps USD at exponent 2 (contracts/README:33).
- **D9.7 grammar ownership.** A closed JSON plan, never SQL text, matches design:303, C06 and APP-020.md:38-40. Turning the result cache off is allowed by C06, "Caching is optional" (SHARED-CONTRACTS:57), and demo:350, apart from M11's ticket disposition.
- **D9.2 envelope.** With the result cache off, one 2 GiB parser fits demo §11.2's 8 GiB (demo:528), subject to M5's budget table.
- **D9.13** is consistent in principle with ADR-003:29/57 (entitlement-carried limits, admission and delivery rechecks); only the values and the storage are wrong (M9).
- **D9.15** fits ADR-003:27: one preprovisioned trial organization per code, no shared `beta` organization, provisioning by the operator and not hidden in signup. Its stated cost of about $1 per organization per month matches demo:537's key price.
- **D9.12** matches C05's tombstone rule (SHARED-CONTRACTS:49), S06 and APP-016.md:40, and correctly targets the `deleted_at` branch at `authorization.rs:174`.
- **D9.14** changes no controlling record.
- **The D9.4 worker chain** resolves the design:319 versus ADR-007:64 contradiction (map §5.3 #10), once named (M13).
- **D9.4 "imports never take `organization_sequences`"** leaves D7.7's lock order (ADR-007:60, 68) intact.

## Phase 2 map §5 gaps that ADR-009 leaves undecided

- **§5.1 #5:** CIDR join, ratios and multi-input execution, including how DuckDB integer `/` is kept out of `exact_rational` results. D9.6 adds the value type only.
- **§5.1 #6:** how committed records reach the confined DuckDB child (it has no database credentials) and what bounds that transfer. B4 is broken (X1).
- **§5.1 #7:** the admitted six-view Northstar definition that APP-025 and N3 need (PH03 authoring is not available).
- **§5.1 #13:** semantic suggestions and "thumbnails" (APP-026.md:39) have no renderer or source.
- **§5.1 #14:** the automated accessibility checker, its pin, checksum and licence (APP-029.md:38; N12).
- **§5.1 #17:** job retention, the stuck-job operator view and dead-letter handling (m8).
- **§5.2 #3:** the source of deterministic mapping suggestions and the proof that suggestions never auto-confirm (APP-018.md:40; R3, "suggest … never apply", design:115).
- **§5.2 #8:** the problem code for an oversize import: demo 422 `narrow_the_query` (demo:372) versus a failed job with a reason (design:382).
- **§5.2 #9:** "visible within 1 s", meaning server state versus browser display, and the poll cadence.
- **§5.2 #11/#12:** the PH02 meaning of "adoption" and of a "label/layout-only patch" (APP-031.md:40).
- **§5.2 #13:** asset selection as a typed parameter, and new revisions versus the revisions that PH01 work pinned.
- **§5.3 #5:** filters in URLs (`spikes/APP-003` `GET /linked?filter=`) versus design:786's "filters travel in bodies".
- **§5.3 #11:** decided implicitly (the worker inserts revisions), but the grants are missing (X2 fix, M13 item 13).
- **§5.4 D9.7:** query cancellation semantics beyond the deadline (partly covered by M3's supersede-cancel).

## Dispositions (2026-10-29)

Recorded by GenUI against the revised `docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md`; the ADR's "Review dispositions" table is the controlling copy. Amendment labels are B1–B18 with B4 withdrawn and not reused.

| Finding | Disposition |
| --- | --- |
| X1 | Fixed. B4 withdrawn; D7.6 unchanged (READ COMMITTED READ WRITE, fences first); snapshot sequence and record rows read in one statement, later reads filtered by `commit_sequence ≤ S`; record input bounded at 100,000 rows / 64 MiB (D9.8). |
| X2 | Fixed. `genui_private.job_queue` and `job_cursor` (B2) behind five `genui_jobs` definer functions with grants (`enqueue_job`, `claim_next_job`, `renew_job`, `release_job`, `request_job_cancel`); unique running index for the single slot; round-robin; enqueue, claim, start, heartbeat and result as separate short transactions fenced by `claim_sequence` (B7); reclaim after lease expiry, dead letter on a fourth claim; `WorkerScope` and worker-only precheck (B9); worker grants (B10); eight-role normalization (B11); cancellation events on the chain of the role that made the transition (B12) (D9.4). Declined: a `job.claimed` audit event. Modified: workspace cancellation goes through `request_job_cancel`, not a direct `genui.jobs` update. |
| M1 | Fixed. Real permission names; `created_by` composite FK and RESTRICTIVE check (nullable for pre-existing rows); creator authority = `source_read` / `record_read` + `record_write` only, `run` and dependency clauses kept; `import_source` organization permission; `revoke-creator-authority`; operator-only sharing named B14 (D9.5, B13–B15). |
| M2 | Fixed. Organization, membership, entitlement and `import_source` checked under the fence with the current epoch at start and result; enqueue epoch kept for audit only (D9.4). |
| M3 | Fixed. B3: 2 running per task, work-conserving, round-robin, supersede of older-epoch queued queries, queue 12 per organization / 20 per task, amends D7.10(3) and design §6.3; supervised broker required for N3 p50 (D9.7). |
| M4 | Fixed. `RLIMIT_NPROC` dropped; `RLIMIT_AS` dropped in favour of the host RSS watch (modified: not kept pending measurement); NOFILE/CORE kept under APP-012's regression; limitations recorded (D9.2, D9.7). |
| M5 | Fixed. Normalization consumes parser output only, typed in Rust; `memory_limit` 1 GiB, `threads=1`, 120 s; import and normalize share one slot; budget table totals 6.5 GiB (D9.2, D9.3). Modified: `parser-host` exits before the normalize child starts. |
| M6 | Fixed, option (a): `normalized_digest` stays the Parquet-bytes digest; `rows_digest` and `original_bytes` added, required for worker inserts (D9.3, D9.6). |
| M7 | Fixed. Per-tenant lifecycle rules (staging 2 d / 1 d / 1 d; trial prefixes 90 d / 7 d; fixture noncurrent 30 d), explicit SSE-KMS on `CopyObject`, APP-016 owner, "expires within 2 days" (D9.3). |
| M8 | Fixed. Upload deadline 120 s with a 64 KiB/s floor; authorize → PUT → verify → enqueue; no transaction during transfer (D9.9). |
| M9 | Fixed. 6,000 queries/hour; stored originals over non-deleted revisions with a member delete; `genui.entitlement_limits` written by `set-limits`; durable versus in-process counters (D9.12, D9.13). |
| M10 | Fixed. APP-013's 0006 creates `query_receipts` with D9.8's columns; APP-023 adds columns only (D9.8). |
| M11 | Fixed. Source cache keyed `(organization_id, normalized_digest)` in the per-organization directory; APP-024 ticket amendment recorded for the Phase 2 plan (D9.7, D9.14). |
| M12 | Fixed. B5 names the CSV bound min(100,000, ⌊320,000 / columns⌋) without requalification; B6 names A5's one-slot reduction (D9.2). |
| M13 | Fixed. Amendments table B1–B18 with old/new text and owning decision, B4 withdrawn, non-amendments stated, mirrored in Consequences. |
| M14 | Decided as a default under "Victor's veto" (B18): beta copy, per-upload attestation, retention purge, application delete; fixture-only imports named as the alternative. Modified: the 90-day lifecycle applies to trial-organization prefixes, not the fixture organizations. |
| m1 | Fixed. CI qualifies the image-extracted WASM bytes; release refuses an unrecorded digest; `--remap-path-prefix`; `env_clear` (D9.1). |
| m2 | Fixed. RESTRICTIVE principal policy on `personal_view_state`; aliases require `admin_policy`; kinds on the `workspace` chain (D9.10). |
| m3 | Fixed. `source_deleted` in the edition; cache eviction on deletion (D9.6, D9.12). |
| m4 | Fixed. `quarantined_at`, `verify-receipts` on the `operator` chain, `source_quarantined` (D9.8). |
| m5 | Fixed. Per-operator/renderer lists in the canonical envelope, scalar columns as registry edition; "additive" defined (D9.6, D9.15). |
| m6 | Fixed. Labelled B16; APP-027 records per-feature byte deltas (D9.11). |
| m7 | Fixed. Pre-registered bindings, fAuth tenants, single-use organizations with key deletion, about $8/month into ADR-008's figure (D9.15). |
| m8 | Fixed. `job_cursor` round-robin, dead letter after three claims, `jobs --stuck` with APP-015, retention to APP-064 (D9.4). |
| m9 | Fixed. `unknown_value_in_aggregate` (D9.7). |
| m10 | Fixed. Managed-only sentences named (D9.15). |
| m11 | Fixed. Queue position and expected wait in the queued state (D9.2, D9.6). |
| §5.1 #5 | Decided: integer CIDR match, ratios reduced in Rust, up to 8 inputs (D9.7). |
| §5.1 #6 | Decided: one-statement read streamed to the child (D9.8). |
| §5.1 #7 | Assigned to APP-025. |
| §5.1 #13 | Assigned: APP-026 PH02 meaning recorded (D9.14); semantic suggestions APP-042. |
| §5.1 #14 | Assigned to APP-029. |
| §5.1 #17 | Decided for APP-015 (dead letter, `jobs --stuck`); retention APP-064; full view APP-066. |
| §5.2 #3 | Assigned to APP-018 with the rule stated. |
| §5.2 #8 | Decided: B17 (413 for declared size; failed job with the parser's code otherwise) (D9.9). |
| §5.2 #9 | Decided: committed state in the upload response, 500 ms poll, 1 s p50 measured by APP-019 (D9.4). |
| §5.2 #11/#12 | Decided as APP-031 ticket amendments (D9.14). |
| §5.2 #13 | Decided: `asset` as a typed filter on new revisions; PH01 work keeps its revisions (D9.3). |
| §5.3 #5 | Decided: filters in POST bodies only (D9.7). |
| §5.3 #11 | Decided: the worker inserts revisions (B8, B10). |
| §5.4 D9.7 | Supersede decided (D9.7); remaining cancellation semantics assigned to APP-024. |

## Re-review (2026-10-29)

**Verdict: changes requested.** No blocker remains. X1 is resolved and reproduced. X2 is resolved apart from one major: as written, lease reclaim can stall the whole queue (R-M1, a one-sentence fix). There are also six minors. Every other disposition holds in the text.

- **Reviewed file:** `docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md` at commit `2057fbc`, SHA-256 `e833e0ddcfb24a3d21987ac58d506c0036482ddffeb20b8f0c493612b8194f9d` (the working copy equals the committed blob). Read-only except this section; nothing committed.
- **Probe:** a throwaway PostgreSQL **16.13** (Homebrew) cluster in the session scratchpad (`…/scratchpad/rr009`).
  - The migration identity was non-superuser `genui_migration` (CREATEROLE, admin on `rds_iam`).
  - `0001` and `0002` were applied unchanged (SHA-256 `f98f67b7…` and `7b163e78…`, as in `manifest.json`).
  - Added on top: D7.7's `organization_sequences` with D7.3's data grants; fixture rows for two organizations; and a minimal sketch of D9.4 written from the ADR text. The sketch covers `genui.jobs` (FORCE RLS, `tenant_scope`), `genui_private.job_queue`/`job_cursor`, the unique partial running index, role `genui_jobs` created like `genui_lock`, its five definer functions with exactly the listed grants, B10's worker grants and the workspace additions.
  - The cluster was stopped afterwards.

### X1: holds

ADR-009:124 keeps D7.6 (READ COMMITTED READ WRITE). It takes the member and work fences first, then reads S = `next_commit_sequence − 1` and the latest revision per record with `commit_sequence ≤ S` in one statement. Later statements filter by `≤ S`. B4 is withdrawn (:7, :29) and not reused.

The probe had four concurrent `genui_data` writers. Each followed D7.9's lock order: fences, then `organization_sequences FOR UPDATE`, then the record row. Each inserted a revision, slept, advanced the sequence, slept and committed: 1,000 commands, 0 errors.

| Probe | Result |
| --- | --- |
| Reader with the fences, then the X1 single statement, run as `genui_data` under FORCE RLS | **1,500 reads, 0 inconsistent.** In every read, the count of revisions `≤ S` equals S and no revision `> S` is visible. |
| Negative control: S in one statement, rows in a later one | **243 of 300** reads saw revisions beyond S. With the `≤ S` filter, the same later reads matched S in all 300. |

This confirms both halves of D9.8:
- the one-statement read is consistent;
- a later statement is stable only with the `commit_sequence ≤ S` filter.

The fence (`lock_member_authority`, `lock_work_revision`) runs in READ COMMITTED READ WRITE with no 40001.

### X2: holds, with R-M1

Probe results (`genui_workspace`/`genui_worker` logins, functions owned by `genui_jobs`):

| # | Claim (ADR-009 line) | Result |
| --- | --- | --- |
| T0 | `genui_jobs` grants as listed (:81) | **Fails.** `ERROR: permission denied for schema genui` inside `enqueue_job`. Passes after `GRANT USAGE ON SCHEMA genui, genui_private TO genui_jobs` (R-m1). |
| T1 | `enqueue_job` 5-per-organization cap (:75) | queued ×5, then `full`. |
| T2 | private relations reachable only through the functions (:69) | `permission denied for table job_queue` (worker) and `job_cursor` (workspace). |
| T3 | scope check on `enqueue_job` (:71) | organization B scope naming organization A → `refused`. |
| T4 | `request_job_cancel` on a queued job (:79) | queue row deleted. `genui.jobs` becomes `cancelled` with `cancel_requested_at` set, through `genui_jobs`' column UPDATE under the caller's RLS scope. |
| T5 | cursor lock makes the cap exact (:75) | Two concurrent enqueues at 4/5 → one `queued`, one `full`; the second waited about 0.5 s on the cursor row. |
| T6 | unscoped claim across organizations (:76) | The worker with no scope claims (`seq=1 attempts=1`). A second claim returns nothing while the lease is unexpired. |
| T7 | `WorkerScope` (:92) and B9/B10 grants | Organization scope, then job principal and current epoch read under scope, then `lock_member_authority` → `true true 1`. The worker's column UPDATE of `genui.jobs` succeeds. |
| T8 | result fencing (:78, :88) | `release_job` with a stale `claim_sequence` → `false`; under another organization's scope → `false`; with the current sequence → `true` and the row is deleted. |
| T9 | running cancel, heartbeat and round-robin (:77, :79, :87) | `requested`; `renew_job` → `true cancel=true`; stale renew → `false`; release → `cancel=true`. Claims alternate A, B, A. |
| T10 | concurrent claimers (:76) | The second claimer SKIP-LOCKs past the first's row, then waits on the running index. It gets **23505** when the first commits within 1 s, or **55P03** (`lock_timeout` 1000) when the first holds 1.5 s. |
| T11 | lease reclaim (:76, :90) | **Fails as written** (R-M1). |
| T12 | cancel racing an in-flight claim (:79) | The cancel waits on the claimer's row lock and re-reads the row as `running` → `requested`, with `cancel_requested=true` on the queue row. |

Text checks of the X2 fix:
- Enqueue, claim, start, heartbeat and result are separate transactions inside 3 s (:83–88; B7).
- Dead letter on the fourth claim (:86).
- `WorkerScope` is never a `TrustedRequest`, and the epoch is not compared with the enqueue value (:92).
- B10 worker grants (:94).
- B11 eight-role normalization with `catalog-query.json`/`qualify_tenant_schema.py` re-pinned (:81). Today's `catalog-query.json` lists six roles; 0003 adds `genui_admit` and APP-015 adds `genui_jobs`.
- B12 cancellation chains by transition owner (:96).
- `job.claimed` declined (:179).

### Majors

**R-M1 — Lease reclaim stalls the queue under the unique running index (D9.4, ADR-009:76 with :69).**

*Evidence.*
- `claim_next_job` "picks … the eligible row (queued and available, or running with an expired lease) of the organization with the oldest `last_claimed_at`".
- The crashed job's organization was stamped when that job was claimed, so every other organization with queued work has an older cursor.
- The claimer therefore picks another organization's queued row and sets it `running`. That collides with the still-`running` expired row.

Probe T11: the organization B row was running with its lease expired, and organization A had three queued rows with an older cursor. Three successive claims each returned `ERROR: duplicate key value violates unique constraint "job_queue_one_running"`.
- The cursor stamp rolls back with the claim, so the same choice repeats indefinitely.
- No job runs until an operator intervenes.
- This defeats APP-015's crash/reclaim requirement and B6's "never silent loss".

A variant that treats a running row as the only candidate reclaimed the expired job on the first call (`seq=2 attempts=2`).

*Fix.* Replace the `claim_next_job` behavior cell with:

> If a `running` row exists, it is the only candidate: unexpired → return nothing; expired → lock it `FOR UPDATE SKIP LOCKED` and reclaim it (increment `claim_sequence` and `attempts`, new 120 s lease). Only when no `running` row exists, pick the oldest-cursor organization's queued, available row.

Add a two-organization crash/reclaim case to APP-015's test.

### Minors

**R-m1 — `genui_jobs` grants are incomplete as written (ADR-009:81).**
- **Schema USAGE.** Without `USAGE` on `genui` and `genui_private`, every function fails (T0). 0002 grants the same to `genui_lock` (`0002:223–224`).
- **Kind check.** `SELECT(organization_id, job_id, state)` on `genui.jobs` does not let `enqueue_job` check that `p_kind` matches the scoped row's `kind`.
- **Fence grant.** B9's EXECUTE on `lock_member_authority` must be issued as `genui_lock`. That role owns the function since `0002:280`; issuing it as `genui_owner` gives `permission denied for function lock_member_authority`.

*Fix:*
- add `USAGE ON SCHEMA genui, genui_private` and `SELECT(kind)` to the `genui_jobs` grant list;
- state that APP-015's migration grants the fence to `genui_worker` under `SET LOCAL ROLE genui_lock`;
- add these tuples to the B10 catalog assertion.

**R-m2 — Ledger: a few changes to controlling records still carry no label (M13 residue).**
1. **ADR-007 D7.14** "Handler deadline 5 s" (ADR-007:88) is set aside for the upload route at ADR-009:130. It appears neither in the table (:9–27) nor in "Not amended" (:29). (The body limit is a new route class, not an amendment.)
2. **Design D11 / §6.4.** "One binary holds every component" (design:228, :238) and the parsers "run inside the worker" (design:319). They are departed from by D9.1's separate `parser-host` executable (ADR-009:35), the same kind of change that D7.10 labelled a "Named deviation" for DuckDB (ADR-007:72).
3. **ADR-007 D7.8.** Its canonical event bytes are "every column except `hash`" (ADR-007:64). ADR-009:96 changes this to "a new column only when it is non-NULL", but B12's New column (:21) names only the added columns.
4. **0002:85** `CHECK (renderer_version=1)`/`(operator_version=1)` broadened to `≥ 1` at ADR-009:108. B13 shows that 0002 CHECK changes are labelled.
5. **ADR-004:20–21.** Every private transaction installs organization, principal and epoch, and "no unscoped membership lookup occurs". The unscoped claim/renew transactions (:85, :87) and `WorkerScope`'s staged installation (:92) are covered in substance by B2/B9, but B9's Old column cites only ADR-004:13.

Nits:
- B8's Where says D9.4, but D9.4's heading (:65) does not mark it.
- D9.5's new operator kinds (:102, :104) are not marked B12.
- "design A6 retention (D9.15 implements it)" (:29): the lifecycle is in D9.3 and the purge is B18, and the two fixture organizations are exempt (:63). State the fixture exemption.

*Fix:*
- add B19 (D7.14 deadline for `POST /v1/sources/uploads`) and B20 (`parser-host` deviation from D11/§6.4), or fold them into existing rows;
- extend B12's New column with the canonical-bytes rule;
- extend B13 or B10 with the 0002:85 CHECK;
- extend B9's Old column with ADR-004:20–21;
- mark B8/B12 at their points of change.

**R-m3 — A failed authority recheck at start or result has no stated outcome, and the epoch can race the fence (ADR-009:86, :88, :92).**
- If "recheck authority" fails (member revoked, entitlement expired, `import_source` withdrawn), the text does not say whether the worker fails the job or rolls back. A rollback leaves the queue row running until lease expiry, then reclaim, and finally a misleading `worker_lost` dead letter.
- `WorkerScope` reads the current epoch in one statement and takes the fence in the next. A sign-out committed between them makes the fence return no row, though the member is still active. That is the exact case M2 says must not cancel a job.

*Fix:*
- A refused recheck ends the job `failed` with a named code (for example `forbidden` or `entitlement_expired`) and one `job.failed` on the `worker` chain, and releases the queue row in the same transaction.
- On a fence miss where the membership is still active, `WorkerScope` re-reads the epoch and retakes the fence once. This is a read in the same transaction, not a mutation retry.

**R-m4 — `queue_position` cannot be computed under the function set (ADR-009:39, :98, :108).**
- The upload response and the poll carry a queue position and expected wait.
- The workspace cannot read `job_queue`, and none of the five functions returns a position. Under round-robin, the position depends on other organizations' rows, which only a definer can see.

*Fix:* either
- define the position as organization-local (the organization's earlier `queued` `genui.jobs` rows, readable under RLS), labelled an estimate; or
- have `enqueue_job` return the position and add a read-only `job_queue_position(p_org, p_job)`, updating "five functions", the Consequences and the campaign assertion.

**R-m5 — Job read authorization is unstated (ADR-009:98).** `GET /v1/jobs/{job}` is guarded only by organization RLS on `genui.jobs`, and the payload carries upload metadata. *Fix:* a job is readable by its `principal_id`, or by a holder of `source_read` on its source; otherwise the uniform 404.

**R-m6 — Claim classification and index scope (ADR-009:69, :76).**
- A concurrent claimer ends in 23505, or in 55P03 when the first claim holds past `lock_timeout` (T10). The text names only 23505.
- The unique running index is one per database, which is per task only under D7.18's single task.

*Fix:*
- classify both 23505 and 55P03 as "nothing claimed";
- write "one job running per deployment (one task under D7.18)", so APP-061 revisits it.

### Disposition check

All other dispositions hold in the revised text:

| Finding | Where it holds |
| --- | --- |
| M1 | :102 — real permission names; `created_by` composite FK and RESTRICTIVE check; creator authority narrowed; `run` and dependency clauses kept; `import_source`; `revoke-creator-authority` |
| M2 | :92 |
| M3 | :116, B3 :13 |
| M4 | :120, :51 |
| M5 | :41–51, :57 |
| M6 | :61 |
| M7 | :57, :63 |
| M8 | :130, apart from the label in R-m2 |
| M9 | :142, :146 |
| M10 | :126 |
| M11 | :118 |
| M12 | :39, B5/B6 |
| M13 | :5–29, apart from R-m2 |
| M14 | :160–168 |
| m1 | :35 |
| m2 | :134 |
| m3 | :108, :142 |
| m4 | :126 |
| m5 | :108, :154 |
| m6 | :138 |
| m7 | :154 |
| m8 | :86, :98, :158 |
| m9 | :114 |
| m10 | :154 |
| m11 | :39, apart from R-m4 |

Map gaps: each is decided or assigned (:156–158 and the table at :205–218).

**Labels.** B1–B18 are each defined in the table and marked at a point of change (B4 is withdrawn, with no other use). The Consequences (:172) mirror them.

**Declined and modified items are reasonable:**
- no `job.claimed` event: the claim sequence is carried in the result event;
- the workspace cancels only through `request_job_cancel`: fewer grants;
- `RLIMIT_AS` dropped for the RSS watch: address-space reservations are a poor proxy for memory;
- `parser-host` exits before normalize: the slot holds one process;
- the 90-day lifecycle is not applied to the fixture organizations: the demo needs that operator-seeded content;
- `created_by` is nullable for pre-existing rows under the additive rule: the RESTRICTIVE `WITH CHECK` still refuses NULL on every new insert.
