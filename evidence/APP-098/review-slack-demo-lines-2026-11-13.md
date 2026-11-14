# Review: Slack demo lines (DS-40) and APP-098 box 5, 2026-11-13

Reviewer: independent Claude (Opus 5.5) subagent. Scope: `e6b71d36` on `app098/slack-demo-lines` (stacked on #113), read plus unit tests. No browser, AWS, dev stand or campaign was run.

## Part 1: the commit. Verdict: approve with changes (minor findings only, no blocker)

The organizations are right in all four places. The draft card uses `member.organization()` of the resolved Slack link (`flows.rs:491`). The filed line uses the confirming member, and `held` already requires the draft's organization to match (`flows.rs:567-575`, `:760`). The filer DM uses the decider, and the filer's link is checked again for the same organization before sending (`flows.rs:807`, `:815-818`). The link DM uses the signed-in confirming member, which is the organization `link_slack` writes (`link.rs:257-267`). No path can pick another tenant's organization. `is_demo_org` is process configuration and holds no request data.

Escaping is sound. `demo_suffix` is a constant and is always appended outside `text::escape`: after the whole message in `confirm`, and as a separate `format!` argument in the other two. It contains no `&`, `<` or `>`. `escape` does not neutralize `*` or `_` (`text.rs:11-23`), which was already true before this commit. At worst a value containing `_` changes italics, with no link or mention injection.

The campaign assertions match the new text: `ends_with("made up for the demo._")` on the link DM and the filed line, the card's context line, and the exact filer DM. The filer DM's `format!` compiles, because the campaign is `cfg(test)` and was built by the run below, and rustfmt accepts it. No other test, admin code or script pins these strings.

- **m1** `slack/link.rs:212-220`: the link page `GET /slack/link` names the persona ("will act as **Diego Ramos**") with no synthetic words, while the DM sent after it now has them. The ticket says that after this slice, only Slack text with no record or person remains (`APP-098.md:254`). That is not accurate. Add `demo_suffix` or the note to the page, or record the page in the ticket.
- **m2** `storage/slack_campaign.rs:754`, `:764-771`: the outsider's link DM (`dm:U0OUTSIDE`, unlisted canary organization) is sent but never checked. Asserting that it does not end with the suffix would show the organization rule end to end. Today only the unit test shows it.
- **m3** `docs/pitch/SCENARIOS.md:113`, `:119`: scenario 5 quotes the card, the "Filed" line and Diego's DM as they read before #112 and this commit. In the demo organization each now ends with the synthetic line. Update the quotes so the presenter is not surprised.
- **n1** `slack_campaign.rs:1053`: `format!("…{}", " _Synthetic…_")` is one literal written as two. A single string literal is clearer and is still an independent pin.
- **n2** `flows.rs:336-337`: DS-05 asks every `is_demo_org` call site to name its DS entry. `demo_suffix`'s comment does not say DS-40, though the `demo_line` comment above it does.
- **n3** The fallback `text` of the block messages still names personas without the words. Slack uses that text for notifications and previews: "Diego Ramos filed a … for your approval" (`flows.rs:1040`) and "Approved by Sarah Chen" (`:957-962`).

## Part 2: box 5. The named gap cannot be reached in any deployed window as configured today

- **Clean trial (the current beta lineage).** The stack sets `GENUI_DEMO_MODE=0` and `GENUI_DEMO_ORGANIZATIONS=''` (`infra/src/stack.ts:351-352`, pinned by `infra/test/shared-broker.test.ts:44`). The clean trial reuses the beta stack, so the database still holds the Northstar and canary rows (`docs/pitch/CLEAN-TRIAL-DEPLOY.md:3`; `stack.ts:224` still lists both tenants). Nothing can reach those rows:
  - Auth routing and the identity registry hold exactly the trial tenant (`infra/scripts/window_requests.py:216-218`, `:266-275`).
  - Every way in requires `registry.agrees`: sessions (`storage/sessions.rs:877`), Slack links (`:932-950`), agent tokens (`:1075`) and every storage transaction (`storage/authorization.rs:212-218`).
  - The deploy wrapper refuses seed, library and grant steps on a clean trial (`ops/beta/deploy.sh:302`, `:328`), and a redeploy of this lineage must stay clean (`:149-150`).
  - The seeds take only the two fixture organizations (`app/runtime/tenant_seed.py:37`, `:90`; `admission_operator.py:1319`, `:1366`), and the trial organization cannot be either of them (`infra/src/shared-broker.ts:24`).
- **Seeded sandbox context (no `cleanTrialFile`).** The switch is on and only Northstar is listed (`stack.ts:351-352`), so Northstar's fixtures carry the DS-40 note. Canary (…0002) is never listed, yet `seed` writes the Northstar fixture and Spend Review into it (`tenant_seed.py:54-56`), and canary has an auth route there (`window_requests.py:218`).
  - The box 5 note's condition "with the switch off" is therefore inaccurate. The real condition is "outside a listed demo organization".
  - It is reachable only through an extra operator grant. The fixture works are granted only to seeded principals with issuer `app008-fixture-only` (`tenant_seed.py:43`, `:59`), which no deployed registry binds. A real canary member sees them only after `grant-permission` or `issue --grant`.
  - Such a window also needs a separately reviewed sandbox deployment (`docs/pitch/DEPLOY-RUNBOOK.md:16`).
- **Dev stand.**
  - Pitch profile: the switch is on and Northstar is listed (`app/src/dev/mod.rs:323-325`). Priya is the Northstar fixture's own principal (`…0066`), so what she sees is labelled.
  - Clean profile: the switch is off (`:327-328`). Every new volume seeds both fixtures into …0002 (`app/src/dev/db.rs:325`), but Alex (`…006f`) has no grants on them (`dev/mod.rs:128-135`). `grant-library-access` grants catalog apps only (`admission_operator.py:1430-1444`), and APP-128 recorded the clean profile as showing 0 apps.

**Judgement:** box 5 can honestly be ticked once #113 and this branch are merged, if the ticket records these limits:
1. Fixtures outside the catalog are labelled only inside a listed demo organization. In a seeded sandbox window, canary's copies are unlabelled. They are reachable only through an operator grant to a real member. This stays with DS-84 (APP-097). Reword the note from "switch off" to "outside a listed demo organization".
2. The clean trial's unreachability was checked by reading, not by a deployed probe.
3. In a demo organization, three surfaces name a persona with no synthetic words:
   - the Slack link page (m1);
   - MCP `forma_list_pending`, whose summary says "filed by Sarah Chen" without the synthetic sentence (`mcp/tools.rs:1230`, `:1304-1323`; only `forma_list_apps` and `forma_run_view` carry it);
   - Slack fallback texts (n3).

   I recommend labelling the link page before ticking, since it is the only HTML page among them. Record the other two.
4. None of the labels has been seen in a browser or in Slack (the ticket's logs say so). Victor's visual QA is still open.
5. Evidence: the 2026-11-12 spot check covered 14 files. Today's two `evidence/APP-098` files that name personas both say "synthetic".

## Commands

- `cargo test --locked -p forma-app-platform --lib slack::`: 11 passed, 0 failed, 438 filtered out.
- `cargo fmt --check -p forma-app-platform`: exit 0, no files changed.
- Not run, per the brief: `just campaign slack`, clippy, `just check`.
