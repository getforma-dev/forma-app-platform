# Independent review: APP-098 DS-40, the demo note on the work page, guest page and Slack

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-13. I did not write this change.
- **Reviewed revision:** `34e44b4` on `app098/demo-note-surfaces`, in the `forma-work` worktree (`git show HEAD`, 13 files, +113/−6).
- **Protocol:** I read the code and ran unit tests, the admin typecheck and the board check. I used no browser, no AWS, no dev stand and no campaigns. I edited nothing except this file.
- **Verdict: approve with changes.** On every surface the organization is judged by the server, and a non-demo organization cannot get the note. No blocker and no major. Four minors (m1 to m4) and two nits.

## What I checked

1. **Organization per surface.**
   - Work page: `query/routes.rs:881` `is_demo_org(member.organization())`, inside `document()`. Every `page::props` call in `routes.rs` (`:977`, `:1310`, `:1340`, `:1373`) uses the `chrome` that `document()` returns. The only other `WorkChrome` is `sharing/live.rs:70`, which only lays out guest parts; its `demo` stays false and is never rendered.
   - Guest page: `sharing/live.rs:755` `is_demo_org(access.organization())`. `GuestAccess.organization` (`storage/shared_access.rs:89`, `:107`) is the organization that owns the share. It comes from `resolve_guest_access`, as the persona-names review also found. The neutral page (`:587`) and the pending page (`:620`) use `..Default::default()`/`..neutral`, so `demo_data` is false there.
   - Slack: `pending_message` (`slack/flows.rs:351`) and `decide` (`:930`) use the `TrustedRequest` resolved from the stored link. `approval_request` (`:1020`) is called from `notify.rs:119` with the event's organization, after `:94` has checked `member.organization() == organization`. No organization comes from a client or a Slack payload.
   - Same rule as the canvas: the canvas also requires its own copy of `GENUI_DEMO_ORGANIZATIONS` (`query/canvas.rs:334-337`, parsed with `uuid_shaped`). For the lowercase UUID organization ids the database produces, the two rules agree. The MCP line (`mcp/tools.rs:629`) already uses `is_demo_org` on its own.
2. **Guest note after a re-check or revoke.** `unavailable()` (`admin/src/shared/document.ts:87-97`) removes the parts, sets the title to "Private app" and hides the sample note (`:91`). It leaves the demo note in place. What a revoked or unverified viewer can still see is that the organization named in the share key (the URL) is a listed demo organization. They already saw that under authorization, and a real organization never renders the note. Nothing the share protects leaks, so I accept the ticket's reasoning. The visible cost is that a "Private app" placeholder still says "records here are made up" (nit n1).
3. **Slack shape.** `text::fields` still caps a block at 10 fields (`text.rs:53`). The new line is a separate one-element context block. Block counts for a listed demo organization:
   - Waiting list, worst case: 1 + 10 × 4 + 1 = 42. That is under Slack's limit of 50 blocks per message. MAX_PENDING is 10 (`flows.rs:27`).
   - Decision, replaced in place: section, fields (plus a section when only "Filed by" is shown), the synthetic context, the link context.
   - Approver DM: at most 6 blocks.
   No campaign or test indexes blocks by position (`buttons()` filters by type, `slack_campaign.rs:460`). Only the `text` fallback reaches notifications, and it is unchanged.
4. **Wording.** "The organization, people and records here are made up for the demo." makes the same claim as the canvas title "The organizations, people and records here are synthetic". It is singular because each page belongs to one organization. It does not claim more than the canvas, except as noted in m2.
5. **Ticket.** The ticket text is accurate apart from m3 and m4.

## Findings

- **m1 (minor, test gap).** No test covers the positive route wiring. The SSR test sets `WorkChrome { demo: true }` and `SharedPageProps { demo_data: true }` directly (`main.rs:1233`, `:1397`). The pitch check (`pitch_qualification.rs:877-882`) runs with demo mode off: it is switched on only at `:2498` and reset at `:2521`. `shared_campaign.rs:725` never switches demo mode on. So deleting `routes.rs:881` or `live.rs:755` would leave every named test green. Suggested fix: inside the pitch campaign's demo section (`:2498-2521`), GET a work page and assert `demo_data`.
- **m2 (minor, wording).** In a demo organization, a work that reads library sources shows both notes. The work page's sample note says "Records you file are your own." (`work/app.tsx:178`), while the demo note says the records "are made up for the demo". That is likely during the pitch, since Northstar adapts library works. MCP avoids this by letting "Synthetic data." win over "Sample data." (`mcp/tools.rs:641-646`). Suggested fix: drop the sample note's records sentence when `demoData` is set, or accept the overlap and record that.
- **m3 (minor, coverage plus ticket).** A demo organization can still miss the note:
  - (a) A guest page that becomes available without a reload (`document.ts:114`, then `:123`) gets the sample note from `syncSampleNote`, but no demo note. This is a pending invite accepted in another tab.
  - (b) Slack messages not built from `claim_blocks`, such as the filer's DM `notify_filer_record` (`flows.rs:763-801`, which names the decider) and the draft and filing cards.

  The "Not covered" lines in `APP-098.md:120` and `:254` list only the connected canvas cards. Add (a) and (b) there.
- **m4 (minor, ticket).** `APP-098.md:119` says Slack "carries the same line". Slack actually posts "Synthetic data: The organization…" with a colon (`flows.rs:328`), while the web pages show a badge followed by the sentence. Either change the wording to "the same sentence", or make Slack match.
- **n1 (nit).** The waiting list repeats the line once for each record, up to 10 times (`flows.rs:351`). One line per message would be quieter. Doing that would mean moving the line out of `claim_blocks` into its three callers.
- **n2 (nit).** In Slack the label is plain text. `*Synthetic data*` would match the web badge's emphasis.

## Commands run (worktree `forma-work`, HEAD `34e44b4`)

- `cargo test --locked -p forma-app-platform --lib slack::`: 11 passed, 0 failed. This includes `the_approver_dm_names_any_record_app`.
- `cargo test --locked -p forma-app-platform --bin forma-app-platform`: 8 passed, 0 failed. `app/admin/dist` was built at 18:24, and `work.*.ir` and `shared.*.ir` contain the new note.
- `npm run -s typecheck` in `app/admin`: exit 0.
- `python3 Tickets/_tools/board.py check`: OK, 178 tickets.
- Not run, per the brief: campaigns, the full `cargo test --lib`, clippy, a browser or Slack. I did not re-check the "red before" claims in the ticket.
