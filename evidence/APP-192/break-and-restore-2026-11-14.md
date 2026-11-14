# APP-192: break-and-restore proofs for the share routes and the dialog (W2b, 2026-11-14)

Branch `trial/w2b-share-dialog`: W4's `trial/w4-private-entries` (`64234d10`) with W2's `trial/w2-admin-cap` merged (`fd40a8c9`, then its fix round `e261ced1`). The guards below are the ones this branch adds on top of W4's storage API; W4's own proofs (the definers' reviewer rule, `owner_only`, the D8 gate) are in [evidence/APP-191/break-and-restore-2026-11-14.md](../APP-191/break-and-restore-2026-11-14.md).

Each guard was broken with a one-occurrence text replacement (a scratch script that prints the file's SHA-256 before and after), the test that owns it was run and failed, and the file was restored with `git checkout -- <file>`. After each restore `git status --short` was empty and the file's SHA-256 matched the committed one again (`app/src/sharing/mod.rs` `36897e64a9cffcda…`, `app/src/storage/sharing.rs` `7f8427ffd0de3194…`, `app/admin/src/sharing/api.ts` `f97657395d4958b5…`). The breaks ran on commit `c94f6b67`; no migration or catalog is involved.

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

Unchanged and asserted as shipped, not broken separately here:

- The uniform 404 for a member without share authority on both POSTs. Storage decides it (`share_work`'s `Permit` and the definers, ADR-011's unchanged clause, broken by W4). The `share-routes` campaign asserts byte-identical 404s for Nia (no access), Rev (a reviewer with no share authority, including Rev's own reviewer request), a nonexistent work, and a grantee who is nobody, revoked or the sharer. Nothing is written.
- The audit rows. The definers write them. The `share-routes` campaign counts one `permission.granted` per grant, and one `permission.revoked` plus one `permission.granted` per organization-wide role change. The revoked row names its revoker and its event.

## Commands

- Unit: `cargo test --lib -p forma-app-platform -- sharing::tests::<name>`.
- Campaigns: `just campaign share-routes` (B4–B6), `just campaign sharing` (B7).
- Canvas: `npx tsx --test test/team-sharing.test.ts` from `app/admin` (B8, B9).

## Limits

- The personal-share revoke is not built server-side here: W4 is adding `Workspace::revoke_work_share`. The dialog's control goes through one client function, `revokeMemberShare`. It is shown only while the server answers `can_revoke_members: true`, and the server says `false` until that route exists. Neither the gate nor the route is proven here.
- B3 breaks the shared front of both POSTs, so the first failing assertion is the person route's. The organization route's 403 cases are in the same test and in the campaign.
- These are synthetic local fixtures, with sessions minted for tests. The hosted sign-in is not exercised.
