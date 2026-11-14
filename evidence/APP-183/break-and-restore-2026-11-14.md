# APP-183 Organization Admin and APP-190 member names: break-and-restore runs, 2026-11-14 (W2)

Each test that proves the `/admin`, Remove and `/me/name` rules was run once with its guard deliberately broken. The run had to fail at that test's assertion. The code was then restored and the test passed again. Branch `trial/w2-admin-cap`, worktree `forma-w2`. The unit tests use a fake admin store. The server-rendered page tests render the native pages from `admin/dist`. `org-admin-routes` is the campaign over real rows: local Docker PostgreSQL, W1's storage, minted sessions.

## Run 1: routes, page and canvas (`5a1a7dae`)

The committed files were broken in one build, and the canvas bundle was rebuilt with the TSX breaks:

| Guard broken | Test run | Failed at |
| --- | --- | --- |
| `show` reads the overview before the admin check | `cargo test -p forma-app-platform --lib -- org_admin` | `an_admin_sees_every_member_and_anyone_else_the_uniform_404_before_any_read`: the overview was read for a non-admin |
| Remove's token check weakened (`!shaped && !verify`) | same | `remove_needs_the_token_and_the_confirmation_and_shows_refusals_plainly`: a bad token was not 403 |
| `remove_route`'s same-origin check removed | same | `the_routes_refuse_strangers_and_cross_site_posts_before_anything_else`: a cross-site post was not 403 |
| `/me/name` checks only for an empty name (no `usable_name`) | `cargo test … -- member_name` | `the_name_needs_the_token_is_judged_and_reaches_storage_trimmed` |
| Canvas TSX: the Admin link shown to everyone | `cargo test -p forma-app-platform --bin forma-app-platform` | `canvas_page_renders_groups_cards_and_receipts_without_javascript`, `main.rs:1930`: `href="/admin"` appeared for a non-admin |
| Admin TSX: a Remove form on every row | same | `org_admin_page_renders_the_members_table_without_javascript`, `main.rs:2055`: the token appeared twice, not once |

Restore: `git checkout --` of the broken files back to `5a1a7dae`, then `npm run build` in `app/admin`. The targeted unit tests and the two server-rendered page tests then passed again. The next full `just check` flagged only the managed source bundle, which did not yet list the new files; `1caeac42` fixed that.

## Run 2: the campaign over real rows (`c9838fa1`)

- **Broken:** `query/canvas.rs` with `is_org_admin: is_org_admin && false`, so no Admin link for anyone.
- **Command:** `just campaign org-admin-routes`.
- **Failed:** `org_admin_routes_campaign.rs:257`, the admin's canvas has no `href="/admin"`.
- **Restore:** the file was copied back from a copy; `git diff --quiet -- app/src/query/canvas.rs` was clean, and the campaign passed again (exit 0).

## Run 3: review m1, m2 and m5 (`62f4e082`)

These breaks were applied in one build with the APP-184 review breaks:

| Guard broken | Test run | Failed at |
| --- | --- | --- |
| m1: `usable_name` without the Unicode Cf check | `cargo test -p forma-app-platform --lib` | `directory::tests::format_characters_make_a_name_unusable`, `directory.rs:287` |
| m2: a refused name answers 422 `problem+json` again | same | `the_name_needs_the_token_is_judged_and_reaches_storage_trimmed`, `member_name.rs:271`: not the 303 to `/canvas?name=invalid` |
| m5: Remove offered on the admin's own row | same | `an_admin_sees_every_member_…`, `org_admin.rs:582` "no Remove on the admin's own row" |
| m5 over real rows | `just campaign org-admin-routes` | `org_admin_routes_campaign.rs:179` "no Remove on the admin's own row (review m5)" |

The campaign stops at its first failure, so its m1 and m2 checks (the zero-width name and `?name=invalid`) were not reached in this broken run. The unit tests above cover both, and the restored campaign passes them.

Restore: `git checkout --` of the broken files back to `62f4e082`; `git status` was clean except `evidence/`. Then `cargo test -p forma-app-platform --lib` gave 429 passed, and `just campaign org-admin-routes` gave exit 0 with 4 checks.

## Not broken

`member_name::tests::the_route_refuses_cross_site_posts_and_strangers` was not targeted in any run.

## Recheck fix round (`f8864d52`): N1, N2, N4, N6 and m3

These are the fixes for the Tier A recheck ([recheck-w2-2026-11-14.md](recheck-w2-2026-11-14.md)). Each guard was broken in the committed `f8864d52`, and the source was restored with `git checkout -- app/src` after every round. `git status` was clean after each restore.

### Round 1: one build, `cargo test -p forma-app-platform --lib`, 6 failed

| Guard broken | Failed at |
| --- | --- |
| N1: `/me/name` skips the member write window (`if false && … admit_member_write`) | `member_name::tests::the_name_shares_the_member_write_window`, `member_name.rs:424`: write 121 got `/canvas`, not `/canvas?name=limited` |
| N2: no visible-base requirement | `directory::tests::a_name_needs_a_visible_base_character`, `directory.rs:454`: `"ㅤ"` (U+3164) was accepted. This also failed `the_name_needs_the_token_…` at `member_name.rs:354` (U+3164 reached storage). |
| N4: the canvas gate accepts any query (`map_or(CanvasQuery::Plain, …)`) | `member_name::tests::the_canvas_accepts_only_the_closed_notices`, `member_name.rs:460`: `""` gave `Plain`, not `Refused` |
| N6: an outage on `GET /admin` before the overview is raw JSON again | `org_admin::tests::an_admin_sees_every_member_…`, `org_admin.rs:677`: no outage page was rendered |
| N6: a Remove outage is raw JSON again | `org_admin::tests::remove_needs_the_token_…`, `org_admin.rs:618`: no `Location` (`/admin?notice=unavailable` expected) |

The `/me/name` outage break was also in this build, but the N2 failure in the same test came first, so it was proven again in round 2.

### Round 2: one build, `cargo test -p forma-app-platform --lib`, 3 failed

| Guard broken | Failed at |
| --- | --- |
| N2: U+2028/U+2029 allowed again | `a_name_needs_a_visible_base_character`, `directory.rs:457`: `"Ana\u{2028}Lopez"` was accepted |
| N6: the overview's outage is raw JSON again | `an_admin_sees_every_member_…`, `org_admin.rs:676`: no outage page for the overview's outage |
| N6: a `/me/name` outage is raw JSON again | `the_name_needs_the_token_…`, `member_name.rs:287`: no `Location` (`/canvas?name=unavailable` expected) |

### Round 3: the campaign, m3

- **Broken:** both guards for a member of another organization, together. `show` let a non-admin through (`Standing::Stranger => {}`), and the store's `overview` turned storage's refusal into an empty list.
- **Command:** `just campaign org-admin-routes`.
- **Failed:** `org_admin_routes_campaign.rs:201`, left 200, right 404: the other organization's member got a page.
- **Why both:** this check has two guards. Either one alone keeps the uniform 404: the route's admin check, pinned by `an_admin_sees_every_member_…`, and storage's refusal of a non-admin overview (`Denied` → 404). So it was proven with both bypassed.

### After the restores

- `cargo test -p forma-app-platform` passed: lib 432, binary 9, doctests 3.
- `just check fmt clippy test admin http-smoke`: all ok.
- `just campaign org-admin-routes`: exit 0 with 5 checks, including the new cross-organization check.
- `just campaign composer`: see the [APP-184](../APP-184/break-and-restore-2026-11-14.md) record.

