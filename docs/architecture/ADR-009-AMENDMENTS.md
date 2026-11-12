# ADR-009 amendments kept outside the locked ADR file

Owner: GenUI. Status: accepted implementation direction for Phase 2 preparation. It authorizes no Phase 2 implementation and no runtime result. Each section is dated and append-only.

**Why this file exists.** `contracts/lock.py` hashes [ADR-009](ADR-009-PHASE-2-DATA-PLANE.md) and [ADR-002](ADR-002-CONTRACT-EDITION.md) whole into `contracts/implementation-lock.json` (edition `impl-2026-11-01.2`). ADR-002 forbids rewriting a published edition's lock in place. One byte added to either ADR therefore fails `lock.py check`, a required workflow, until a new contract edition opens. So amendments to ADR-009 live here.

From its date, a section here controls over the ADR-009 text it names. When the next contract edition opens, its owner copies each section into ADR-009 as a dated amendment, and the edition decisions (E5, E6, E7 and E11) into `contracts/README.md`. That edition then writes its own lock, and this file keeps a pointer to the copy.

Line numbers such as "ADR-009:82" refer to the locked file, SHA-256 `8a52d3cb7efa6e8451633de44634c3f3fcd3107044be4c6c94b8cf85c765172f`, unchanged since `29b6ff6`.

## 2026-11-12: re-review residuals and edition-two decisions (APP-088)

Victor selected APP-088 on 2026-11-12. The sources are the [re-review](../../evidence/APP-015/adr-009-review.md) (section "Re-review (2026-10-29)", which read ADR-009 at `2057fbc`) and items 5, 6, 7 and 11 of the [contract-edition review](../../evidence/APP-015/contract-edition/independent-review.md). [ADR-010](ADR-010-NOV-2-MVP-CUT.md) filed both as APP-088.

**What ADR-009 already holds.** Commit `3f5ec2a` (2026-10-29) applied the re-review's fixes in place, before ADR-010 filed APP-088. It replaced the claim cell, extended the grants, added B19–B23 and wrote a "Re-review dispositions" table (ADR-009:226–237). The replaced wording survives in `2057fbc` and in the re-review's quotations. This section checks each fix against the re-review and APP-088. It confirms what holds, and completes or supersedes what falls short, citing the line.

### R-M1: the running row is the only claim candidate (D9.4)

**Holds:** the `claim_next_job` cell (ADR-009:82). If a `running` row exists, it is the only candidate. If its lease is unexpired, the call returns nothing. If expired, the row is locked `FOR UPDATE SKIP LOCKED` and reclaimed. Only when no running row exists does the call pick the queued, available row of the organization with the oldest `last_claimed_at`.

**Added**, so that no reading leaves an expired row unreclaimed:
- The existence test reads the running row without `SKIP LOCKED`. A running row locked by a concurrent claimer still exists, so that call returns nothing. It never falls through to a queued row, which would collide on the unique running index.
- The running row is the candidate whatever its `kind`. `p_kinds` filters only the queued pick.
- A reclaim does what a first claim does: `claim_sequence` + 1, `attempts` + 1, a new 120 s lease, and a stamp on that organization's cursor. Step 3's fourth-claim dead letter (ADR-009:92) still applies.
- So the first claim after a lease expires reclaims the job. No operator is needed, and no queued row is set `running` while any running row exists.

**Superseded:** no current ADR-009 line. The wording R-M1 refuted was "picks … the eligible row (queued and available, or running with an expired lease) of the organization with the oldest `last_claimed_at`" (ADR-009:76 at `2057fbc`, quoted in the re-review). `3f5ec2a` replaced it, and this amendment confirms the replacement. On 2026-11-12 a search of `docs/`, the active ticket folders and the Phase 2 plan found no other `claim_next_job` text that leaves an expired running row unreclaimed.

**Test** (APP-015, plan Task A1): `expired_running_row_is_the_only_claim_candidate_across_two_organizations`. Organization B's job is `running` with an expired lease (the crash). Organization A has queued jobs and the older cursor. The first claim returns B's job with `claim_sequence` 2 and `attempts` 2. B's old worker's `release_job` with sequence 1 releases nothing. After B's job is released, the next claim returns A's oldest queued job.

### R-m1: `genui_jobs` grants and the fence grantor (D9.4, B10)

**Holds:** ADR-009:87 grants `genui_jobs` USAGE on schemas `genui` and `genui_private` and `SELECT(kind)` on `genui.jobs`. ADR-009:98 issues the EXECUTE on `lock_member_authority` to `genui_worker` as `genui_lock`.

**Added:**
- APP-015's migration issues that grant under `SET LOCAL ROLE genui_lock`. `genui_lock` has owned the function since 0002:280. Issued as `genui_owner`, the grant fails with `permission denied for function lock_member_authority`.
- B10's catalog assertion includes these tuples, grantor included. That assertion is the schema campaign's check that the grant set equals D7.3 plus D7.1 plus B10 (ADR-009:178):
  - `genui_jobs` USAGE on schema `genui` and on schema `genui_private`;
  - `genui_jobs` SELECT(`organization_id`, `job_id`, `state`, `kind`) and UPDATE(`state`, `cancel_requested_at`, `updated_at`) on `genui.jobs`;
  - `genui_jobs`' privileges on `job_queue` and `job_cursor`, exactly as ADR-009:87 lists them;
  - `genui_worker` EXECUTE on `genui_private.lock_member_authority(uuid, uuid, bigint)`, grantor `genui_lock`.
- A comparison of grantee and privilege that ignores the grantor does not meet this.

**Superseded:** nothing.

**Test** (plan Task A1): `genui_jobs_role_attributes_and_grants_match_d9_4` and `grant_set_equals_d7_3_plus_d7_1_plus_b10` assert the tuples above.

### R-m2: labels, marks and the fixture exemption

**Holds:** B19–B23 are in the Amendments table (ADR-009:28–32). APP-088 names three of them by the row they extend:
- **B21** (ADR-009:30) is B12's canonical-bytes rule. Read B12's New column (ADR-009:21) as including it.
- **B22** (ADR-009:31) is the 0002:85 CHECK. It has its own row, as B13 (ADR-009:22) has for 0002's permission CHECK.
- **B23** (ADR-009:32) extends B9. Read B9's Old column (ADR-009:18) as citing ADR-004:13 and ADR-004:20–21.

**Added: marks at the points of change.** ADR-009:7 promises that each label is marked where it changes the text. From this date these sentences carry these labels:

| Label | Point of change |
| --- | --- |
| B8 | D9.4 step 5, "write the revision (normalize only)" (ADR-009:94); D9.4 Grants, `genui_worker` INSERT on `source_revisions` (ADR-009:100) |
| B12 | D9.5's `creator_authority.revoked` (ADR-009:108) and `permission.granted` / `permission.revoked` (ADR-009:110); D9.8's `revision.quarantined` (ADR-009:132); D9.12's `source.deleted` (ADR-009:148); D9.13's `limits.set` (ADR-009:152); B18's `organization.objects_purged` (ADR-009:171) |
| B19 | D9.9, "The route's deadline is its own" (ADR-009:136) |
| B20 | D9.1, "ships as a separate executable, `parser-host`" (ADR-009:41) |
| B21 | D9.4 Audit, "canonical event bytes include a new column only when it is non-NULL" (ADR-009:102) |
| B22 | D9.6, "their CHECK broadens to `≥ 1`" (ADR-009:114) |
| B23 | D9.4 step 2, "Claim (worker, no scope)" (ADR-009:91); step 4's unscoped `renew_job` (ADR-009:93); Worker authority (ADR-009:98) |

**Superseded:** ADR-009:35's reason "design A6 retention (D9.15 implements it)".

**New:** D9.3's lifecycle rules (ADR-009:69) and B18's purge (ADR-009:171) implement design A6, not D9.15. The two fixture organizations are exempt from the 90-day expiry of current objects. They hold operator-seeded synthetic content that the demo needs and a rerun rebuilds. Their staging objects still expire after 2 days, and their `s/` noncurrent versions after 30 days (ADR-009:69). The re-review accepted this exemption as reasonable.

### R-m3: a refused recheck ends the job in one transaction (D9.4)

**Superseded:** ADR-009:98, "The epoch is read and the fence taken in one statement, so a sign-out between them cannot stop the job spuriously (R-m3)." One statement narrows the race but does not close it:
- `lock_member_authority` is a VOLATILE plpgsql definer. It returns nothing unless the installed `genui.membership_epoch` equals its argument (0002:247–249).
- It reads the membership `FOR SHARE` in its own statement (0002:252–254). Under READ COMMITTED that statement sees a sign-out that committed after the outer statement began.
- The epoch comparison then fails although the member is still active.

**New:**
- `WorkerScope` installs the organization, reads the job's principal and that member's current epoch under scope, installs them, and takes the fence.
- On a fence miss, it re-reads the membership under scope in the same transaction. If the membership is active, it installs the epoch just read and takes the fence once more. This is a read inside one transaction, not a mutation retry, so ADR-004's rule against automatic mutation retries is unchanged.
- A second miss, an inactive membership, or a failed `precheck` (organization, membership, entitlement, `import_source`) is a refused recheck. Two epoch advances inside one step's transaction therefore end the job visibly; the rule allows one retake, not a loop.
- A refused recheck at start (step 3, ADR-009:92) or at result (step 5, ADR-009:94) commits all of the following in that step's one transaction:
  - `release_job` with the claim's sequence, which deletes the queue row;
  - `genui.jobs.state = 'failed'` with `failure_code = 'authority_revoked'`;
  - no revision;
  - exactly one `job.failed` event on the `worker` chain.
- If `release_job` releases nothing (a newer claim owns the job), the transaction rolls back and writes nothing.
- A refused recheck never rolls back and leaves the row `running` until its lease expires. That path would end, after reclaims, as a misleading `worker_lost` dead letter.
- `authority_revoked` is the one code for every refused recheck. Edition two's open `failure_code` grammar already carries it (E11).

This completes ADR-009:98's failure sentence, which named the code and the event but not the single transaction.

**Tests** (plan Task A2): `signout_keeps_job_running_but_revoked_entitlement_fails_it_authority_revoked` (existing), `fence_miss_with_active_membership_retakes_once`, and `refused_recheck_fails_releases_and_writes_one_job_failed_in_one_transaction`.

### R-m4: `queue_position` is an organization-local estimate (D9.2, D9.4)

The re-review offered two fixes. This takes the first, which edition two's contract already states: `queue_position` is "the organization position `enqueue_job` returns, 1-based" (`contracts/README.md`, edition `impl-2026-10-29.2`).

- `queue_position` is the job's 1-based place among its organization's jobs that have not started.
- `enqueue_job` returns it with `queued`. It counts the organization's queued `job_queue` rows, this one included, under its cursor lock.
- Each poll of `GET /v1/jobs/{job}` recomputes it from `genui.jobs`: 1 + the organization's rows in state `queued` created earlier (ordered by `created_at`, then `job_id`). The workspace reads those rows under its RLS scope, so the poll needs no new definer function.
- `expected_wait_seconds` = `queue_position` × the in-process p50 job duration (D9.2, ADR-009:45), at enqueue and on every poll.
- Both are estimates. As edition two requires, they travel together and only on a `queued` job. They ignore other organizations' queued work, which round-robin interleaves. A poll may also count a claimed job whose start has not committed, so it can read one higher than `enqueue_job`'s count.

**Superseded:** ADR-009:104's "and the task-wide count ahead". No contract field carries it, and a poll could not recompute it. `genui_jobs` keeps five functions. ADR-009:77, the Consequences (ADR-009:178) and the campaign's reachability assertion are unchanged.

**Test** (plan Task A2): `states_queued_running_failed_done_cancelled_readable_with_position_and_wait` checks that the upload response and a later poll agree. It also checks that the position drops by one when an earlier job of the same organization starts or is cancelled.

### R-m5: who can read a job (D9.4)

**Superseded:** ADR-009:104's "or by a member holding `import_source` on the organization".

**New:**
- `GET /v1/jobs/{job}` returns the job to its `principal_id`, or to a member who holds `source_read` on the job's source. Creator authority counts, because D9.5 makes it equal to `source_read` on that source (ADR-009:108).
- The shared precheck and the member fence run first.
- Everyone else, inside or outside the organization, gets the uniform 404, the same as for an absent job.
- The job's source is the `sources` row its enqueue inserted (an import, D9.4 step 1) or the source its normalize payload names (APP-018).
- `import_source` lets a member upload. It does not disclose other members' jobs or their upload metadata.

**Not decided here:** who may cancel a job. ADR-009 does not say; APP-015 states it (plan Task A2).

**Test** (plan Task A2): `job_read_is_uniform_404_for_other_principals_without_source_read`, renamed from `…_without_import_source`.

### R-m6: a failed claim, and one running job per deployment (D9.4)

**Holds:** ADR-009:82 classifies a concurrent claimer's unique violation (23505) on the running index, and a lock wait past `lock_timeout` (55P03), as nothing claimed. The claim transaction rolls back and the runner polls again. This applies to `claim_next_job` only. D7.9's mapping of 55P03 to 409 `busy`/`in_flight` is unchanged everywhere else.

**Superseded:** ADR-009:75's "which makes 'one job running per task' (B6) a database fact".

**New:** "which makes one job running per deployment (one task under D7.18) a database fact".
- The unique partial index spans the database. It equals one per task only while D7.18 runs exactly one task.
- B6 (ADR-009:15) and D9.2's "one job runs per task and therefore per organization" (ADR-009:45) hold only under D7.18.
- **APP-061** (PH05) revisits this before two tasks share the database. D7.18 assigns it rolling deployment with shared state. It either keeps one job per deployment or replaces the index with a per-task slot.

**Test** (plan Task A2): `concurrent_claimers_classify_unique_violation_and_lock_timeout_as_nothing_claimed` (existing).

### Edition-two decisions (D9.6; contract-edition review items 5, 6, 7 and 11)

Edition `impl-2026-10-29.2` kept these items as "documented here" in `contracts/README.md`. That README and the edition are locked. Each item now has a decision, and the next edition copies it into its README.

**E5, derivable identity lists (item 5): kept.**
- The publication and receipt `operators`/`renderers` lists stay. Every registered version is 1, so today the lists are derivable from the definition.
- They carry information from the first edition that raises one operator's or renderer's version. That edition adds a vector whose list differs from the registry edition and retains every version-1 vector.
- `window_minutes` stays a denominator of definition schema version 2 with no `ratio` version change, because no version-1 definition can carry it.
- Any later change to what an existing definition computes raises that operator's version.
- Owner: APP-022, the versioned operator registry (plan Task B3).

**E6, `limit_exceeded` and the 429 scope (item 6): kept as decided.**
- `limit_exceeded` is the code of every 429 that the D9.2 job queues, the D9.7 query queues and the D9.13 limits produce. It names `limit` (one of the five) and `retry_after_seconds`, and only such a 429 carries them.
- 429s from ADR-006's HTTP-layer rate limits stay outside this definition.
- `organization_queue` and `task_queue` stay shared by the job queue (5 per organization, 20 per task) and the query queue (12 and 20). The route tells which: a job-creating route reports the job queue, and a `/v1/work/…/queries/…` route the query queue. No route reports both.
- Emitters: APP-015 and APP-019 for job and import limits, APP-024 for query limits (plan Tasks A2, A7 and B5).

**E7, row framing, row grammar and unknown values (item 7): framing kept, grammar assigned.**
- The array framing stands. `rows_digest` is the digest of one forma-json-v1 array of typed rows in sorted order, and the edition-two vectors pin it.
- APP-018 defines the typed row grammar and the representation of an unknown value (plan Task A6). D9.3 makes an integer outside int64 unknown with its original text, and the plan treats a missing value the same way.
- Decided here: an unknown value is distinguishable from every typed value, carries the original cell text verbatim, and contains no float.
- The grammar changes what a digested row may contain, so it enters the contract in the next edition (ADR-002). APP-018 opens that edition before any revision with a `rows_digest` is stored on a managed database. It adds Python, JavaScript and Rust vectors that contain an unknown value, and retains the edition-two `rows_digest` vectors.
- APP-016 stores the revision and refuses a worker insert without `rows_digest` or `original_bytes` (plan Task A3). It does not interpret rows.

**E11, the `failure_code` vocabulary (item 11): open in edition two.**
- `failure_code` stays the identifier grammar, not an enumeration.
- The platform itself writes `worker_lost` (the fourth claim), `authority_revoked` (R-m3) and `resource_limit` (the RSS kill of D9.2). Parser refusal reasons, `fuel_exhausted` among them (B24), pass through verbatim.
- APP-017 lists in its evidence the refusal reasons `parser-host` can emit (plan Task A5). Closing the vocabulary is a later edition's decision, once that list is stable.
- `cancelled` is a state, never a `failure_code`. The runtime never writes `failed` with `failure_code = 'cancelled'`, and APP-015 asserts that. Edition two still accepts the pair, so the next edition adds a negative vector that refuses it.
- Item 11's other points stand as accepted. The old lock file's identity is recorded and not retained. ADR-009 is locked on purpose because it controls the edition, and that lock is why this file exists.

### Consumers

APP-015 implements R-M1 and R-m1–R-m6 in migration 0008 (plan Task A1) and its runtime (Task A2). APP-016 (Task A3) and APP-018 (Task A6) implement E7, APP-017 (Task A5) and APP-015 E11, APP-019 and APP-024 E6, and APP-022 E5. The Phase 2 plan's tasks name this section. The edition-two review's unrun limitation, the Docker-backed campaign that seeds through `tenant_seed.py`, is closed by the run in [evidence/APP-015/residuals-2026-11-12](../../evidence/APP-015/residuals-2026-11-12/README.md).
