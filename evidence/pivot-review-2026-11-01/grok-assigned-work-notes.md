# Grok assigned work notes

## Codex review follow-up (2026-11-01)

Codex merged pull requests 7, 9, 10, 11, and 12 into `pivot/pitch-mvp` at `1a285e2`. Pull requests 5, 6, 8, and 13 were left open with comments. Those comments are fixed and pushed. CI was still running when this section was written, so nothing new is merged yet.

- #5 `7b2c902` loads `/canvas#g-{work}` through the existing placement path when the library lists the work and the board has not mounted it. An unlisted work is not fetched. A mounted work is focused and not placed again. Admin tests 55 passed. `just check admin test http-smoke` passed. `just campaign canvas` passed (5 checks). A fresh browser click of the work-page link was not repeated; no browser tool was connected, and ports 4001–4003 and 4010 were left alone.
- #6 `2725630` reclassifies the four clean-customer verdicts. Rows 13–16 from the merged files walk stay. A supported Shape request was not re-run; the `:5510` instance is gone.
- #8 `bae2466` narrows the folder-authoring gap and records gap 5 as the merged library check (`1b10ab1`).
- #13 `9b249bf` completes the prepared signup callback after a fixture admission takes the last slot. The callback is denied with no session, principal, membership, or entitlement, and no second redeemed slot. `record_pending_admission` returns exhausted without a `denied_for_reconciliation` row; that audit is `admit_redemption`'s when a pending admission already exists. The restart note now says same process, same stores. `just campaign invite-races` 12 checks, 56.71s. `just campaign invite` 13 checks, 40.33s.
- #14 https://github.com/getforma-dev/forma-app-platform/pull/14 names the parser release subdirectories. That was the nonblocking note on merged pull request 10.

Pull request 12's optional note about Add and drag sharing a refusal message was left as optional. It was not a required change.


Written 2026-11-01. This is the session that opened pull requests 5 through 13 on `getforma-dev/forma-app-platform`, then reviewed them. Nothing in this session was merged. Nothing was deployed. The running instances on ports 4001–4003 and 4010 were left alone.

This file is untracked in `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-mvp`. That checkout is on `pivot/app-135-clean-audit` (pull request 6). Committing this file there would put it inside the clean-customer audit. See question 1.

`evidence/APP-135/` in that same checkout was already untracked. It is still untracked.

## Open pull requests

Trunk is `pivot/pitch-mvp` at `af63feba41aa825a972dde354e0a5d2195f7d608`. Each pull request is still open and GitHub reports it mergeable. Parent tickets stay in `Tickets/0-backlog/`. None of them is marked Done.

| PR | Branch | Head | What it is |
| --- | --- | --- | --- |
| [#5](https://github.com/getforma-dev/forma-app-platform/pull/5) | `pivot/local-surfaces` | `574e26d0c10bd92b8f15e35f1211ee572193df7b` | Dev workspace 404 and records on records-only work pages |
| [#6](https://github.com/getforma-dev/forma-app-platform/pull/6) | `pivot/app-135-clean-audit` | `c9f6dfb192d2afb8f53a85781c13e3ff3df7426e` | APP-135 clean-customer walk |
| [#7](https://github.com/getforma-dev/forma-app-platform/pull/7) | `pivot/app-096-rehearsal` | `718e4a8a0a6d5aec695b49d4347af943ea7bb310` | APP-096 rehearsal notes |
| [#8](https://github.com/getforma-dev/forma-app-platform/pull/8) | `pivot/app-129-gaps` | `bec53ee5fd6a535b04e55970febb7e24b98c48f6` | APP-129 gap matrix |
| [#9](https://github.com/getforma-dev/forma-app-platform/pull/9) | `pivot/app-135-files` | `e5f87377cd7a5aa0f8d5fea0e6ae648709b4646e` | APP-135 files and folders walk |
| [#10](https://github.com/getforma-dev/forma-app-platform/pull/10) | `pivot/development-docs` | `5fb32cbbd1fa747d06eeaca2b1a3e9185ae8a294` | DEVELOPMENT.md dry-run corrections |
| [#11](https://github.com/getforma-dev/forma-app-platform/pull/11) | `pivot/app-129-library-check` | `1b10ab13001e8d9b4e340e55c60e928c1a743d03` | `contracts/make_library.py --check` in CI |
| [#12](https://github.com/getforma-dev/forma-app-platform/pull/12) | `pivot/app-104-library-drag` | `198023532deff1fea10f4a4fa9fe57b77ce61827` | Drag a library card onto the canvas |
| [#13](https://github.com/getforma-dev/forma-app-platform/pull/13) | `pivot/app-082-invite-races` | `ca6d960b651b28463f4d6df73dff95c41d80bae0` | Route-level invitation races and provider faults |

Worktrees, one per pull request:

- #5 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-audit-574e26d` (detached at `574e26d`)
- #6 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-mvp`
- #7 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-rehearsal`
- #8 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-gaps`
- #9 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-files`
- #10 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-devdocs`
- #11 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-library-check`
- #12 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-library-drag`
- #13 `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-invite-races`

Three other worktrees were already on this machine (`pivot/review-routing`, `pivot/pitch-script`, `pivot/ci-green`). This session did not use them.

I did not post GitHub reviews or approvals. Codex still has the review.

## Review result

### #5 — no code change after review

CI on `574e26d` was green on both the push and pull_request events when this note was written (app-001, app-003, auth-contract, contracts, fixtures, foundation, parser, parser sources, pitch-campaigns, planning, parser-byte qualification, query, Socket, tenant-foundation).

The release build with no provider returns no registration issuer, so the registry check cannot be skipped. `dev-stand` uses `DEV_ISSUER = "app008-fixture-only"`. The work page sets `records_only` only when there is no runnable view and the outline has collections. Record links are rendered only when `outline.can_write` is true.

Left as suggestions, not changed:

- `read_only_schema` can still return an outline with empty collections and `can_write` false. I did not treat that as a defect in this change.
- A route comment still says the work page always renders the run form.

### #6, #7, #8, #9 — documentation defects, fixed and pushed

These were wrong statements in the planning docs. `python3 Tickets/_tools/board.py check` passed on each worktree after the edit. Front matter did not change, so the board was not regenerated.

#6 `85f5edb` — "Keep the clean-customer register distinct from the files walk." Rows 9–12 stay the clean-customer findings (form Add with no confirm, Shape `could_not_shape`, empty version-0 work-page link, new-browser placement). The ticket says pull request 9 uses rows 13–16 and both tables must be kept.

#7 `718e4a8` — "Stop the recording notes from contradicting the rehearsal." Checklist step 6, pitfall 5, and the honesty notes no longer say Sarah sees the claim automatically, and they no longer say pitfalls 1–4 are confirmed customer defects. On `af63feb`, a freshly placed Expenses card did not show the follow-up claim within 20 seconds. The shot still says to reload. Pitfalls 1–3 were not reproduced. Pitfall 4 was not re-run.

#8 `bec53ee` — "Correct the edition-label seed refusal." `EDITION` is written only onto `catalog.json`. Seeded bundle rows such as `expense-claims.json` have no `edition` key. `library_catalog` refuses a catalog whose edition is not `LIBRARY_EDITION` (`library catalog refused`). `seed-library` surfaces that as `seed_bundle_refused` before it compares rows. `existing seed differs` fires when a seeded row's values change. Changing `catalog.edition`, `EDITION`, and `LIBRARY_EDITION` together, while they stay equal, reports `changed: false`. Stored definitions are not overwritten. The pin that changes when the catalog bytes change is `seed_library_sha256`.

#9 `e5f8737` — "Correct the broken-CSV evidence and renumber the files register." The files register is rows 13–16 (empty folder plus CSV, rename, empty file, malformed CSV). `18-broken.png` shows Club notes with two files: `reading.csv` (Data check, 2 rows) and `broken.csv` (26 bytes, Make data). There is no `notes.txt` and no preview sheet. "may be empty" is the nullable checkbox on every column of every preview, including a successful `reading.csv`. The empty-file result remains `422 unsupported_file`. The suggested fix stays: refuse a CSV the parser cannot read as a table.

One review claim was checked and left unchanged. The gap matrix tells the next change to run `make_library.py --check` with the runtime virtualenv. `scripts/dev/setup.sh` unions `app/runtime/requirements.lock` and `contracts/requirements.lock` into that environment. The invite-races runtime virtualenv has `jsonschema` 4.26.0 and `duckdb` 2.0.0. Running that interpreter from the gaps worktree printed `{"files": 40, "apps": 20}` and exited 0.

Left as suggestions, not changed:

- #7 checklist items 9 and 11, and shot 16, still say "not re-run today" while the header says "not re-run on af63feb." The timing budget (3 minutes 45 seconds, beat 1 at 75 seconds against shots 1–5 at 78 seconds) was not recalculated.
- #8 the summary line "Folder publication does not" is broader than the code. Model "Build the app" already passed `require_authored`. `publish_rule_built` is the data-card path.
- #10 the dry-run says `just parser` writes files at the `.cache/parser-target/` root. They land in `release/` and `wasm32-unknown-unknown/release/`.

### #10 — no defect

The dry-run corrections match the scripts: port bases, an already-running server, missing `node_modules`, and the parser starting with the app. The workspace 404 in `docs/DEVELOPMENT.md` is still true on `af63feb`. The 200 is on `574e26d` only (pull request 5).

### #11 — no defect

`--check` compares generator bytes, writes nothing, and on a stale tree exits 1 with `{"stale":[...]}` on stderr. An unchanged tree is `{"files": 40, "apps": 20}`. The implementation lock lists 56 artifacts because `contracts.yml` is itself locked. Edition `impl-2026-10-31.4` and catalog `impl-2026-10-28.1` are unchanged. DuckDB stays on the APP-005 lock at 2.0.0 and is not added to `contracts/requirements.lock`. CI for this pull request was green.

### #12 — one product bug, fixed, tested, and pushed

The preview-size fix is `9ffbdf33eed94268a7d57797aa4a3e0c88aaa899`, "Size the library drop preview from the canvas kind." The commit before that was `63358c9` ("Admit the library drag module in the service image."). Head is now `198023532deff1fea10f4a4fa9fe57b77ce61827`, which only changes the foundation toolchain install described below.

The pane label collapses `summary` and `status` to `number`, and `timeline` and `chart` to `chart`. `cardFootprint` only knows the canvas sizes (`summary` 400×250, `timeline` 620×340, and the app, form, records, and ledger sizes). Anything else, including the pane labels `number` and `chart`, is 440×300. The drag was passing the pane label, so a summary ghost was 440×300 and a timeline ghost was 440×300. The drop point itself was already correct. Chart and status already match the 440×300 fallback, so they did not show the bug.

The fix keeps both words. `LibraryCardProps.canvas_kind` is the board size. The visible label stays the pane kind. The server-rendered row sets `data-canvas-kind`. A row built in the session copies it from `boardWorks()`. The drag footprint reads `canvasKind`, and falls back to the pane label only when that attribute is absent. `app`, `form`, `records`, and `ledger` use the same word in both places.

APP-104 stays Backlog. Fences are still open. Shared groups, hover-scrub, and viewport-nearest loading were out of this task.

The earlier browser walk on this worktree (Diego, port base 4700, canvas zoom 125%) confirmed the drop point, a miss, Escape, no duplicate, Add, and touch scrolling. That server was stopped. I did not start it again for the size fix, and I did not touch ports 4001–4003 or 4010. The size fix is covered by the tests below, including the server-rendered `data-canvas-kind="summary"` attribute. I have not re-measured the ghost box in a browser after this commit.

Demo mode (`data-demo="on"`) still skips `loadLayout`, so a pitch reload opens on the default row. That is the existing APP-098 rule for every placement. `saveSoon` still writes the layout. See question 2.

### #13 — no product bug

Head `ca6d960b651b28463f4d6df73dff95c41d80bae0`. CI on that commit was green, including both tenant-foundation runs (push `41272366067`, pull_request `41272362340`). The step "Qualify route-level invitation races and provider faults (APP-082)" succeeded. Local campaign: 12 checks, 53.58 seconds. `cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings` passed. `bash scripts/dev/campaigns.sh verify` matched 31 ignored tests. Board check passed.

What the campaign shows, in short:

- One invitation is single-use and email-bound. The honest last-slot race is two validates while the first holds the code row. The loser gets the neutral page `page=invite; failed=true; limited=false`. One live pending row remains, `redeemed_count` stays 0 until sign-in, and the loser does not start PAR.
- Two concurrent sign-ins of the same subject produce one principal, one entitlement, `redeemed_count` 1, one `admission.redeemed`, and two sessions.
- Both revocation orders are covered. Revocation-first does not arm the `admit` checkpoint, because that checkpoint is after the SQL returns and would hang the callback.
- An entitlement that expires while mint is paused denies the new sign-in and does not leave a live session cookie. Denial clears the cookie with `Max-Age=0`. Existing sessions get the uniform 404.
- Provider faults use the loopback fake only. `drop_before` on that fake is "received, then the connection dropped", so the client records `unknown`, which is what a non-definitive answer is. An unreachable host is the `failed` / `not_sent` case. Create is one POST. Reconcile after the grace window does not send a second create.

APP-082 stays Backlog. Acceptance items 1 and 2 are checked from the local run. Item 3 is unchecked: independent review, and the APP-074 / APP-076 reruns, are not done. See question 4.

A residual I did not change: `sign_in_on` clears the old auth limiter, and the restart case passes because the new limiter is fresh. A lock timeout on the racing validate is the same neutral 400 as a real slot loss.

## Tests for the #12 fix

Run in `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-library-drag` on the sources that became `9ffbdf3`.

- `cargo fmt --check -p forma-app-platform` passed.
- `cd app/admin && npx tsc --noEmit` passed.
- `cd app/admin && npm run build` passed. Canvas JavaScript 159,704 of 373,760 bytes gzip. Forma contribution 26,126 of 28,420 bytes. The built `app/admin/dist` is gitignored.
- `cd app/admin && npm test` passed: 54 tests, 0 failed. That includes the new test "a library row keeps the canvas size apart from the pane label" and the five existing drag tests.
- `cargo test --locked -p forma-app-platform --lib query::canvas::tests::the_library_pane_groups_numbers_and_counts -- --exact` passed.
- `cargo test --locked -p forma-app-platform --bin forma-app-platform tests::canvas_page_renders_groups_cards_and_receipts_without_javascript -- --exact` passed. The rendered page contains `data-canvas-kind="summary"`.
- `python3 Tickets/_tools/board.py check` passed: 137 tickets, 8 phases, 14 sprints, 80 requirements.

The first placement of that new admin test called `indexRow` before the older library test, and the older test compares `findApps('3')[0]` by object identity. The new test now runs last. I did not weaken the older assertion.

## CI

Checked again after the toolchain commits. Pull requests 5 through 13 are open, mergeable, and green. Each head has 28 completed checks, all SUCCESS, counting both the push and pull_request events. Heads are the SHAs in the table above. Nothing was merged.

The foundation failure is not a formatting defect. Local `cargo fmt --check -p forma-app-platform` passes on the doc commits. The uploaded `local-preparation.json` for run `41274629964` shows `cargo fmt --check` exiting 1 with:

```
info: syncing channel updates for 1.93.1-x86_64-unknown-linux-gnu
info: downloading 2 components
info: rolling back changes
error: failed to install component: 'clippy-preview-x86_64-unknown-linux-gnu', detected conflict: 'bin/cargo-clippy'
```

`rust-toolchain.toml` asks for clippy, rustfmt, and the wasm target. The foundation job installed rustfmt only. `cargo fmt` then asks rustup to install the rest, and that install conflicts with `bin/cargo-clippy`. A rerun of `41274629964` (pull request 6) failed with the same text. A rerun of `41264727180` (pull request 10, push) failed the same way. Pull requests 7 and 9 passed the step on both events without a code change, so the conflict is not on every runner.

I pushed `Install clippy with the foundation toolchain.` onto the pull requests that had a failed foundation run, and onto the drag pull request while its checks were still queued:

- #6 `c9f6dfb192d2afb8f53a85781c13e3ff3df7426e`. The audit sentence remains `85f5edb`.
- #10 `5fb32cbbd1fa747d06eeaca2b1a3e9185ae8a294`. The docs commit remains `e0be49c`.
- #12 `198023532deff1fea10f4a4fa9fe57b77ce61827`. The preview-size commit remains `9ffbdf3`.

The install line now matches the other workflows: clippy, rustfmt, and `wasm32-unknown-unknown`. Those three pull requests restarted CI and then went green, including foundation. I left #7, #8, and #9 without that commit. Their heads went green too, so the conflict did not fire on those runners.

## What this session delivered, in order

1. Pull request 5. Dev workspace and records-only work pages. Reviewed clean aside from the two suggestions above.
2. Pull request 6. Clean-customer audit. Register rows kept distinct from the files walk.
3. Pull request 7. Pitch rehearsal corrections.
4. Pull request 8. APP-129 gap matrix, including the edition-label correction.
5. Pull request 9. Files and folders audit, including the broken-CSV correction and register rows 13–16.
6. Pull request 10. DEVELOPMENT.md dry-run.
7. Pull request 11. Library generator `--check` in contract CI, DuckDB kept on the APP-005 lock.
8. Pull request 12. Library-card drag, the preview-size fix in `9ffbdf3`, then the foundation toolchain install in `1980235`.
9. Pull request 13. Twelve route-level invitation race and provider-fault checks, wired into `tenant-foundation.yml` and `scripts/dev/campaigns.tsv`.

Authorization, receipts, idempotency, and the admission rules were left in place. Model output is still admitted data. No new permission, route, or layout store was added for the drag. The invitation campaign uses the existing PostgreSQL fixture, the operator CLI, and the loopback fake provider.

## Open questions

1. Should `GROKASSIGNEDWORKNOTES.md` stay untracked here, or be committed? If it should be committed, which branch? The checkout that contains it is pull request 6.
2. Demo canvases reload onto the default row and ignore a saved drop. That is the current APP-098 rule, and it applies to every placement, not only a drag. Should the pitch instead restore the drop coordinate?
3. When pull requests 6 and 9 merge, git will conflict on `Tickets/0-backlog/APP-135.md` and on the shared register. Keep rows 9–12 from #6 and rows 13–16 from #9. Taking either file wholesale drops one walk.
4. APP-082 item 3 is still open: an independent review, plus rerunning the APP-074 and APP-076 campaigns. I left that box unchecked. Do you want those reruns scheduled, or is Codex's review the item 3 review?
5. The recording now tells the Expenses shot to reload if the follow-up claim is not on the card within 20 seconds. Is that the line you want told?
6. Pull request 8's matrix says to run `make_library.py --check` from the runtime virtualenv. That works, because setup unions the two locks. Pull request 11 runs the same check from the contracts virtualenv and installs DuckDB from `spikes/APP-005/requirements.lock`. Both are real. Which one should the docs describe after both land?
7. Do you want the leftover suggestions changed? They are the `read_only_schema` outline and the work-page comment on #5, the rehearsal timing words on #7, the "Folder publication" summary line on #8, and the parser-cache subdirectory wording on #10.
8. The foundation toolchain install now includes clippy and the wasm target on pull requests 6, 10, and 12. Pull requests 7 and 9 passed foundation without that commit. Pull request 8 was still running. Do you want the same line on 7, 8, and 9 so the job cannot hit the clippy conflict on the next run? The three copies I pushed will conflict with each other only if someone edits that line differently. The change is the same text.
9. Fences are still unchecked on APP-104. Shared groups, hover-scrub, and viewport-nearest loading were not part of the drag task. Say if any of those is the next assignment.
10. APP-082's public last slot cannot be two different subjects finishing hosted registration on one invitation. The test states that limit. Tell me if the scenario you wanted is a different race than the one the campaign locks.
