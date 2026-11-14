# APP-192: break-and-restore proofs for the share routes and the dialog (W2b, 2026-11-14)

Branch `trial/w2b-share-dialog`: W4's `trial/w4-private-entries` (`64234d10`) with W2's `trial/w2-admin-cap` merged (`fd40a8c9`, then its fix round `e261ced1`). The guards below are the ones this branch adds on top of W4's storage API; W4's own proofs (the definers' reviewer rule, `owner_only`, the D8 gate) are in [evidence/APP-191/break-and-restore-2026-11-14.md](../APP-191/break-and-restore-2026-11-14.md).

Each guard was broken with a one-occurrence text replacement (a scratch script that prints the file's SHA-256 before and after), the test that owns it was run and failed, and the file was restored with `git checkout -- <file>`. After each restore `git status --short` was empty and the file's SHA-256 matched the committed one again (`app/src/sharing/mod.rs` `36897e64a9cffcda…`, `app/src/storage/sharing.rs` `7f8427ffd0de3194…`, `app/admin/src/sharing/api.ts` `f97657395d4958b5…`). B1 to B9 ran on commit `c94f6b67`; B10 ran on the uncommitted CSS fix before it was committed. No migration or catalog is involved.

| # | Guard | Where | Break | What failed | Restored |
| --- | --- | --- | --- | --- | --- |
| B1 | One encoding: the share body is exactly `grantee_principal_id` and `role` | `sharing/mod.rs` `parse_share` | the member-count check removed and `record_write` admitted as a key | `sharing::tests::share_body_is_closed`: `{"grantee_principal_id":…,"role":"viewer","record_write":false}` parsed (`Some((…, Viewer))` instead of `None`) | test passes |
| B2 | The organization body is exactly `{"role": …}` | `sharing/mod.rs` `parse_organization_share` | the `object.len() != 1` check removed | `sharing::tests::organization_body_names_a_role_or_none`: `{"role":"viewer","record_write":false}` parsed (`Some(Some(Viewer))`) | test passes |
| B3 | Fetch metadata on both POSTs (the common front `share_request`) | `sharing/mod.rs` | the `csrf::same_origin` check removed | `sharing::tests::routes_without_a_session_are_the_uniform_404`: a cross-site POST answered 404 instead of 403 | all 7 `sharing::tests` pass |
| B4 | A reviewer grant from a sharer who is not a reviewer is the plain 422 `reviewer_not_allowed` | `sharing/mod.rs` `role_refusal` | the route lets the reviewer role through for any sharer | `just campaign share-routes`: Hal's reviewer share answered the uniform 404 `{"code":"not_found","status":404}` instead of `{"code":"reviewer_not_allowed","status":422}` (storage still refused it; only the plain code was lost) | campaign passes |
| B5 | The dialog's read requires share authority (the unrevoked creator or a `share_work` holder, with `run`) | `storage/sharing.rs` `work_sharing` | `require(!authority.is_empty())` removed | `just campaign share-routes`: Rev, a reviewer with no share authority, read the dialog (the share list with Ana Diaz and her role) instead of the uniform 404 | campaign passes |
| B6 | Reviewer is offered only to a sharer who is a reviewer | `storage/sharing.rs` `work_sharing` | `Reviewer` pushed whenever `record_write` is held | `just campaign share-routes`: Hal's roles read `["viewer","contributor","reviewer"]` instead of `["viewer","contributor"]` | campaign passes |
| B7 | A work with source dependencies or folder files is shared person by person: 422 `organization_share_unavailable` | `sharing/mod.rs` `share_organization` | the availability test skipped for every role but reviewer | `just campaign sharing`: the S05 work's organization-wide viewer share answered `(404, "not_found")` instead of `(422, "organization_share_unavailable")` | campaign passes |
| B8 | The dialog offers only the roles the server names | `admin/src/sharing/api.ts` `offeredRoles` | every role offered | `test/team-sharing.test.ts` 6, "reviewer is offered only when the server says this sharer may grant it", fails | 9 of 9 pass |
| B9 | The dialog's person share sends the grantee and the role only | `admin/src/sharing/api.ts` `memberShareRequest` | `record_write` added beside the role | `test/team-sharing.test.ts` 2 and 8 fail (the body keys and the body sent) | 9 of 9 pass |
| B10 | A hidden control in the section stays hidden (the smoke's first run found "Stop sharing with everyone" shown while nothing was shared) | `admin/src/styles/sharing.css` | `.share-team [hidden] { display: none; }` removed | `test/team-sharing.test.ts` 10 fails | 10 of 10 pass; restored from a copy before commit (SHA-256 `dce231434d7f…` again) |

Unchanged and asserted as shipped, not broken separately here:

- The uniform 404 for a member without share authority on both POSTs. Storage decides it (`share_work`'s `Permit` and the definers, ADR-011's unchanged clause, broken by W4). The `share-routes` campaign asserts byte-identical 404s for Nia (no access), Rev (a reviewer with no share authority, including Rev's own reviewer request), a nonexistent work, and a grantee who is nobody, revoked or the sharer. Nothing is written.
- The audit rows. The definers write them. The `share-routes` campaign counts one `permission.granted` per grant, and one `permission.revoked` plus one `permission.granted` per organization-wide role change. The revoked row names its revoker and its event.

## Commands

- Unit: `cargo test --lib -p forma-app-platform -- sharing::tests::<name>`.
- Campaigns: `just campaign share-routes` (B4–B6), `just campaign sharing` (B7).
- Canvas: `npx tsx --test test/team-sharing.test.ts` from `app/admin` (B8 to B10).

## Limits

- The personal-share revoke is not built server-side here: W4 is adding `Workspace::revoke_work_share`. The dialog's control goes through one client function, `revokeMemberShare`. It is shown only while the server answers `can_revoke_members: true`, and the server says `false` until that route exists. Neither the gate nor the route is proven here.
- B3 breaks the shared front of both POSTs, so the first failing assertion is the person route's. The organization route's 403 cases are in the same test and in the campaign.
- These are synthetic local fixtures, with sessions minted for tests. The hosted sign-in is not exercised.

## Round 2: the review's fix round (2026-11-14)

This round answers the Tier A review in [review-w2b-2026-11-14.md](review-w2b-2026-11-14.md). It ran on `92215d40` with the same method as round 1.

- Each break was a one-occurrence replacement by a scratch script that prints the SHA-256 before and after.
- The owning test was run once with the break in place.
- The file was then restored with `git checkout -- <file>`. Every restore left `git status --short` empty and the file's SHA-256 equal to the committed one:

| File | Committed SHA-256 |
| --- | --- |
| `app/src/storage/sharing.rs` | `6f36057448e18613…` |
| `app/src/sharing/mod.rs` | `b2116d1959681281…` |
| `app/admin/src/sharing/dialog.ts` | `0aaff39e7ee407d2…` |
| `app/admin/src/canvas/live.ts` | `218914447cf8f770…` |
| `app/src/mcp/tools.rs` | `862ae8d58b44bc5b…` |

**Tests first.** Before the storage change, the extended `share-routes` was run without its new `private_lists` assertion. All its new gap checks passed on the old code:

- `contributor_not_allowed`;
- a member of another organization;
- 413 and 429.

The new listing check failed there. After an operator `revoke-permission` of Ana's `record_write` and a demotion of Rev, the dialog still said Ana was a contributor and Rev a reviewer. With the assertion in, the run failed at the missing `private_lists`. The unit and TypeScript tests were written first and failed to compile or run against the old code, as expected.

| # | Guard | Break | What failed |
| --- | --- | --- | --- |
| B11 | A share row whose `record_read` is gone lists nothing | the listing's `record_read` condition replaced by `TRUE` | `share-routes`: Ana was still listed after her `record_read` was revoked (`[("Ana Diaz","viewer"),…]`, at `share_routes_campaign.rs:844`) |
| B12 | The shown role is the live permissions', not the share rows' | the reviewer test reads `work_shares.role='reviewer'` instead of `record_review` | `share-routes`: after Rev's demotion he still read `reviewer` (right: `contributor`) |
| B13 | Contributor is offered only to a sharer with `record_write` (the review's G1, MINOR-1) | `Contributor` pushed for every sharer | `share-routes`: Vic, a viewer-only `share_work` holder, read `["viewer","contributor"]` instead of `["viewer"]` |
| B14 | An organization-wide change is announced only when it widens access | `rank(after) > rank(before)` changed to `!=` | `sharing::tests::an_organization_share_is_announced_only_when_it_widens`: `Some(Reviewer) -> Some(Viewer)` announced |
| B15 | `private_lists` is computed from the work's `own` lists | `> 0` changed to `>= 0` | `sharing` campaign: S05 read `private_lists: true` (right: `false`) |
| B16 | An outage keeps the section, with a message and a retry (MINOR-3) | the old line `section.hidden = sharing === null` | `team-sharing-dialog.test.ts` 4 |
| B17 | "Can review all entries" for everyone is confirmed first (MINOR-7) | the confirmation bypassed | `team-sharing-dialog.test.ts` 5 |
| B18 | The stop control shows only while a share with everyone is live (the review's G3) | `everyoneStop.hidden = false` | `team-sharing-dialog.test.ts` 2 |
| B19 | The section offers only the roles the server names (the review's G2) | `render` offers every role | `team-sharing-dialog.test.ts` 1 |
| B20 | A share notice only for an app this canvas does not have | the `known` test ignored | `team-sharing.test.ts` 11 |
| B21 | MCP `forma_share` with `record_write: true` shares as a contributor | `from_record_write(record_write && false)` | `mcp` campaign: Priya held `["record_read"]` (right: also `record_write`) |
| B22 | The member write window applies to both POSTs | the `admit_member_write` check removed | `share-routes`: the exhausted member's POST answered `(404, "not_found")` instead of `(429, "too_many_requests")` |

Not broken separately:

- The another-organization 404 is structural:
  - the organization comes from the session;
  - RLS `tenant_scope`;
  - `organization_share`'s scoped organization (W4's clause).

  `share-routes` asserts the outcome: byte-identical 404s for Oz on every route and as a grantee, and `can_open` false for Oz while the share with everyone is live.
- The person-share reviewer confirmation is tested (`team-sharing-dialog.test.ts` 6) but was not broken separately. It is the same component as B17.

The one smoke of the first round, in `smoke-2026-11-14/`, records the earlier copy ("Can add and edit their entries"). It was not rerun for this round.

## Round 3: W4's revoke API merged (2026-11-14)

This round followed the merge of W4's fix round, `126cd74c`, which brought:

- `Workspace::revoke_work_share`;
- `Shared.removed` for a lower re-share;
- the per-work lock (MAJOR-1);
- the RESTRICTIVE policy `work_shares_everyone`.

**Tests first, on the merged tree.** `share-routes` failed on the gate (`can_revoke_members` false for the owner). With only that assertion relaxed, it failed one check earlier: right after the owner's reviewer share, the dialog listed nobody (`members: []`). W4's policy now hides personal share records from the application roles, so a listing built on those records could never list anyone again. A demotion would also leave no live record behind (W4's `share_work` revokes the earlier records and writes none when it grants nothing). The listing now reads live permissions only.

The breaks ran on `e90fc96a` (B23–B29) and on the uncommitted owner-exclusion change (B30), with the same method as round 1. Each restore left the file's SHA-256 equal to the committed one, and `git status --short` empty (B23–B29).

| # | Guard | Break | What failed |
| --- | --- | --- | --- |
| B23 | "Remove access" is offered to the app's owner alone | `can_revoke_members` true for every sharer | `share-routes`: Hal, a `share_work` holder, read `can_revoke_members: true` |
| B24 | A refused removal is the uniform 404 | `Denied` answered as a 200 removal | `share-routes`: Hal's `{"grantee_principal_id":Vic,"role":"none"}` answered 200 instead of 404 |
| B25 | The list reads `record_read` rows only | `share_work` rows read too | `share-routes`: Hal and Vic were listed twice |
| B26 | The composer lineage forgets a removed person | `retain` removed | `composer::lineage::tests::a_removed_person_is_not_shared_the_adaptation`: the adaptation still went to Sarah |
| B27 | A person share is announced only when it grants something | `announces_person` returns `Shared::changed()` | `sharing::tests::a_person_share_is_announced_only_when_it_grants`: a demotion that only removed rows was announced |
| B28 | A removal is believed only from an answer naming the person and `none` | the check removed | `team-sharing.test.ts` 12 |
| B29 | The dialog shows "Remove access" only when the server allows it | the gate ignored | `team-sharing-dialog.test.ts` 8 |
| B30 | The person picker leaves out the app owner (W4 r1: no share names them) | the filter removed | `team-sharing-dialog.test.ts` 10; restored from a copy before commit (SHA-256 `b75559361102244a…`) |

**MAJOR-1 at the route** is a regression check in `share-routes`. It holds the owner's change uncommitted, puts Hal's stop in flight, commits, and then requires the final state to match the final answer. It also checks the other order. The lock itself is W4's definer; W4 broke it in their own record (`evidence/APP-191/break-and-restore-2026-11-14.md`), and it was not broken again here.

## Round 4: W4's explicit share modes (2026-11-14)

W4's round `1aa84fda` made the share mode a parameter: `Set` is exact and demotes, `Ensure` only adds. The dialog's person route passes `Set`. The new `POST .../share/add` passes `Ensure`; it is the canvas approver step (`actions.ts`), and the dev seed and the `actions` and `workflows` campaigns use it in that role. MCP, Slack, the composer and a lineage adaptation pass `Ensure`.

Both breaks ran on `8c239874`, with the same method as round 1. Each restore left `git status --short` empty and the SHA-256 equal to the committed one (`app/src/sharing/mod.rs` `694bf2d7c1037d4d…`, `app/admin/src/canvas/actions.ts` `c8e912ff850a5b92…`).

| # | Guard | Break | What failed |
| --- | --- | --- | --- |
| B31 | The automatic share only adds: a reviewer approver keeps `record_review` | `/share/add` passes `Set` | `share-routes`: after the owner's contributor add, Fay held `["record_read","record_write"]` (right: also `record_review`), so the approver share demoted a reviewer |
| B32 | The canvas approver step uses the additive route | `actions.ts` builds the exact person request | `team-sharing.test.ts` 13 |

The demotion check is re-verified for W4's r4: a demotion through the dialog's `Set` leaves exactly one live record, and the listing (live permissions) shows the lower role.
| B33 (superseded by B35) | A busy store (a lock cycle between two members, W4's coming r7) is the retryable 409 `busy` with `retry_after_seconds`, never 503 | `Busy` mapped like any outage | `sharing::tests::a_busy_store_is_the_retryable_409_and_nothing_else_changes`: `(503, "unavailable")` instead of `(409, "busy")`; restored from a copy before commit (`9a71f18948f23540…`) |
| B34 (superseded by B36) | The dialog says a busy share plainly and asks for a retry | the 409 copy removed | `team-sharing.test.ts` 7; restored from a copy before commit (`fedaa53b3c3a3130…`) |

## Round 5: W4's final round, and `Busy` as 429 (2026-11-14)

W4's final round, `903572c7`, plus its final-check record `a32aa597`, made the sharing definers answer `Busy` after a lock cycle (r7), and ADR-007 now names 429 with `Retry-After` for it. All three share routes, the person share, the removal and the organization-wide share, answer `Busy` through one mapping (`storage_refusal`): 429 `{"code":"busy","status":429}` with `Retry-After: 2`, distinct from the member write limit's `too_many_requests`. The body names nothing else. `Error::Busy`'s text is now generic ("busy; try again in a moment"); it was "record snapshot capacity reached", which is wrong after a lock cycle. B33 and B34 answered 409 and are superseded by B35 and B36.

**The route check** (`share-routes`, its 16th): a raw session holds Fay's membership row `FOR UPDATE`. `share_work` and `revoke_work_share` both take that row, so they wait past `lock_timeout` (1 s) and the definer answers `Busy`. A held per-work sharing lock does the same to the organization-wide share. Each route answers 429 `busy` with `Retry-After: 2`, nothing is written, and once the lock is released the share goes through.

| # | Guard | Break | What failed |
| --- | --- | --- | --- |
| B35 | A share definer's `Busy` answers 429 `busy` with `Retry-After` on the route | `Busy` mapped like any outage | `share-routes`: Fay's person share under the held row answered `(503, "unavailable")` (right: `(429, "busy")`); restored from a copy (SHA-256 `3e83e570480fdb3b…`) |
| B36 | The dialog says a busy share plainly ("Sharing is busy right now. Nothing was changed. Try again in a moment."), naming nobody | the copy removed | `team-sharing.test.ts` 7; restored from a copy |

The unit test `sharing::tests::a_busy_store_is_the_retryable_429_and_nothing_else_changes` checks the mapping alone, header included.

Adaptation in the merge: W4's `private-entries` campaign called `share_with_lineage` with its earlier signature (`record_write: true`). It now names `ShareRole::Contributor` and `ShareMode::Ensure`, which is what that call meant.
