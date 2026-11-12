# APP-014 independent review of the final state (PR 81 + PR 86)

- **Reviewer:** Claude subagent, independent of the implementers Grok (PR 81) and the Claude session that wrote PR 86.
- **Date:** 2026-11-12 UTC.
- **Commits reviewed:**
  - `5695869`, `main` at review time, which is the checkout I ran.
  - PR 86, merged as `32e6ac9` with head `2425727`. Its commits are `bab24f8` ("Assert APP-014 lock identity instead of only recording it") and `2425727` ("Close the APP-014 identity review findings").
  - PR 81, merged as `7741e5a` with head `634c86f`.
- **Files in scope:** `app/src/storage/race_qualification.rs`, `app/src/storage/command_qualification.rs` and `evidence/APP-014/*`.
- **What I wrote:** this file only. I made no commit, no push, no `gh` write and no AWS call.

Both campaign files are byte-identical between `2425727` and `5695869`. Later merges (APP-078, APP-084, APP-169) changed `pool.rs` and `admission_qualification.rs`. They left `commands.rs` and `sessions.rs` untouched. My runs therefore exercise the reviewed harness against the production command path as it stands on `main` today.

## Method

1. I read the governing texts:
   - `AGENTS.md`;
   - the Done criteria in `Tickets/README.md`;
   - the review tiers in `docs/planning/QUALITY-GATES.md` (APP-014 is Tier B);
   - the ticket `Tickets/4-done/APP-014.md`;
   - `evidence/APP-014/README.md` and the earlier Tier B review `independent-review.md` (findings m1, m2, n1).
2. I read the diffs `7741e5a..bab24f8..2425727` for both campaign files. I then read the current `observe_lock` and its helpers, and every one of the 20 call sites, together with the holder SQL each site uses.
3. I checked that no assertion was weakened since the Tier B baseline `14dc68d`. The only removed assertions were replaced by APP-113's reviewer-decides changes, which are equally strict.
4. I ran both campaigns twice on this checkout. I compared my reports field by field with the committed `*-identity-2026-11-05.json` reports, ignoring pids, timings and random ids.
5. I probed the `observe_lock` SQL verbatim on a disposable PostgreSQL 16.15 container (the same image digest `5db6dac2ef5d` the campaigns use). I tested the busy, in_flight, fallback and two-waiter shapes, then removed the container.
6. I read CI with `gh run list` and `gh run view --log`, which are read-only.

## Acceptance boxes

| # | Box | Evidence (file:line) | Judgment |
|---|---|---|---|
| 1 | Lost response, then a retry with the same key and payload but a now-stale expected version. After current authorization it returns the original outcome, with one revision and one mutation audit. | Group `lost_response_retry…` (`race_qualification.rs:592-732`): a byte-identical 200 and `Replayed`, `payload_mismatch` with the recovery view, a revoke/grant cycle, and a pool/store restart. Tally `[2,2,2]` and claims are unchanged. Report checks 1–4 carry tally, status and version values. | **Met.** Unchanged from the Tier B judgment. My two runs reproduce the committed report exactly. |
| 2 | Same-key and different-key races commit one intended write. Changed-payload reuse and stale distinct edits conflict, with authorized recovery. Other principals cannot retrieve the key. | Groups `same_key_and_different_key…` (`:736-1017`) and `other_principal_cannot_retrieve_key` (`:1020-1198`). The waits are observed at `:753-754`, `:781-782`, `:824-825` and `:893-894`, plus the route waits. The no-wait case asserts `lock_waiters == 0` (`:1047`, reported at `:1067`). | **Met.** |
| 3 | A paused command races revocation or adoption in both orders. An authoritative change that lands first prevents a stale commit. An earlier authorized commit stays recorded. Current visibility governs delivery. | Groups `adoption_vs_commit_both_orders` (`:1201-1371`) and `revocation_vs_commit_both_orders` (`:1375-1674`). There are 15 observed waits in all, including both held-past-`lock_timeout` loops (`:1265-1280`, `:1464-1482`). The closing chain check is at `:1678-1735`. | **Met.** |
| 4 | m1, m2 and n1 are closed (details in the next three rows). | — | **Met overall.** The relation for the three busy waits is an inference: see N1. |
| 4·m1 | The observation identifies the waiter, the blocker and the relation, or for a same-key claim wait, `transactionid` with one `idle in transaction` blocker. Zero waiters are asserted before the second actor spawns. | `observe_lock` (`race_qualification.rs:171-242`) does three things. It records the waiter pid and role, `wait_event`, `locktype`, the waiter's granted tuple relations, the blockers, the holders of the same lock object, and the blockers' written tables (`:183-206`). It requires exactly one ungranted lock in `genui` (`:215`). It requires every blocker to hold the waited object (`:228-234`). `quiet()` (`:162-169`) runs before the contending actor at all 20 sites: command `:776`, `:826`, `:893`, `:1582`; race `:751`, `:779`, `:822`, `:891`, `:1242`, `:1271`, `:1298`, `:1356`, `:1441`, `:1473`, `:1491`, `:1530`, `:1606`. In the four held-writer orders the contending actor is the resumed command, so `quiet()` runs before `release`. | **Met.** See the identity breakdown below. |
| 4·m2 | The reports hold observed values beside `name` and `passed`. | Every one of the 21 command checks and the 26 race checks (24 group checks plus 2 closing) has a non-empty `observed`, in both `command-campaign-identity-2026-11-05.json` and `race-campaign-identity-2026-11-05.json`. Each lock observation is embedded. | **Met.** Note N6: some values are literals written after an equal assertion. |
| 4·n1 | The README links the 21-check command report. | The README links `command-campaign-observed.json` and `command-campaign-identity-2026-11-05.json` (21 checks each, with observed values) and the historical `evidence/APP-013/review-fixes/command-campaign.json` (21 checks). It says `command-campaign.json` is the 20-check run. | **Met.** |

### Which waits are tied to a specific actor (is the README honest?)

There are 20 observed waits: five in the command campaign and fifteen in the race campaign. They fall into three groups.

**Specific holder asserted with `blocked_by` (7 waits).** At these waits `blockers == [holder]` and the holder holds the waited object.

| Wait | Holder identity | Waiter role | Relation |
|---|---|---|---|
| Command same-key (`command_qualification.rs:790-793`) | The single `idle in transaction` backend while the first actor is paused (`:778-783`); see N5 | `genui_data` | — |
| Command busy and in_flight (`:892`, `:898-906`) | `pg_backend_pid()` of the holder connection | `genui_data` | `genui.memberships`, via `on_table` |
| Race adoption held-past-timeout, both iterations (`race_qualification.rs:1270-1276`) | `pg_backend_pid()` of the writer | Not asserted | `genui.saved_work`, via `on_table` |
| Race member held-past-timeout, both iterations (`:1472-1478`) | `pg_backend_pid()` of the writer | Not asserted | `genui.memberships`, via `on_table` |

**Relation asserted, actor not asserted (10 waits).** These use `on_relation`, which requires the waiter's granted tuple lock on the named table plus a non-empty blocker set:
- command `:837` (sequence row);
- race `:825` and `:894` (sequence row);
- race `:1245`, `:1301` and `:1359` (saved work);
- race `:1444`, `:1494`, `:1542` and `:1618` (memberships).

**Same-key claim waits (3 waits).** These use `on_claim`, which asserts `transactionid` and one `idle in transaction` blocker: race `:754` and `:782`, and the command route `:1604`.

At all 13 waits without `blocked_by`, the built-in single-waiter and holds-the-object checks still hold. The pids are recorded but not compared with a known actor.

The README says "`blocked_by` … at seven waits", "`waiter_role` asserts `genui_data` at the three command waits" and "At the other 13 observed waits … the specific actor is not asserted. For a `transactionid` wait the object holder is by construction the transaction holding the row." All three statements are **accurate and complete**. The ticket log says the same. `2425727` also corrected the over-claim in `bab24f8`'s commit message ("each tied to its holder").

The recorded values are consistent with the claimed actors:
- the session-resolution waiter is `genui_workspace`;
- the real `revoke-membership` waiters are `genui_migration`;
- the `spawn_writer` adoption waiters are `postgres`;
- the busy waits have `relations: []` and `holder_writes: [the touched table]`.

## Review of `2425727` specifically

**Settle condition** (`race_qualification.rs:211-214`). PostgreSQL's `ProcSleep` puts the backend in the lock's wait queue, so `pg_locks` already shows the lock as ungranted, before `WaitLatch` sets `wait_event_type = 'Lock'`. The event is also cleared briefly on each latch wake-up, such as the deadlock check. Requiring every ungranted row to be `Lock` therefore only skips transient polls.

- It cannot pass a wrong wait. Identity is still checked afterwards: one row, blockers contained in holders, and the per-site `on_claim`, `on_relation`, `blocked_by` and `on_table` checks.
- A persistent non-`Lock` ungranted row would make the call time out after 10 s. That fails closed.
- It adds no material flake risk. The bounded waits last about 1,000 ms (`lock_timeout`) against a poll interval of about 10 ms. The observer connection is local and plain-TCP (`qualification.rs:438-454`).
- It fixes a real defect in `bab24f8`: `row.get::<String>(2)` on a NULL `wait_event` would have panicked.

**Holder matching on the same lock object** (`:187-197`, `:228-234`). The match compares the full lock tag with `IS NOT DISTINCT FROM`, so NULL fields such as `database` on a `transactionid` lock compare correctly. It excludes the waiter itself.

- For `transactionid` waits, which are all 20 in this campaign, only the owning transaction holds that tag. The containment check therefore cannot add discrimination there; the README says so.
- It does discriminate for heavyweight `tuple`-lock waits. My two-waiter probe showed the second waiter's tuple-lock holder correctly as the first waiter.
- The check is sound and fails closed. `pg_blocking_pids` "soft" blockers (queued waiters ahead) cannot occur, because a single waiter is asserted first. My probe also showed a second waiter produces two rows, so `:215` fails.

**`holder_writes` / `on_table`** (`:201-204`, `:280-292`). Probed on PostgreSQL 16.15:
- A busy-shape wait (holder `NO KEY UPDATE`; waiter `KEY SHARE`, then `FOR SHARE`) holds no tuple lock: `relations {}`, `holder_writes {m}`, blockers and holders both equal to the holder. This confirms the README's explanation.
- An in_flight-shape wait names the table through the waiter's tuple lock, and `holder_writes {}` for a `FOR UPDATE`-only holder.

The fallback names the tables the blocker *wrote*, not the table of the waited row. Findings N1 and N2 below follow from that.

## Findings

**Blocking:** none.

**Non-blocking:**

- **N1. The busy-wait relation is an inference, valid here because of the holder statements.**
  - My probe's fallback case: a holder updates table `x` and locks a row of `m` `FOR NO KEY UPDATE`; the waiter waits on that `m` row. The observation reports `holder_writes {x}`, so `on_table(_, "x")` would pass and `on_table(_, "m")` would fail.
  - In this campaign the inference is valid. Each busy holder writes exactly the table it row-locks, and nothing else: `command_qualification.rs:913`, `race_qualification.rs:451` and `:459`. `blocked_by` pins the blocker to that holder.
  - The README ("its table is the holder's write") and the ticket log disclose the mechanism. "Satisfies item 4's relation requirement" is fair only with that qualifier.
  - **Suggestion:** say in the helper's doc comment that it relies on the holder writing only that table.
- **N2. `2425727` weakened the in_flight relation assertion.**
  - `bab24f8` used `on_relation(…, "memberships")` at the command in_flight wait. `2425727` replaced it with `on_table`, which accepts `names || written`.
  - At the command in_flight site nothing changes in practice: a `FOR UPDATE`-only holder writes nothing, so `holder_writes` is `[]`.
  - At the race in_flight iterations, `adopt` and `revoke_member` also `UPDATE` the row, so a missing tuple relation would no longer fail (`race_qualification.rs:1276`, `:1478`).
  - The reports show the tuple relation is present (`relations: ["genui.saved_work"]` and `["genui.memberships"]`).
  - **Suggestion:** assert `on_relation` for the in_flight code and use the fallback only for busy.
- **N3. `on_claim` does not require `relations` to be empty** (`race_qualification.rs:263-272`). So at the three same-key sites it would also accept a row-lock wait on the same blocker. The typed result (`Replayed`, `PayloadMismatch`, `InFlight`) and the D7.9 claim-first order discriminate the two, and the recorded `relations: []` shows the actual wait was the unique-index wait.
- **N4. `quiet()` uses a different predicate from `observe_lock`.** It counts `pg_stat_activity` rows with `wait_event_type = 'Lock'` (`:122-128`), not ungranted `pg_locks` rows. A waiter in the pre-`WaitLatch` window that `2425727` handles would be invisible to it. This is practically unreachable, because the first actor is always paused or idle when `quiet()` runs.
- **N5. The command same-key holder pid is not the connection's own `pg_backend_pid()`.** It is "the only `idle in transaction` backend in `genui`" (`command_qualification.rs:778-783`), taken after the pause and before the second actor starts, with uniqueness enforced by `query_one`. That is a genuine identification of the paused actor. It overlaps with `on_claim`'s blocker-state check, and it does not use the connection's self-reported pid.
- **N6. Some reported values are literals.** Six race checks and eight command checks write literals into `observed` (for example `race_qualification.rs:657`, `:731`, `:1067`; `command_qualification.rs:528`). Each follows an `assert_eq!` on the same value, so the values are equivalent whenever `passed: true`. They are not captured at runtime.
- **N7. Records to fill when Done is recorded** (not review blockers):
  - The ticket's Branch / PR field and Log do not name **PR 86**, its merge `32e6ac9`, or its exact-source CI. That CI is Tenant foundation **42011768857** (pull_request) and **42011762114** (push) on `2425727`, both success. In run 42011768857 I read the log: `native_command_campaign` ok (31.49 s) and `native_race_campaign` ok (37.85 s).
  - The README's dated 2026-10-29 "Recommendation (not done here)" says neither campaign is a CI step. That is now superseded: `.github/workflows/tenant-foundation.yml:68-71` runs both. A one-line dated note would keep the README current.

I verified the ticket log's PR 81 identity claim: `app/src` is tree `9eb18d7` at `c56d8aa`, `634c86f` and `7741e5a`.

## Commands run (this checkout, `5695869`, macOS, Docker)

| Command | Result |
|---|---|
| `cd …/forma-board && export PWD=$(pwd) && just campaign command` (run 1, 03:33:26–03:34:12Z) | `test result: ok. 1 passed`, 37.41 s (45.12 s wall). The report has 21 checks, `passed: true` and five observations. |
| `just campaign race` (run 1, 03:34:21–03:35:04Z) | `1 passed`, 39.54 s (43.15 s wall). The report has `passed: true`, 26 checks, 17 commits, `elapsed_ms` 39531, and `session_wait_observation` 404 `not_found`. |
| `just campaign command`, then `just campaign race` (run 2, 03:35:36–03:37:04Z) | `1 passed` in 36.12 s; `1 passed` in 50.15 s (`elapsed_ms` 50150). |
| Python field diff, ignoring pids, timings and UUIDs: my reports against `*-identity-2026-11-05.json`, and run 1 against run 2 | The race reports are **identical** in all 26 check texts and every observed value, including role, `wait_event`, `locktype`, `relations` and `holder_writes` at all 15 waits. The command reports differ only in `first_outcome.committed_at`, `retry_expires_at` and `payload_digest`, which vary per run. |
| Disposable `postgres` container (image `5db6dac2ef5d`, PostgreSQL 16.15, label `app014-review-probe=1`) running the `observe_lock` SQL verbatim on four shapes | The busy, in_flight, fallback and two-waiter results quoted above. Removed with `docker rm -f`. Afterwards `docker ps -a` lists only the two dev `pitch-db` containers, which I did not touch. |
| `gh run list --commit 2425727…`, `gh run view 42011768857 --log`, `gh pr view 86`, `gh run list --commit 5695869… --workflow "Tenant foundation qualification"` | All checks on `2425727` are success. PR 86 was MERGED on 2026-11-05 at head `2425727`. On `main` `5695869`, Tenant foundation 43144269940 is success. On the merge commit `32e6ac9`, the push runs were cancelled (superseded); the head's exact-source runs are the binding ones. |

The campaigns use their own labeled fixtures and removed them. The running dev pitch instance from this worktree was not stopped or reset. The worktree had unrelated uncommitted APP-169 edits from another session, which I left alone.

## Not verified / limits

- I did not run `just check`, clippy, fmt or the other campaigns. CI on `2425727` and on `main` covers them, and I did not re-derive that coverage.
- I did not run a source mutation against the new assertions (for example a wrong holder pid or a wrong table). The probe exercises the SQL semantics, not the Rust harness under mutation.
- I did not observe the settle window empirically. That judgment rests on the PostgreSQL `ProcSleep` ordering, and it can only delay a poll.
- Local Docker PostgreSQL with password over TLS for the roles. No RDS or hosted run: the RDS lock-observation repeat is APP-080's.
- Synthetic identities only. Held writers are fixture-only SQL, as disclosed in the README.
- I reviewed boxes 1–3 for regressions since the Tier B review. I did not re-derive them from scratch.

**Verdict: no blocking finding — Done may be recorded** (once N7's PR 86 and CI identifiers are written into the ticket).
