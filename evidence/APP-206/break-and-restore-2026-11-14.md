# APP-206: approver routing, proofs and checks (builder B4, 2026-11-14)

Branch `release/c-b4-approvers`, built on `35d47c37` (`release/c-participation`). The decision is [ADR-020](../../docs/architecture/ADR-020-PARTICIPATION-PRIMITIVES.md) §8, candidate A. Its §17 items 16 to 21 are this builder's clarifications (numbered 9 to 14 on the builder's branch; renumbered at the Release C integration, [APP-209 record](../APP-209/integration-2026-11-14.md)). The commits are named in the ticket log.

## What changed, in one paragraph

When a member files a record and names an approver, the server now decides how that approver is reached, and says so in the draft. A **reviewer** of the work who can open it already sees the record, so nothing is shared and the filer reads "Filed for X's review." A member the filer may share with (**shareable**) gets today's automatic contributor share. **Anyone else** gets no share and the note "X is not a reviewer of this app; the record is filed and visible to its reviewers." The note names no one else's access. A student who cannot share therefore never sees "The share with X was refused" for the teacher. Nothing new is written, and no path widens anyone's access.

## The interface

- **Storage.** `storage::Workspace::approver_standing(request, app, work, grantee) -> Result<ApproverStanding>`, in `app/src/storage/sharing.rs`. `ApproverStanding` is `Reviewer`, `Shareable` or `Unreachable`, with the wire names `reviewer`, `shareable` and `unreachable`. It is one `REPEATABLE READ READ ONLY` snapshot with no fence, and it writes nothing. A filer who cannot write the work, a malformed id and the filer naming themselves are all `Denied` (ADR-020 §17.18).
- **`POST /v1/act`.** Each draft approver now carries its standing. The shape is `"approver": {"principal_id", "label", "standing"}|null, "approvers": [{"principal_id", "label", "standing"}, …]`.
- **Canvas.** `app/admin/src/canvas/actions.ts` exports `shareWith(draft) -> {note, sent}`, `reviewerNote` and `unreachableNote`. It posts `/share/add` only when the standing is exactly `shareable`.
- **MCP.** A held draft shows `approvers[].{label, standing}`, with labels only and no principal ids. `forma_commit` judges each approver again after the commit and calls `share_work` only for `shareable`. Its result gains `approvers[].{label, standing}`, the standing judged at that moment, or `null` if it could not be read.
- **Slack.** Confirm judges each approver again after the commit and shares only `shareable`. The filed message keeps ", sent to *X* for approval" for a share and adds the two notes, with labels escaped and bold.

## Tests first

- **`actions::tests`.** `each_draft_approver_carries_its_server_judged_standing` failed before `draft_document` emitted the standing. The output was `left: [{"label":"Olive Park","principal_id":…}, …]`, with no `standing`.
- **Canvas.** `admin/test/canvas-approvers.test.ts` failed before the code existed: "does not provide an export named 'reviewerNote'".
- **`actions` campaign, against a stub.** `approver_standing` first returned `Unreachable` for everyone. The campaign then failed at `actions_campaign.rs:683` with `left: "unreachable"`, `right: "shareable"`: Sarah on Diego's own work, the pitch story's approver. The real read then made it pass.
- **MCP and Slack unit tests.** These were written before their test hooks existed (`approver_notes`, `draft_document_for_tests`, `approver_sentences`). That first red was a compile failure, not a failing run. The guards they cover are proved by the breaks below.

## What the campaigns prove, over real rows

The routing world is shared by the three campaigns (`actions_campaign.rs`: `routing_world`). Its app is the private-entries campaign's general definition: a private `claims` list with a `submit` create, a shared `notes` list, and no source dependency. The members are:

- **Olive**, the app's owner and the creator of its work.
- **Rita**, Olive's personal reviewer.
- **Sam**, a contributor under the organization-wide share. The operator granted him `discover` and `run`, and he has no share authority. He stands for a student.
- **Nia**, an active member with no access.
- **Hal**, who has the app in his library.

"Rows unchanged" means the counts in `app_permissions`, `work_permissions` and `work_shares`, plus the admission chain's `permission.granted` events, are the same before and after.

| Case | `actions` (canvas path) | `mcp` | `slack` |
| --- | --- | --- | --- |
| Sam names the owner | draft `reviewer`; no share request; Olive's listing shows it; rows unchanged | `reviewer`; "Filed for Olive Park's review."; rows unchanged; Olive sees it | "Filed for *Olive Park*'s review."; no "sent to"; rows unchanged; Olive sees it |
| Sam names a personal reviewer | `reviewer`; Rita sees it; rows unchanged | same, Rita | same, Rita |
| Sam names a member under a live organization-wide reviewer share | Nia: `reviewer`; Nia sees it; rows unchanged | Hal: same | Hal: same |
| Sam names a non-reviewer | `reviewer` then `unreachable` for "Olive and Nia"; Nia does not see it | `unreachable`; the plain note; Nia does not see it; rows unchanged | the plain note; Nia does not see it; rows unchanged |
| A filer with share authority names a non-reviewer | Olive names Nia: `shareable`; `/share/add` 200; one new `work_shares` row | Olive names Hal: `shareable`; "Shared with Hal Moreno for approval."; one new share row | Olive names Hal: ", sent to *Hal Moreno* for approval"; one new share row |
| Forged standing | Sam's draft replayed with both standings set to `shareable`: both `/share/add` are 404; rows unchanged | the held draft is judged again at the commit: held `unreachable`, Olive then makes Nia a reviewer, and the commit reaches Nia as `reviewer`; a replayed commit reaches nobody | judged again at Confirm: a card drafted while Nia was unreachable, Nia then made a reviewer by Olive, Confirm reaches her as `reviewer` ("Filed for *Nia Brooks*'s review."), she sees it, rows unchanged |
| The owner cannot open a member's own work (§17.16) | Hal's own placed work names Olive: `shareable`, not `reviewer`; after the share Olive sees it; Diego naming the library's owner (`…0066`) is `shareable` | — | — |

The storage matrix in the `actions` campaign calls `approver_standing` directly:

- `Reviewer`: a personal reviewer; the work's unrevoked creator; the owner once she can open the work; the owner and a personal reviewer for Sam.
- `Shareable`: Olive for Nia, Hal and Sam before the organization-wide share; Diego for the library owner and for Sarah; Hal for Olive before his share.
- `Unreachable`: for Sam on Olive's work, Nia, Hal, a member of another organization, tenant_seed's revoked member and a never-member; for Olive on Hal's work, Sam. A contributor never holds `share_work`.
- `Denied`: Sam, Nia and Hal before they can write; Olive naming herself; a malformed id.
- Live, never cached: the organization-wide share moved back to contributor makes Nia `unreachable` again.

The pitch stories keep their behavior. Diego's rehearsed claim names Sarah as `shareable`, and every existing check of the `actions`, `mcp`, `slack` and `workflows` campaigns still passes (the APP-107 two-level workflow included).

## Break and restore

Every guard was broken alone by a script (`breaks_b4.py`, kept in the session scratchpad). For each break, the script:

1. replaced exactly one span of `HEAD`, after asserting the span was unique;
2. ran the command that should catch the break;
3. recorded what failed;
4. restored the file with `git checkout --`;
5. required the file's blob hash to equal `HEAD:<file>` and `git status --porcelain --untracked-files=no` to be empty before the next break.

S1 to K1 ran at `ddfa8c2e`. K2 ran at `86ff3ef3`, which adds the Slack recheck case it needs; none of the product code changed between the two commits. Every restore was byte-identical. Breaks S, A, M and K are in Rust (storage, the action path, MCP, Slack); C is the canvas.

**A filer without share authority never causes a share.** M2 and K1 remove the very guard that would stop such a share on the MCP and Slack paths, so that `share_work` runs for every standing. Even then, the rows-unchanged assertion that runs before the note check passed, because the definer still judges the filer's own authority. The campaign then failed on the wording. On the canvas path, the forged-standing case proves the same thing with the guard intact: both `/share/add` requests are 404 and the rows are unchanged.

| Break | Guard | File: change | Command | Caught (exit, s) | What failed | Restored |
| --- | --- | --- | --- | --- | --- | --- |
| S1 | the reviewer rule (reviewer_clause) in the reviewer read | `app/src/storage/sharing.rs`: `reviewer_clause("w.app_id", "w.work_id", grantee_sql)` → `"TRUE"` | `just campaign actions` | yes (101, 194) | `actions`: Diego naming Sarah, who can open his work after his share, became `Reviewer`, expected `Shareable` (`actions_campaign.rs:2530`). Without the reviewer rule, anyone with access counts as a reviewer. | yes |
| S2 | the approver can open the work now (the record_read work clause) | `app/src/storage/sharing.rs`: `record_read_clause(grantee_sql)` → `"TRUE"` | `just campaign actions` | yes (101, 99) | `actions`: Diego naming the library's owner `…0066`, who cannot open his work, became `Reviewer`, expected `Shareable` (`:2530`). This is §17.16's case. The first run of this break failed earlier, in the pre-existing APP-168 records search (503 under machine load, before the APP-206 phase); it was rerun alone and failed here. | yes |
| S3 | only the filer's own share authority makes an approver shareable | `app/src/storage/sharing.rs`: `… => Ok(ApproverStanding::Unreachable),` → `… => Ok(ApproverStanding::Shareable),` | `just campaign actions` | yes (101, 199) | `actions`: Olive on Hal's work naming Sam became `Shareable`, expected `Unreachable` (`:2590`, "a contributor never holds share_work, so cannot route"). | yes |
| S4 | the filer must hold record_write on the work | `app/src/storage/sharing.rs`: `.precheck(Action::RecordWrite, Resource::Work { app, work })` → `.precheck(Action::Discover, Resource::App(app))` | `just campaign actions` | yes (101, 143) | `actions`: Sam, who cannot write Olive's work yet, learned `Reviewer`, expected `Denied` (`:2486`, "a member who cannot write the work learns nothing"). | yes |
| S5 | the filer naming themselves is refused | `app/src/storage/sharing.rs`: `require(grantee != request.principal)?;` removed | `just campaign actions` | yes (101, 168) | `actions`: Olive naming herself got `Reviewer`, expected `Denied` (`:2492`). | yes |
| A1 | resolve judges every approver's standing | `app/src/actions/resolve.rs`: the `approver_standing` call in `resolve_draft` → `let _ = &approver.principal;` | `just campaign actions` | yes (101, 92) | `actions`: the pitch's rehearsed claim drafted Sarah as `unreachable`, expected `shareable` (`:683`). The fail-closed default reached the wire. | yes |
| A2 | a name match alone is unreachable (fail closed) | `app/src/actions/resolve.rs`: `standing: ApproverStanding::Unreachable` (in `match_approver`) → `Shareable` | `cargo test --locked -p forma-app-platform --lib actions::tests::` | yes (101, 53) | `actions::tests::a_name_match_alone_reaches_no_one_and_the_notes_name_only_the_approver`: `Shareable`, expected `Unreachable`. | yes |
| A3 | the draft carries each approver's standing | `app/src/actions/routes.rs`: `"standing": approver.standing.name()` → `"standing_": …` | `cargo test --locked -p forma-app-platform --lib actions::tests::` | yes (101, 26) | `actions::tests::each_draft_approver_carries_its_server_judged_standing`: the wire had no `standing`. | yes |
| C1 | the canvas sends a share request only for a shareable approver | `app/admin/src/canvas/actions.ts`: both `if (approver.standing …) { notes.push(…); continue; }` lines removed | `npx --prefix app/admin tsx --test app/admin/test/canvas-approvers.test.ts` | yes (1, 1) | `canvas-approvers.test.ts`: tests 1, 2 and 4 failed, because requests were sent for reviewer, unreachable and missing standings. | yes |
| C2 | the canvas fails closed on a missing or unknown standing | `app/admin/src/canvas/actions.ts`: `if (approver.standing !== 'shareable')` → `if (approver.standing === 'unreachable')` | `npx --prefix app/admin tsx --test app/admin/test/canvas-approvers.test.ts` | yes (1, 1) | `canvas-approvers.test.ts`: test 2 failed, because a missing or unknown standing sent a request. | yes |
| M1 | forma_commit judges the standing again after the commit | `app/src/mcp/tools.rs`: the post-commit `approver_standing` call → `Ok(approver.standing)` (the held draft's) | `just campaign mcp` | yes (101, 125) | `mcp`: the draft held as `unreachable` stayed `unreachable` at the commit after Nia became a reviewer (`mcp_campaign.rs:1736`). | yes |
| M2 | forma_commit calls share_work only for a shareable approver | `app/src/mcp/tools.rs`: the `Reviewer` and `Unreachable` arms → `Ok(_) => match stores` (share for every standing) | `just campaign mcp` | yes (101, 98) | `mcp`: `share_work` was called for Sam's reviewer approver. **The rows-unchanged assertion before it passed** (the definer refused), and the check failed on the note "It could not be shared with the approver." instead of "Filed for Olive Park's review." (`:1637`). | yes |
| K1 | Slack's Confirm calls share_work only for a shareable approver | `app/src/slack/flows.rs`: the same as M2, in `slack/flows.rs` | `just campaign slack` | yes (101, 93) | `slack`: same as M2. **Rows unchanged passed**, and the message lacked "Filed for *Olive Park*'s review." (`slack_campaign.rs:1150`). | yes |
| K2 | Slack's Confirm judges the standing again after the commit | `app/src/slack/flows.rs`: the same as M1, in `slack/flows.rs` | `just campaign slack` | yes (101, 79) | `slack`: the card drafted while Nia was unreachable was confirmed with the stale standing, so the message gave the unreachable note instead of "Filed for *Nia Brooks*'s review." (`:1260`). | yes |

## Commands and results

Local macOS, toolchains from `just setup`, Docker 29.8.2. CI was not observed.

- `cargo test --locked -p forma-app-platform --lib actions::tests::`: 10 passed, 1 ignored (the real-model smoke); the two APP-206 tests are among them.
- `cargo test --locked -p forma-app-platform --lib mcp::tests::`: 13 passed. `slack::tests::`: 11 passed.
- `npx --prefix app/admin tsx --test app/admin/test/canvas-approvers.test.ts app/admin/test/team-sharing.test.ts`: 18 passed. `npm --prefix app/admin run typecheck`: clean.
- `just campaign actions`: passed (20 checks, 103 s). `just campaign mcp`: passed (15 checks, 94 s). `just campaign slack`: passed (12 checks, 57 s, at `86ff3ef3` with the recheck case).
- `just campaign share-routes`: passed (50 s). `just campaign workflows`: passed (63 s). `just campaign sharing`: passed (23 s). `just campaign private-entries`: passed (61 s).
- `just check`: every step passed except two. The steps are fmt, admin, clippy, clippy-dev, test, http-smoke, msrv, catalog, infra, py-scripts, py-runtime, shared-broker, py-qualification, py-app-scripts, py-ops, py-infra, schema-pins, contracts and bash-n.
  - `py-promotion` fails with the same three tests as on the base (`test_committed_release_is_bound_to_this_tree` and two errors). The cause is that `ops/promotion/schema-release.json` is release A's record. This tree's deployment-contract digest is `714c8c12…`, the same as APP-202 recorded, so B4 does not move it.
  - `board` failed only because this file did not exist yet.
  - After the Slack recheck case, the evidence and the ticket move: `just check fmt clippy clippy-dev admin board` all ok.
- `python3 Tickets/_tools/board.py render`, then `check`: "OK: 187 tickets; 9 phases; 15 sprints; 80 requirements; design digests and active links verified."

## Limits

- **No independent review yet.** The Tier A review that APP-206's last acceptance item asks for has not been done. It should cover who can be reached and what the standing discloses.
- **The canvas path in the campaign is an emulation.** `canvas_share_step` reproduces `shareWith`'s rule over HTTP. `shareWith` itself is proved by the node test with a stubbed `fetch`. No browser ran it.
- **Part of the reviewer read is untested by a break.** `discover_clause` is not broken on its own: no fixture has a reviewer who can read the work but lacks `discover`. The entitlement and active-membership join is defense in depth beside `reviewer_clause`'s own membership test.
- **What the standing discloses.** A filer learns one fact about each member they named from the members list: whether that member reviews this work and can open it (§8's accepted disclosure, narrowed by §17.16).
- **The canvas's notes are stale by one commit.** The canvas acts on the standing read before the commit (§17.19). MCP and Slack judge it again.
- **A student with only the organization-wide share cannot use the action path.** This is unchanged by APP-206. The action, MCP and Slack catalogs are `composer_context`'s library, which needs explicit `discover` and `run` rows on the app. Without operator grants, `/v1/act` answers the uniform 404 (`nothing_actionable`). The campaigns give Sam those grants, as the operator would. Release C's class setup must do the same.
- **One unrelated flaky run.** One `slack` run, made in parallel with `mcp` while another builder's `just check` was loading the machine, failed at a 5-second wait in the pre-existing hostile-clicks section, before the APP-206 phase. A rerun alone passed (80 s).
