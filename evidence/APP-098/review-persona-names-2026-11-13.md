# Independent review: APP-098 DS-44, persona names only inside a listed demo organization

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-13. I did not write this change.
- **Reviewed revision:** `f39c96d` on `app098/persona-names-scope`, in the `forma-work` worktree (`git show HEAD`, 18 files, +243/−79).
- **Protocol:** I read the code and ran unit tests, `cargo fmt --check` and the board check. I used no browser, no AWS, no dev stand and no campaigns. I edited nothing except this file.
- **Verdict: approve with changes.** The gate is correct, and every label is built with an organization the server has already judged. No blocker and no major. Two ticket sentences and one doc comment need correcting (m1 to m3). n1 is optional.

## What I checked

1. **Organization per label (no cross-tenant or attacker-chosen organization found).**
   - Slack `notify::approvers` (`slack/notify.rs:79-119`): `organization` comes from the server's event hub. Each linked member is re-resolved and skipped unless `member.organization() == organization` (`:93`), and the listing is read as that member. So `approval_request(organization, …)` labels the record's own organization.
   - `flows::decide` (`flows.rs:852-940`), `pending_message` (`:326-342`) and `notify_filer_record` (`:754-783`) all use the `TrustedRequest` resolved from the stored Slack link, never from the Slack payload. The filer DM labels the decider in `decider.organization()`, and the filer is re-checked to be in that organization (`:789`).
   - `slack/link.rs:212`, `:266`: the session member.
   - Guest record cards (`sharing/live.rs:670-676`): `access.organization()` is the URL's organization, but `resolve_access` (`storage/shared_access.rs:607-621`) builds `GuestAccess` only for an accepted, live share in that organization for this account. So it is always the organization that owns the records.
   - `library_source` (`query/canvas.rs:1739`), `assemble` (`:1456`, `:1464`), the pending check (`:1749`), the top bar (`:1794`), `query/routes.rs:848`, `actions/routes.rs:848`, `record_search.rs:64`, `resolve.rs:821`, `:828`, `:974`, `mcp/tools.rs:1295`, `composer/talk.rs:440`: each takes the organization of the session or MCP member whose listing it labels.
2. **`match_approver` (`actions/resolve.rs:299-305`).** In production every `Member` is built at one site, `storage/sharing.rs:174-177`, with `member_label(&request.organization, …)` in the same transaction that reads the memberships of `request.organization`. The callers are `resolve.rs:672` (`members(member)`) and MCP `tools.rs:1353-1358` (`members(&caller.request)`). Both are request-scoped.
   - A real display name equal to the neutral label (an operator writing "Member 0000abcd" for principal `0000abcd-…`) would now be skipped where it used to match. That is a refusal, never a wrong match, and only the operator's file can cause it, in a demo organization. It does not matter.
   - A name equal to another principal's neutral handle would still match its owner. That is also operator-only and harmless.
3. **Missed callers.** `grep -rnE "directory::(label|display_name|neutral_label)|member_label" app/src`: every production caller passes an organization (the list in 1). The compiler enforces this for the re-signed `records_cards`, `ledger_cards`, `records_document(_with)`, `approval_request`, `claim_blocks`, `match_target`, `target_score`, `ask::user` and `library_source`. Nothing else reads `names()` (`directory.rs:41`, used only at `:137` and `:154`).
4. **Demo organization unchanged.** `is_demo_org` (`demo.rs:72-77`) needs the switch and the listing. Names load only with the switch on (`directory.rs:66-90`). So in a listed organization `display_name` returns exactly what it did before. In a non-demo organization it now returns none, which is the intended change.
   - The canvas note's `is_demo` (`canvas.rs:334-337`) adds a copy of the same `GENUI_DEMO_ORGANIZATIONS` list, so "the same rule" holds.
   - **Parallel tests:** only the two tests touched here (`directory.rs:202`, `actions/tests.rs:197`) and the `demo.rs` tests (`:122`, `:317`) change the demo switches or the names map, and all of them hold `demo::lock_for_tests()` for their whole run. The other tests use organization `…0000d2`, which is never listed, so a concurrent test cannot make them see a name.
   - The commit also removes an old race: `member_labels_show_no_provider_identity` used to read principal `…6d` with no organization while the actions test named `…6d` "Sarah Chen".
   - A panic between configure and reset leaves only organization `…d1` switched on, and no unlocked test uses it.
5. **Ticket text:** mostly accurate; see m1 and m2. `cargo test --lib` 415 passed matches my run. "Both failed with the gate removed": I did not re-run this, because I may not edit source. It follows from the code: without the gate, `display_name(OTHER, SARAH)` and `match_approver("Sarah", &listed(OTHER))` both find Sarah.

## Findings

- **m1 (minor, ticket).** `Tickets/2-in-progress/APP-098.md:420` says "The three campaigns that set persona names". Four campaigns set them: `workflows_campaign.rs:274`, `mcp_campaign.rs:359`, `slack_campaign.rs:507` and `actions_campaign.rs:467`. Each sets them inside its own `ORG`, with `demo::configure_for_tests(true, &[ORG])` right after. So the claim holds for all four, but the count is wrong.
- **m2 (minor, ticket wording).** `APP-098.md:128` says "Every label is built for the viewer's organization", and then lists the guest page. A guest is not a member of that organization. Its cards are labelled for the sharing organization (the records' organization, judged by the accepted share). Suggest "the viewer's or, on the guest page, the sharing organization".
- **m3 (minor, stale doc).** The `app/src/directory.rs:8-9` module doc still says "beta members get a name only when the operator writes one". After this commit a member outside a listed demo organization never gets a name, even if the file lists them.
  - `directory.rs:20` ("behind GENUI_DEMO_MODE since 2026-11-12") should also name the 2026-11-13 organization scope.
  - `demo.rs:63-65` (`is_on` "only … the persona display names") is still true for the load, but DS-44 now also uses `is_demo_org`.
- **n1 (nit, design).** `match_approver` now treats "named" as `label != neutral_label(principal)`, which ties it to the label format. The existing assertion `match_approver("Member", &members).is_none()` (`actions/tests.rs:222`) guards that. A `Member { name: Option<String> }` field would make it structural. Optional; not needed for this change.

**Outside this commit (already in box 5, `APP-098.md:247-253`):** Slack and the guest page show persona names in a demo organization without a "Synthetic data" note. The campaigns call `configure_for_tests` without the test lock. That only matters if they run in one process with the unit tests.

## Commands and results

- `cargo test --locked -p forma-app-platform --lib`: 415 passed, 0 failed, 34 ignored (6.02 s).
- `cargo test --locked -p forma-app-platform --lib directory::`: 3 passed, among them `display_names_apply_only_inside_a_listed_demo_organization`.
- `cargo fmt -p forma-app-platform --check`: clean.
- `python3 Tickets/_tools/board.py check`: "OK: 178 tickets; 9 phases; 15 sprints; 80 requirements".
- Not run: clippy, binary tests, campaigns (outside this review's scope).
