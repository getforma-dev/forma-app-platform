# Independent review: canvas UX closures (APP-104, 106, 109, 112, 118, 119, 120, 121)

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-12. I did not build these tickets or write the lead's closure mappings.
- **Reviewed source:** `main` at `5695869` (code, tests, docs) and closure commit `2e84398` (closure files, `ci-run.md`, reports, `Tickets/0-backlog/APP-177.md`).
- **Protocol:** `Tickets/_tools/REVIEW.md`. No browser was opened, following Victor's instruction for this pass.

## What I read and ran

- **Read:** AGENTS.md; Tickets/README.md (the Done definition); ADR-011, ADR-017, ADR-018 and ADR-019's landing paragraph; the eight tickets with their full Logs; the eight closure files, the closure README and `ci-run.md`; APP-176 and APP-177.
- **Code on main:** `canvas.rs`, `canvas_tests.rs`, `main.rs`, `composer/commands.rs`, `actions/routes.rs`, `actions/tests.rs`, `storage/queries.rs` (`own_works`), `storage/pitch_qualification.rs`, the operator's permission scopes, and the canvas sources `library.ts`, `library-drag.ts`, `slash.ts`, `shortcut-tab.ts`, `controls.ts`, `unite.ts`, `ask.ts`, `app.tsx`. Docs: `docs/pitch/GUIDE.md`, `SCENARIOS.md`.
- **Ran in forma-verify (`app/admin`):** `npx tsx --test` on eight files. All 79 tests passed: canvas-commands 8, canvas-library 14, canvas-library-drag 6, canvas-drag-end 24, canvas-control-pointer 2, canvas-polish 4, app-view-controller 16, app-view-layout 5.
- **Git, read-only:** every delivery commit the closures cite is an ancestor of `5695869`. ADR-018 (`c7e97fa`, 2026-11-02) and PR #48 reached `main` inside PR #51 (`92133c6`, 2026-11-03).
- **Ran in forma-closure:** `python3 Tickets/_tools/board.py check` **fails** with "Stale indexes; run render" on TRACKER, PH09, PHASES, S15, SPRINTS, UX, CATEGORIES, COVERAGE and index.json. APP-177 was added without a render.
- **Not run:** cargo and the campaigns. For those I rely on the lead's reports (pitch, actions, query, schema), whose check text I read against the source.

## Is "Done with a recorded supersession" honest?

**Yes, with conditions.** The rules support it:

- ADR-018 is accepted, was delivered by a Done ticket (APP-168), and "supersedes older fan-out/per-app-subcard UX instructions without rewriting their historical evidence" (ADR-018:19).
- AGENTS.md forbids silent rewrites of design history. Keeping the delivered boxes ticked and recording the supersession is therefore the prescribed form. Unticking or rewording them would be the rewrite.
- The behavior landed in the agreed source of its time: the pitch trunk `pivot/pitch-mvp`, the deploy branch under ADR-017 until ADR-019. APP-104 was live on beta (APP-106.md:71).

One precision point. The superseded behaviors (per-card Add, `/card` placement, per-card stacks and drag, Unite and Break apart) **never existed on `main` as trunk**. They were replaced on the pitch trunk on November 2, and PR #51 landed the already-replaced code. The README's "merged into `main` with PR 22–48" is imprecise: those PRs merged into the pitch trunk, and the October 30–31 work entered it by direct merges (for example `de88266`).

"Do not treat a planned checkbox as proof" therefore requires: **(a)** each ticket's own Markdown, which is authoritative, says per box what is superseded and by what (the evidence file alone is not enough); **(b)** "on `main` today" claims cite only tests that test the box; **(c)** surfaces on `main` that still advertise superseded behavior get an owner. (a) waits for the moves. (b) and (c) fail in places, listed below. I found no box that was false when ticked, beyond limits the logs already disclose.

## Cross-cutting findings

1. **Major: the Shortcuts tab on `main` advertises removed paths.**
   - `shortcut-tab.ts:27` says "Library → expand an app → Add, or drag the card onto the canvas · /card expenses.ledger". But App rows have no expander (canvas-library.test.ts:524–536), `/card` refuses app parts (library.ts:735–753), and drag binds only standalone works (library.ts:534).
   - `:39` and `:73` point to "the Ask tab", which does not exist (app.tsx:748–750: Library, Commands, Shortcuts). `:54–55` still describe legacy stacks.
   - GUIDE.md:52, :55 and :114 still teach `/card` placement and Unite/Break apart. SCENARIOS.md:19 says "fourteen verbs".
   - APP-176 owns only `stacks.ts`, so nothing owns this drift.
2. **Major: no browser has observed the `/` dropdown.** APP-119 through APP-123 all log "browser walk not done", and `startSlash` (slash.ts:402) has no DOM test. The data functions it renders are tested directly (`slashGroups` and `slashAddressSection`, consumed at slash.ts:604 and :614), so the risk is low. But each ticket's Verification asked for a walk, and only Victor can waive that.
3. **Minor: dates and ranges are off.** APP-112 and APP-118 cite "October 30" for walks logged 2026-10-31 UTC. APP-120's range `eb92a5f..608dfff` ends on an APP-121 commit, and APP-121's ends on `b7820c2`, a merge into `pivot/talk`. The README lists "118 to 120" while the commit includes APP-121.
4. **Minor: stale Branch/PR fields to fix at the move.** APP-106.md:19 and APP-109.md:19 say "not merged"; APP-121.md:19 says "Not started"; APP-104.md:19 says "Fences and the other leftovers", while the 11-02 Log says the fences scope only.
5. **Major (process): the board check fails on `2e84398`.** Render before any push, or CI's `board` step will fail.

## Per-ticket findings

### APP-104

- **Major: the drag row is wrong.** The closure (closure:19) says "the dragged row is a whole app or a file Card". Only standalone works get `data-lib-entry-drag` (library.ts:534, `cardFrom` at :1105); whole apps cannot be dragged. The stronger evidence is uncited: canvas-library-drag.test.ts:24–103 (pan/zoom drop point, outside and Escape, no duplicate, touch).
- **Major: the controls row is narrower than claimed** (closure:17).
  - drag-end and control-pointer cover Move, Hand, Space and native controls only.
  - Nothing on `main` tests zoom keys, Ctrl+wheel, pinch, Shift+0/1/2, nudge, Shift-click or Delete.
  - The typing guard (controls.ts:239–280) is tested only for `?` (canvas-commands.test.ts:426–430), and `?` is now Ask.
- **Minor: the waiting-record row** (closure:16) rests on actions 1c, which checks group JSON, not the clean-board `auto="pending"` rule (canvas.rs:1722–1735). That rule is untested: code inspection plus the 10-30 walk.
- **Minor: an uncited test exists.** The SSR pane test the Verification asked for is at main.rs:1746–1760.
- **Required:** APP-104.md:48 (fences) is still `- [ ]`. Move it with a pointer to APP-177.
- **Verdict: Done acceptable** after the per-box supersession note for boxes 2, 3, 6 and 7, the fences edit and the corrections. No blocker.

### APP-106

- **Major: the "on main" column cites other surfaces.**
  - presentation.test.ts:59 tests the app-action dialog, not Focus.
  - app-assistant.test.ts:116 tests the assistant panel.
  - The Focus test (app-view-controller.test.ts:261) checks read order and draft only. It does not check F, ←/→, Esc with no layout change, the trap or reduced motion.
  - The live-event row is inference from the campaigns.
- **Minor:** restate the walk's own limits: Sarah's live approval was not walked, and the check used the member's own commit (APP-106.md:73).
- On `main`, `/unite` means Open and `/split` refuses (unite.ts:427–434), consistent with ADR-018.
- **Verdict: Done acceptable.** The 10-30 walk is thorough. Say that Focus keys, trap, reduced motion and phone width have no current automated check.

### APP-109

- **Major: the privacy citation is wrong** (closure:9: "the `pitch` stand … isolation check covers other members").
  - That check uses a cross-organization canary and never calls `/v1/notes`. No test calls `/v1/notes` or `own_works` (queries.rs:1615).
  - The right evidence exists and is uncited: pitch "sharing", "the teammate's group route was the uniform 404 before the share" (pitch_qualification.rs:1080), plus work-scoped `record_read`.
  - At delivery this box was code inspection ("Not walked: a second member", APP-109.md:50), and it still is.
- **Minor:** the unit test at actions/tests.rs:405 checks sidecar shape, not persistence. Reload and other-browser survival rest on the 10-30 walk, and the admin `/note` test uses a spy.
- **Verdict: Done acceptable** after the citation fix and the stated limit. Follow-up test: a teammate's `/v1/notes` returns only their own work, and the author's notes work is 404 to them.

### APP-112

- **Major: box 3 cites the wrong module** (closure:9).
  - `command-sheet.ts` `sheetGroups` is the Commands tab. The `?` sheet is the hand-written `SHORTCUTS` (shortcut-tab.ts:23–75), now stale (finding 1).
  - The real test is the pane test's Shortcuts assertions (canvas-commands.test.ts:401–407).
  - "No gaps remain" holds as of the 10-31 audit, not today.
- **Minor:** the plain-words test is uncited (canvas-polish.test.ts:6).
- **Verdict: Done acceptable as a delivery record** once the stale Shortcuts rows have a recorded owner (a new ticket or an APP-176 scope note) and the citation is fixed.

### APP-118

- **Major: box 4 overstates the comparison** (closure:10: "the registry's `VERBS`").
  - The test compares `slashGroups('')` and `sheetGroups('')` with a test-local literal (canvas-commands.test.ts:73), after registering stubs for `unite`, `split` and `arrange` itself (:52–69).
  - The `?` sheet no longer reads the registry: shortcut-tab.ts imports only `GRAMMAR`. "One source" therefore holds for two of three surfaces.
- **Minor:** box 1's `/folder ` and `/apps ` suggestions are untested on `main`; they rest on the 10-31 walk.
- **Minor:** box 3's recorded deviation (APP-118.md:79, no saved `/apps` card possible) is not restated.
- **Verdict: Done acceptable** with these corrections.

### APP-119 (walk never done)

- **Boxes 3–4: acceptable without a walk.** Evidence: commands.rs:558 and :601 (`address_hint` at :166), the pitch "slash commands" check, the admin `/apps` search test, and actions 1a.
- **Boxes 1–2: acceptable with a recorded limit.** The Commands-tab half is fake-DOM tested (canvas-commands.test.ts:376–377); the `/` menu half is data-level only (finding 2).
- **Minor: "behave exactly as before" (closure:9) omits later changes.**
  - A bare `/expense` now places the app (APP-120).
  - `/big-claims <question>` opens the assistant (canvas-commands.test.ts:181).
  - `/pto <words>` is untested.
- **Verdict: Done acceptable** if the Log states that the dropdown was never observed, and Victor records his check or a waiver.

### APP-120 (walk never done)

- **Box 1:** data-level only, as in APP-119.
- **Minor: the `?`→Ask supersession** rests on APP-122, which is still Backlog, not on ADR-018. Say so.
- **Box 2:** `/card` placement is superseded by ADR-018; resolution is tested (canvas-commands.test.ts:128).
- **Box 3:** fake-DOM tested (canvas-library.test.ts:167). "n of m cards" is now the whole-work view status ("Card view", "Browse view"), which the closure omits.
- **Box 4:** the sentence is present (GUIDE.md:39, SCENARIOS.md:19), but both docs still teach superseded behavior (finding 1).
- **Verdict: Done acceptable** with the same waiver note as APP-119.

### APP-121 (boxes unticked; walk never done)

I read the pitch "naming (APP-121)" check (pitch_qualification.rs:1638–2029) against each box.

- **Box 1, met.**
  - Rename to "Team spend" at `/team-spend` is idempotent (:1699–1706), with one `app.labeled` event (:1707–1711).
  - The teammate sees the name and address with the `expenses` and `expense` aliases (:1713–1733).
  - `/spend` stays Vendor spend's (:1734–1750).
  - Non-owner and outsider get the uniform 404 (:1671–1681).
- **Box 2, met.** `/exp` is listed for the teammate only (:1808–1834), and verb and address clashes are refused (:1835–1853). `/app:exp` is shown client-side as `/app:e`, the same path (canvas-commands.test.ts:502), and on the server only as `namespaced` (:1719).
- **Box 3, met with narrowings.** "Q3 spend" reaches the teammate via `/v1/commands` (:1873–1888) and the canvas header in the admin test (canvas-library.test.ts:334–363).
  - "Everywhere" excludes MCP and Slack, and on `main` Library card rows are hidden metadata.
  - "Receipts unchanged" is a count of revisions, publications and receipts (:1654–1659, :1889), not their content.
- **Box 4, met.**
  - ENABLE+FORCE RLS (:1908–1911); another organization's scope reads none (:1902); the schema qualifier's 0014 check passes.
  - The 404 is proved on the write route (:1671–1689), and `/v1/commands` excludes the label (:1892–1895).
  - The group and MCP read routes rest on general isolation plus RLS.
- **The missing walk is acceptable for all four boxes:** each is observable over HTTP or in storage. The ✎ editor and double-click rename are not boxes.
- **Verdict: tick and close.** Record the narrowings and the Verification deviation, and fix Branch/PR.

## The APP-177 split

The split is right. Ticking the box would be false, deleting it would silently rewrite APP-104, and leaving it would block APP-104 forever. A keep-or-drop decision under ADR-018 is the correct first step, and GUIDE.md:174 already says "Fences are not built yet".

Fixes: render the board; edit APP-104.md:48 and :19 to point to APP-177. Also, APP-177's "Why" quotes "after macOS Stacks, BumpTop's piles and Fences" as Victor's words, but APP-104.md:35 quotes only "add the ability to stack cards and shuffle through them"; unquote the rest.

## Result

**No acceptance blocker.** Changes are requested on the closure records before the moves: findings 1, 2 and 5, the major findings above, and per-box supersession notes in each ticket.

**Limits:** no browser; no cargo or campaign re-run (I read the lead's reports); test coverage was judged from the source.
