# Break-and-restore evidence: organization roles (APP-225, APP-226; 2026-11-15)

Branch `feat/app-225-org-roles`, worktree `forma-roles`. Each break edits one rule in
place with a reversible script (the exact edit, then its exact reverse; the files are
compared byte for byte with the state before the break), runs the test that guards it,
records the failing assertion, and restores. Logs are local only (`break-*.log` in the
session scratchpad); the lines quoted here are copied from them.

## (a) The last-owner check in `genui_private.set_member_role` (`app/migrations/0025_organization_roles.sql`)

Break: the guard `IF v_other_owners=0 THEN RETURN QUERY SELECT 'last_owner' …` becomes
`IF false THEN …`; the manifest is repinned to the broken migration and `catalog-v25.json`
is regenerated from it in a disposable PostgreSQL 16.13 (`generate_schema_catalog.py`), so
the break reaches the tests instead of the catalog pin (`check-schema-pins.py`: `"agree": true`
on the broken pins too).

- `just campaign org-roles` fails at the last-owner section (`org_roles_campaign.rs:543`): with every
  other owner demoted, the one owner left demotes themselves to admin:
  `assertion left == right failed  left: Changed  right: Refused("last_owner")`.
  The 150-cell ladder before it still passes (two owners exist there, so the guard is never reached),
  which is why the campaign demotes the others first.

Restored: the edit reversed (`grep -c "BREAK (a)"` is 0), the catalog regenerated and the manifest
repinned from the restored SQL; `catalog-v25.json` and `0025_organization_roles.sql` are byte-identical
to before the break (`3b43677d…6cf8` and `5d237941…b904`); `just campaign org-roles` passes
(`org-roles campaign: 9 checks passed`).

## (b) The viewer write clause in SQL (`authorization::not_viewer`, `app/src/storage/authorization.rs`)

Break: `not_viewer` composes `TRUE` instead of the membership clause, so `work_clause` for
`record_write`, `reviewer_clause` and the `edit_definition`, `review_publish`, `share_work` and
`export` branches of `permissions` no longer judge the role.

- `just campaign org-roles` fails at the first viewer-walk check (`org_roles_campaign.rs:941`):
  the viewer's record listing now says they can write
  (`the listing tells the viewer it cannot write`: `can_write` came back true from the after-fence
  `record_write` grant, which the direct contributor share carries).
  Had the campaign gone on, the viewer's `commit_record` would have been refused by migration 0025's
  RESTRICTIVE `viewer_read_only_insert` policy at the table (SQLSTATE 42501, `Error::Unavailable`
  rather than `Denied`): the second layer holds when the first is broken, and the assertion on
  `Denied` would still have failed.

Restored: the edit reversed (`grep -c "BREAK (b)"` is 0); `just campaign org-roles` passes
(`org-roles campaign: 9 checks passed`).

## (c) The manager-can't-set-admin rule in the Rust ladder (`app/src/storage/authorization.rs`)

Break: `OrgRole::manages` answers `Some(Self::Admin)` for a manager (a manager would then manage
admins and could set admin).

- `cargo test --locked -p forma-app-platform --lib -- org_admin::tests::the_select_offers_only_the_roles_the_caller_may_set`
  fails (`org_admin.rs:1100`): the manager's select offers `["admin", "manager", "member", "viewer"]` on
  the manager, member and viewer rows, and `["manager", "member", "viewer"]` on the manager's own row
  (`left`), where the table says `["member", "viewer"]` on members and viewers only and nothing on the
  manager's own row (`right`).
- `cargo test --locked -p forma-app-platform --lib -- org_admin::tests::change_role_needs_the_token_the_confirmation_and_the_ladder`
  fails (`org_admin.rs:1271`): the forged form (a manager posting `role=admin` with the confirmation)
  answers `/admin?notice=role_changed` (`left`) instead of `/admin?notice=role_not_allowed` (`right`),
  and the fake store records the call.
- The campaign would fail at the ladder cell `Manager on Member -> Admin`
  (`OrgRole::may_set vs the definer`): the definer refuses `not_allowed` while the Rust ladder allows,
  and the two implementations no longer agree. Not run for this break: the two unit tests fail first,
  and the definer (0025) is the second layer the campaign proves cell by cell.

Restored: the edit reversed (`grep -c "BREAK (c)"` is 0); both tests pass
(`test result: ok. 2 passed`).

## Screenshots

Pinned Playwright 1.64.1 (`spikes/APP-003/admin`, `npm ci` in this worktree with no browser download),
driving installed Chrome headless with `reducedMotion: 'reduce'`, against this worktree's clean dev
stand (`just dev clean -d`, port base 7100, Alex Rivera), whose database was migrated to 0025 at start.
The persona's membership was set to `owner` and the other seeded members to `admin`, `manager`,
`member` and `viewer` with one SQL statement in the dev database, so the page shows every role.

| File | Width | What it shows |
| --- | --- | --- |
| [admin-1280.png](admin-1280.png) | 1280 | The Role column, the legend and the helper line; a select on every active row offering Owner, Admin, Manager, Member and Viewer (the owner's view), the confirmation box, Change role, and Remove on every row but the owner's own; the removed member's role as text. |
| [admin-375.png](admin-375.png) | 375 | The same page on a phone: the legend wraps, the table scrolls sideways inside its wrapper (`document.documentElement.scrollWidth <= clientWidth`, no page overflow), every select and button reachable. |

The script read, on both pages: status 200, one `Role` column header, six `select[name="role"]`
(the six active rows), the five role options, five legend items, the helper sentence, and no
horizontal overflow of the document. A first pass with the persona as a manager showed the
select on the member and viewer rows only, with `Member` and `Viewer` as its options and no
confirmation box (the manager's view), as the unit test pins.

The Playwright script and the dev stand were one-off; the script lived in the session scratchpad
and is not committed. The dev stand was stopped afterwards with `just dev clean stop`, which keeps
its data volume.
