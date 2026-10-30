# Independent review: migration 0003 (plan Task 4, APP-073 slice 1)

**Commit reviewed:** `8fcf925` (feat(schema): migration 0003 admission ledger and definer principal), merged to `phase-one-spikes` as `e39c1bb`. The checkout was at `3f5ec2a` during the review; `git diff --stat 8fcf925 HEAD -- app/ scripts/ .github/ fixtures/` is empty, so every source below is byte-identical to `8fcf925`.

**Controlling text:** ADR-007 D7.1, D7.2, D7.3, D7.8, D7.12, D7.17, D7.19 and Consequences (`docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md:11-26,28-44,62-64,78-80,98-100,106-112`); ADR-004; ADR-003 §Code lifecycle and admission and the cross-service paragraph (`ADR-003-PUBLIC-BETA-ACCESS.md:43,47-51`); ADR-006 callback row and Flow 2 (`ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md:33,43`); plan Task 4 (`docs/planning/PHASE-1-EXECUTION-PLAN.md:120-134`); `Tickets/3-in-review/APP-073.md`; the implementer's `evidence/APP-073/schema-0003/README.md`; the runner review `evidence/APP-073/runner-review.md` (M1, m1–m3).

**Method.** Read-only review; nothing committed; the only file written in the repository is this one. (1) Read `0003_admission.sql` in full against D7.1/D7.3/D7.8 and 0002's fences, and the 0003 additions to `qualify_tenant_schema.py`, `catalog-query.json`, `runtime.py`, `generate_schema_catalog.py`, `check-schema-pins.py`, the tests and the workflow. (2) Ran the local checks. (3) Ran the schema campaign exactly as the README records. (4) Ran eleven adversarial probes (`P1`–`P11`, script and JSON in the session scratchpad `rev/probes.py`, `rev/probes.json`, not in the repository) against a disposable PostgreSQL 16.13 container built through the campaign's own `LocalPostgres` (0001, `apply_migrations` to v3, operator seed, genesis heads). (5) Read the GitHub Actions run of the branch head. Every container I started was removed (`container_removed: true` in both reports; `docker ps -a` afterwards shows only the two containers that predate this review and belong to other sessions).

## Verdict

**changes requested** — no blockers, two majors (code-side refusals of a live pending admission are not recorded for reconciliation as ADR-006 requires; a concurrent same-identity admission through two codes raises 23505 instead of the typed `already_member`). Both fixes are local to `admit_redemption` and one was verified on a disposable database. Everything else I tried to break held: privilege boundary, search_path, scope checks, lock order, last-slot arithmetic, replay, audit-chain integrity, grant set (one surplus column, m2), catalog normalization and the CI wiring.

## Local and CI results (exact)

| Command | Result |
| --- | --- |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime` | 39 tests, OK (12.1 s) |
| `app/runtime/.venv/bin/python -m unittest discover -s scripts -p 'test_*.py'` | 5 tests, OK |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`; five version pins = 3; four bound pins = 524,288; `catalog-v2.json` → 2, `catalog-v3.json` → 3 |
| `app/runtime/.venv/bin/python <scratch>/with_contracts.py app/runtime/qualify_tenant_schema.py --output <scratch>/rev/campaign.json` | exit 0, `passed: true`, **276 checks**, `container_removed` and `fresh_container_removed` true |
| `app/runtime/.venv/bin/python <scratch>/with_contracts.py <scratch>/rev/probes.py` | results below; `container_removed: true` |
| GitHub Actions run `40947220411` (Tenant foundation qualification, head `3f5ec2a`) | `success`; the new step "Check schema-version and catalog-bound pins, runner and pin-check unit tests" ran (`agree: true`, 5 and 39 tests), then the schema campaign and the native TLS campaign passed |

## Findings

### Blocker

None.

### Major

**M1. A pending admission refused because its code was meanwhile revoked, expired or exhausted stays `live` and leaves no record; ADR-006 requires it to be denied and recorded for reconciliation.**

- ADR-006 Flow 2 (`ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md:43`): "A pending admission whose code has meanwhile expired, been revoked or exhausted, or that outlives its 24 hours, is denied and recorded for operator reconciliation". ADR-003 (`ADR-003-PUBLIC-BETA-ACCESS.md:43`): GenUI "must still refuse admission when its own current checks fail, record the partial outcome and reconcile the unused identity". ADR-007 declares both controlling for their subjects (`ADR-007…md:3`); D7.1 states only the 24-hour case ("An expired pending row is marked `denied_for_reconciliation`") and is silent on the code-side cases, so the narrower reading is not required by D7.1 and is not listed among the README's implementation choices.
- Implementation: `0003_admission.sql:356-359` (revoked, expired code) and `:364-366` (exhausted) `RETURN` before the audit append, so the pending row remains `state='live'` and no `admission.denied_for_reconciliation` event exists. Only the 24-hour branch (`:349-352`) records. The campaign pins the omission: `qualify_tenant_schema.py:457-462` asserts `row_counts(org0)==before` after the revoked, code-expired and exhausted refusals.
- Probe P9 (revoke the code while the pending admission is live, then `admit_redemption`): outcome `revoked`, pending state afterwards `live`, audit events added `0`.
- Consequence: the provider identity created for that pending admission (APP-073 acceptance: "completed provider identity creation followed by locally expired/revoked/exhausted admission produces no membership/entitlement grant") has no GenUI denial record on the admission chain, and the row keeps reading as a live admission. An operator `code.extended` on an expired code would also silently revive a pending admission that was already refused.
- Fix (`0003_admission.sql:356-366`), reusing the existing append and return path (which already handles a non-`admitted` `v_outcome`):

```sql
 ELSIF v_code.status='revoked' THEN
  UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  v_outcome:='revoked'; v_kind:='admission.denied_for_reconciliation';
 ELSIF v_code.status='expired' OR v_code.expires_at<=now() THEN
  UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  v_outcome:='expired'; v_kind:='admission.denied_for_reconciliation';
 ELSE
  SELECT count(*) INTO v_others FROM genui.code_reservations r WHERE r.organization_id=p_org AND r.code_id=v_code_id
   AND r.kind='pending' AND r.state='live' AND r.expires_at>now() AND r.reservation_id<>p_pending_admission;
  IF v_code.status='exhausted' OR v_code.max_redemptions-v_code.redeemed_count-v_others<1 THEN
   UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
   v_outcome:='exhausted'; v_kind:='admission.denied_for_reconciliation';
  ELSE
   -- unchanged admitted branch (principal … entitlement, v_outcome:='admitted', …)
  END IF;
 END IF;
```

  `already_member` keeps consuming nothing (D7.1 says so explicitly). Change `qualify_tenant_schema.py:457-462` to expect, for each of the three cases, `audit_events +1`, pending state `denied_for_reconciliation`, and a retry answering `expired` (the `state<>'live'` branch at `0003:346-347`); change the `admission.denied_for_reconciliation` count at `:561` from 1 to 4. Regenerate `catalog-v3.json` (function bodies are in the catalog) and re-pin `manifest.json`. If the team prefers the current behavior, amend ADR-006:43 and D7.1 to say code-side refusals leave the row live and that the reconciliation record is `provider_invitations` plus the code's status; one of the two texts must change.

**M2. One identity admitted concurrently through two different codes raises SQLSTATE 23505 instead of the typed `already_member`.**

- D7.1 (`ADR-007…md:13`): every function "returns a typed `outcome` and never raises for a business refusal"; `already_member` is the D7.1 outcome for exactly this business case. The README records it as an observation (`README.md:80`) and defers to D7.9's internal 23505 classification, but D7.9 classifies SQLSTATEs for commands; nothing maps a 23505 from `admit_redemption` to `Admission::AlreadyMember`, and a 23505 from this call could equally come from any other unique key, so storage cannot tell them apart.
- Probe P1: codes A and B, one identity with a live pending admission on each; `admit_redemption` through A holds its transaction, the call through B waits on the `principals` unique key (observed `wait_event_type='Lock'`) and fails `SQLSTATE 23505` when A commits; a sequential retry of B answers `already_member`.
- Fix (`0003_admission.sql:368-369`), verified as probe P1b by replacing the body on the disposable database:

```sql
  INSERT INTO genui.principals(organization_id,principal_id,issuer,provider_tenant_id,provider_subject)
   VALUES (p_org,v_principal,p_issuer,p_provider_tenant,p_provider_subject)
   ON CONFLICT (organization_id,issuer,provider_tenant_id,provider_subject) DO NOTHING;
  IF NOT FOUND THEN RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'already_member'::text; RETURN; END IF;
```

  P1b result: first `admitted`, second waited on the lock and then answered `already_member`; the loser's pending row stayed `live` and its code's `redeemed_count` stayed 0, identical to the sequential `already_member` case. The insert is the first write of the branch, so returning there writes nothing, needs no savepoint or EXCEPTION block (ADR-004's no-savepoint rule holds), and the conflict-target columns are already in `genui_admit`'s SELECT grant. Add a race case (4) to `qualify_tenant_schema.py` before `:520` (same identity, two codes, observed lock wait: `admitted` and `already_member`, one principal), and delete the 23505 sentence from `README.md:80`.

### Minor

**m1. Two concurrent sign-outs of one member that each take the 0002 member fence first deadlock (route to Task 5/APP-009).** Probe P2: both transactions run `lock_member_authority` (member `FOR SHARE`) and then `advance_member_epoch` (member `FOR UPDATE`): one returns `2`, the other `SQLSTATE 40P01`. With ADR-004's `lock_timeout` 1000 ms and the default `deadlock_timeout` 1 s the loser sees 40P01 or 55P03 nondeterministically. 0003 is not wrong (it locks org → member → head exactly as D7.1 says), but D7.9's "the member fence always precedes any session-row write, and sign-out advances the epoch … before deleting session rows" reads naturally as fence → advance → delete. Fix: state in D7.9 (and implement in Task 5) that a sign-out calls `advance_member_epoch` **instead of** `lock_member_authority`; the definer performs the same context and epoch check (`0003:420-428`) under `FOR UPDATE`, so it is the fence for that transaction.

**m2. `genui_admit` holds SELECT on `principals.principal_id`, which no function reads.** D7.1 grants "SELECT on the columns those functions read"; the bodies read `principals` only in `EXISTS (SELECT 1 … WHERE organization_id, issuer, provider_tenant_id, provider_subject)` (`0003:288-289,353-354`). Probe P8: after `REVOKE SELECT(principal_id) ON genui.principals FROM genui_admit`, a full reserve → pending → admit succeeds (`admitted`). The M2 fix does not need it either. Fix: drop `principal_id` from the SELECT list at `0003:187` and from `qualify_tenant_schema.py:72`, regenerate `catalog-v3.json`, re-pin.

**m3. A malformed or unbound verified identity raises instead of following choice 1's no-row convention.** Probe P6: `record_pending_admission` with a 300-character or empty subject raises 23514 (`code_reservations` CHECK, `0003:77`); with an issuer/tenant pair that has no `provider_tenant_bindings` row it raises 23503 (`0003:82`). Probe P7: with the org's binding set `active=false`, a full admission still answers `admitted`. These are storage-guaranteed inputs (ADR-006:33 maps `(issuer, tenant_id)` through the closed registry), so not a business refusal, but the convention should be uniform. Fix: in `record_pending_admission` and `admit_redemption` add to the entry guard `OR length(p_issuer) NOT BETWEEN 1 AND 512 OR length(p_provider_tenant) NOT BETWEEN 1 AND 256 OR length(p_provider_subject) NOT BETWEEN 1 AND 256` (the 0002 column bounds) and `RETURN`; record in the README that binding existence and `active` are the storage module's registry check (`genui_admit` has no SELECT on `provider_tenant_bindings`, and granting it would be a D7.1 amendment).

**m4. D7.8 does not yet state the conventions the chain now depends on; the hash input relies on a fixed-length `previous_hash` that no constraint enforces.** Choices 5 and 6 (`README.md:67-68`) are sound and the campaign recomputes every hash independently, including the D7.8 genesis formula (`qualify_tenant_schema.py:96-118`). But APP-077 must create genesis heads identically, and D7.8 says neither that hashes are written `sha256:<64 hex>` nor that a genesis head holds `chain_sequence = 1` (so the first event is 2). D7.8's input `previous_hash ‖ canonical bytes` has no separator, so it is unambiguous only while `previous_hash` has one fixed length, yet `audit_events.previous_hash`, `audit_events.hash` and `audit_heads.head_hash` are plain `text` (`0003:103,112`). Fix: amend D7.8 with both conventions (one sentence each), and type the three columns `genui.digest` (as the README's choice 12 already tightened `writer_role`), then regenerate and re-pin.

**m5. With `audit_events` insert-only for its owner, no organization can be deleted, and the redemption → event link is verified for one row only.** Probe P4: the owner deleting a freshly inserted organization that nothing references fails `42501 permission denied for table audit_events` (the NO ACTION referential check runs `FOR KEY SHARE` on `audit_events` as its owner, which `0003:172` stripped of UPDATE; the same mechanism the README gives for dropping `code_redemptions.audit_event_id`'s FK, so choice 9's reasoning is confirmed). Organizations becoming permanent is consistent with D7.8 but is not written anywhere; record it in D7.8 or D7.19 so an operator teardown is not designed around DELETE. For the dropped FK, the campaign checks the join for one principal (`qualify_tenant_schema.py:409-413`); add to `verify_chain` (or a sibling) the set check that every `code_redemptions.audit_event_id` resolves to an `admission.redeemed` event of the same organization, code, actor and `outcome_digest = payload_digest`, so the chain verification covers what the FK would have.

**m6. README corrections.** (a) `README.md:81`: STRICT folding happens with **any** NULL constant in a scalar call, not only when all arguments are NULL; probe P3 as `genui_control`: `resolve_code(NULL)` → `NULL`, `advance_member_epoch(<uuid>, NULL, 'session.sign_out')` → `NULL`, while `SELECT * FROM reserve_code(<uuid>, NULL, 'x')` in FROM still checks EXECUTE (42501). Still harmless (no body runs). (b) "no CI run" is now stale: run `40947220411` passed every step. (c) The README lists 13 implementation choices, not 8.

**m7. Runner review m3 landed in the pin check only.** `check-schema-pins.py:93-112` asserts each `catalog-vk.json` (k ≥ 2) records both version CHECKs as `version <= k` and that consecutive catalogs differ; the runtime half (`catalog_resources` requiring pairwise-distinct catalogs, `runner-review.md` m3 fix) is absent (`runtime.py:379-382`). Acceptable: `proven_catalog_version` already refuses unless exactly one catalog matches (fail closed), and distinct per-k CHECK rows imply pairwise distinctness. Record the disposition rather than change code.

### Observations (agree with the implementer; for later tasks)

- Entitlement reads for Task 5's `precheck` are not granted by 0003 (D7.3 places them with APP-009/010/012); Task 5 needs that grant in its own migration before the precheck can run as those roles (`README.md:79`).
- One identity can hold several live pending admissions (`README.md:80`); after the first redemption the others answer `already_member` and keep a slot until their 24 hours lapse. Not reachable for unbounded holding (each needs a fresh presignup and a provider sign-in), but Task 5 should either refuse a second live pending admission for the same identity and code in `record_pending_admission` or accept the bound in writing.
- `record_pending_admission` does not compare the presignup's `context_digest` with anything (D7.1 gives it no context argument), so ADR-003:49's "tied to the intended admission/browser context" rests on the storage module keeping the reservation id inside the pre-login flow record.
- `replayed` returns the current epoch and no membership status (choice 11); the callback must still run the fence and entitlement check before minting, as ADR-006:33 already requires.
- A compromised `genui_workspace` connection can set any `genui.principal_id`/`genui.membership_epoch` and so advance any member's epoch (sign-out denial of service), and can admit any identity it names given a valid code digest. Both are inherent to ADR-004's GUC-scope trust model and match 0002's fences; no 0003 change is implied.

## Implementation choices (README:61-75), judged

| # | Choice | Judgment |
| --- | --- | --- |
| 1 | No-row refusals for scope/org/unknown-code/malformed-digest; `mismatch`, `reservation_expired` for unknown ids; 55000 for a missing head | Accept; extend to malformed identity (m3). |
| 2 | Suspended org refuses the four scoped functions; sign-out still works | Accept. |
| 3 | `reserve_code` exhaustion uses D7.17 `remaining`; presignups never count | Accept; matches D7.1 "judges … exhaustion on the scoped row" and D7.17. |
| 4 | Audit events per function, column choices | Accept, except the code-side denials that must also append (M1). |
| 5 | `sha256:<hex>` hashes; canonical bytes of every column but `hash`, second-truncated UTC `occurred_at` | Accept; independently recomputed by the campaign; write into D7.8 (m4). |
| 6 | Genesis head at `chain_sequence = 1`, first event 2 | Accept (forced by `safe_counter`); must be written into D7.8 before APP-077 (m4). |
| 7 | `trial_days = 30` CHECK; entitlement fixed 30 days; kind copied | Accept (D7.17). |
| 8 | Typed template columns with per-permission shape CHECK and FKs instead of `resource_id` | Accept; stronger than a single `resource_id` and still "explicit (resource, permission) pairs". |
| 9 | `pending_admission_id`/`entitlement_id` on redemptions; no FK on `audit_event_id` | Accept; the privilege mechanism is real (P4); add the set check (m5). |
| 10 | RESTRICTIVE operator-chain policies for `genui_owner` | Accept; required by D7.8's "every role that holds INSERT". P5: `genui_migration` without `SET ROLE` (inheriting `genui_owner`) is bound too (admission-chain insert refused, 42501). |
| 11 | `replayed` returns the current epoch | Accept (observation above). |
| 12 | NOT NULL on `writer_role` | Accept (redundant on `audit_heads`, whose PK already implies it). |
| 13 | Liveness = `state='live' AND expires_at > now()`; only `admit_redemption` moves an expired row | Accept for arithmetic; the recording gap is M1. |

## Claims I tried to refute and could not

1. **Definer privilege boundary.** All six functions are `SECURITY DEFINER STRICT VOLATILE` with `search_path=pg_catalog, pg_temp`, owned by `genui_admit`, PUBLIC revoked, EXECUTE only to `genui_workspace` (`0003:208-226,447-465`; campaign `qualify_tenant_schema.py:172-219`). Every relation in every body is schema-qualified (read line by line); every function and operator used (`gen_random_uuid`, `sha256`, `convert_to`, `encode`, `to_json`, `to_char`, `date_trunc`, `current_setting`, `nullif`, `~`, `||`) resolves in `pg_catalog`, which precedes `pg_temp`, and functions/operators are never looked up in `pg_temp`; `genui_workspace` has no TEMP on `genui` anyway (P10). No body contains dynamic SQL, accepts a permission name or a bypass flag. `genui_admit` is NOLOGIN NOINHERIT, member of nothing, reachable only from `genui_migration` with SET and no INHERIT, and holds no CREATE after `0003:467`.
2. **Scope and caller-supplied ids.** Each scoped function requires `genui.organization_id = p_org` (`0003:232,265,321,421`) and evaluates RLS under the caller's scope; the code id is taken from the scoped reservation, templates are filtered by `(p_org, code_id)` and are FK-bound to same-organization apps, saved work and sources; foreign and mismatched-scope calls return no row or `mismatch` (campaign `:338-339,420-421`). `advance_member_epoch` additionally requires principal and epoch GUCs equal to its arguments and the current epoch (`0003:420-428`), exactly as `lock_member_authority` (0002:247-249). The nonmember path (organization scope, no principal) reaches the three admission functions and cannot reach `advance_member_epoch`.
3. **A4.** No body writes `genui.organizations`; the campaign checks bodies and the tuples' `xmin/ctid/status` (`:213-217,589-592`).
4. **Lock order and deadlocks.** Admission functions take `organizations FOR SHARE` → code `FOR UPDATE` → reservation/pending `FOR UPDATE` → (new rows) → admission `audit_heads FOR UPDATE`; `advance_member_epoch` takes org → member `FOR UPDATE` → head. After taking the head no function waits on another row (later FK checks hit rows the same transaction already holds or inserted), so no cycle exists among the six or with 0002's `FOR SHARE` fences or the operator's org → member/code → operator-head order. The only waits are the intended ones (code row; principals unique key, M2; member row behind a fence, m1). The unlocked pre-read of the reservation's `code_id` is safe because `code_id` and `kind` are not updatable by `genui_admit`.
5. **Remaining-redemptions arithmetic and the last slot.** All writers of a code serialize on its row; each count runs as a new READ COMMITTED statement after the lock, so it sees the previous holder's committed pending row; the invariant `redeemed_count + live pending ≤ max_redemptions` holds through every path (exhausted at admit is reachable only by operator action, as the campaign notes). The campaign's observed-lock-wait races (`:465-520`) show one admission per last slot in both orders and replay on identical concurrent redemptions; my run reproduced them.
6. **Replay and mismatch.** Identity is compared before the replay branch, the stored `payload_digest` must match, a different identity, payload, unknown or foreign pending admission gives `mismatch`, and a replay writes nothing (`0003:333-345`; campaign `:414-422`).
7. **Audit chain.** Append protocol is D7.8's: head `FOR UPDATE`, `+1`, `previous_hash = head_hash`, `sha256("forma:audit-event:v1" ‖ 0x00 ‖ previous_hash ‖ canonical bytes)` with keys in ASCII order and explicit nulls (`0003:297-309,385-405,432-444`); the campaign recomputes every hash in Python from the D7.8 genesis formula and the tail equals the head for both organizations, with a tamper positive control (`:546-563`); P11 re-verified the admission chain after my probes (12 events). The four RESTRICTIVE policies bind both INSERT holders and both head updaters to their own chain and closed kinds; nobody holds UPDATE, DELETE or TRUNCATE on `audit_events` (`0003:161-172`; campaign `:221-241,564-587`; P5).
8. **Grant set vs D7.1/D7.3.** The catalog's (role, relation, privilege, column) set equals catalog-v2's plus the literal `genui_admit` set; no application role holds anything on a 0003 relation or on the two indexes; app roles cannot read the indexes or write any 0003 relation; only workspace executes the six. The one surplus column is m2.
9. **A3 normalization.** `genui_admit` was added to every fixed-role list in `catalog-query.json` (roles, three memberships lists, effective_boundaries, role_database_settings, default_acl, role_reachability: eight occurrences, equal to `genui_lock`'s) and to the campaign's creator-bookkeeping list and compatibility repair; the only other query change is `octet_length(body)<=524288`. The 0003 relations, columns, constraints, indexes, policies, ACLs and function bodies are all inside the projection.
10. **Runner review fixes.** M1: `probe()` and `migrate()` require the manifest's `database_version == DATABASE_SCHEMA_VERSION` before connecting (`runtime.py:310,482`), with a test that both refuse a synthetic v4 manifest without calling `psycopg.connect` (`test_runtime.py:277-290`); the workflow runs the pin check, the scripts tests and the runtime tests before the campaigns (`.github/workflows/tenant-foundation.yml:29-33`), in the venv the previous step builds from both hash-locked requirement files, and run `40947220411` executed it successfully (5 and 39 tests). m1: one `CATALOG_BOUND` in `runtime.py` used for the file bound and `max_output_bytes`, and a second agreement group covering `max_output_bytes`, the SQL `octet_length` literal, `CATALOG_BOUND` and `catalog.rs` `BOUND` (`check-schema-pins.py:38-44,80-90`), with a mismatch test. m2: the generator names `body` or `file` and both lengths are in the receipt (`generate_schema_catalog.py:58-62,71-74`). m3: see m7. All actions in the workflow are pinned by full commit SHA.
11. **D7.12 mechanics.** Both version CHECKs move to `BETWEEN 1 AND 3` (`0003:19-22`); `GRANT genui_admit TO genui_migration WITH INHERIT FALSE, SET TRUE` and the temporary `CREATE ON SCHEMA genui_private` with its revocation are recorded (`0003:15,458,467`); `.dockerignore` admits the two new files; the campaign's v1→v3 upgrade proves each intermediate catalog, so the committed catalog-v1/v2/v3 equal the database's projection.
