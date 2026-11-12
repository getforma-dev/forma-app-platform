# Independent review: PR #92 follow-up commits (APP-122, APP-116, APP-102, APP-108)

- **Reviewer:** an independent Claude subagent (Opus 5.5). It did not write these commits.
- **Date:** 2026-11-12.
- **Reviewed source:** `closure/acceptance-gaps` in `forma-app-082n2`: `f3505dd`, `154b5e7`, `9ea5b9b` (head `9ea5b9b`). The commits through `b2ba6b7` were judged earlier and are not reviewed again.
- **Method:** `forma-verify/Tickets/_tools/REVIEW.md`. All file:line references are at `9ea5b9b`.

## What I read and ran

- **Read:**
  - the three diffs;
  - `admin/src/canvas/{shape,records,timing,ledger,render}.ts`;
  - `storage/{agents,governance}.rs`, `mcp/{mod,tools}.rs`, `actions/{routes,receipt}.rs`;
  - `migrations/0010_agent_tokens.sql` and `0013_agent_governance.sql`;
  - the campaign context in `mcp_campaign.rs` and `box_campaign.rs`.
  - I grepped `contracts/`, `app/admin` and `docs/` for anything that pins the records listing. Nothing does: `recordsJson` checks only the ids and arrays.
- **Ran in `forma-app-082n2`:**
  - `npx tsx --test` on `test/shape-apply-revert.test.ts` (2 passed), `test/ledger-totals.test.ts` (1 passed), and `test/canvas-polish.test.ts` with `test/canvas-cards.test.ts` (15 passed);
  - `cargo test --locked -p forma-app-platform --lib the_multipart_body_is_strict` (1 passed) and `--lib` (406 passed, 34 ignored);
  - `cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings`: clean.
- **Mutations:** each file was restored from a scratch copy afterward. `git diff` and `git status` are clean at the end.
  - Shape (`shape.ts`):
    - Apply without `labelShaped`: fails.
    - Revert without `showSection(card, null)`: fails.
    - An extra direct `fetch` POST in `applyCard`: fails.
    - A `storeCard()` call in `applyCard`: **passes** (finding 1).
  - Ledger (`ledger.ts`): each of these fails the test.
    - `>=` at the range end;
    - `<=` at the range start;
    - no collection filter;
    - count only records with an amount;
    - the status filter ignored.
  - Receipt: a `Debug` that also prints the first four bytes fails the new equality. The old `contains("PNG")` check would have passed it.
  - Records wiring: `records.ts` with `via_tool` and `via_ms` dropped: the admin tests **pass** (finding 7).
- **Not run:** Docker campaigns and the browser. The recorded results are taken as stated: box 18, mcp 12, slack and workflows passed.

## f3505dd: Shape preview, Apply, Revert and Cancel

The test drives the real path:

- `runShape` → `resolveTarget` → `shapeCard` → `loadTable`/`recordsJson`;
- then `requestShape` → `cardView` → `showSection` → `confirmDock`;
- then `applyCard` → `labelShaped` → `revertCard`.

It would fail if Apply did not label the card or Revert did not restore it. It would also fail on any extra `fetch` from `shape.ts` or the three real modules.

1. **Minor: the spies are permissive.** `shape-apply-revert.test.ts:148` makes each spied import `?.()`, so an unmocked call returns `undefined`. A write routed through a spied module would never reach `fetch`, so the "only two requests" assert (`:194`) cannot see it. Such writes include `storeCard`, `shareCard` or `removeCard` in `./cards`, and `addCards` or `placeUtilityCard` in `./board`. Mutation M3 confirmed this. Fix: make an unmocked name throw `unexpected <path>.<name>`.
2. **Minor: the two tests share module state.** `:151` imports an identical `data:` URL twice, so ESM caching returns the same `shape.ts` instance, and `appliedCards` and `previews` carry over. When test 1 fails before Revert, the Cancel test (`:199`) fails too, through `before = appliedCards.get(...)`; run alone, it passes. Fix: make each bundle URL unique, for example with a comment holding a counter.
3. **Minor: the highlight is never asserted.** Acceptance 3 says a highlighted ledger previews. I probed the preview: its Nobu `tr` carries `data-highlight="crimson"`, but nothing at `:172-177` checks for it.

## 154b5e7: tool and duration on the card

**Authorization and tenancy hold.**

- The lateral join (`storage/agents.rs:263`) is bound to `x.organization_id = a.organization_id` and the attribution's own `token_id`. The outer query is bound to `$1`, the app and the work.
- RLS on `agent_access_log` is `tenant_scope` only (`0013_agent_governance.sql:76-80`).
- `genui_workspace` holds column `SELECT` on every column read: `tool`, `duration_ms`, `touched_records`, `outcome`, `occurred_at` and `chain_sequence` (`:83-87`).
- The read runs only after `list_records` authorizes the member (`routes.rs:824-841`).

**Matching is sound for the normal path.**

- The attribution's `created_at` is `now()` in its own transaction.
- The log row's `occurred_at` is `date_trunc('second', now())` in a later transaction (`governance.rs:338`). So the commit's own row always satisfies `occurred_at >= trunc(created_at)`.
- An `isError` call is logged as `refused` (`mcp/mod.rs:422`), so failed commits never match.
- A replay writes no attribution (`tools.rs:1141`), and `ORDER BY chain_sequence LIMIT 1` takes the first commit.
- The `tool = forma_commit` filter excludes the same-second draft.

**Escaping holds.** `agentCall` (`timing.ts:114`) admits only `^forma_[a-z_]{1,40}$` and a finite, non-negative number. The chip is a text node, and the title is set with `setAttribute`.

4. **Minor: query cost.** The only index is `(organization_id, token_id, chain_sequence)` (`0013:73`).
   - Each lateral probe walks the token's log from its first row and filters every heap row until it reaches the commit's second. Cost grows with the token's history.
   - That happens per attribution: up to `ATTRIBUTION_LIMIT` = 500 (`agents.rs:41`), or every attribution of the work if the planner joins before the sort.
   - It runs on every records listing read and every live refresh. It is negligible at demo scale, but O(attributions × token calls) for a busy 30-day token.
   - Suggested fix: an index on `(organization_id, token_id, tool, occurred_at)`, ordered by `occurred_at, chain_sequence`.
5. **Minor: a new exposure across members.** The access log is otherwise readable only by the token's owner (`governance.rs:425` filters on `principal_id`). The listing now shows another member's agent call duration to anyone who can read the work. The tool is always `forma_commit`, so it adds nothing. This is low sensitivity and acceptable for display, but it should be recorded as a decision in the ticket or the ADR.
6. **Minor: matching edge cases (display only).**
   - (a) If one token commits two versions of a record within one second, both versions show the first call's duration.
   - (b) The log write is best-effort (`mcp/mod.rs:438`). If the commit's row is missing, the attribution matches the next allowed `forma_commit` by that token on that record (the replay or a later version), with no upper time bound.
7. **Minor: the tests cannot catch a wrong match or broken wiring.**
   - `via_tool == forma_commit` (`mcp_campaign.rs:758`, `:818`) holds by construction once any row matches, and `via_ms` is checked only by range. Matching the replay row (`DESC`) would pass. The "first commit, not the replay" comment (`:756-757`) is not asserted. The campaign's `scalar` helper could compare `via_ms` with the first commit's `duration_ms`.
   - The admin test covers only `agentCall` and `activityText`. Dropping the arguments at `records.ts:156` and `:172` passes all the admin tests.

## 9ea5b9b: the three test fixes

- **Box teammate control (`box_campaign.rs:1003`):** sound. `not_found` comes only from `Resolved::None` or the uniform refusal (`tools.rs:1418`). A scope refusal, an invalid argument or a successful build would fail it.
  8. **Minor: the commit message overstates its reach.** It calls this "the first end-to-end run of folder resolution by name", but only a miss on a name runs. Maya's positive call still uses `"1"` (`:949`), so a resolver that never matched a name would still pass.
- **Receipt `Debug` (`receipt_tests.rs:106`):** the exact equality now fails on any byte leak. Confirmed by mutation.
- **`ledger-totals.test.ts`:** each mutation fails it. The people and category sums check USD only, not EUR (trivial).

## Verdict

**No blocking findings: fit to merge.**

- Findings 1, 2 and 7 weaken the tests' ability to fail. They are worth a small follow-up.
- Finding 4 should be fixed before real agent traffic.
- Finding 5 needs a recorded decision.

CI on `9ea5b9b` was not inspected by this reviewer.
