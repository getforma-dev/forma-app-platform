# Review: guest page and connected card demo note (APP-098 DS-40), 2026-11-13

Reviewer: Claude (Opus 5.5), independent, read-only. Scope: `cce4580e` on `app098/guest-notes-client` (stacked on `app098/demo-note-surfaces`, PR #112). Code and unit tests only; no browser, campaign or dev stand.

**Verdict: approve with changes.** No blocker, no major. Three minor points, two of them ticket text.

## Questions

1. **`syncSampleNote` unchanged.** Yes. `document.ts:57-58` passes the old values to `syncNote` (`:39-55`): selector `[data-sample-data]`, class `sample-note`, badge `span.badge` "Sample data", text `` ` ${SAMPLE_NOTE_TEXT}` ``, `sampleNoteHidden` untouched (`:19-21`). The only change in order is that `hidden` is now computed before the `!host` return. That has no effect, because `sampleNoteHidden` is pure. The two existing sample-note tests pass unchanged.
2. **Unverified or unavailable reads.** I found no path that shows the demo note without an available read, or keeps it hidden after one.
   - Page load: an available SSR page has `[data-shared-parts]`, so `wasAvailable` is true and `unavailable('Checking current access…')` (`document.ts:166`) hides the server-rendered note (`:110`). `authorized` then schedules `refresh`. The note comes back only after `props.available` and the authority-version check pass (`:128-134`, then `:143`).
   - In-place availability: a pending page has no note. The first available refresh only sets `wasAvailable` and returns (`:133`). The next authorized refresh creates the note in `.shared-page-head`, which is always rendered (`app.tsx:19`).
   - Every offline, error, mask, `visibilitychange`, `pagehide`, `pageshow` and record-components-unavailable path goes through `unavailable()`.
   - Card: `attach` masks first (`canvas-controller.ts:98`, hide at `:59`). `epoch` is incremented only in `mask` (`:52`), so a stale `refreshData` returns without showing the note (`:78`). An unavailable read removes the card (`:79`). An error masks it (`:91`). Only a fully checked read reaches `:89`. `.connected-owner` is cleared, not removed, so the host persists.
   - Neither class has a `display` rule, so `hidden` applies. `SharedPageProps` serializes `demo_data` in snake_case (`live.rs:124`), the name the client reads.
3. **Both notes, or neither.** Neither case occurs. The server sets `sample_data: outline.sample_data && !is_demo_org(..)` and `demo_data: is_demo_org(..)` on the only available builder (`live.rs:755-756`). The client syncs both notes from the same `props`, in the same tick, on every path (`document.ts:109-110`, `:142-143`; `canvas-controller.ts:58-59`, `:88-89`). The two notes are hidden together on every mask path. A demo sample work's available read has `demo_data: true`, so its note always shows.
4. **Ticket text.** Accurate in substance. I confirmed "failed with the routine's hiding removed" and "8 passed (2 failed)" by mutation, and "checked by reading" holds (see Commands). See m1 and m2.

## Findings

- **m1 (minor, ticket history), `Tickets/2-in-progress/APP-098.md:120`.** The struck text paraphrases the old bullet instead of keeping it. It also deletes outright the recorded rationale: "server-rendered only … not hidden while access is re-checked, because whether the sharing organization is a demo is not data the share protects". The demo-note review accepted that rationale (item 2, n1). This commit reverses it, which is fine and fixes n1, but the ticket should keep the original sentence struck and say that it was reversed for parity with the sample note.
- **m2 (minor, ticket record), `APP-098.md:19`.** The Branch / PR field still names only `app098/demo-note-surfaces` for DS-40. `app098/guest-notes-client` appears only in the log line (`:427`). Add it, or record that it will be folded into PR #112.
- **m3 (minor, tests), `test/connected-authority.test.ts:5-16`.** The wiring is untested, as the ticket says. With all four `syncDemoNote` call sites deleted, every admin test still passes. The fake `Element` there has `parent` but no `parentElement`, so `startConnectedApps` reaches `syncNote(null, …)` and returns early. Adding `parentElement` to the fake would let the existing test assert that the note is hidden on mask and created on recovery. This is optional, since the ticket records the limit.
- nit: `canvas-controller.ts:58-59`, `:88-89` and `document.ts:109-110`, `:142-143` each look up the host twice. One local variable per site would do.

## Commands (in `forma-work/app/admin` unless noted)

- `npm run -s typecheck`: exit 0.
- `npx tsx --test test/sharing.test.ts test/connected-authority.test.ts`: 8 tests, 8 pass, 0 fail.
- Mutation in a scratch copy (worktree untouched): `note.hidden = hidden` became `note.hidden = false`, and the same command gave 6 pass, 2 fail (tests 7 and 8, both note tests).
- Mutation in a scratch copy: all `syncDemoNote(` calls deleted, then `npx tsx --test test/*.test.ts` gave 300 pass, 1 fail. The failure is `canvas-commands.test.ts` APP-121, which reads `../../src/composer/labels.rs` and so cannot run from the scratch copy. It is unrelated.
- `python3 Tickets/_tools/board.py check` (repo root): OK, 179 tickets.
- Not rerun: `just check admin test http-smoke` (claimed passed in the log line).
