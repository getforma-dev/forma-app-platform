# Delivery roadmap

The original six-phase platform design remains the long-term plan. The authoritative board now also carries PH07 (post-MVP completion) and PH08 (the November 2 product pivot). Implementation has progressed substantially; phase completion still requires its recorded gate. Use [DIRECTION.md](DIRECTION.md), [first-run](first-run/README.md) and the [November 1 review](PIVOT-REVIEW-2026-11-01.md) for current work, not the historical first-ticket queue. **GenUI** owns the application; **FormaStack** owns framework and identity-provider contracts. Counts and individual statuses come from the generated board rather than a second handwritten inventory.

| Phase | Sprints | Tickets | What we will have at the end |
|---|---|---|---|
| [1 — Foundations and first saved workflow](../../Tickets/phases/PH01.md) | S01–S02 | APP-001–014, APP-073–078 (20) | One authorized query, receipt and durable decision that survives retry, conflict and reopen |
| [2 — Working data and workspace](../../Tickets/phases/PH02.md) | S03–S05 | APP-015–031, APP-081, APP-087–088 (20) | Real file imports, correct results, accessible canvas/forms and saved work |
| [3 — App creation, governance and AI](../../Tickets/phases/PH03.md) | S06–S08 | APP-032–047 (16) | General app authoring, preview, governed publication, version changes and bounded AI |
| [4 — Complete two-app demonstration](../../Tickets/phases/PH04.md) | S09–S10 | APP-048–057 (10) | Northstar and Spend Review work through the same build, with sharing, reuse and recovery proven |
| [5 — Managed pilot controls](../../Tickets/phases/PH05.md) | S11 | APP-058–066, APP-080 (10) | Hosted deployment, service identity, audit, support and deletion controls |
| [6 — Qualification and pilot learning](../../Tickets/phases/PH06.md) | S12 | APP-067–072, APP-079 (7) | Measured release readiness, a controlled real-data pilot, and observed customer reuse |
| [7 — Post-MVP completion](../../Tickets/phases/PH07.md) | S13 | APP-082–086 (5) | Every acceptance sentence and review residual deferred by ADR-010 for the November 2 MVP is proven or dispositioned |

| [8 — November 2 product pivot](../../Tickets/phases/PH08.md) | S14 | Current PH08 ticket list | A clean first-member workflow, coherent app page and real sharing; retain earlier seeded rehearsal as development evidence |

The maintained [phase board](../../Tickets/PHASES.md) shows progress. Each phase page contains entry requirements, deliverables, checks and evidence gates, applicable FormaStack blockers and its ordered tickets. [Sprints](../../Tickets/SPRINTS.md) divide the phase into smaller batches; [categories](../../Tickets/CATEGORIES.md) group ownership. `Tickets/plan.json` owns phase definitions; ticket Markdown owns individual work status.

## Execution and completion

For the active pivot, follow the app-page sequence and first-run dependencies in DIRECTION.md. Select the later platform work by its actual dependencies. Independent eligible tickets may begin while another phase's integration gate waits; this does not pass that gate or waive a required check. Phase acceptance remains sequential and requires ticket completion plus reviewed evidence.

PH01 directly waited on FORMA-232's budget disposition, FORMA-246 native conformance and FORMA-247 hosted authentication; all three are cleared (FORMA-247 and FORMA-248 by founder disposition, [ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md)), and PH01's beta go-live trial waits on FORMA-366 (the qualified mail image). PH05 and PH06 wait on FORMA-365 (the deferred hosted identity, custody and rotation campaign) before real-organization identity readiness. Their scope and unblock evidence are in the [cross-team handoff](CROSS-TEAM-HANDOFF.md). Downstream local dependencies preserve these gates.

The [quality gates](QUALITY-GATES.md) specify the tests, verifications and release trials required by the design for each phase. A phase cannot finish merely because its ticket list is checked off. Record exact candidate/environment identities, commands/results, negative/failure cases and independent review in its gate record. Preserve failed trials and rerun affected checks after fixes.

PH05 records scope and prepares controls. PH06 first qualifies the candidate with APP-067–071, then makes the live-data entry decision at APP-072 entry, then observes recurring customer use. Customer learning is the end of PH06, not a prerequisite for entering the pilot.

Sprints and phases are scope batches without invented dates or velocity. Refine estimates before selection and split any task that cannot yield a reviewable result within five focused implementation days. Observational pilot time may exceed that effort estimate.

[Deferred scope](DEFERRED.md) remains outside these phases. Source designs are preserved unchanged; later decisions and measured deviations are recorded explicitly.
