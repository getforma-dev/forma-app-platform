# W3 follow-up reviews, recorded by the coordinator (2026-11-14)

The independent Fable reviewer of W3 (the reviewer of [`review-w3-2026-11-14.md`](review-w3-2026-11-14.md)) carried out two more reviews on 2026-11-14 and reported them to the coordinator (Claude, Opus 5.5). This file is the coordinator's record of those reports.

The reviewer wrote the first review into `review-w3-2026-11-14.md` as local commit `ceb0d81d` on `trial/w3-ops`. The permission check refused the reviewer's push, so the coordinator did not push that commit on its behalf and reported it to Victor. PR #121 is built from the pushed `de7c2b4a` instead.

## 1. Follow-up review of `de7c2b4a` (record_review grantable and revocable): approve

What `de7c2b4a` adds:
- `record_review` joins the operator's `GRANTABLE` set. A grant is refused with `permission_unsupported` until the database is at schema 24; a revoke is ungated.
- `ops/promotion/admission.py` gains `grant-reviewer` and `revoke-reviewer`, with matching `overrides.json` entries.
- The runbook gains §3, step 8.

What the reviewer verified:
- **Gate order:** `begin()`, then the version read, then the version check. Only after that do the member lock, the INSERT and the audit event run. Nothing is locked or written before the gate.
- **Which row the revoke deletes:** exactly the `work_permissions` `record_review` row that W4's reviewer clause reads, with the epoch advanced by one and one `permission.revoked` event.
- **What else can make someone a reviewer:**
  - the app's owner;
  - an unrevoked creator of the work;
  - a live reviewer share with everyone in the organization.

  None of these is the operator's to end.
- **The overrides** have the exact shape, and their example IDs resolve only under the trial organization's scope.
- **No widening:** `share_work` and `export` stay ungrantable, and an access code still cannot carry `record_review`.
- **Breaking the gate:** the reviewer broke it four ways in memory, and each break failed its test.

`just check py-runtime py-promotion py-ops` passed at `de7c2b4a`: 216, 219 and 40 tests.

| Finding | Where it went |
| --- | --- |
| f1: a `share_work` demotion left `record_review` in place | Fixed in W4's fix rounds (f1, then r5's set and ensure modes). |
| f2: the operator's revoke does not end a reviewer share with everyone | Now stated in runbook §3, step 8. |
| f3 (pre-existing): code templates may carry `share_work` and `export`, contrary to the comment on `GRANTABLE` | Recorded; not changed here. |

## 2. Recheck of `5e3aaa6f..c7ecd5ad` (R1–R5) and the merge `193e4c35`: approve

### R1, the purge and the identity schema

Fixed as far as the schema allows.
- `foreign_keys()` covers every child schema.
- The purge refuses with `purge_foreign_children_unknown` unless the only outside children are the three identity tables.
- On those tables the purge:
  1. lifts FORCE inside the transaction;
  2. deletes the organization's `shared_auth_flows`;
  3. restores FORCE and proves it is back.

  After that it refuses with `guest_linked_shares_present` if any `linked_apps` or `share_events` row names the organization. That refusal rolls everything back.
- Every statement is scoped `WHERE organization_id` under the organization's RLS scope.

Minor: the flows delete also matches `provider_organization_id`. Runbook §10 now says so.

### R2–R5

Fixed, in the runbook and ADR-008:
- the FormaStack deletion request and its written acknowledgement;
- the timeouts;
- the `admission-reconcile` override;
- the five audit heads.

### The merge `193e4c35`

Approved.
- The only conflict was `ops/promotion/test_scan_renewal.py`, resolved to main's version.
- `scan_renewal.py` and `test_scan_renewal.py` are byte-identical to main's.
- Nothing was lost or doubled, and there are no conflict markers.

The reviewer ran:
- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 219 tests OK (4 skipped).
- `just check py-ops py-promotion`: ok.
- `py-runtime`, using the W3 worktree's virtual environment: 216 tests OK. These include the real-schema tests of the purge and of guest shares.

### Must change before merge, now done in this PR

The runbook now matches main's plan:
- Release B is migrations 0023 and 0024, schema 22 → 24. The restore drill expects schema 24.
- §1, release A, is marked done, with its record.
- §8 cites the November 14 disposition, release A's spent pin and the absent release file. The obsolete "if release A has not landed by Nov 19" paragraph is gone.
- §3's duplicate step numbers are fixed.

### Required follow-up, before Dec 14 (recorded on APP-188)

After migration 0024 is live:
- **At schema 24 or later,** the purge deletes the organization's `linked_apps` rows under the lifted FORCE and ignores `share_events`, whose key 0024 drops.
- **Below 24,** it refuses as now.

Runbook §10 says so. Revoking shares does not clear the refusal today.
